//! The verifiable random draw for slot committees.
//!
//! Each slot every eligible validator evaluates a verifiable random function
//! over the slot seed and its own key. A verifiable random function produces a
//! random value together with a proof. Anyone checks the proof with the public
//! key alone. The private key never leaves the node.
//!
//! Three properties follow. The output looks random to every observer, so a
//! targeted attack on the next author has nothing to aim at. The output is
//! fixed by the seed and the key, so every node computes the same committee
//! from the same block. The proof travels with the claim, so a validator that
//! states an output it never produced fails the check.
//!
//! The draw uses the sr25519 verifiable random function carried by the
//! dependency set. The transcript mixes the slot number and the slot seed under
//! a Cerulea label, so a draw from one deployment carries no meaning in
//! another.

use codec::{Decode, Encode};
use sp_core::crypto::{VrfPublic, VrfSecret};
use sp_core::sr25519::vrf::{VrfPreOutput, VrfSignData, VrfSignature, VrfTranscript};
use sp_core::sr25519::{Pair, Public};
use sp_core::Pair as PairTrait;
use sp_keystore::KeystorePtr;

use crate::engines::{EngineError, Slot, SlotSeed};
use crate::CERULEA_KEY_TYPE;

/// The label of the Cerulea slot draw transcript.
pub const DRAW_LABEL: &[u8] = b"cerulea.authoring.draw.v1";

/// The context the draw output bytes are taken under.
pub const DRAW_OUTPUT_CONTEXT: &[u8] = b"cerulea.authoring.draw.output.v1";

/// The domain of the slot number inside the transcript.
pub const SLOT_DOMAIN: &[u8] = b"slot";

/// The domain of the slot seed inside the transcript.
pub const SEED_DOMAIN: &[u8] = b"seed";

/// The input of one slot draw.
///
/// Every node builds the same input for one slot from chain state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct DrawInput {
    /// The slot the draw serves.
    pub slot: Slot,
    /// The seed of that slot.
    pub seed: SlotSeed,
}

impl DrawInput {
    /// Build the input of one slot draw.
    pub fn new(slot: Slot, seed: SlotSeed) -> Self {
        DrawInput { slot, seed }
    }

    /// The transcript the draw signs.
    pub fn transcript(&self) -> VrfTranscript {
        let slot_bytes = self.slot.0.to_be_bytes();
        VrfTranscript::new(
            DRAW_LABEL,
            &[
                (SLOT_DOMAIN, &slot_bytes[..]),
                (SEED_DOMAIN, &self.seed.0[..]),
            ],
        )
    }

    /// The signing data the draw signs.
    pub fn sign_data(&self) -> VrfSignData {
        VrfSignData::new(self.transcript())
    }
}

/// The proof that one validator produced one draw output.
///
/// The proof travels with the block and with the committee claim. Every node
/// checks the proof against the validator public key.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct DrawProof(pub VrfSignature);

/// One draw result.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct DrawResult {
    /// The validator that produced the draw.
    pub validator: Public,
    /// The thirty two output bytes of the draw.
    pub output: [u8; 32],
    /// The proof anyone checks with the validator public key.
    pub proof: DrawProof,
}

impl DrawResult {
    /// The draw output read as a number.
    ///
    /// The first sixteen bytes read as one big endian number. A lower number
    /// wins a lower committee rank.
    pub fn value(&self) -> u128 {
        draw_value(&self.output)
    }
}

/// Read a draw output as a number.
pub fn draw_value(output: &[u8; 32]) -> u128 {
    let mut head = [0u8; 16];
    head.copy_from_slice(&output[..16]);
    u128::from_be_bytes(head)
}

/// Produce the draw of one validator holding its key pair.
///
/// Block production on a node holding its key in memory calls this. A node
/// holding its key in the keystore calls [`draw_from_keystore`].
pub fn draw(pair: &Pair, input: &DrawInput) -> DrawResult {
    let data = input.sign_data();
    let signature = pair.vrf_sign(&data);
    let output = pair.make_bytes::<32>(DRAW_OUTPUT_CONTEXT, &input.transcript());
    DrawResult {
        validator: pair.public(),
        output,
        proof: DrawProof(signature),
    }
}

/// Produce the draw of one validator holding its key in the node keystore.
///
/// The private key stays inside the keystore. The call reports a problem where
/// the keystore holds no key for this validator.
pub fn draw_from_keystore(
    keystore: &KeystorePtr,
    validator: &Public,
    input: &DrawInput,
) -> Result<DrawResult, EngineError> {
    let data = input.sign_data();
    let signature = keystore
        .sr25519_vrf_sign(CERULEA_KEY_TYPE, validator, &data)
        .map_err(|reason| EngineError::KeystoreRefused {
            detail: format!("{:?}", reason),
        })?
        .ok_or(EngineError::DrawKeyAbsent)?;
    let output = output_bytes(validator, input, &signature.pre_output)?;
    Ok(DrawResult {
        validator: *validator,
        output,
        proof: DrawProof(signature),
    })
}

/// Check one draw and read its output.
///
/// The check needs the validator public key alone. A proof that fails the check
/// reports a problem and the claimed output is discarded.
pub fn verify_draw(
    validator: &Public,
    input: &DrawInput,
    proof: &DrawProof,
) -> Result<[u8; 32], EngineError> {
    let data = input.sign_data();
    if !validator.vrf_verify(&data, &proof.0) {
        return Err(EngineError::DrawProofRejected { slot: input.slot.0 });
    }
    output_bytes(validator, input, &proof.0.pre_output)
}

