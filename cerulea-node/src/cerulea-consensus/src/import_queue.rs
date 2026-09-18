//! Block import for the Dynamic Consensus Framework.
//!
//! Import accepts a block only where the header carries a Cerulea seal and that
//! seal passes every check. The unsigned authorship path is gone. Earlier
//! releases read the author from an unsigned digest item naming a public key,
//! so any node built a block naming the scheduled author and the verifier
//! accepted it. Import now reads the seal alone.
//!
//! # What import checks
//!
//! 1. **The header carries a Cerulea seal.** A header without one never
//!    imports.
//! 2. **The seal verifies against the author key.** The signature covers the
//!    pre-seal header hash and the seal claim.
//! 3. **The slot draw of the author verifies.** The draw proof covers the slot
//!    number and the slot seed derived from the parent hash.
//! 4. **The author is the scheduled author.** The framework pallet names the
//!    validator scheduled for that block, and the seal author matches it.
//! 5. **The author holds a seat.** The framework pallet lists the active
//!    validator set, and the seal author sits in it.
//!
//! The committee membership check and the rank timing check live in
//! `engines::authoring::verify`. Import reaches those two once the framework
//! pallet publishes the slot committee and the slot seed.

use crate::engines::authoring::draw::{derive_slot_seed, verify_result, DrawInput};
use crate::engines::authoring::seal::{check_seal_signature, extract_seal, BlockSeal};
use crate::engines::Slot;
use crate::metrics::ConsensusMetrics;
use crate::types::ValidatorMetrics;
use cerulea_runtime::AccountId;
use log::{debug, error, info, warn};
use pallet_cerulea_dcf::DcfApi as RuntimeDcfApi;
use sc_client_api::Backend;
use sc_consensus::{BlockCheckParams, BlockImport, BlockImportParams, ImportResult, Verifier};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_consensus::Error;
use sp_runtime::traits::{Block as BlockTrait, Header as HeaderT, NumberFor, SaturatedConversion};
use std::sync::Arc;

/// What import learned about the author of one block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedAuthorship {
    /// The account of the author.
    pub author: AccountId,
    /// The slot the block serves.
    pub slot: Slot,
    /// The rank the author claimed.
    pub rank: u8,
}

/// Read the account of a seal author.
pub fn author_account(seal: &BlockSeal) -> AccountId {
    AccountId::from(seal.claim.author.0)
}

/// The import queue of the Dynamic Consensus Framework.
pub struct DcfImportQueue<B, C, BE>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
    BE: Backend<B>,
{
    client: Arc<C>,
    metrics: ValidatorMetrics,
    consensus_metrics: Option<ConsensusMetrics>,
    _phantom: std::marker::PhantomData<(B, BE)>,
}

