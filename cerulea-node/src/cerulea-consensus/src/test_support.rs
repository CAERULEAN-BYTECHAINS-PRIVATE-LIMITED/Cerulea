//! Test support for the services of the Commit Engine.
//!
//! The Commit Engine services read the chain through a client. The test chain
//! below keeps real block headers in the in-memory backend the client crate
//! ships, so finality and justifications land where a node keeps them. The
//! runtime answers come from a table each test fills before the call. A call
//! whose answer the table lacks returns a runtime error, so every error path
//! of a service stays reachable from a test.
//!
//! The propagation layer below holds no peers. A gossip engine built on it
//! keeps every message a service multicasts, and a test reads the message
//! back by its topic.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::Arc;

use codec::{Decode, Encode};
use futures::Stream;
use parking_lot::Mutex;
use sc_client_api::backend::{Backend as BackendT, BlockImportOperation as _, NewBlockState};
use sc_client_api::{
    BlockImportNotification, BlockchainEvents, ClientImportOperation, Finalizer,
    FinalityNotifications, ImportNotifications, LockImportRun, StorageEventStream, StorageKey,
    UnpinWorkerMessage,
};
use sc_network::config::MultiaddrWithPeerId;
use sc_network::{
    Multiaddr, NetworkBlock, NetworkEventStream, NetworkPeers, ObservedRole, PeerId, ProtocolName,
    ReputationChange,
};
use sc_network_gossip::GossipEngine;
use sc_network_sync::{SyncEvent, SyncEventStream};
use sc_transaction_pool_api::{
    ImportNotificationStream, InPoolTransaction, PoolStatus, ReadyTransactions, TransactionPool,
    TransactionSource, TransactionStatus, TransactionStatusStreamFor, TxInvalidityReportMap,
};
use sc_utils::mpsc::{tracing_unbounded, TracingUnboundedReceiver, TracingUnboundedSender};
use sp_api::{ApiError, ApiRef, ProvideRuntimeApi};
use sp_blockchain::{
    Backend as _, BlockStatus, CachedHeaderMetadata, HeaderBackend, HeaderMetadata, Info,
};
use sp_core::{ed25519, Pair as PairT, H256};
use sp_runtime::traits::{BlakeTwo256, Block as BlockT, Hash as HashT, Header as HeaderT};
use sp_runtime::{generic::Digest, Justification, OpaqueExtrinsic};

use pallet_cerulea_dcf::{
    DcfApi, EpochStats, RuntimeEpochHistory, ScoreBreakdown, SystemConstants, UptimeStats,
    ValidatorProfile,
};
use pallet_cerulea_dvf::keys::{PublishedDraw, SlotCommitteeSeat, ValidatorKeySet};
use pallet_cerulea_dvf::{CeruleaAuthoringApi, DvfApi, FinalityInfo};

use crate::dvf_gossip::{DvfVoteMessage, RoundTimeoutMessage};

/// The block type every client-backed test runs on.
pub(crate) type Block = cerulea_runtime::opaque::Block;

/// The header type of that block.
pub(crate) type Header = <Block as BlockT>::Header;

/// The account type of the Cerulea runtime.
pub(crate) type AccountId = cerulea_runtime::AccountId;

/// The in-memory backend the test chain keeps its headers in.
pub(crate) type InMemBackend = sc_client_api::in_mem::Backend<Block>;

/// The error type the transaction pool reports.
pub(crate) type PoolError = sc_transaction_pool_api::error::Error;

