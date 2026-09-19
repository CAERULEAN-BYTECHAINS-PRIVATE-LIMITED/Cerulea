//! Single-round commitment per domain.
//!
//! Agreement completes in one round. The author collects signatures, combines
//! them and publishes the aggregate. A conflict domain commits as soon as the
//! aggregated Trust Score weight crosses the threshold for the highest
//! transaction class present in that domain.
//!
//! Per-domain commitment is what lets one deployment carry several agreement
//! points at once. A domain holding a warehouse scan commits at the routine
//! threshold. A domain holding a treasury movement in the same block waits for
//! the financial threshold. The scan therefore becomes irreversible ahead of
//! the movement without a second round and without a second chain.
//!
//! # Safety
//!
//! Every threshold sits above half the total weight. Two sets of signers each
//! carrying more than half the weight share at least one member. A validator
//! signs one block per slot, so two blocks at one height never both cross a
//! threshold. That is what keeps two conflicting blocks from committing
//! together.

use std::collections::BTreeMap;

use sp_core::ed25519;

use super::aggregate::AggregateSignature;
use crate::engines::{
    CommitEngine, ConflictDomain, Engine, EngineError, EngineKind, Slot, TransactionClass,
    TrustScore, CONSTITUTIONAL_THRESHOLD_PERCENT, FINANCIAL_THRESHOLD_PERCENT,
    ROUTINE_THRESHOLD_PERCENT, STRUCTURAL_THRESHOLD_PERCENT,
};

/// The lowest threshold any class carries, in percent.
///
/// Every threshold sits above half the total weight. Two signer sets above half
/// share a member, so two conflicting blocks never both commit.
pub const THRESHOLD_FLOOR_PERCENT: u8 = 51;

/// The threshold each transaction class requires.
///
/// The deployment profile carries this table. The Assurance Engine applies it.
/// The Amendment Engine governs a change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThresholdTable {
    routine: u8,
    financial: u8,
    structural: u8,
    constitutional: u8,
}

impl Default for ThresholdTable {
    fn default() -> Self {
        ThresholdTable::standard()
    }
}

impl ThresholdTable {
    /// The table a deployment starts from.
    pub const fn standard() -> Self {
        ThresholdTable {
            routine: ROUTINE_THRESHOLD_PERCENT,
            financial: FINANCIAL_THRESHOLD_PERCENT,
            structural: STRUCTURAL_THRESHOLD_PERCENT,
            constitutional: CONSTITUTIONAL_THRESHOLD_PERCENT,
        }
    }

    /// Build a table from four percentages.
    ///
    /// Each value holds at or above the floor and at or below one hundred. A
    /// value below the floor rises to the floor.
    pub fn new(routine: u8, financial: u8, structural: u8, constitutional: u8) -> Self {
        let hold = |percent: u8| percent.clamp(THRESHOLD_FLOOR_PERCENT, 100);
        ThresholdTable {
            routine: hold(routine),
            financial: hold(financial),
            structural: hold(structural),
            constitutional: hold(constitutional),
        }
    }

    /// The threshold one class requires, in percent.
    pub const fn percent_for(&self, class: TransactionClass) -> u8 {
        match class {
            TransactionClass::Routine => self.routine,
            TransactionClass::Financial => self.financial,
            TransactionClass::Structural => self.structural,
            TransactionClass::Constitutional => self.constitutional,
        }
    }

    /// The table as the four class entries.
    pub const fn entries(&self) -> [(TransactionClass, u8); 4] {
        [
            (TransactionClass::Routine, self.routine),
            (TransactionClass::Financial, self.financial),
            (TransactionClass::Structural, self.structural),
            (TransactionClass::Constitutional, self.constitutional),
        ]
    }

    /// Report whether every entry sits at or above the floor.
    pub const fn respects_the_floor(&self) -> bool {
        self.routine >= THRESHOLD_FLOOR_PERCENT
            && self.financial >= THRESHOLD_FLOOR_PERCENT
            && self.structural >= THRESHOLD_FLOOR_PERCENT
            && self.constitutional >= THRESHOLD_FLOOR_PERCENT
    }
}

