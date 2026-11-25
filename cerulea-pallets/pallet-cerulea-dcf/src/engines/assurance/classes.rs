//! The transaction class table of the Assurance Engine.
//!
//! The Assurance Engine applies the share of validator weight that suits what a
//! transaction does. A warehouse scan and a treasury movement run on one chain
//! under one framework. The scan commits at a low share. The treasury movement
//! commits at a high share and waits a delay window.
//!
//! A transaction class is a label the Blueprint action definition carries. The
//! class names the share of aggregated Trust Score weight the transaction needs
//! before commitment. Four classes exist.
//!
//! | Class | Covers | Share of weight | Delay |
//! |---|---|---|---|
//! | Routine | Scans, readings, log entries | 51 percent | none |
//! | Financial | Transfers, settlements, payments | 67 percent | none |
//! | Structural | Schema, permission and validator set changes | 80 percent | none |
//! | Constitutional | Policy amendment, upgrade authority | 90 percent | mandatory window |
//!
//! The table above is the profile a deployment starts from. The Amendment
//! Engine governs every later change to it. The node-side Commit Engine holds
//! the same four shares as plain constants. The two stay equal by review.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;

/// The number of transaction classes.
pub const CLASS_COUNT: usize = 4;

/// The share of aggregated weight a routine transaction needs, in percent.
pub const ROUTINE_THRESHOLD_PERCENT: u8 = 51;

/// The share of aggregated weight a financial transaction needs, in percent.
pub const FINANCIAL_THRESHOLD_PERCENT: u8 = 67;

/// The share of aggregated weight a structural transaction needs, in percent.
pub const STRUCTURAL_THRESHOLD_PERCENT: u8 = 80;

/// The share of aggregated weight a constitutional transaction needs, in
/// percent.
pub const CONSTITUTIONAL_THRESHOLD_PERCENT: u8 = 90;

/// The blocks a constitutional change waits before it takes effect.
///
/// The window gives every participant time to read the change and respond
/// before the change binds. One hundred blocks matches one epoch at the
/// starting epoch length.
pub const CONSTITUTIONAL_DELAY_BLOCKS: u32 = 100;

/// The lowest share of aggregated weight any class may carry.
///
/// A share at or below half lets two disjoint groups each reach the share, so
/// two conflicting results could commit at one height. Fifty one percent is the
/// lowest share that keeps a single result.
pub const MIN_THRESHOLD_PERCENT: u8 = 51;

/// The highest share of aggregated weight any class may carry.
///
/// A share of one hundred percent halts every commitment as soon as one
/// validator falls silent. Ninety nine percent keeps progress possible.
pub const MAX_THRESHOLD_PERCENT: u8 = 99;

/// The class of a transaction.
///
/// The Blueprint action definition carries the class. Higher variants demand a
/// higher share of aggregated weight. The order of the variants is the order of
/// severity, so a plain comparison finds the highest class present.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub enum TransactionClass {
    /// Scans, readings and log entries.
    #[default]
    Routine,
    /// Transfers, settlements and payments.
    Financial,
    /// Schema, permission and validator set changes.
    Structural,
    /// Policy amendment and upgrade authority.
    Constitutional,
}

impl TransactionClass {
    /// Every class, in order of severity.
    pub const ALL: [TransactionClass; CLASS_COUNT] = [
        TransactionClass::Routine,
        TransactionClass::Financial,
        TransactionClass::Structural,
        TransactionClass::Constitutional,
    ];

    /// The position of this class inside the table.
    pub const fn index(&self) -> usize {
        match self {
            TransactionClass::Routine => 0,
            TransactionClass::Financial => 1,
            TransactionClass::Structural => 2,
            TransactionClass::Constitutional => 3,
        }
    }

    /// Build a class from its position inside the table.
    pub const fn from_index(index: usize) -> Option<TransactionClass> {
        match index {
            0 => Some(TransactionClass::Routine),
            1 => Some(TransactionClass::Financial),
            2 => Some(TransactionClass::Structural),
            3 => Some(TransactionClass::Constitutional),
            _ => None,
        }
    }

    /// A short label for logs and interface copy.
    pub const fn label(&self) -> &'static str {
        match self {
            TransactionClass::Routine => "routine",
            TransactionClass::Financial => "financial",
            TransactionClass::Structural => "structural",
            TransactionClass::Constitutional => "constitutional",
        }
    }

    /// A plain sentence stating what this class covers.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            TransactionClass::Routine => {
                "Scans, readings and log entries. A simple majority of weight commits the record."
            }
            TransactionClass::Financial => {
                "Transfers, settlements and payments. Two thirds of weight commits the record."
            }
            TransactionClass::Structural => {
                "Schema, permission and validator set changes. Four fifths of weight commits the record."
            }
            TransactionClass::Constitutional => {
                "Policy amendment and upgrade authority. Nine tenths of weight commits the record, and the change waits a delay window."
            }
        }
    }
}