/// One answer key: the runtime call, the block it answers for, and the
/// encoded arguments.
type AnswerKey = (&'static str, Option<H256>, Vec<u8>);

/// The table of runtime answers one test fills.
///
/// Each answer sits under the name the runtime export carries, such as
/// `DvfApi_get_current_round`. An answer set without a block answers at every
/// block. An answer set for one block wins at that block.
#[derive(Default)]
pub(crate) struct RuntimeAnswers {
    table: Mutex<HashMap<AnswerKey, Vec<u8>>>,
}

impl RuntimeAnswers {
    /// Answer one call with one value at every block.
    pub(crate) fn set<A: Encode, T: Encode>(&self, call: &'static str, args: A, answer: T) {
        self.table
            .lock()
            .insert((call, None, args.encode()), answer.encode());
    }

    /// Answer one call with one value at one block.
    pub(crate) fn set_at<A: Encode, T: Encode>(
        &self,
        call: &'static str,
        at: H256,
        args: A,
        answer: T,
    ) {
        self.table
            .lock()
            .insert((call, Some(at), args.encode()), answer.encode());
    }

    /// Remove every answer to one call, so the call fails from now on.
    pub(crate) fn forget(&self, call: &'static str) {
        self.table.lock().retain(|(held, _, _), _| *held != call);
    }

    /// Read the answer to one call.
    fn answer<T: Decode>(
        &self,
        call: &'static str,
        at: H256,
        args: Vec<u8>,
    ) -> Result<T, ApiError> {
        let found = {
            let table = self.table.lock();
            table
                .get(&(call, Some(at), args.clone()))
                .or_else(|| table.get(&(call, None, args)))
                .cloned()
        };
        match found {
            Some(raw) => T::decode(&mut &raw[..]).map_err(|error| {
                ApiError::FailedToDecodeReturnValue {
                    function: call,
                    error,
                    raw: raw.clone(),
                }
            }),
            None => Err(ApiError::Application(
                format!(
                    "The test runtime holds no answer for {}. Fix: set the answer before the call.",
                    call
                )
                .into(),
            )),
        }
    }
}

/// The runtime view one client call returns.
pub(crate) struct RuntimeView {
    answers: Arc<RuntimeAnswers>,
}

impl RuntimeView {
    fn ask<T: Decode>(&self, call: &'static str, at: H256, args: impl Encode) -> Result<T, ApiError> {
        self.answers.answer(call, at, args.encode())
    }
}

sp_api::mock_impl_runtime_apis! {
    impl DcfApi<Block, AccountId, u128, u32> for RuntimeView {
        #[advanced]
        fn get_api_version(&self, at: H256) -> Result<u32, ApiError> {
            self.ask("DcfApi_get_api_version", at, ())
        }
        #[advanced]
        fn get_validator_scores(&self, at: H256) -> Result<Vec<(AccountId, u64)>, ApiError> {
            self.ask("DcfApi_get_validator_scores", at, ())
        }
        #[advanced]
        fn get_current_epoch(&self, at: H256) -> Result<u32, ApiError> {
            self.ask("DcfApi_get_current_epoch", at, ())
        }
        #[advanced]
        fn get_validator_stake_score(&self, at: H256, validator: AccountId) -> Result<u64, ApiError> {
            self.ask("DcfApi_get_validator_stake_score", at, validator)
        }
        #[advanced]
        fn get_validator_inference_score(&self, at: H256, validator: AccountId) -> Result<u64, ApiError> {
            self.ask("DcfApi_get_validator_inference_score", at, validator)
        }
        #[advanced]
        fn get_consensus_weights(&self, at: H256) -> Result<(u64, u64), ApiError> {
            self.ask("DcfApi_get_consensus_weights", at, ())
        }
        #[advanced]
        fn is_validator_active(&self, at: H256, validator: AccountId) -> Result<bool, ApiError> {
            self.ask("DcfApi_is_validator_active", at, validator)
        }
        #[advanced]
        fn get_expected_author(&self, at: H256, block_number: u32) -> Result<Option<AccountId>, ApiError> {
            self.ask("DcfApi_get_expected_author", at, block_number)
        }
        #[advanced]
        fn get_validator_score_history(&self, at: H256, validator: AccountId) -> Result<Vec<u64>, ApiError> {
            self.ask("DcfApi_get_validator_score_history", at, validator)
        }
        #[advanced]
        fn get_validator_participation(&self, at: H256, validator: AccountId) -> Result<(u32, u32), ApiError> {
            self.ask("DcfApi_get_validator_participation", at, validator)
        }
        #[advanced]
        fn get_active_validators(&self, at: H256) -> Result<Vec<AccountId>, ApiError> {
            self.ask("DcfApi_get_active_validators", at, ())
        }
        #[advanced]
        fn get_validator_last_active(&self, at: H256, validator: AccountId) -> Result<u32, ApiError> {
            self.ask("DcfApi_get_validator_last_active", at, validator)
        }
        #[advanced]
        fn validate_block_author(&self, at: H256, block_number: u32, author: AccountId) -> Result<(), ApiError> {
            self.ask("DcfApi_validate_block_author", at, (block_number, author))
        }
        #[advanced]
        fn get_validator_profile(&self, at: H256, account_id: AccountId) -> Result<Option<ValidatorProfile<AccountId, u128, u32>>, ApiError> {
            self.ask("DcfApi_get_validator_profile", at, account_id)
        }
        #[advanced]
        fn get_validator_score_breakdown(&self, at: H256, validator: AccountId) -> Result<Option<ScoreBreakdown>, ApiError> {
            self.ask("DcfApi_get_validator_score_breakdown", at, validator)
        }
        #[advanced]
        fn get_validator_uptime(&self, at: H256, validator: AccountId) -> Result<Option<UptimeStats>, ApiError> {
            self.ask("DcfApi_get_validator_uptime", at, validator)
        }
        #[advanced]
        fn get_slashing_history(&self, at: H256, validator: AccountId) -> Result<Vec<pallet_cerulea_pos::SlashingRecord<u128, u32>>, ApiError> {
            self.ask("DcfApi_get_slashing_history", at, validator)
        }
        #[advanced]
        fn get_system_constants(&self, at: H256) -> Result<SystemConstants<u128, u32>, ApiError> {
            self.ask("DcfApi_get_system_constants", at, ())
        }
        #[advanced]
        fn get_validator_cooldown_status(&self, at: H256, validator: AccountId) -> Result<Option<u32>, ApiError> {
            self.ask("DcfApi_get_validator_cooldown_status", at, validator)
        }
        #[advanced]
        fn get_validator_detailed_cooldown_status(&self, at: H256, validator: AccountId) -> Result<Option<(u32, bool)>, ApiError> {
            self.ask("DcfApi_get_validator_detailed_cooldown_status", at, validator)
        }
        #[advanced]
        fn get_inference_result(&self, at: H256, account_id: AccountId) -> Result<Option<u64>, ApiError> {
            self.ask("DcfApi_get_inference_result", at, account_id)
        }
        #[advanced]
        fn get_epoch_history(&self, at: H256, epoch_number: u32) -> Result<Option<RuntimeEpochHistory<AccountId>>, ApiError> {
            self.ask("DcfApi_get_epoch_history", at, epoch_number)
        }
        #[advanced]
        fn get_recent_epochs(&self, at: H256, n: u32) -> Result<Vec<RuntimeEpochHistory<AccountId>>, ApiError> {
            self.ask("DcfApi_get_recent_epochs", at, n)
        }
        #[advanced]
        fn get_governance_mode(&self, at: H256) -> Result<bool, ApiError> {
            self.ask("DcfApi_get_governance_mode", at, ())
        }
        #[advanced]
        fn get_validator_consensus_contribution(&self, at: H256, validator: AccountId) -> Result<Option<(u64, u64, u64)>, ApiError> {
            self.ask("DcfApi_get_validator_consensus_contribution", at, validator)
        }
        #[advanced]
        fn get_epoch_config(&self, at: H256) -> Result<pallet_cerulea_dcf::EpochConfig, ApiError> {
            self.ask("DcfApi_get_epoch_config", at, ())
        }
        #[advanced]
        fn get_validator_epoch_stats(&self, at: H256, validator: AccountId, epoch: u32) -> Result<Option<EpochStats>, ApiError> {
            self.ask("DcfApi_get_validator_epoch_stats", at, (validator, epoch))
        }
        #[advanced]
        fn get_total_validators_count(&self, at: H256) -> Result<u32, ApiError> {
            self.ask("DcfApi_get_total_validators_count", at, ())
        }
        #[advanced]
        fn get_validator_set_info(&self, at: H256) -> Result<(u32, u32, u32), ApiError> {
            self.ask("DcfApi_get_validator_set_info", at, ())
        }
        #[advanced]
        fn get_validators_by_score(&self, at: H256) -> Result<Vec<(AccountId, u64)>, ApiError> {
            self.ask("DcfApi_get_validators_by_score", at, ())
        }
        #[advanced]
        fn get_misbehavior_report_count(&self, at: H256, validator: AccountId) -> Result<u32, ApiError> {
            self.ask("DcfApi_get_misbehavior_report_count", at, validator)
        }
        #[advanced]
        fn get_misbehavior_reporters(&self, at: H256, validator: AccountId) -> Result<Vec<AccountId>, ApiError> {
            self.ask("DcfApi_get_misbehavior_reporters", at, validator)
        }
        #[advanced]
        fn get_misbehavior_evidence(&self, at: H256, validator: AccountId, reporter: AccountId) -> Result<Option<Vec<u8>>, ApiError> {
            self.ask("DcfApi_get_misbehavior_evidence", at, (validator, reporter))
        }
        #[advanced]
        fn is_validator_at_risk(&self, at: H256, validator: AccountId) -> Result<bool, ApiError> {
            self.ask("DcfApi_is_validator_at_risk", at, validator)
        }
        #[advanced]
        fn get_validator_leave_request(&self, at: H256, validator: AccountId) -> Result<Option<u32>, ApiError> {
            self.ask("DcfApi_get_validator_leave_request", at, validator)
        }
        #[advanced]
        fn validate_expected_author(&self, at: H256, block_number: u32, actual_author: AccountId) -> Result<bool, ApiError> {
            self.ask("DcfApi_validate_expected_author", at, (block_number, actual_author))
        }
        #[advanced]
        fn get_validator_stake(&self, at: H256, validator: AccountId) -> Result<u128, ApiError> {
            self.ask("DcfApi_get_validator_stake", at, validator)
        }
        #[advanced]
        fn get_leave_request_status(&self, at: H256, validator: AccountId) -> Result<Option<(u32, u32, bool)>, ApiError> {
            self.ask("DcfApi_get_leave_request_status", at, validator)
        }
        #[advanced]
        fn get_epoch_manager_config(&self, at: H256) -> Result<(u64, u64, u32, u32, u32, u32, u64, u32, u32, u32, u32), ApiError> {
            self.ask("DcfApi_get_epoch_manager_config", at, ())
        }
        #[advanced]
        fn get_epoch_length(&self, at: H256) -> Result<u32, ApiError> {
            self.ask("DcfApi_get_epoch_length", at, ())
        }
        #[advanced]
        fn validate_block_author_strict(&self, at: H256, block_number: u32, actual_author: AccountId) -> Result<Result<(), u8>, ApiError> {
            self.ask("DcfApi_validate_block_author_strict", at, (block_number, actual_author))
        }
        #[advanced]
        fn report_author_mismatch(&self, at: H256, block_number: u32, expected: Option<AccountId>, actual: AccountId) -> Result<Result<(), sp_runtime::DispatchError>, ApiError> {
            self.ask("DcfApi_report_author_mismatch", at, (block_number, expected, actual))
        }
        #[advanced]
        fn report_successful_block_authorship(&self, at: H256, block_number: u32, author: AccountId) -> Result<Result<(), sp_runtime::DispatchError>, ApiError> {
            self.ask("DcfApi_report_successful_block_authorship", at, (block_number, author))
        }
        #[advanced]
        fn report_missed_block(&self, at: H256, block_number: u32, expected_author: AccountId) -> Result<Result<(), sp_runtime::DispatchError>, ApiError> {
            self.ask("DcfApi_report_missed_block", at, (block_number, expected_author))
        }
        #[advanced]
        fn get_governance_config(&self, at: H256) -> Result<Vec<u8>, ApiError> {
            self.ask("DcfApi_get_governance_config", at, ())
        }
        #[advanced]
        fn get_parameter_value(&self, at: H256, parameter: Vec<u8>) -> Result<Option<Vec<u8>>, ApiError> {
            self.ask("DcfApi_get_parameter_value", at, parameter)
        }
        #[advanced]
        fn validate_parameter_value(&self, at: H256, parameter: Vec<u8>, value: Vec<u8>) -> Result<bool, ApiError> {
            self.ask("DcfApi_validate_parameter_value", at, (parameter, value))
        }
        #[advanced]
        fn get_latest_invariant_report(&self, at: H256) -> Result<Option<Vec<u8>>, ApiError> {
            self.ask("DcfApi_get_latest_invariant_report", at, ())
        }
        #[advanced]
        fn get_invariant_report_for_epoch(&self, at: H256, epoch: u32) -> Result<Option<Vec<u8>>, ApiError> {
            self.ask("DcfApi_get_invariant_report_for_epoch", at, epoch)
        }
        #[advanced]
        fn has_invariant_violations(&self, at: H256) -> Result<bool, ApiError> {
            self.ask("DcfApi_has_invariant_violations", at, ())
        }
        #[advanced]
        fn get_system_metrics(&self, at: H256) -> Result<Vec<u8>, ApiError> {
            self.ask("DcfApi_get_system_metrics", at, ())
        }
        #[advanced]
        fn get_performance_indicators(&self, at: H256) -> Result<Vec<u8>, ApiError> {
            self.ask("DcfApi_get_performance_indicators", at, ())
        }
        #[advanced]
        fn get_metrics_last_updated(&self, at: H256) -> Result<u32, ApiError> {
            self.ask("DcfApi_get_metrics_last_updated", at, ())
        }
    }

    impl DvfApi<Block, u32, AccountId, H256> for RuntimeView {
        #[advanced]
        fn get_dvf_finalized_block(&self, at: H256) -> Result<u32, ApiError> {
            self.ask("DvfApi_get_dvf_finalized_block", at, ())
        }
        #[advanced]
        fn get_current_epoch(&self, at: H256) -> Result<u32, ApiError> {
            self.ask("DvfApi_get_current_epoch", at, ())
        }
        #[advanced]
        fn get_validator_set(&self, at: H256) -> Result<Vec<AccountId>, ApiError> {
            self.ask("DvfApi_get_validator_set", at, ())
        }
        #[advanced]
        fn get_validator_weights(&self, at: H256) -> Result<Vec<(AccountId, u128)>, ApiError> {
            self.ask("DvfApi_get_validator_weights", at, ())
        }
        #[advanced]
        fn get_finality_threshold_perbill(&self, at: H256) -> Result<sp_runtime::Perbill, ApiError> {
            self.ask("DvfApi_get_finality_threshold_perbill", at, ())
        }
        #[advanced]
        fn get_finality_info(&self, at: H256, block_number: u32) -> Result<FinalityInfo<u32, H256>, ApiError> {
            self.ask("DvfApi_get_finality_info", at, block_number)
        }
        #[advanced]
        fn get_finality_checkpoint_interval(&self, at: H256) -> Result<u32, ApiError> {
            self.ask("DvfApi_get_finality_checkpoint_interval", at, ())
        }
        #[advanced]
        fn get_validator_set_id(&self, at: H256) -> Result<u32, ApiError> {
            self.ask("DvfApi_get_validator_set_id", at, ())
        }
        #[advanced]
        fn get_current_round(&self, at: H256) -> Result<u32, ApiError> {
            self.ask("DvfApi_get_current_round", at, ())
        }
        #[advanced]
        fn get_validator_set_id_changed_at(&self, at: H256) -> Result<Option<u32>, ApiError> {
            self.ask("DvfApi_get_validator_set_id_changed_at", at, ())
        }
        #[advanced]
        fn get_vote_retention_rounds(&self, at: H256) -> Result<u32, ApiError> {
            self.ask("DvfApi_get_vote_retention_rounds", at, ())
        }
        #[advanced]
        fn get_vote_tally(&self, at: H256, block_hash: H256) -> Result<u128, ApiError> {
            self.ask("DvfApi_get_vote_tally", at, block_hash)
        }
        #[advanced]
        fn submit_dvf_justification(&self, at: H256, justification: pallet_cerulea_dvf::DvfJustification<H256, AccountId, sp_runtime::MultiSignature>) -> Result<Result<(), sp_runtime::DispatchError>, ApiError> {
            self.ask("DvfApi_submit_dvf_justification", at, justification)
        }
    }

    impl CeruleaAuthoringApi<Block, AccountId> for RuntimeView {
        #[advanced]
        fn active_validator_keys(&self, at: H256) -> Result<Vec<(AccountId, ValidatorKeySet)>, ApiError> {
            self.ask("CeruleaAuthoringApi_active_validator_keys", at, ())
        }
        #[advanced]
        fn validator_commit_weights(&self, at: H256) -> Result<Vec<(AccountId, u32)>, ApiError> {
            self.ask("CeruleaAuthoringApi_validator_commit_weights", at, ())
        }
        #[advanced]
        fn slot_committee(&self, at: H256, slot: u64, parent_hash: [u8; 32]) -> Result<Vec<SlotCommitteeSeat<AccountId>>, ApiError> {
            self.ask("CeruleaAuthoringApi_slot_committee", at, (slot, parent_hash))
        }
        #[advanced]
        fn authoring_epoch(&self, at: H256) -> Result<u32, ApiError> {
            self.ask("CeruleaAuthoringApi_authoring_epoch", at, ())
        }
        #[advanced]
        fn published_epoch_draw(&self, at: H256, validator: AccountId, epoch: u32) -> Result<Option<PublishedDraw>, ApiError> {
            self.ask("CeruleaAuthoringApi_published_epoch_draw", at, (validator, epoch))
        }
    }
}


/// Build one header at one height on one parent.
///
/// The salt sets the state root, so two headers at one height and one parent
/// take two hashes.
pub(crate) fn header_at(number: u32, parent: H256, salt: u8) -> Header {
    <Header as HeaderT>::new(
        number,
        H256::zero(),
        H256::repeat_byte(salt),
        parent,
        Digest::default(),
    )
}

/// The chain one client-backed test reads.
pub(crate) struct TestChain {
    backend: Arc<InMemBackend>,
    /// The runtime answers this chain gives.
    pub(crate) answers: Arc<RuntimeAnswers>,
    import_sinks: Mutex<Vec<TracingUnboundedSender<BlockImportNotification<Block>>>>,
    unpin_sink: TracingUnboundedSender<UnpinWorkerMessage<Block>>,
    _unpin_stream: Mutex<TracingUnboundedReceiver<UnpinWorkerMessage<Block>>>,
}

impl TestChain {
    /// Build a chain from genesis to the best height.
    ///
    /// Every block up to the finalized height enters as final. Every block
    /// above it enters as the new best block.
    pub(crate) fn build(finalized: u32, best: u32) -> Arc<Self> {
        let (unpin_sink, unpin_stream) = tracing_unbounded("cerulea-test-unpin", 1_024);
        let chain = TestChain {
            backend: Arc::new(InMemBackend::new()),
            answers: Arc::new(RuntimeAnswers::default()),
            import_sinks: Mutex::new(Vec::new()),
            unpin_sink,
            _unpin_stream: Mutex::new(unpin_stream),
        };
        let mut parent = H256::zero();
        for number in 0..=best {
            let state = if number <= finalized {
                NewBlockState::Final
            } else {
                NewBlockState::Best
            };
            parent = chain.insert(header_at(number, parent, 0), state);
        }
        Arc::new(chain)
    }

    /// Build a chain whose committed head sits at genesis.
    pub(crate) fn with_blocks(best: u32) -> Arc<Self> {
        TestChain::build(0, best)
    }

    fn insert(&self, header: Header, state: NewBlockState) -> H256 {
        let hash = header.hash();
        self.backend
            .blockchain()
            .insert(hash, header, None, None, state)
            .expect("the in-memory chain takes the header");
        hash
    }

    /// The hash of the canonical block at one height.
    pub(crate) fn hash_at(&self, number: u32) -> H256 {
        self.backend
            .blockchain()
            .hash(number)
            .expect("the in-memory chain answers")
            .expect("the chain holds a block at that height")
    }

    /// The header that would extend the best block.
    pub(crate) fn next_header(&self) -> Header {
        let info = self.info();
        header_at(info.best_number + 1, info.best_hash, 0)
    }

    /// Import one header as the new best block.
    pub(crate) fn import(&self, header: Header) -> H256 {
        self.insert(header, NewBlockState::Best)
    }

    /// The Commit Engine justification one block carries.
    pub(crate) fn commit_justification(&self, hash: H256) -> Option<Vec<u8>> {
        self.backend
            .blockchain()
            .justifications(hash)
            .expect("the in-memory chain answers")
            .and_then(|held| held.get(crate::CERULEA_COMMIT_ENGINE_ID).cloned())
    }

    /// Tell every import listener that the block at one height arrived.
    pub(crate) fn announce_import(&self, number: u32) {
        let hash = self.hash_at(number);
        let header = self
            .header(hash)
            .expect("the in-memory chain answers")
            .expect("the chain holds the header");
        for sink in self.import_sinks.lock().iter() {
            let notification = BlockImportNotification::new(
                hash,
                sp_consensus::BlockOrigin::NetworkBroadcast,
                header.clone(),
                true,
                None,
                self.unpin_sink.clone(),
            );
            let _ = sink.unbounded_send(notification);
        }
    }

    /// Close every import stream, so every listener stops.
    pub(crate) fn close_imports(&self) {
        self.import_sinks.lock().clear();
    }
}

impl HeaderBackend<Block> for TestChain {
    fn header(&self, hash: H256) -> sp_blockchain::Result<Option<Header>> {
        self.backend.blockchain().header(hash)
    }

    fn info(&self) -> Info<Block> {
        self.backend.blockchain().info()
    }

    fn status(&self, hash: H256) -> sp_blockchain::Result<BlockStatus> {
        self.backend.blockchain().status(hash)
    }

    fn number(&self, hash: H256) -> sp_blockchain::Result<Option<u32>> {
        self.backend.blockchain().number(hash)
    }

    fn hash(&self, number: u32) -> sp_blockchain::Result<Option<H256>> {
        self.backend.blockchain().hash(number)
    }
}

impl HeaderMetadata<Block> for TestChain {
    type Error = sp_blockchain::Error;

    fn header_metadata(&self, hash: H256) -> Result<CachedHeaderMetadata<Block>, Self::Error> {
        self.backend.blockchain().header_metadata(hash)
    }

    fn insert_header_metadata(&self, hash: H256, metadata: CachedHeaderMetadata<Block>) {
        self.backend
            .blockchain()
            .insert_header_metadata(hash, metadata)
    }

    fn remove_header_metadata(&self, hash: H256) {
        self.backend.blockchain().remove_header_metadata(hash)
    }
}

impl ProvideRuntimeApi<Block> for TestChain {
    type Api = RuntimeView;

    fn runtime_api(&self) -> ApiRef<'_, Self::Api> {
        RuntimeView {
            answers: self.answers.clone(),
        }
        .into()
    }
}