/// Report whether a weight crosses a threshold over a total.
///
/// The comparison multiplies rather than divides, so no rounding step loses a
/// vote.
pub const fn crosses(signed: u128, total: u128, percent: u8) -> bool {
    if total == 0 {
        return false;
    }
    signed.saturating_mul(100) >= total.saturating_mul(percent as u128)
}

/// What one domain reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomainCommitment {
    /// The position the domain held in the domain list.
    pub domain: usize,
    /// The highest transaction class the domain carried.
    pub highest_class: TransactionClass,
    /// The share the class requires, in percent.
    pub required_percent: u8,
    /// The Trust Score weight the signers carried.
    pub signed_weight: u128,
    /// The Trust Score weight the whole set carries.
    pub total_weight: u128,
    /// Whether the domain committed.
    pub committed: bool,
}

/// The Commit Engine tally for one block.
///
/// The tally holds the weight each signer carries. A validator counts once. The
/// tally answers whether a class threshold is crossed.
#[derive(Clone, Debug)]
pub struct DomainCommitter {
    slot: Slot,
    block_hash: [u8; 32],
    table: ThresholdTable,
    total_weight: u128,
    signers: BTreeMap<ed25519::Public, TrustScore>,
}

impl DomainCommitter {
    /// Open a tally for one block.
    pub fn open(
        slot: Slot,
        block_hash: [u8; 32],
        total_weight: u128,
        table: ThresholdTable,
    ) -> Result<Self, EngineError> {
        if total_weight == 0 {
            return Err(EngineError::EmptyValidatorSet);
        }
        Ok(DomainCommitter {
            slot,
            block_hash,
            table,
            total_weight,
            signers: BTreeMap::new(),
        })
    }

    /// Open a tally over a validator set.
    pub fn over_set(
        slot: Slot,
        block_hash: [u8; 32],
        weights: &[TrustScore],
        table: ThresholdTable,
    ) -> Result<Self, EngineError> {
        let total = weights.iter().map(|weight| weight.weight()).sum();
        DomainCommitter::open(slot, block_hash, total, table)
    }

    /// The slot this tally serves.
    pub fn slot(&self) -> Slot {
        self.slot
    }

    /// The block this tally serves.
    pub fn block_hash(&self) -> [u8; 32] {
        self.block_hash
    }

    /// The threshold table this tally applies.
    pub fn table(&self) -> ThresholdTable {
        self.table
    }

    /// The Trust Score weight the signers carry.
    pub fn signed_weight(&self) -> u128 {
        self.signers.values().map(|weight| weight.weight()).sum()
    }

    /// The Trust Score weight the whole set carries.
    pub fn total_weight(&self) -> u128 {
        self.total_weight
    }

    /// The number of validators that signed.
    pub fn signer_count(&self) -> usize {
        self.signers.len()
    }

    /// The share the signers carry, in percent.
    pub fn signed_percent(&self) -> u8 {
        if self.total_weight == 0 {
            return 0;
        }
        let share = self.signed_weight().saturating_mul(100) / self.total_weight;
        share.min(100) as u8
    }

    /// Take the signers from one published aggregate.
    ///
    /// The set arrives in the order the collector held. The call checks the
    /// envelope before it counts anything.
    pub fn absorb_aggregate(
        &mut self,
        aggregate: &AggregateSignature,
        members: &[ed25519::Public],
        weights: &[TrustScore],
    ) -> Result<usize, EngineError> {
        if aggregate.slot != self.slot {
            return Err(EngineError::VoteSlotMismatch {
                claimed: aggregate.slot.0,
                expected: self.slot.0,
            });
        }
        if aggregate.block_hash != self.block_hash {
            return Err(EngineError::VoteBlockMismatch { slot: self.slot.0 });
        }
        aggregate.verify(members)?;
        let mut taken = 0;
        for position in aggregate.signers.positions() {
            let validator = match members.get(position) {
                Some(key) => *key,
                None => continue,
            };
            let weight = weights.get(position).copied().unwrap_or_default();
            if self.add_signer(validator, weight)? {
                taken += 1;
            }
        }
        Ok(taken)
    }

