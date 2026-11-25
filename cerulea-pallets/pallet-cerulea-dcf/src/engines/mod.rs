//! The pallet-side engines of the Dynamic Consensus Framework.
//!
//! The Dynamic Consensus Framework is the agreement mechanism of the Cerulea
//! Blockchain. Eight engines carry the work. Three of them run inside the node
//! and hold the block pipeline. Five of them run inside the pallet set and hold
//! the rules the chain enforces.
//!
//! | Engine | Where it runs |
//! |---|---|
//! | Commit Engine | Consensus crate |
//! | Authoring Engine | Consensus crate |
//! | Execution Engine | Consensus crate |
//! | Assurance Engine | This module, `assurance` |
//! | Standing Engine | This module, `standing` |
//! | Amendment Engine | This module, `amendment` |
//! | Custody Engine | The custody pallet |
//! | Admission Engine | The validator registry pallet |
//!
//! Every engine here works in `no_std`. Each one exposes plain functions over
//! plain values, so the same code answers a chain call and an off-chain
//! question.

pub mod amendment;
pub mod assurance;
pub mod standing;