impl LockImportRun<Block, InMemBackend> for TestChain {
    fn lock_import_and_run<R, Err, F>(&self, f: F) -> Result<R, Err>
    where
        F: FnOnce(&mut ClientImportOperation<Block, InMemBackend>) -> Result<R, Err>,
        Err: From<sp_blockchain::Error>,
    {
        let _import = self.backend.get_import_lock().write();
        let mut operation = ClientImportOperation {
            op: self.backend.begin_operation()?,
            notify_imported: None,
            notify_finalized: None,
        };
        let outcome = f(&mut operation)?;
        self.backend.commit_operation(operation.op)?;
        Ok(outcome)
    }
}

impl Finalizer<Block, InMemBackend> for TestChain {
    fn apply_finality(
        &self,
        operation: &mut ClientImportOperation<Block, InMemBackend>,
        block: H256,
        justification: Option<Justification>,
        _notify: bool,
    ) -> sp_blockchain::Result<()> {
        operation.op.mark_finalized(block, justification)
    }

    fn finalize_block(
        &self,
        block: H256,
        justification: Option<Justification>,
        notify: bool,
    ) -> sp_blockchain::Result<()> {
        self.lock_import_and_run(|operation| {
            self.apply_finality(operation, block, justification, notify)
        })
    }
}

