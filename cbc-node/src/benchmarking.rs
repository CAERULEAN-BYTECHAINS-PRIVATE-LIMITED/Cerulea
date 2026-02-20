//! Setup code for [`super::command`] which would otherwise bloat that module.
//!
//! Should only be used for benchmarking as it may break in other contexts.

use crate::service::FullClient;

use runtime::{AccountId, Balance, BalancesCall, SystemCall};
use sc_cli::Result;
use sc_client_api::BlockBackend;
use cbc_runtime as runtime;
use sp_core::{Encode, Pair};
use sp_inherents::{InherentData, InherentDataProvider};
use sp_keyring::Ed25519Keyring;
use sp_runtime::{OpaqueExtrinsic, SaturatedConversion};

use std::{sync::Arc, time::Duration};

/// Generates extrinsics for the `benchmark overhead` command.
///
/// Note: Should only be used for benchmarking.
pub struct RemarkBuilder {
    client: Arc<FullClient>, // Shared reference to the blockchain client
}

impl RemarkBuilder {
    /// Creates a new [`Self`] from the given client.
    ///
    /// # Arguments
    /// * `client` - A shared reference to the blockchain client.
    pub fn new(client: Arc<FullClient>) -> Self {
        Self { client }
    }
}

impl frame_benchmarking_cli::ExtrinsicBuilder for RemarkBuilder {
    /// Returns the name of the pallet being benchmarked.
    fn pallet(&self) -> &str {
        "system"
    }

    /// Returns the name of the extrinsic being benchmarked.
    fn extrinsic(&self) -> &str {
        "remark"
    }

    /// Builds an `OpaqueExtrinsic` for the `remark` call.
    ///
    /// # Arguments
    /// * `nonce` - The transaction nonce.
    fn build(&self, nonce: u32) -> std::result::Result<OpaqueExtrinsic, &'static str> {
        let acc = Ed25519Keyring::Bob.pair(); // Use Bob's keypair for signing
        let extrinsic: OpaqueExtrinsic = create_benchmark_extrinsic(
            self.client.as_ref(),
            acc,
            SystemCall::remark { remark: vec![] }.into(), // Empty remark
            nonce,
        )
        .into();

        Ok(extrinsic)
    }
}

/// Generates `Balances::TransferKeepAlive` extrinsics for the benchmarks.
///
/// Note: Should only be used for benchmarking.
pub struct TransferKeepAliveBuilder {
    client: Arc<FullClient>, // Shared reference to the blockchain client
    dest: AccountId,         // Destination account for the transfer
    value: Balance,          // Amount to transfer
}

impl TransferKeepAliveBuilder {
    /// Creates a new [`Self`] from the given client, destination, and value.
    ///
    /// # Arguments
    /// * `client` - A shared reference to the blockchain client.
    /// * `dest` - The destination account ID.
    /// * `value` - The amount to transfer.
    pub fn new(client: Arc<FullClient>, dest: AccountId, value: Balance) -> Self {
        Self { client, dest, value }
    }
}

impl frame_benchmarking_cli::ExtrinsicBuilder for TransferKeepAliveBuilder {
    /// Returns the name of the pallet being benchmarked.
    fn pallet(&self) -> &str {
        "balances"
    }

    /// Returns the name of the extrinsic being benchmarked.
    fn extrinsic(&self) -> &str {
        "transfer_keep_alive"
    }

    /// Builds an `OpaqueExtrinsic` for the `transfer_keep_alive` call.
    ///
    /// # Arguments
    /// * `nonce` - The transaction nonce.
    fn build(&self, nonce: u32) -> std::result::Result<OpaqueExtrinsic, &'static str> {
        let acc = Ed25519Keyring::Bob.pair(); // Use Bob's keypair for signing
        let extrinsic: OpaqueExtrinsic = create_benchmark_extrinsic(
            self.client.as_ref(),
            acc,
            BalancesCall::transfer_keep_alive { dest: self.dest.clone().into(), value: self.value }
                .into(),
            nonce,
        )
        .into();

        Ok(extrinsic)
    }
}

/// Create a transaction using the given `call`.
///
/// # Arguments
/// * `client` - A reference to the blockchain client.
/// * `sender` - The keypair of the sender.
/// * `call` - The runtime call to include in the transaction.
/// * `nonce` - The transaction nonce.
///
/// # Returns
/// An `UncheckedExtrinsic` ready to be submitted to the blockchain.
pub fn create_benchmark_extrinsic(
    client: &FullClient,
    sender: sp_core::ed25519::Pair,
    call: runtime::RuntimeCall,
    nonce: u32,
) -> runtime::UncheckedExtrinsic {
    let genesis_hash = client.block_hash(0).ok().flatten().expect("Genesis block exists; qed");
    let best_hash = client.chain_info().best_hash;
    let best_block = client.chain_info().best_number;

    // Calculate the transaction's era (validity period)
    let period = runtime::configs::BlockHashCount::get()
        .checked_next_power_of_two()
        .map(|c| c / 2)
        .unwrap_or(2) as u64;

    // Transaction extensions for validation
    let tx_ext: runtime::TxExtension = (
        frame_system::CheckNonZeroSender::<runtime::Runtime>::new(),
        frame_system::CheckSpecVersion::<runtime::Runtime>::new(),
        frame_system::CheckTxVersion::<runtime::Runtime>::new(),
        frame_system::CheckGenesis::<runtime::Runtime>::new(),
        frame_system::CheckEra::<runtime::Runtime>::from(sp_runtime::generic::Era::mortal(
            period,
            best_block.saturated_into(),
        )),
        frame_system::CheckNonce::<runtime::Runtime>::from(nonce),
        frame_system::CheckWeight::<runtime::Runtime>::new(),
        pallet_transaction_payment::ChargeTransactionPayment::<runtime::Runtime>::from(0),
        frame_metadata_hash_extension::CheckMetadataHash::<runtime::Runtime>::new(false),
        frame_system::WeightReclaim::<runtime::Runtime>::new(),
    );

    // Create the raw payload for signing
    let raw_payload = runtime::SignedPayload::from_raw(
        call.clone(),
        tx_ext.clone(),
        (
            (),
            runtime::VERSION.spec_version,
            runtime::VERSION.transaction_version,
            genesis_hash,
            best_hash,
            (),
            (),
            (),
            None,
            (),
        ),
    );

    // Sign the payload
    let signature = raw_payload.using_encoded(|e| sender.sign(e));

    // Create the signed extrinsic
    runtime::UncheckedExtrinsic::new_signed(
        call,
        sp_runtime::AccountId32::from(sender.public()).into(),
        runtime::Signature::Ed25519(signature),
        tx_ext,
    )
}

/// Generates inherent data for the `benchmark overhead` command.
///
/// # Returns
/// A result containing the inherent data or an error.
pub fn inherent_benchmark_data() -> Result<InherentData> {
    let mut inherent_data = InherentData::new();
    let d = Duration::from_millis(0); // Set timestamp to 0 for benchmarking
    let timestamp = sp_timestamp::InherentDataProvider::new(d.into());

    // Provide inherent data for the timestamp
    futures::executor::block_on(timestamp.provide_inherent_data(&mut inherent_data))
        .map_err(|e| format!("creating inherent data: {:?}", e))?;
    Ok(inherent_data)
}