/// Check one draw result against its own claimed output.
pub fn verify_result(input: &DrawInput, result: &DrawResult) -> Result<(), EngineError> {
    let output = verify_draw(&result.validator, input, &result.proof)?;
    if output != result.output {
        return Err(EngineError::DrawOutputMismatch { slot: input.slot.0 });
    }
    Ok(())
}

fn output_bytes(
    validator: &Public,
    input: &DrawInput,
    pre_output: &VrfPreOutput,
) -> Result<[u8; 32], EngineError> {
    validator
        .make_bytes::<32>(DRAW_OUTPUT_CONTEXT, &input.transcript(), pre_output)
        .map_err(|reason| EngineError::DrawOutputUnreadable {
            detail: format!("{:?}", reason),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_keystore::testing::MemoryKeystore;
    use sp_keystore::Keystore;
    use std::sync::Arc;

    fn validator(seed: u8) -> Pair {
        Pair::from_seed(&[seed; 32])
    }

    fn input(slot: u64) -> DrawInput {
        DrawInput::new(Slot(slot), SlotSeed([7u8; 32]))
    }

    #[test]
    fn a_draw_verifies_against_the_public_key_alone() {
        let pair = validator(1);
        let input = input(9);
        let result = draw(&pair, &input);
        let output = verify_draw(&pair.public(), &input, &result.proof)
            .expect("the proof checks against the public key");
        assert_eq!(output, result.output);
        assert!(verify_result(&input, &result).is_ok());
    }

    #[test]
    fn a_draw_repeats_for_the_same_slot_and_the_same_key() {
        let pair = validator(1);
        let first = draw(&pair, &input(9));
        let second = draw(&pair, &input(9));
        assert_eq!(first.output, second.output);
        assert_eq!(first.value(), second.value());
    }

    #[test]
    fn two_slots_produce_two_outputs() {
        let pair = validator(1);
        assert_ne!(
            draw(&pair, &input(9)).output,
            draw(&pair, &input(10)).output
        );
    }

    #[test]
    fn two_seeds_produce_two_outputs() {
        let pair = validator(1);
        let one = draw(&pair, &DrawInput::new(Slot(9), SlotSeed([1u8; 32])));
        let two = draw(&pair, &DrawInput::new(Slot(9), SlotSeed([2u8; 32])));
        assert_ne!(one.output, two.output);
    }

    #[test]
    fn two_validators_produce_two_outputs() {
        let input = input(9);
        assert_ne!(
            draw(&validator(1), &input).output,
            draw(&validator(2), &input).output
        );
    }

    #[test]
    fn a_proof_from_another_validator_fails_the_check() {
        let input = input(9);
        let honest = draw(&validator(1), &input);
        let other = validator(2).public();
        assert!(matches!(
            verify_draw(&other, &input, &honest.proof),
            Err(EngineError::DrawProofRejected { .. })
        ));
    }

    #[test]
    fn a_proof_for_another_slot_fails_the_check() {
        let pair = validator(1);
        let result = draw(&pair, &input(9));
        assert!(matches!(
            verify_draw(&pair.public(), &input(10), &result.proof),
            Err(EngineError::DrawProofRejected { .. })
        ));
    }

    #[test]
    fn a_claimed_output_that_differs_from_the_proof_fails_the_check() {
        let pair = validator(1);
        let input = input(9);
        let mut result = draw(&pair, &input);
        result.output[0] ^= 0xff;
        let error = verify_result(&input, &result).expect_err("the claim fails the check");
        assert!(matches!(error, EngineError::DrawOutputMismatch { .. }));
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn a_draw_from_the_keystore_matches_a_draw_from_the_pair() {
        let keystore = MemoryKeystore::new();
        let public = keystore
            .sr25519_generate_new(CERULEA_KEY_TYPE, Some("//Cerulea//one"))
            .expect("the keystore generates a key");
        let keystore: KeystorePtr = Arc::new(keystore);
        let input = input(12);
        let result =
            draw_from_keystore(&keystore, &public, &input).expect("the keystore holds the key");
        assert_eq!(result.validator, public);
        verify_result(&input, &result).expect("the keystore draw checks out");
    }

    #[test]
    fn a_keystore_without_the_key_names_the_fix() {
        let keystore: KeystorePtr = Arc::new(MemoryKeystore::new());
        let absent = validator(3).public();
        let error = draw_from_keystore(&keystore, &absent, &input(1))
            .expect_err("the keystore holds no key for this validator");
        assert_eq!(error, EngineError::DrawKeyAbsent);
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn the_draw_value_reads_the_leading_bytes() {
        let mut output = [0u8; 32];
        output[0] = 1;
        assert_eq!(draw_value(&output), 1u128 << 120);
        assert_eq!(draw_value(&[0u8; 32]), 0);
        assert_eq!(draw_value(&[0xff; 32]), u128::MAX);
    }

    #[test]
    fn the_draw_spreads_across_the_number_range() {
        let input = input(5);
        let values: Vec<u128> = (0..24u8)
            .map(|seed| draw(&validator(seed + 1), &input).value())
            .collect();
        let mut sorted = values.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), values.len(), "every draw takes its own value");
        let low = values
            .iter()
            .filter(|value| **value < u128::MAX / 2)
            .count();
        assert!(low > 4, "the draw reaches the lower half");
        assert!(low < 20, "the draw reaches the upper half");
    }

    #[test]
    fn a_draw_result_round_trips_through_scale() {
        let result = draw(&validator(1), &input(9));
        let bytes = result.encode();
        let back = DrawResult::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(back, result);
    }

    #[test]
    fn a_draw_input_round_trips_through_scale() {
        let value = input(9);
        let bytes = value.encode();
        assert_eq!(DrawInput::decode(&mut &bytes[..]).expect("decodes"), value);
    }
}