impl BlockchainEvents<Block> for TestChain {
    fn import_notification_stream(&self) -> ImportNotifications<Block> {
        let (sink, stream) = tracing_unbounded("cerulea-test-imports", 1_024);
        self.import_sinks.lock().push(sink);
        stream
    }

    fn every_import_notification_stream(&self) -> ImportNotifications<Block> {
        self.import_notification_stream()
    }

    fn finality_notification_stream(&self) -> FinalityNotifications<Block> {
        tracing_unbounded("cerulea-test-finality", 1_024).1
    }

    fn storage_changes_notification_stream(
        &self,
        _filter_keys: Option<&[StorageKey]>,
        _child_filter_keys: Option<&[(StorageKey, Option<Vec<StorageKey>>)]>,
    ) -> sp_blockchain::Result<StorageEventStream<H256>> {
        Err(sp_blockchain::Error::Backend(
            "The test chain keeps no storage. Fix: read state through the runtime answers."
                .to_string(),
        ))
    }
}

/// One extrinsic the submission log holds.
#[derive(Debug)]
pub(crate) struct PooledExtrinsic {
    data: Arc<OpaqueExtrinsic>,
    hash: H256,
    priority: u64,
    longevity: u64,
    requires: Vec<Vec<u8>>,
    provides: Vec<Vec<u8>>,
}

