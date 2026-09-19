//! The verifier of every target network.
//!
//! Every target network of the specification sits here. All seven are
//! delivered together. Each module holds the proof shape of its network,
//! the verifier that checks the proof and the builders the off-chain
//! driver mirrors.
//!
//! | Module | Network | What the acceptance rests on |
//! |---|---|---|
//! | `evm` | Networks running Ethereum Virtual Machine bytecode | A trie walk under an attested header |
//! | `cosmos` | Cosmos networks | A signed commit of the validator set |
//! | `solana` | Solana networks | A threshold of registered relayers |
//! | `polkadot` | Polkadot networks | A settlement signed by the authority set |
//! | `avalanche` | Avalanche networks | A share of the validator weight |
//! | `bitcoin` | The Bitcoin network | The work above the block |
//! | `rollup` | Layer 2 rollups | A settled output root past its window |

pub mod avalanche;
pub mod bitcoin;
pub mod cosmos;
pub mod evm;
pub mod polkadot;
pub mod rollup;
pub mod solana;
