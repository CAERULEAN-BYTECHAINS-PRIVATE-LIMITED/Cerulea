//! Threshold resolution for one domain.
//!
//! A domain is one group of transactions that touch overlapping state. The
//! Execution Engine forms the domains. Each transaction in a domain carries a
//! class. A domain carrying several classes commits at the share of the highest
//! class present, so the strictest member sets the bar for the whole group.
//!
//! The Commit Engine calls [`domain_commits`] on every aggregation step. The
//! call takes the aggregated Trust Score weight of the signers, the total Trust
//! Score weight of the active set, and the share the domain needs.

use super::classes::{ClassTable, TransactionClass};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;

/// A reason threshold resolution fails.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum ThresholdError {
    /// The domain holds no transaction, so no class is present.
    DomainEmpty,
    /// The active set carries no weight, so no share can be measured.
    TotalWeightZero,
}

impl ThresholdError {
    /// The sentence a person reads, naming the fix.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            ThresholdError::DomainEmpty => {
                "The domain holds no transaction. Add at least one transaction before resolving the share."
            }
            ThresholdError::TotalWeightZero => {
                "The active set carries no weight. Admit at least one validator holding standing."
            }
        }
    }
}

/// The share one domain needs, resolved from the classes present.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct DomainAssurance {
    /// The highest class any transaction in the domain carries.
    pub highest_class: TransactionClass,
    /// The share of aggregated weight the domain needs, in percent.
    pub threshold_percent: u8,
    /// The weight the signers must reach.
    pub required_weight: u128,
    /// The blocks the domain waits before the change takes effect.
    pub delay_blocks: u32,
}

impl DomainAssurance {
    /// Report whether an aggregated weight reaches the share.
    pub const fn reached(&self, aggregated_weight: u128) -> bool {
        aggregated_weight >= self.required_weight
    }

    /// Report whether the domain waits a delay window.
    pub const fn waits_delay_window(&self) -> bool {
        self.delay_blocks > 0
    }
}

/// The highest class present among a set of transactions.
///
/// The class order runs from routine through constitutional, so the highest
/// variant is the strictest member of the domain.
pub fn highest_class_present<I>(classes: I) -> Result<TransactionClass, ThresholdError>
where
    I: IntoIterator<Item = TransactionClass>,
{
    let mut highest: Option<TransactionClass> = None;
    for class in classes {
        highest = match highest {
            Some(held) if held >= class => Some(held),
            _ => Some(class),
        };
    }
    highest.ok_or(ThresholdError::DomainEmpty)
}

/// The weight the signers of a domain must reach.
///
/// The value rounds upward, so a share of 51 percent over a total of 100 needs
/// 51 and a share of 67 percent over a total of 10 needs 7. Rounding upward
/// keeps the share honest at small set sizes.
pub fn required_weight(total_weight: u128, threshold_percent: u8) -> u128 {
    let percent = threshold_percent as u128;
    let numerator = total_weight.saturating_mul(percent);
    numerator.div_ceil(100)
}

/// Resolve the share one domain needs.
///
/// The call reads the class table, finds the highest class present and returns
/// the share, the weight to reach and the delay window that class carries.
pub fn resolve_domain<I>(
    table: &ClassTable,
    classes: I,
    total_weight: u128,
) -> Result<DomainAssurance, ThresholdError>
where
    I: IntoIterator<Item = TransactionClass>,
{
    if total_weight == 0 {
        return Err(ThresholdError::TotalWeightZero);
    }
    let highest_class = highest_class_present(classes)?;
    let threshold_percent = table.threshold_percent(highest_class);
    Ok(DomainAssurance {
        highest_class,
        threshold_percent,
        required_weight: required_weight(total_weight, threshold_percent),
        delay_blocks: table.delay_blocks(highest_class),
    })
}