impl<B, C, BE> DcfImportQueue<B, C, BE>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
    BE: Backend<B>,
{
    /// Build an import queue over a client.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            metrics: ValidatorMetrics::default(),
            consensus_metrics: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Build an import queue that reports to the metrics registry.
    pub fn new_with_metrics(client: Arc<C>, consensus_metrics: ConsensusMetrics) -> Self {
        Self {
            client,
            metrics: ValidatorMetrics::default(),
            consensus_metrics: Some(consensus_metrics),
            _phantom: std::marker::PhantomData,
        }
    }

    /// The import counters this queue holds.
    pub fn get_metrics(&self) -> &ValidatorMetrics {
        &self.metrics
    }

    /// Return the counters to zero.
    pub fn reset_metrics(&mut self) {
        self.metrics = ValidatorMetrics::default();
        info!("Commit Engine import: counters reset");
    }

    /// Check the seal of one block header.
    ///
    /// The call reports one sentence naming the fix wherever a check fails.
    pub fn verify_authorship(&self, header: &B::Header) -> Result<SealedAuthorship, String> {
        let block_number = (*header.number()).saturated_into::<u32>();

        // Check one. The header carries a Cerulea seal.
        let seal = extract_seal(header).ok_or_else(|| {
            format!(
                "Block {} carries no Cerulea seal. Fix: produce the block on a node holding the \
                 author key for that slot.",
                block_number
            )
        })?;

        // Check two. The seal verifies against the author key it names.
        check_seal_signature(header, &seal).map_err(|reason| reason.to_string())?;

        // Check three. The slot draw of the author verifies against the slot
        // seed derived from the parent hash.
        let seed = derive_slot_seed(header.parent_hash().as_ref(), seal.claim.slot);
        let input = DrawInput::new(seal.claim.slot, seed);
        verify_result(&input, &seal.claim.draw).map_err(|reason| reason.to_string())?;

        let author = author_account(&seal);
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        // Check four. The author holds a seat in the active validator set.
        let active = api.get_active_validators(best_hash).map_err(|reason| {
            format!(
                "The active validator set failed to load: {:?}. Fix: check that the node runtime \
                 matches the chain specification.",
                reason
            )
        })?;
        if active.is_empty() {
            return Err(format!(
                "Block {} arrived while the active validator set holds no members. Fix: admit at \
                 least one validator through the Admission Engine.",
                block_number
            ));
        }
        if !active.contains(&author) {
            return Err(format!(
                "Block {} names an author outside the active validator set. Fix: admit the \
                 validator through the Admission Engine before it authors.",
                block_number
            ));
        }

        // Check five. The author is the validator scheduled for that block.
        let scheduled = api
            .get_expected_author(best_hash, block_number)
            .map_err(|reason| {
                format!(
                    "The scheduled author failed to load: {:?}. Fix: check that the node runtime \
                     matches the chain specification.",
                    reason
                )
            })?;
        if scheduled.as_ref() != Some(&author) {
            if let Some(metrics) = &self.consensus_metrics {
                metrics.record_author_mismatch();
            }
            if let Err(reason) =
                api.report_author_mismatch(best_hash, block_number, scheduled.clone(), author)
            {
                warn!(
                    "Commit Engine import: the mismatch report failed: {:?}",
                    reason
                );
            }
            return Err(format!(
                "Block {} names an author other than the scheduled one. Fix: let the scheduled \
                 validator author that slot.",
                block_number
            ));
        }

        Ok(SealedAuthorship {
            author,
            slot: seal.claim.slot,
            rank: seal.claim.rank.0,
        })
    }
}

#[async_trait::async_trait]
impl<B, C, BE> BlockImport<B> for DcfImportQueue<B, C, BE>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
    BE: Backend<B>,
{
    type Error = Error;

    async fn check_block(
        &self,
        block: BlockCheckParams<B>,
    ) -> std::result::Result<ImportResult, Self::Error> {
        let block_number = block.number.saturated_into::<u32>();
        debug!("Commit Engine import: checking block #{}", block_number);

        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        let active = api.get_active_validators(best_hash).map_err(|reason| {
            sp_consensus::Error::ClientImport(format!(
                "The active validator set failed to load: {:?}. Fix: check that the node runtime \
                 matches the chain specification.",
                reason
            ))
        })?;
        if active.is_empty() {
            error!(
                "Commit Engine import: block #{} arrived with no active validators",
                block_number
            );
            return Ok(ImportResult::imported(false));
        }
        Ok(ImportResult::imported(true))
    }

    async fn import_block(
        &self,
        mut block: BlockImportParams<B>,
    ) -> std::result::Result<ImportResult, Self::Error> {
        let block_number = (*block.header.number()).saturated_into::<u32>();
        let block_hash = block.header.hash();

        let authorship = self.verify_authorship(&block.header).map_err(|reason| {
            error!(
                "Commit Engine import: block #{} rejected. {}",
                block_number, reason
            );
            sp_consensus::Error::ClientImport(reason)
        })?;

        block.fork_choice = Some(sc_consensus::ForkChoiceStrategy::LongestChain);

        info!(
            "Commit Engine import: block #{} ({:?}) sealed by {:?} at slot {} rank {}",
            block_number, block_hash, authorship.author, authorship.slot.0, authorship.rank
        );

        Ok(ImportResult::Imported(sc_consensus::ImportedAux {
            header_only: false,
            clear_justification_requests: false,
            needs_justification: false,
            bad_justification: false,
            is_new_best: true,
        }))
    }
}

