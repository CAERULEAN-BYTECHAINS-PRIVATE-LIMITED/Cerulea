//! The Execution Engine.
//!
//! The Execution Engine groups pending transactions into conflict domains, runs
//! each domain on its own core, and produces one state root per domain beside a
//! combined root. Each transaction carries hard limits on computation, memory
//! and storage. A transaction that crosses a limit aborts on its own and the
//! rest of the domain carries on.

pub mod partition;

pub use partition::{DisjointSet, DomainPartitioner};
