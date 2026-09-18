//! The Execution Engine.
//!
//! The Execution Engine groups pending transactions into conflict domains, runs
//! each domain on its own core, and produces one state root per domain beside a
//! combined root. Each transaction carries hard limits on computation, memory
//! and storage. A transaction that crosses a limit aborts on its own and the
//! rest of the domain carries on.

pub mod ordering;
pub mod partition;
pub mod runner;

pub use ordering::{order_domains, order_members, OrderKey};
pub use partition::{DisjointSet, DomainPartitioner};
pub use runner::{
    DomainOutcome, DomainRunner, DomainStore, ExecutionReport, TransactionOutcome,
    TransactionRunner,
};