    /// Settle one domain against the tally.
    pub fn settle(&self, domain: usize, highest_class: TransactionClass) -> DomainCommitment {
        let required_percent = self.table.percent_for(highest_class);
        let signed_weight = self.signed_weight();
        DomainCommitment {
            domain,
            highest_class,
            required_percent,
            signed_weight,
            total_weight: self.total_weight,
            committed: crosses(signed_weight, self.total_weight, required_percent),
        }
    }

    /// Settle every domain of one block against the tally.
    pub fn settle_all(&self, domains: &[ConflictDomain]) -> Vec<DomainCommitment> {
        domains
            .iter()
            .enumerate()
            .map(|(position, domain)| self.settle(position, domain.highest_class))
            .collect()
    }
}

impl Engine for DomainCommitter {
    const KIND: EngineKind = EngineKind::Commit;
}

impl CommitEngine for DomainCommitter {
    type ValidatorId = ed25519::Public;

    fn add_signer(
        &mut self,
        validator: Self::ValidatorId,
        weight: TrustScore,
    ) -> Result<bool, EngineError> {
        if self.signers.contains_key(&validator) {
            return Ok(false);
        }
        self.signers.insert(validator, weight);
        Ok(true)
    }

    fn crossed(&self, class: TransactionClass) -> bool {
        crosses(
            self.signed_weight(),
            self.total_weight,
            self.table.percent_for(class),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::commit::collect::{cast_vote, SignatureCollector};
    use crate::engines::DEFAULT_THRESHOLD_TABLE;
    use crate::CERULEA_KEY_TYPE;
    use sp_keystore::testing::MemoryKeystore;
    use sp_keystore::{Keystore, KeystorePtr};
    use std::sync::Arc;

    const SLOT: Slot = Slot(120);
    const BLOCK: [u8; 32] = [31u8; 32];

    struct Network {
        keystores: Vec<KeystorePtr>,
        keys: Vec<ed25519::Public>,
        weights: Vec<TrustScore>,
    }

    fn build(count: usize) -> Network {
        let mut keystores = Vec::new();
        let mut keys = Vec::new();
        for index in 0..count {
            let memory = MemoryKeystore::new();
            let key = memory
                .ed25519_generate_new(
                    CERULEA_KEY_TYPE,
                    Some(&format!("//Cerulea//commit//{}", index)),
                )
                .expect("the keystore generates a key");
            keystores.push(Arc::new(memory) as KeystorePtr);
            keys.push(key);
        }
        let weights = vec![TrustScore::from_percent(100); count];
        Network {
            keystores,
            keys,
            weights,
        }
    }

    fn committer(network: &Network) -> DomainCommitter {
        DomainCommitter::over_set(SLOT, BLOCK, &network.weights, ThresholdTable::standard())
            .expect("the tally opens")
    }

    fn sign_into(network: &Network, committer: &mut DomainCommitter, count: usize) {
        for index in 0..count {
            committer
                .add_signer(network.keys[index], network.weights[index])
                .expect("the signer counts");
        }
    }

    #[test]
    fn the_standard_table_holds_the_four_thresholds() {
        let table = ThresholdTable::standard();
        assert_eq!(table.entries(), DEFAULT_THRESHOLD_TABLE);
        assert_eq!(table.percent_for(TransactionClass::Routine), 51);
        assert_eq!(table.percent_for(TransactionClass::Financial), 67);
        assert_eq!(table.percent_for(TransactionClass::Structural), 80);
        assert_eq!(table.percent_for(TransactionClass::Constitutional), 90);
        assert!(table.respects_the_floor());
        assert_eq!(ThresholdTable::default(), table);
    }

    #[test]
    fn a_threshold_below_the_floor_rises_to_the_floor() {
        let table = ThresholdTable::new(10, 20, 30, 200);
        assert_eq!(table.percent_for(TransactionClass::Routine), 51);
        assert_eq!(table.percent_for(TransactionClass::Constitutional), 100);
        assert!(table.respects_the_floor());
    }

    #[test]
    fn a_routine_domain_commits_before_a_constitutional_domain() {
        let network = build(100);
        let mut committer = committer(&network);
        let domains = vec![
            ConflictDomain {
                members: vec![0],
                highest_class: TransactionClass::Routine,
            },
            ConflictDomain {
                members: vec![1],
                highest_class: TransactionClass::Constitutional,
            },
        ];
        sign_into(&network, &mut committer, 60);
        let settled = committer.settle_all(&domains);
        assert!(settled[0].committed, "the routine domain commits at sixty");
        assert!(
            !settled[1].committed,
            "the constitutional domain still waits at sixty"
        );
        assert_eq!(settled[0].required_percent, 51);
        assert_eq!(settled[1].required_percent, 90);

        sign_into(&network, &mut committer, 90);
        let settled = committer.settle_all(&domains);
        assert!(settled[0].committed);
        assert!(
            settled[1].committed,
            "ninety carries the constitutional domain"
        );
    }

    #[test]
    fn each_class_crosses_at_its_own_share() {
        let network = build(100);
        let mut committer = committer(&network);
        sign_into(&network, &mut committer, 51);
        assert!(committer.crossed(TransactionClass::Routine));
        assert!(!committer.crossed(TransactionClass::Financial));
        sign_into(&network, &mut committer, 67);
        assert!(committer.crossed(TransactionClass::Financial));
        assert!(!committer.crossed(TransactionClass::Structural));
        sign_into(&network, &mut committer, 80);
        assert!(committer.crossed(TransactionClass::Structural));
        assert!(!committer.crossed(TransactionClass::Constitutional));
        sign_into(&network, &mut committer, 90);
        assert!(committer.crossed(TransactionClass::Constitutional));
        assert_eq!(committer.signer_count(), 90);
        assert_eq!(committer.signed_percent(), 90);
    }

    #[test]
    fn a_second_signature_from_one_validator_leaves_the_tally_unchanged() {
        let network = build(10);
        let mut committer = committer(&network);
        assert!(committer
            .add_signer(network.keys[0], network.weights[0])
            .expect("the signer counts"));
        assert!(!committer
            .add_signer(network.keys[0], network.weights[0])
            .expect("the signer counts"));
        assert_eq!(committer.signer_count(), 1);
    }

    #[test]
    fn two_conflicting_blocks_never_both_commit_at_one_height() {
        let network = build(9);
        let other_block = [32u8; 32];
        // Every validator signs one block of the two. The two tallies split the
        // set in every way, so the test walks each split.
        for pattern in 0u16..(1 << 9) {
            let mut first = DomainCommitter::over_set(
                SLOT,
                BLOCK,
                &network.weights,
                ThresholdTable::standard(),
            )
            .expect("the tally opens");
            let mut second = DomainCommitter::over_set(
                SLOT,
                other_block,
                &network.weights,
                ThresholdTable::standard(),
            )
            .expect("the tally opens");
            for index in 0..9 {
                if pattern & (1 << index) == 0 {
                    first
                        .add_signer(network.keys[index], network.weights[index])
                        .expect("the signer counts");
                } else {
                    second
                        .add_signer(network.keys[index], network.weights[index])
                        .expect("the signer counts");
                }
            }
            let both = first.crossed(TransactionClass::Routine)
                && second.crossed(TransactionClass::Routine);
            assert!(!both, "two conflicting blocks never both commit");
        }
    }

    #[test]
    fn a_domain_commits_at_the_highest_class_it_carries() {
        let network = build(100);
        let mut committer = committer(&network);
        sign_into(&network, &mut committer, 70);
        let mixed = ConflictDomain {
            members: vec![0, 1],
            highest_class: TransactionClass::Structural,
        };
        let settled = committer.settle(0, mixed.highest_class);
        assert_eq!(settled.required_percent, 80);
        assert!(!settled.committed);
        assert_eq!(settled.highest_class, TransactionClass::Structural);
        assert_eq!(settled.signed_weight, 70 * TrustScore::FULL as u128);
        assert_eq!(settled.total_weight, 100 * TrustScore::FULL as u128);
    }

    #[test]
    fn a_published_aggregate_carries_its_signers_into_the_tally() {
        let network = build(6);
        let set: Vec<(ed25519::Public, TrustScore)> = network
            .keys
            .iter()
            .zip(network.weights.iter())
            .map(|(key, weight)| (*key, *weight))
            .collect();
        let mut collector =
            SignatureCollector::open(SLOT, BLOCK, &set, 0, 1_000).expect("the collector opens");
        let members = collector.members().to_vec();
        for member in &members {
            let position = network
                .keys
                .iter()
                .position(|key| key == member)
                .expect("the member sits in the set");
            let vote = cast_vote(&network.keystores[position], member, SLOT, BLOCK)
                .expect("the validator signs");
            collector.accept(vote, 1).expect("the vote lands");
        }
        let aggregate = AggregateSignature::assemble(&collector).expect("the envelope assembles");
        let weights = vec![TrustScore::from_percent(100); members.len()];
        let mut committer =
            DomainCommitter::over_set(SLOT, BLOCK, &weights, ThresholdTable::standard())
                .expect("the tally opens");
        let taken = committer
            .absorb_aggregate(&aggregate, &members, &weights)
            .expect("the aggregate lands");
        assert_eq!(taken, 6);
        assert!(committer.crossed(TransactionClass::Constitutional));
        // A second pass counts nothing twice.
        assert_eq!(
            committer
                .absorb_aggregate(&aggregate, &members, &weights)
                .expect("the aggregate lands"),
            0
        );
    }

    #[test]
    fn an_aggregate_for_another_block_is_rejected() {
        let network = build(3);
        let set: Vec<(ed25519::Public, TrustScore)> = network
            .keys
            .iter()
            .map(|key| (*key, TrustScore::from_percent(100)))
            .collect();
        let collector = SignatureCollector::open(SLOT, [77u8; 32], &set, 0, 1_000)
            .expect("the collector opens");
        let aggregate = AggregateSignature::assemble(&collector).expect("the envelope assembles");
        let members = collector.members().to_vec();
        let weights = vec![TrustScore::from_percent(100); members.len()];
        let mut committer =
            DomainCommitter::over_set(SLOT, BLOCK, &weights, ThresholdTable::standard())
                .expect("the tally opens");
        let error = committer
            .absorb_aggregate(&aggregate, &members, &weights)
            .expect_err("another block fails");
        assert!(matches!(error, EngineError::VoteBlockMismatch { .. }));
    }

    #[test]
    fn a_tally_over_an_empty_set_names_the_fix() {
        let error = DomainCommitter::over_set(SLOT, BLOCK, &[], ThresholdTable::standard())
            .expect_err("an empty set reports a problem");
        assert_eq!(error, EngineError::EmptyValidatorSet);
    }

    #[test]
    fn the_crossing_check_rounds_no_vote_away() {
        // Three validators of equal weight. Two of three is sixty six percent,
        // which crosses fifty one and falls short of sixty seven.
        assert!(crosses(2, 3, 51));
        assert!(!crosses(2, 3, 67));
        assert!(crosses(3, 3, 100));
        assert!(!crosses(0, 3, 51));
        assert!(!crosses(1, 0, 51));
    }

    #[test]
    fn the_engine_names_itself() {
        assert_eq!(DomainCommitter::KIND, EngineKind::Commit);
        assert_eq!(DomainCommitter::engine_name(), "Commit Engine");
        let network = build(3);
        let committer = committer(&network);
        assert_eq!(committer.slot(), SLOT);
        assert_eq!(committer.block_hash(), BLOCK);
        assert_eq!(committer.table(), ThresholdTable::standard());
        assert_eq!(committer.total_weight(), 3 * TrustScore::FULL as u128);
    }
}