impl InPoolTransaction for PooledExtrinsic {
    type Transaction = Arc<OpaqueExtrinsic>;
    type Hash = H256;

    fn data(&self) -> &Self::Transaction {
        &self.data
    }

    fn hash(&self) -> &Self::Hash {
        &self.hash
    }

    fn priority(&self) -> &u64 {
        &self.priority
    }

    fn longevity(&self) -> &u64 {
        &self.longevity
    }

    fn requires(&self) -> &[Vec<u8>] {
        &self.requires
    }

    fn provides(&self) -> &[Vec<u8>] {
        &self.provides
    }

    fn is_propagable(&self) -> bool {
        true
    }
}

/// The ready queue the submission log hands out.
pub(crate) struct ReadyList(std::vec::IntoIter<Arc<PooledExtrinsic>>);

impl Iterator for ReadyList {
    type Item = Arc<PooledExtrinsic>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl ReadyTransactions for ReadyList {
    fn report_invalid(&mut self, _tx: &Self::Item) {}
}

/// A local transaction pool that keeps every extrinsic it takes.
///
/// The log takes every submission as ready, or refuses each one with the
/// error a test names.
#[derive(Default)]
pub(crate) struct SubmissionLog {
    taken: Mutex<Vec<Arc<PooledExtrinsic>>>,
    refusal: Mutex<Option<fn() -> PoolError>>,
}

impl SubmissionLog {
    /// Every extrinsic the log took, oldest first.
    pub(crate) fn taken(&self) -> Vec<OpaqueExtrinsic> {
        self.taken
            .lock()
            .iter()
            .map(|held| (*held.data).clone())
            .collect()
    }

