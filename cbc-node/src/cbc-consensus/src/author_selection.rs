//! Author selection implementation for the consensus engine

use crate::error::{ConsensusError, ConsensusResult};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::NumberFor;
use sp_core::ed25519::Public;
use cbc_runtime::AccountId;
use pallet_cbc_dcf::DcfApi as RuntimeDcfApi;
use std::sync::Arc;

/// Fetch the expected block author for a given block number/slot using the DCF runtime API.
pub fn get_expected_author<B, C>(client: Arc<C>, block_number: u32) -> ConsensusResult<Public>
where
    B: sp_runtime::traits::Block,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
{
    let api = client.runtime_api();
    let best_hash = client.info().best_hash;
    match api.get_expected_author(best_hash, block_number) {
        Ok(Some(account_id)) => Ok(Public::from_raw(*account_id.as_ref())),
        Ok(None) => Err(ConsensusError::AuthorSelection("No expected author returned by runtime".into())),
        Err(e) => Err(ConsensusError::AuthorSelection(format!("Runtime API error: {:?}", e))),
    }
}