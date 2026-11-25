//! The Assurance Engine.
//!
//! The Assurance Engine applies the share of validator weight that suits what a
//! transaction does. Three files carry the work. `classes` holds the class
//! table. `threshold` resolves the share one domain needs. `delay` enforces the
//! window a constitutional change waits.
//!
//! A domain is one group of transactions that touch overlapping state. The
//! Execution Engine forms the domains. A domain carrying several classes
//! commits at the share of the highest class present.

pub mod classes;
pub mod delay;
pub mod threshold;

pub use classes::{
    ClassEntry, ClassTable, ClassTableError, TransactionClass, CLASS_COUNT,
    CONSTITUTIONAL_DELAY_BLOCKS, CONSTITUTIONAL_THRESHOLD_PERCENT, FINANCIAL_THRESHOLD_PERCENT,
    MAX_THRESHOLD_PERCENT, MIN_THRESHOLD_PERCENT, ROUTINE_THRESHOLD_PERCENT,
    STRUCTURAL_THRESHOLD_PERCENT,
};

pub use threshold::{
    domain_commits, highest_class_present, required_weight, resolve_domain, DomainAssurance,
    ThresholdError,
};

pub use delay::{DelayError, DelayWindow, ScheduledChange};