    /// Refuse every later submission with the given error.
    pub(crate) fn refuse_with(&self, refusal: fn() -> PoolError) {
        *self.refusal.lock() = Some(refusal);
    }

    fn take(&self, extrinsic: OpaqueExtrinsic) -> Result<H256, PoolError> {
        if let Some(refusal) = *self.refusal.lock() {
            return Err(refusal());
        }
        let hash = BlakeTwo256::hash_of(&extrinsic);
        self.taken.lock().push(Arc::new(PooledExtrinsic {
            data: Arc::new(extrinsic),
            hash,
            priority: 0,
            longevity: u64::MAX,
            requires: Vec::new(),
            provides: vec![hash.as_bytes().to_vec()],
        }));
        Ok(hash)
    }

    fn ready_list(&self) -> ReadyList {
        ReadyList(self.taken.lock().clone().into_iter())
    }
}

#[async_trait::async_trait]
impl TransactionPool for SubmissionLog {
    type Block = Block;
    type Hash = H256;
    type InPoolTransaction = PooledExtrinsic;
    type Error = PoolError;

    async fn submit_at(
        &self,
        _at: H256,
        _source: TransactionSource,
        xts: Vec<OpaqueExtrinsic>,
    ) -> Result<Vec<Result<H256, Self::Error>>, Self::Error> {
        Ok(xts.into_iter().map(|xt| self.take(xt)).collect())
    }

    async fn submit_one(
        &self,
        _at: H256,
        _source: TransactionSource,
        xt: OpaqueExtrinsic,
    ) -> Result<H256, Self::Error> {
        self.take(xt)
    }

    async fn submit_and_watch(
        &self,
        _at: H256,
        _source: TransactionSource,
        xt: OpaqueExtrinsic,
    ) -> Result<Pin<Box<TransactionStatusStreamFor<Self>>>, Self::Error> {
        self.take(xt)?;
        Ok(Box::pin(futures::stream::iter(vec![
            TransactionStatus::Ready,
        ])))
    }

    async fn ready_at(
        &self,
        _at: H256,
    ) -> Box<dyn ReadyTransactions<Item = Arc<Self::InPoolTransaction>> + Send> {
        Box::new(self.ready_list())
    }

    fn ready(&self) -> Box<dyn ReadyTransactions<Item = Arc<Self::InPoolTransaction>> + Send> {
        Box::new(self.ready_list())
    }

    fn report_invalid(
        &self,
        _at: Option<H256>,
        invalid_tx_errors: TxInvalidityReportMap<H256>,
    ) -> Vec<Arc<Self::InPoolTransaction>> {
        let mut taken = self.taken.lock();
        let (removed, kept): (Vec<_>, Vec<_>) = taken
            .drain(..)
            .partition(|held| invalid_tx_errors.contains_key(&held.hash));
        *taken = kept;
        removed
    }