/// One row of the class table.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct ClassEntry {
    /// The class this row describes.
    pub class: TransactionClass,
    /// The share of aggregated weight the class needs, in percent.
    pub threshold_percent: u8,
    /// The blocks the class waits before the change takes effect.
    pub delay_blocks: u32,
}

impl ClassEntry {
    /// Report whether this class waits a delay window.
    pub const fn waits_delay_window(&self) -> bool {
        self.delay_blocks > 0
    }
}

/// A reason a class table fails inspection.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum ClassTableError {
    /// A row names a class that another row already named.
    DuplicateClass,
    /// A row is absent for one of the four classes.
    MissingClass,
    /// A share sits below the lowest share a class may carry.
    ThresholdTooLow,
    /// A share sits above the highest share a class may carry.
    ThresholdTooHigh,
    /// A higher class carries a share at or below a lower class.
    ThresholdOrderBroken,
    /// The constitutional class carries no delay window.
    ConstitutionalDelayAbsent,
}

impl ClassTableError {
    /// The sentence a person reads, naming the fix.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            ClassTableError::DuplicateClass => {
                "Two rows name one class. Give each of the four classes one row."
            }
            ClassTableError::MissingClass => {
                "A class carries no row. Give each of the four classes one row."
            }
            ClassTableError::ThresholdTooLow => {
                "A share sits below 51 percent. Raise that share to 51 percent or above."
            }
            ClassTableError::ThresholdTooHigh => {
                "A share sits above 99 percent. Lower that share to 99 percent or below."
            }
            ClassTableError::ThresholdOrderBroken => {
                "A stricter class carries a share at or below a lighter class. Raise the stricter share."
            }
            ClassTableError::ConstitutionalDelayAbsent => {
                "The constitutional class carries no delay window. Set the delay to one block or more."
            }
        }
    }
}

/// The class table of one deployment.
///
/// The table holds one row for each of the four classes. The Assurance Engine
/// reads the table on every domain commitment. The Amendment Engine governs
/// every change to the table.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct ClassTable {
    /// One row for each class, in order of severity.
    pub entries: [ClassEntry; CLASS_COUNT],
}

impl ClassTable {
    /// The profile a deployment starts from.
    ///
    /// Routine 51 percent. Financial 67 percent. Structural 80 percent.
    /// Constitutional 90 percent with a mandatory delay window.
    pub const fn starting_profile() -> Self {
        ClassTable {
            entries: [
                ClassEntry {
                    class: TransactionClass::Routine,
                    threshold_percent: ROUTINE_THRESHOLD_PERCENT,
                    delay_blocks: 0,
                },
                ClassEntry {
                    class: TransactionClass::Financial,
                    threshold_percent: FINANCIAL_THRESHOLD_PERCENT,
                    delay_blocks: 0,
                },
                ClassEntry {
                    class: TransactionClass::Structural,
                    threshold_percent: STRUCTURAL_THRESHOLD_PERCENT,
                    delay_blocks: 0,
                },
                ClassEntry {
                    class: TransactionClass::Constitutional,
                    threshold_percent: CONSTITUTIONAL_THRESHOLD_PERCENT,
                    delay_blocks: CONSTITUTIONAL_DELAY_BLOCKS,
                },
            ],
        }
    }

    /// The row for one class.
    pub fn entry(&self, class: TransactionClass) -> ClassEntry {
        self.entries[class.index()]
    }

    /// The share of aggregated weight one class needs, in percent.
    pub fn threshold_percent(&self, class: TransactionClass) -> u8 {
        self.entry(class).threshold_percent
    }

    /// The blocks one class waits before the change takes effect.
    pub fn delay_blocks(&self, class: TransactionClass) -> u32 {
        self.entry(class).delay_blocks
    }

    /// Replace the share one class carries.
    ///
    /// The caller inspects the table afterwards. A table that fails inspection
    /// never reaches storage.
    pub fn set_threshold_percent(&mut self, class: TransactionClass, percent: u8) {
        self.entries[class.index()].threshold_percent = percent;
    }

    /// Replace the delay window one class carries.
    pub fn set_delay_blocks(&mut self, class: TransactionClass, blocks: u32) {
        self.entries[class.index()].delay_blocks = blocks;
    }