#[async_trait::async_trait]
impl<B, C, BE> Verifier<B> for DcfImportQueue<B, C, BE>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
    BE: Backend<B>,
{
    async fn verify(
        &self,
        mut block: BlockImportParams<B>,
    ) -> Result<BlockImportParams<B>, String> {
        let block_number = (*block.header.number()).saturated_into::<u32>();
        let authorship = self.verify_authorship(&block.header)?;
        debug!(
            "Commit Engine import: block #{} passed the seal checks at slot {} rank {}",
            block_number, authorship.slot.0, authorship.rank
        );
        if block.fork_choice.is_none() {
            block.fork_choice = Some(sc_consensus::ForkChoiceStrategy::LongestChain);
        }
        Ok(block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::authoring::draw::draw;
    use crate::engines::authoring::seal::{seal_and_apply, SealClaim};
    use crate::engines::Rank;
    use crate::CERULEA_KEY_TYPE;
    use sp_core::{ed25519, sr25519, Pair as PairTrait};
    use sp_keystore::testing::MemoryKeystore;
    use sp_keystore::{Keystore, KeystorePtr};
    use sp_runtime::generic::{Digest, DigestItem, Header};
    use sp_runtime::traits::BlakeTwo256;
    use std::sync::Arc;

    type TestHeader = Header<u32, BlakeTwo256>;

    fn header(number: u32) -> TestHeader {
        TestHeader::new(
            number,
            Default::default(),
            Default::default(),
            Default::default(),
            Digest::default(),
        )
    }

    fn keystore() -> KeystorePtr {
        Arc::new(MemoryKeystore::new())
    }

    #[test]
    fn a_header_without_a_seal_carries_no_author() {
        let head = header(4);
        assert!(extract_seal(&head).is_none());
    }

    #[test]
    fn an_unsigned_author_digest_carries_no_authorship() {
        // Earlier releases named the author in an unsigned digest item. Import
        // reads the seal alone, so an unsigned item names nobody.
        let mut head = header(4);
        let author = AccountId::from([9u8; 32]);
        head.digest_mut().push(DigestItem::PreRuntime(
            crate::CERULEA_ENGINE_ID,
            codec::Encode::encode(&author),
        ));
        head.digest_mut().push(DigestItem::Consensus(
            crate::CERULEA_ENGINE_ID,
            codec::Encode::encode(&author),
        ));
        assert!(extract_seal(&head).is_none());
    }

    #[test]
    fn a_sealed_header_reads_back_its_author_account() {
        let keystore = keystore();
        let seal_key = keystore
            .ed25519_generate_new(CERULEA_KEY_TYPE, Some("//Cerulea//author"))
            .expect("the keystore generates a key");
        let draw_pair = sr25519::Pair::from_seed(&[5u8; 32]);
        let mut head = header(4);
        let seed = derive_slot_seed(head.parent_hash().as_ref(), Slot(4));
        let input = DrawInput::new(Slot(4), seed);
        let claim = SealClaim::new(Slot(4), Rank::FIRST, seal_key, draw(&draw_pair, &input));
        let seal = seal_and_apply(&keystore, &mut head, claim).expect("the keystore seals it");
        assert_eq!(author_account(&seal), AccountId::from(seal_key.0));
        check_seal_signature(&head, &seal).expect("the seal checks out");
        verify_result(&input, &seal.claim.draw).expect("the draw checks out");
    }

    #[test]
    fn a_seal_from_another_key_fails_the_signature_check() {
        let keystore = keystore();
        let seal_key = keystore
            .ed25519_generate_new(CERULEA_KEY_TYPE, Some("//Cerulea//author"))
            .expect("the keystore generates a key");
        let draw_pair = sr25519::Pair::from_seed(&[5u8; 32]);
        let mut head = header(4);
        let seed = derive_slot_seed(head.parent_hash().as_ref(), Slot(4));
        let input = DrawInput::new(Slot(4), seed);
        let claim = SealClaim::new(Slot(4), Rank::FIRST, seal_key, draw(&draw_pair, &input));
        let seal = seal_and_apply(&keystore, &mut head, claim).expect("the keystore seals it");
        let mut forged = seal.clone();
        forged.claim.author = ed25519::Pair::from_seed(&[1u8; 32]).public();
        assert!(check_seal_signature(&head, &forged).is_err());
    }

    #[test]
    fn the_slot_seed_follows_the_parent_hash_and_the_slot() {
        let one = derive_slot_seed(&[1u8; 32], Slot(4));
        let two = derive_slot_seed(&[2u8; 32], Slot(4));
        let three = derive_slot_seed(&[1u8; 32], Slot(5));
        assert_ne!(one, two);
        assert_ne!(one, three);
        assert_eq!(one, derive_slot_seed(&[1u8; 32], Slot(4)));
    }
}