    fn futures(&self) -> Vec<Self::InPoolTransaction> {
        Vec::new()
    }

    fn status(&self) -> PoolStatus {
        let taken = self.taken.lock();
        PoolStatus {
            ready: taken.len(),
            ready_bytes: taken.iter().map(|held| held.data.encode().len()).sum(),
            future: 0,
            future_bytes: 0,
        }
    }

    fn import_notification_stream(&self) -> ImportNotificationStream<H256> {
        futures::channel::mpsc::channel(16).1
    }

    fn on_broadcasted(&self, _propagations: HashMap<H256, Vec<String>>) {}

    fn hash_of(&self, xt: &OpaqueExtrinsic) -> H256 {
        BlakeTwo256::hash_of(xt)
    }

    fn ready_transaction(&self, hash: &H256) -> Option<Arc<Self::InPoolTransaction>> {
        self.taken
            .lock()
            .iter()
            .find(|held| held.hash == *hash)
            .cloned()
    }

    async fn ready_at_with_timeout(
        &self,
        _at: H256,
        _timeout: std::time::Duration,
    ) -> Box<dyn ReadyTransactions<Item = Arc<Self::InPoolTransaction>> + Send> {
        Box::new(self.ready_list())
    }
}

/// A propagation layer that holds no peers.
///
/// A gossip engine on this layer keeps every message it multicasts and sends
/// it nowhere. Every peer operation answers for an empty peer set.
#[derive(Clone, Default)]
pub(crate) struct PeerlessNetwork;

#[async_trait::async_trait]
impl NetworkPeers for PeerlessNetwork {
    fn set_authorized_peers(&self, _peers: HashSet<PeerId>) {}

    fn set_authorized_only(&self, _reserved_only: bool) {}

    fn add_known_address(&self, _peer_id: PeerId, _addr: Multiaddr) {}

    fn report_peer(&self, _peer_id: PeerId, _cost_benefit: ReputationChange) {}

    fn peer_reputation(&self, _peer_id: &PeerId) -> i32 {
        0
    }

    fn disconnect_peer(&self, _peer_id: PeerId, _protocol: ProtocolName) {}

    fn accept_unreserved_peers(&self) {}

    fn deny_unreserved_peers(&self) {}

    fn add_reserved_peer(&self, _peer: MultiaddrWithPeerId) -> Result<(), String> {
        Err("The test network holds no peers. Fix: run the test against a live network.".to_string())
    }

    fn remove_reserved_peer(&self, _peer_id: PeerId) {}