    /// Inspect the table against every rule the Assurance Engine holds.
    ///
    /// Each of the four classes carries exactly one row. Each share sits inside
    /// the permitted span. A stricter class carries a strictly higher share.
    /// The constitutional class carries a delay window of one block or more.
    pub fn inspect(&self) -> Result<(), ClassTableError> {
        let mut seen = [false; CLASS_COUNT];
        for entry in self.entries.iter() {
            if seen[entry.class.index()] {
                return Err(ClassTableError::DuplicateClass);
            }
            seen[entry.class.index()] = true;

            if entry.threshold_percent < MIN_THRESHOLD_PERCENT {
                return Err(ClassTableError::ThresholdTooLow);
            }
            if entry.threshold_percent > MAX_THRESHOLD_PERCENT {
                return Err(ClassTableError::ThresholdTooHigh);
            }
        }
        if seen.iter().any(|held| !held) {
            return Err(ClassTableError::MissingClass);
        }

        let mut previous = 0u8;
        for class in TransactionClass::ALL.iter() {
            let share = self.threshold_percent(*class);
            if share <= previous && *class != TransactionClass::Routine {
                return Err(ClassTableError::ThresholdOrderBroken);
            }
            previous = share;
        }

        if self.delay_blocks(TransactionClass::Constitutional) == 0 {
            return Err(ClassTableError::ConstitutionalDelayAbsent);
        }
        Ok(())
    }
}

impl Default for ClassTable {
    fn default() -> Self {
        ClassTable::starting_profile()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_starting_profile_carries_the_four_shares() {
        let table = ClassTable::starting_profile();
        assert_eq!(table.threshold_percent(TransactionClass::Routine), 51);
        assert_eq!(table.threshold_percent(TransactionClass::Financial), 67);
        assert_eq!(table.threshold_percent(TransactionClass::Structural), 80);
        assert_eq!(
            table.threshold_percent(TransactionClass::Constitutional),
            90
        );
    }

    #[test]
    fn the_constitutional_class_alone_waits_a_delay_window() {
        let table = ClassTable::starting_profile();
        assert_eq!(table.delay_blocks(TransactionClass::Routine), 0);
        assert_eq!(table.delay_blocks(TransactionClass::Financial), 0);
        assert_eq!(table.delay_blocks(TransactionClass::Structural), 0);
        assert_eq!(
            table.delay_blocks(TransactionClass::Constitutional),
            CONSTITUTIONAL_DELAY_BLOCKS
        );
        assert!(table
            .entry(TransactionClass::Constitutional)
            .waits_delay_window());
        assert!(!table.entry(TransactionClass::Routine).waits_delay_window());
    }

    #[test]
    fn the_starting_profile_passes_inspection() {
        assert_eq!(ClassTable::starting_profile().inspect(), Ok(()));
    }

    #[test]
    fn a_share_below_the_lowest_permitted_fails_inspection() {
        let mut table = ClassTable::starting_profile();
        table.set_threshold_percent(TransactionClass::Routine, 50);
        assert_eq!(table.inspect(), Err(ClassTableError::ThresholdTooLow));
    }

    #[test]
    fn a_share_above_the_highest_permitted_fails_inspection() {
        let mut table = ClassTable::starting_profile();
        table.set_threshold_percent(TransactionClass::Constitutional, 100);
        assert_eq!(table.inspect(), Err(ClassTableError::ThresholdTooHigh));
    }

    #[test]
    fn a_stricter_class_below_a_lighter_class_fails_inspection() {
        let mut table = ClassTable::starting_profile();
        table.set_threshold_percent(TransactionClass::Financial, 51);
        assert_eq!(table.inspect(), Err(ClassTableError::ThresholdOrderBroken));
    }

    #[test]
    fn a_constitutional_class_without_a_window_fails_inspection() {
        let mut table = ClassTable::starting_profile();
        table.set_delay_blocks(TransactionClass::Constitutional, 0);
        assert_eq!(
            table.inspect(),
            Err(ClassTableError::ConstitutionalDelayAbsent)
        );
    }

    #[test]
    fn class_order_runs_from_routine_to_constitutional() {
        assert!(TransactionClass::Routine < TransactionClass::Financial);
        assert!(TransactionClass::Financial < TransactionClass::Structural);
        assert!(TransactionClass::Structural < TransactionClass::Constitutional);
    }

    #[test]
    fn every_class_maps_to_its_position_and_back() {
        for class in TransactionClass::ALL.iter() {
            assert_eq!(TransactionClass::from_index(class.index()), Some(*class));
        }
        assert_eq!(TransactionClass::from_index(CLASS_COUNT), None);
    }

    #[test]
    fn every_inspection_failure_names_a_fix() {
        let failures = [
            ClassTableError::DuplicateClass,
            ClassTableError::MissingClass,
            ClassTableError::ThresholdTooLow,
            ClassTableError::ThresholdTooHigh,
            ClassTableError::ThresholdOrderBroken,
            ClassTableError::ConstitutionalDelayAbsent,
        ];
        for failure in failures.iter() {
            assert!(failure.plain_language().len() > 20);
        }
    }
}