/// Report whether a domain commits at the aggregated weight given.
///
/// The Commit Engine calls this on every aggregation step. The domain commits
/// on the first step where the aggregated weight reaches the share.
pub fn domain_commits<I>(
    table: &ClassTable,
    classes: I,
    total_weight: u128,
    aggregated_weight: u128,
) -> Result<bool, ThresholdError>
where
    I: IntoIterator<Item = TransactionClass>,
{
    let assurance = resolve_domain(table, classes, total_weight)?;
    Ok(assurance.reached(aggregated_weight))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::assurance::classes::ClassTable;

    fn table() -> ClassTable {
        ClassTable::starting_profile()
    }

    #[test]
    fn the_highest_class_present_wins() {
        let mixed = [
            TransactionClass::Routine,
            TransactionClass::Structural,
            TransactionClass::Financial,
        ];
        assert_eq!(
            highest_class_present(mixed),
            Ok(TransactionClass::Structural)
        );
    }

    #[test]
    fn an_empty_domain_resolves_to_a_named_failure() {
        let empty: [TransactionClass; 0] = [];
        assert_eq!(
            highest_class_present(empty),
            Err(ThresholdError::DomainEmpty)
        );
        assert!(ThresholdError::DomainEmpty.plain_language().len() > 20);
    }

    #[test]
    fn a_domain_of_one_class_carries_that_share() {
        let resolved =
            resolve_domain(&table(), [TransactionClass::Financial], 1_000).expect("resolves");
        assert_eq!(resolved.highest_class, TransactionClass::Financial);
        assert_eq!(resolved.threshold_percent, 67);
        assert_eq!(resolved.required_weight, 670);
        assert!(!resolved.waits_delay_window());
    }

    #[test]
    fn a_mixed_domain_commits_at_the_strictest_share() {
        let mixed = [
            TransactionClass::Routine,
            TransactionClass::Routine,
            TransactionClass::Constitutional,
        ];
        let resolved = resolve_domain(&table(), mixed, 1_000).expect("resolves");
        assert_eq!(resolved.highest_class, TransactionClass::Constitutional);
        assert_eq!(resolved.threshold_percent, 90);
        assert_eq!(resolved.required_weight, 900);
        assert!(resolved.waits_delay_window());
    }

    #[test]
    fn the_required_weight_rounds_upward() {
        // Ten validators at one unit each. Two thirds of ten is 6.7 units.
        // Seven units commit. Six units do not.
        assert_eq!(required_weight(10, 67), 7);
        assert_eq!(required_weight(100, 51), 51);
        assert_eq!(required_weight(3, 90), 3);
    }

    #[test]
    fn a_total_weight_of_zero_resolves_to_a_named_failure() {
        assert_eq!(
            resolve_domain(&table(), [TransactionClass::Routine], 0),
            Err(ThresholdError::TotalWeightZero)
        );
        assert!(ThresholdError::TotalWeightZero.plain_language().len() > 20);
    }

    /// Design assertion 5.
    ///
    /// A routine transaction finalises before a constitutional transaction in
    /// the same block. Both domains see the same signers arriving in the same
    /// order. The routine domain reaches its share at an earlier step.
    #[test]
    fn a_routine_domain_commits_before_a_constitutional_domain() {
        let table = table();
        let total_weight = 100u128;

        let mut routine_step = None;
        let mut constitutional_step = None;

        // Signers arrive one at a time, each carrying one unit of weight.
        for step in 1..=100u128 {
            if routine_step.is_none()
                && domain_commits(&table, [TransactionClass::Routine], total_weight, step)
                    .expect("resolves")
            {
                routine_step = Some(step);
            }
            if constitutional_step.is_none()
                && domain_commits(
                    &table,
                    [TransactionClass::Constitutional],
                    total_weight,
                    step,
                )
                .expect("resolves")
            {
                constitutional_step = Some(step);
            }
        }

        assert_eq!(routine_step, Some(51));
        assert_eq!(constitutional_step, Some(90));
        assert!(routine_step < constitutional_step);
    }

    #[test]
    fn every_class_commits_at_its_own_step() {
        let table = table();
        let total_weight = 1_000u128;
        let expected = [
            (TransactionClass::Routine, 510u128),
            (TransactionClass::Financial, 670),
            (TransactionClass::Structural, 800),
            (TransactionClass::Constitutional, 900),
        ];
        for (class, weight) in expected.iter() {
            assert!(
                domain_commits(&table, [*class], total_weight, *weight).expect("resolves"),
                "class {} commits at its own share",
                class.label()
            );
            assert!(
                !domain_commits(&table, [*class], total_weight, weight - 1).expect("resolves"),
                "class {} holds below its own share",
                class.label()
            );
        }
    }

    #[test]
    fn an_amended_table_moves_the_share() {
        let mut table = table();
        table.set_threshold_percent(TransactionClass::Financial, 75);
        assert_eq!(table.inspect(), Ok(()));
        let resolved =
            resolve_domain(&table, [TransactionClass::Financial], 1_000).expect("resolves");
        assert_eq!(resolved.required_weight, 750);
    }
}