    fn set_reserved_peers(
        &self,
        _protocol: ProtocolName,
        _peers: HashSet<Multiaddr>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn add_peers_to_reserved_set(
        &self,
        _protocol: ProtocolName,
        _peers: HashSet<Multiaddr>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn remove_peers_from_reserved_set(
        &self,
        _protocol: ProtocolName,
        _peers: Vec<PeerId>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn sync_num_connected(&self) -> usize {
        0
    }

    fn peer_role(&self, _peer_id: PeerId, _handshake: Vec<u8>) -> Option<ObservedRole> {
        None
    }

    async fn reserved_peers(&self) -> Result<Vec<PeerId>, ()> {
        Ok(Vec::new())
    }
}

impl NetworkEventStream for PeerlessNetwork {
    fn event_stream(
        &self,
        _name: &'static str,
    ) -> Pin<Box<dyn Stream<Item = sc_network::Event> + Send>> {
        Box::pin(futures::stream::pending())
    }
}

/// A sync layer that tracks no peers.
#[derive(Clone, Default)]
pub(crate) struct PeerlessSync;

impl SyncEventStream for PeerlessSync {
    fn event_stream(&self, _name: &'static str) -> Pin<Box<dyn Stream<Item = SyncEvent> + Send>> {
        Box::pin(futures::stream::pending())
    }
}

impl NetworkBlock<H256, u32> for PeerlessSync {
    fn announce_block(&self, _hash: H256, _data: Option<Vec<u8>>) {}

    fn new_best_block_imported(&self, _hash: H256, _number: u32) {}
}

/// The gossip engine of the commit protocol on a network with no peers.
pub(crate) fn peerless_gossip() -> Arc<std::sync::Mutex<GossipEngine<Block>>> {
    let (_set, notifications) = sc_network::config::NonDefaultSetConfig::new(
        ProtocolName::from(crate::dvf_gossip::DVF_PROTOCOL_NAME),
        Vec::new(),
        1024 * 1024,
        None,
        sc_network::config::SetConfig::default(),
    );
    let engine = GossipEngine::new(
        PeerlessNetwork,
        PeerlessSync,
        notifications,
        crate::dvf_gossip::DVF_PROTOCOL_NAME,
        Arc::new(sc_network_gossip::DiscardAll),
        None,
    );
    Arc::new(std::sync::Mutex::new(engine))
}

/// Report whether a gossip engine holds one message under its own topic.
///
/// The commit protocol files every message under the hash of its bytes.
pub(crate) fn gossip_holds(
    engine: &Arc<std::sync::Mutex<GossipEngine<Block>>>,
    message: &[u8],
) -> bool {
    let topic = H256::from(sp_core::hashing::blake2_256(message));
    let mut receiver = engine
        .lock()
        .expect("the gossip engine lock holds")
        .messages_for(topic);
    let mut found = false;
    while let Ok(Some(notification)) = receiver.try_next() {
        if notification.message == message {
            found = true;
        }
    }
    found
}

/// A validator context that records every request a validator makes.
#[derive(Default)]
pub(crate) struct RecordingContext {
    /// One line for each request, in order.
    pub(crate) requests: Vec<String>,
}

impl sc_network_gossip::ValidatorContext<Block> for RecordingContext {
    fn broadcast_topic(&mut self, topic: H256, force: bool) {
        self.requests
            .push(format!("broadcast topic {:?} force {}", topic, force));
    }

    fn broadcast_message(&mut self, topic: H256, message: Vec<u8>, force: bool) {
        self.requests.push(format!(
            "broadcast message {:?} of {} bytes force {}",
            topic,
            message.len(),
            force
        ));
    }

    fn send_message(&mut self, who: &PeerId, message: Vec<u8>) {
        self.requests
            .push(format!("send message to {} of {} bytes", who, message.len()));
    }

    fn send_topic(&mut self, who: &PeerId, topic: H256, force: bool) {
        self.requests
            .push(format!("send topic {:?} to {} force {}", topic, who, force));
    }
}

/// One validator holding an ed25519 seal key from a fixed seed.
pub(crate) struct Voter {
    /// The seal key pair.
    pub(crate) pair: ed25519::Pair,
    /// The account, which carries the bytes of the seal key.
    pub(crate) account: AccountId,
}

impl Voter {
    /// The validator whose seed repeats one byte.
    pub(crate) fn from_seed(seed: u8) -> Self {
        let pair = ed25519::Pair::from_seed(&[seed; 32]);
        let account = AccountId::from(pair.public().0);
        Voter { pair, account }
    }

    /// The seal key.
    pub(crate) fn public(&self) -> ed25519::Public {
        self.pair.public()
    }

    /// One signed vote.
    pub(crate) fn vote(
        &self,
        epoch: u32,
        set: u32,
        round: u32,
        height: u32,
        block: H256,
    ) -> DvfVoteMessage<H256, AccountId> {
        let mut vote = DvfVoteMessage {
            epoch_id: epoch,
            validator_set_id: set,
            round_number: round,
            block_number: height,
            block_hash: block,
            validator_account_id: self.account.clone(),
            validator_public_key: self.pair.public(),
            signature: ed25519::Signature::from_raw([0u8; 64]),
        };
        vote.signature = self
            .pair
            .sign(&crate::vote_creator::vote_payload(&vote));
        vote
    }

    /// One signed round timeout statement.
    pub(crate) fn timeout(
        &self,
        epoch: u32,
        set: u32,
        round: u32,
        height: u32,
        voted: Option<H256>,
    ) -> RoundTimeoutMessage<H256, AccountId> {
        let mut statement = RoundTimeoutMessage {
            epoch_id: epoch,
            validator_set_id: set,
            round_number: round,
            block_number: height,
            voted,
            validator_account_id: self.account.clone(),
            validator_public_key: self.pair.public(),
            signature: ed25519::Signature::from_raw([0u8; 64]),
        };
        statement.signature = self.pair.sign(&statement.payload());
        statement
    }
}

/// The log lines one test thread records.
///
/// The node services report most decisions through the log alone. A test
/// starts a capture, runs the service, and reads back the lines the service
/// wrote on that thread. Each test thread keeps its own lines, so tests
/// running side by side never read each other.
struct ThreadLog;

thread_local! {
    static CAPTURED: RefCell<Option<Vec<(log::Level, String)>>> = const { RefCell::new(None) };
}

impl log::Log for ThreadLog {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        CAPTURED.with(|captured| captured.borrow().is_some())
    }

    fn log(&self, record: &log::Record) {
        CAPTURED.with(|captured| {
            if let Some(lines) = captured.borrow_mut().as_mut() {
                lines.push((record.level(), record.args().to_string()));
            }
        });
    }

    fn flush(&self) {}
}

static THREAD_LOG: ThreadLog = ThreadLog;
static INSTALL_THREAD_LOG: std::sync::Once = std::sync::Once::new();

/// A running log capture on the current thread.
pub(crate) struct LogCapture {
    _private: (),
}

/// Start capturing the log lines of the current thread.
pub(crate) fn capture_logs() -> LogCapture {
    INSTALL_THREAD_LOG.call_once(|| {
        let _ = log::set_logger(&THREAD_LOG);
        log::set_max_level(log::LevelFilter::Trace);
    });
    CAPTURED.with(|captured| *captured.borrow_mut() = Some(Vec::new()));
    LogCapture { _private: () }
}

impl LogCapture {
    /// Every captured line, oldest first.
    pub(crate) fn lines(&self) -> Vec<String> {
        CAPTURED.with(|captured| {
            captured
                .borrow()
                .as_ref()
                .map(|lines| lines.iter().map(|(_, line)| line.clone()).collect())
                .unwrap_or_default()
        })
    }

    /// The captured lines at one level, oldest first.
    pub(crate) fn lines_at(&self, level: log::Level) -> Vec<String> {
        CAPTURED.with(|captured| {
            captured
                .borrow()
                .as_ref()
                .map(|lines| {
                    lines
                        .iter()
                        .filter(|(held, _)| *held == level)
                        .map(|(_, line)| line.clone())
                        .collect()
                })
                .unwrap_or_default()
        })
    }

    /// Report whether any captured line holds the fragment.
    pub(crate) fn has(&self, fragment: &str) -> bool {
        self.count(fragment) > 0
    }

    /// The number of captured lines holding the fragment.
    pub(crate) fn count(&self, fragment: &str) -> usize {
        self.lines()
            .iter()
            .filter(|line| line.contains(fragment))
            .count()
    }
}

impl Drop for LogCapture {
    fn drop(&mut self) {
        CAPTURED.with(|captured| *captured.borrow_mut() = None);
    }
}
