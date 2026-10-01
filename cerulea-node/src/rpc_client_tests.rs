//! The node interface methods, called against the real client.
//!
//! Every test calls a method through the JSON-RPC module the node serves and
//! reads the answer the real runtime gave. The fixture chain carries one
//! slashing, one inference challenge, two authored blocks and nine missed
//! slots, so each answer carries a figure the chain itself recorded.

use std::sync::Arc;

use cerulea_runtime::opaque::Block;
use cerulea_runtime::AccountId;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::rpc_params;
use jsonrpsee::types::ErrorObjectOwned;
use jsonrpsee::{MethodsError, RpcModule};
use pallet_cerulea_dcf::DcfApi;
use pallet_cerulea_pos::PosApi;
use serde::de::DeserializeOwned;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;

use crate::chain_fixture_tests::{alice, bob, fixture, serve_fixture, ViewAt, MISSED_BLOCKS};
use crate::fork_detection::ForkReport;
use crate::rpc::{
    BlockAuthoringStats, CeruleaRpcApiImpl, CeruleaRpcApiServer, ChainApiImpl, ChainApiServer,
    ChallengeWindow, ConsensusWeights, DcfRpcApiImpl, DcfRpcApiServer, DvfRpcApiImpl,
    DvfRpcApiServer, ForkDetectionRpcApiImpl, ForkDetectionRpcApiServer, FullDeps, HealthStatus,
    InferenceResult, PoiRpcApiImpl, PoiRpcApiServer, PosRpcApiImpl, PosRpcApiServer, RateLimiter,
    RpcMethodDescription, RpcSecurityConfig, SystemStatus, TrustScore, ValidatorProfile,
};

/// The sentence every closed extension answers with.
const CLOSED: &str = "Cerulea RPC extensions are disabled. Use --enable-cerulea-extensions flag.";

type Backend = sc_service::TFullBackend<Block>;

fn open() -> RpcSecurityConfig {
    RpcSecurityConfig {
        enable_cerulea_extensions: true,
        ..RpcSecurityConfig::default()
    }
}

/// The interface the node serves over the fixture chain.
fn served(extensions: bool, with_metrics: bool) -> RpcModule<()> {
    let chain = fixture();
    crate::rpc::create_full(FullDeps {
        client: chain.client.clone(),
        pool: chain.pool.clone(),
        rpc_config: RpcSecurityConfig {
            enable_cerulea_extensions: extensions,
            ..RpcSecurityConfig::default()
        },
        consensus_metrics: if with_metrics {
            chain.metrics.clone()
        } else {
            None
        },
        backend: chain.backend.clone(),
    })
    .expect("the interface builds over the fixture chain")
}

/// Every Cerulea method over one reader, outside `create_full`.
fn cerulea_methods<C>(client: Arc<C>, security: RpcSecurityConfig) -> RpcModule<()>
where
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: pallet_cerulea_pos::PosApi<Block, AccountId, u128>
        + cerulea_runtime::pallet_cerulea_poi::PoiApi<Block, AccountId>
        + pallet_cerulea_dcf::DcfApi<Block, AccountId, u128, u32>
        + pallet_cerulea_dvf::DvfApi<Block, u32, AccountId, sp_core::H256>,
{
    let mut module = RpcModule::new(());
    module
        .merge(PosRpcApiServer::into_rpc(PosRpcApiImpl::new(
            client.clone(),
        )))
        .unwrap();
    module
        .merge(PoiRpcApiServer::into_rpc(PoiRpcApiImpl::new(
            client.clone(),
        )))
        .unwrap();
    module
        .merge(DcfRpcApiServer::into_rpc(DcfRpcApiImpl::new(
            client.clone(),
            security.clone(),
        )))
        .unwrap();
    module
        .merge(CeruleaRpcApiServer::into_rpc(CeruleaRpcApiImpl::new(
            client.clone(),
            security.clone(),
        )))
        .unwrap();
    module
        .merge(DvfRpcApiServer::into_rpc(DvfRpcApiImpl::new(
            client.clone(),
            security.clone(),
        )))
        .unwrap();
    module
        .merge(ForkDetectionRpcApiServer::into_rpc(
            ForkDetectionRpcApiImpl::<C, Backend>::new(client.clone(), security),
        ))
        .unwrap();
    module
}

/// Call one method and read the answer or the refusal.
async fn call<T: DeserializeOwned + Clone>(
    module: &RpcModule<()>,
    method: &str,
    params: jsonrpsee::core::params::ArrayParams,
) -> Result<T, ErrorObjectOwned> {
    match module.call::<_, T>(method, params).await {
        Ok(value) => Ok(value),
        Err(MethodsError::JsonRpc(error)) => Err(error),
        Err(other) => panic!("The method {method} failed outside the interface: {other:?}"),
    }
}

/// Call one method and read the refusal.
async fn refusal(
    module: &RpcModule<()>,
    method: &str,
    params: jsonrpsee::core::params::ArrayParams,
) -> ErrorObjectOwned {
    call::<serde_json::Value>(module, method, params)
        .await
        .expect_err("the method refuses the call")
}

#[test]
fn the_rate_limiter_admits_the_window_allowance_and_refuses_the_next_request() {
    let limiter = RateLimiter::new(60, 2);
    assert!(limiter.check_rate_limit("10.0.0.1"));
    assert!(limiter.check_rate_limit("10.0.0.1"));
    assert!(!limiter.check_rate_limit("10.0.0.1"));
    // A second address holds its own allowance.
    assert!(limiter.check_rate_limit("10.0.0.2"));

    let expiring = RateLimiter::new(0, 1);
    assert!(expiring.check_rate_limit("10.0.0.3"));
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(expiring.check_rate_limit("10.0.0.3"));
}

#[test]
fn the_default_security_settings_keep_the_extensions_closed() {
    let settings = RpcSecurityConfig::default();
    assert!(!settings.enable_cerulea_extensions);
    assert!(!settings.expose_unsafe_methods);
    assert_eq!(settings.rate_limit_window, 60);
    assert_eq!(settings.rate_limit_requests, 100);
}

#[tokio::test]
async fn a_node_with_closed_extensions_serves_the_standard_methods_alone() {
    let module = served(false, false);
    let names: Vec<&str> = module.method_names().collect();
    assert!(names.contains(&"system_accountNextIndex"));
    assert!(names.contains(&"payment_queryInfo"));
    for cerulea in [
        "chain_getChainName",
        "pos_getValidatorScore",
        "cerulea_health",
        "cerulea_getDeployments",
        "fork_getStatus",
    ] {
        assert!(!names.contains(&cerulea), "{cerulea} stays closed");
    }

    let nonce: u32 = call(&module, "system_accountNextIndex", rpc_params![alice()])
        .await
        .unwrap();
    // Alice signed eleven transactions in block one and five in block two.
    assert_eq!(nonce, 16);
}

#[tokio::test]
async fn every_closed_extension_names_the_flag_that_opens_it() {
    let chain = fixture();
    let module = cerulea_methods(chain.client.clone(), RpcSecurityConfig::default());
    for (method, params) in [
        ("dcf_getCurrentAuthor", rpc_params![]),
        ("dcf_getExpectedAuthor", rpc_params![3u32]),
        ("dcf_getValidatorScores", rpc_params![]),
        ("dcf_getConsensusWeights", rpc_params![]),
        ("cerulea_getCurrentEpoch", rpc_params![]),
        ("cerulea_getValidatorProfile", rpc_params![alice()]),
        ("cerulea_getTrustScore", rpc_params![alice()]),
        ("cerulea_listValidators", rpc_params![]),
        ("cerulea_getStatus", rpc_params![]),
        ("cerulea_describe", rpc_params![]),
        ("cerulea_health", rpc_params![]),
        ("cerulea_getBlockAuthoringStats", rpc_params![alice()]),
        ("cerulea_getAllBlockAuthoringStats", rpc_params![]),
        ("cerulea_getFinalizedHead", rpc_params![]),
        ("cerulea_getFinalizedHash", rpc_params![]),
        ("cerulea_isBlockFinalized", rpc_params![1u32]),
        ("cerulea_getCurrentRound", rpc_params![]),
        ("cerulea_getAccumulatedWeight", rpc_params!["0x00"]),
        ("cerulea_getValidatorSetId", rpc_params![]),
        ("cerulea_getValidatorWeights", rpc_params![]),
        (
            "fork_checkPeers",
            rpc_params![vec!["http://127.0.0.1:1"], 1u32],
        ),
        ("fork_getStatus", rpc_params![]),
    ] {
        let error = refusal(&module, method, params).await;
        assert_eq!(error.code(), -32001, "{method}");
        assert_eq!(error.message(), CLOSED, "{method}");
    }
}

#[tokio::test]
async fn the_chain_name_method_answers_with_the_chain_name() {
    let module = served(true, false);
    let name: String = call(&module, "chain_getChainName", rpc_params![])
        .await
        .unwrap();
    assert_eq!(name, "Cerulea");

    let direct = ChainApiServer::into_rpc(ChainApiImpl {
        client: fixture().client.clone(),
    })
    .remove_context();
    let name: String = call(&direct, "chain_getChainName", rpc_params![])
        .await
        .unwrap();
    assert_eq!(name, "Cerulea");
}

#[tokio::test]
async fn the_stake_methods_read_the_slashing_the_chain_recorded() {
    let chain = fixture();
    let best = chain.client.info().best_hash;
    let api = chain.client.runtime_api();
    for module in [served(true, false), served(true, true)] {
        let score: u32 = call(&module, "pos_getValidatorScore", rpc_params![alice()])
            .await
            .unwrap();
        assert_eq!(score, api.get_validator_score(best, alice()).unwrap());
        let stake: u128 = call(&module, "pos_getValidatorStake", rpc_params![alice()])
            .await
            .unwrap();
        assert_eq!(
            stake,
            PosApi::get_validator_stake(&*api, best, alice()).unwrap()
        );
        assert!(stake > 0);

        // The root origin recorded one offence against Alice in block two.
        let slashed: u32 = call(&module, "pos_getSlashingCount", rpc_params![alice()])
            .await
            .unwrap();
        assert_eq!(slashed, 1);
        let status: serde_json::Value =
            call(&module, "pos_getValidatorStatus", rpc_params![alice()])
                .await
                .unwrap();
        assert_eq!(status, serde_json::json!("slashed"));

        // Bob holds no validator seat.
        let status: serde_json::Value = call(&module, "pos_getValidatorStatus", rpc_params![bob()])
            .await
            .unwrap();
        assert_eq!(status, serde_json::json!("inactive"));
    }
}

#[tokio::test]
async fn the_stake_status_reads_active_before_the_slashing() {
    let module = cerulea_methods(ViewAt::block(1), open());
    let status: serde_json::Value = call(&module, "pos_getValidatorStatus", rpc_params![alice()])
        .await
        .unwrap();
    assert_eq!(status, serde_json::json!("active"));
    let slashed: u32 = call(&module, "pos_getSlashingCount", rpc_params![alice()])
        .await
        .unwrap();
    assert_eq!(slashed, 0);
}

#[tokio::test]
async fn the_inference_methods_read_the_result_and_the_challenge() {
    let module = served(true, false);
    let result: Option<InferenceResult> =
        call(&module, "poi_getInferenceResult", rpc_params![alice()])
            .await
            .unwrap();
    let result = result.expect("the development validator carries an inference result");
    assert_eq!(result.result, 80);
    let confidence: Option<u32> = call(&module, "poi_getInferenceConfidence", rpc_params![alice()])
        .await
        .unwrap();
    assert_eq!(confidence, Some(result.confidence));

    let none: Option<InferenceResult> = call(&module, "poi_getInferenceResult", rpc_params![bob()])
        .await
        .unwrap();
    assert!(none.is_none());
    let none: Option<u32> = call(&module, "poi_getInferenceConfidence", rpc_params![bob()])
        .await
        .unwrap();
    assert!(none.is_none());

    let window: ChallengeWindow = call(&module, "poi_getChallengeWindow", rpc_params![])
        .await
        .unwrap();
    assert_eq!(window.start_block, 0);
    assert_eq!(window.end_block, 50);

    // Alice carries a result and lodged no challenge.
    let status: serde_json::Value = call(&module, "poi_getInferenceStatus", rpc_params![alice()])
        .await
        .unwrap();
    assert_eq!(status, serde_json::json!("submitted"));
    // Bob lodged a challenge and carries no result of his own.
    let status: serde_json::Value = call(&module, "poi_getInferenceStatus", rpc_params![bob()])
        .await
        .unwrap();
    assert_eq!(status, serde_json::json!("verified"));

    // Before the challenge Bob stood with nothing on record.
    let earlier = cerulea_methods(ViewAt::block(1), open());
    let status: serde_json::Value = call(&earlier, "poi_getInferenceStatus", rpc_params![bob()])
        .await
        .unwrap();
    assert_eq!(status, serde_json::json!("pending"));
}

#[tokio::test]
async fn the_framework_methods_name_the_scheduled_author_and_the_weights() {
    let chain = fixture();
    let best = chain.client.info().best_hash;
    let module = served(true, false);

    let current: Option<AccountId> = call(&module, "dcf_getCurrentAuthor", rpc_params![])
        .await
        .unwrap();
    assert_eq!(current, Some(alice()));
    let next: Option<AccountId> = call(&module, "dcf_getExpectedAuthor", rpc_params![12u32])
        .await
        .unwrap();
    assert_eq!(next, Some(alice()));

    let scores: Vec<(AccountId, u64)> = call(&module, "dcf_getValidatorScores", rpc_params![])
        .await
        .unwrap();
    assert_eq!(
        scores,
        chain
            .client
            .runtime_api()
            .get_validator_scores(best)
            .unwrap()
    );
    assert!(scores.iter().any(|(validator, _)| *validator == alice()));

    let weights: ConsensusWeights = call(&module, "dcf_getConsensusWeights", rpc_params![])
        .await
        .unwrap();
    let (stake_part, inference_part) = chain
        .client
        .runtime_api()
        .get_consensus_weights(best)
        .unwrap();
    assert_eq!(weights.pos_weight, stake_part);
    assert_eq!(weights.poi_weight, inference_part);
}

#[tokio::test]
async fn the_unified_methods_compose_the_validator_profile() {
    let chain = fixture();
    let best = chain.client.info().best_hash;
    let api = chain.client.runtime_api();
    let module = served(true, false);

    let epoch: u32 = call(&module, "cerulea_getCurrentEpoch", rpc_params![])
        .await
        .unwrap();
    assert_eq!(epoch, 0);

    let validators: Vec<AccountId> = call(&module, "cerulea_listValidators", rpc_params![])
        .await
        .unwrap();
    assert_eq!(validators, vec![alice()]);

    let profile: ValidatorProfile =
        call(&module, "cerulea_getValidatorProfile", rpc_params![alice()])
            .await
            .unwrap();
    assert_eq!(profile.account, alice());
    assert_eq!(
        profile.stake,
        PosApi::get_validator_stake(&*api, best, alice()).unwrap()
    );
    assert_eq!(
        profile.pos_score,
        api.get_validator_score(best, alice()).unwrap()
    );
    assert!(matches!(
        profile.status,
        crate::rpc::ValidatorStatus::Slashed
    ));
    let (authored, missed) = api.get_validator_participation(best, alice()).unwrap();
    assert_eq!(profile.authored_blocks, authored);
    assert_eq!(profile.missed_blocks, missed);

    let trust: TrustScore = call(&module, "cerulea_getTrustScore", rpc_params![alice()])
        .await
        .unwrap();
    assert_eq!(trust.total, profile.trust_score);
    let (stake_part, inference_part) = api.get_consensus_weights(best).unwrap();
    assert_eq!(
        trust.pos_component,
        u64::from(profile.pos_score) * stake_part
    );
    assert_eq!(trust.poi_component, profile.poi_score * inference_part);

    // Bob holds no seat, so the profile reads inactive and empty.
    let outsider: ValidatorProfile =
        call(&module, "cerulea_getValidatorProfile", rpc_params![bob()])
            .await
            .unwrap();
    assert!(matches!(
        outsider.status,
        crate::rpc::ValidatorStatus::Inactive
    ));
    assert_eq!(outsider.poi_score, 0);
    assert_eq!(outsider.authored_blocks, 0);

    // Before the slashing Alice read active.
    let earlier = cerulea_methods(ViewAt::block(1), open());
    let profile: ValidatorProfile = call(
        &earlier,
        "cerulea_getValidatorProfile",
        rpc_params![alice()],
    )
    .await
    .unwrap();
    assert!(matches!(
        profile.status,
        crate::rpc::ValidatorStatus::Active
    ));
}

#[tokio::test]
async fn the_status_and_health_methods_report_one_validator_as_degraded() {
    let module = served(true, false);
    let status: SystemStatus = call(&module, "cerulea_getStatus", rpc_params![])
        .await
        .unwrap();
    assert_eq!(status.current_epoch, 0);
    assert_eq!(status.active_validators, 1);
    assert_eq!(status.total_validators, 1);
    assert_eq!(status.last_finalized_block, 2);
    assert!(matches!(
        status.consensus_health,
        crate::rpc::ConsensusHealth::Degraded
    ));

    let health: HealthStatus = call(&module, "cerulea_health", rpc_params![])
        .await
        .unwrap();
    assert!(health.is_healthy);
    assert_eq!(health.issues, vec!["Low validator count: 1".to_string()]);

    // A reader standing at block one sits behind the finalised block two.
    let behind = cerulea_methods(ViewAt::block(1), open());
    let health: HealthStatus = call(&behind, "cerulea_health", rpc_params![])
        .await
        .unwrap();
    assert!(!health.is_healthy);
    assert!(health
        .issues
        .contains(&"Node is behind finalized block".to_string()));
}

#[tokio::test]
async fn the_describe_method_lists_every_unified_method_once() {
    let module = served(true, false);
    let methods: Vec<RpcMethodDescription> = call(&module, "cerulea_describe", rpc_params![])
        .await
        .unwrap();
    assert_eq!(methods.len(), 30);
    let names: std::collections::BTreeSet<&str> =
        methods.iter().map(|method| method.name.as_str()).collect();
    assert_eq!(names.len(), methods.len());
    for expected in [
        "cerulea_getCurrentEpoch",
        "cerulea_getValidatorProfile",
        "cerulea_health",
    ] {
        assert!(names.contains(expected), "{expected}");
    }
    assert!(methods
        .iter()
        .all(|method| !method.description.is_empty() && !method.returns.is_empty()));
}

#[tokio::test]
async fn the_authoring_statistics_read_the_missed_slots() {
    let chain = fixture();
    let best = chain.client.info().best_hash;
    let module = served(true, false);
    let (authored, missed) = chain
        .client
        .runtime_api()
        .get_validator_participation(best, alice())
        .unwrap();
    assert_eq!(authored, 2);
    assert_eq!(missed, MISSED_BLOCKS);

    let stats: BlockAuthoringStats = call(
        &module,
        "cerulea_getBlockAuthoringStats",
        rpc_params![alice()],
    )
    .await
    .unwrap();
    assert_eq!(stats.authored_blocks, 2);
    assert_eq!(stats.missed_blocks, MISSED_BLOCKS);
    assert_eq!(stats.expected_blocks, 2 + MISSED_BLOCKS);
    let rate = 2.0 / f64::from(2 + MISSED_BLOCKS) * 100.0;
    assert!((stats.participation_rate - rate).abs() < 1e-9);
    assert_eq!(stats.consecutive_misses, 5);
    assert_eq!(
        stats.last_authored_block,
        Some(
            chain
                .client
                .runtime_api()
                .get_validator_last_active(best, alice())
                .unwrap()
        )
    );

    // Bob carries no participation record, so the rate reads full.
    let idle: BlockAuthoringStats = call(
        &module,
        "cerulea_getBlockAuthoringStats",
        rpc_params![bob()],
    )
    .await
    .unwrap();
    assert_eq!(idle.expected_blocks, 0);
    assert_eq!(idle.participation_rate, 100.0);
    assert_eq!(idle.consecutive_misses, 0);
    assert_eq!(idle.last_authored_block, None);

    let all: Vec<(AccountId, BlockAuthoringStats)> =
        call(&module, "cerulea_getAllBlockAuthoringStats", rpc_params![])
            .await
            .unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].0, alice());
    assert_eq!(all[0].1.missed_blocks, MISSED_BLOCKS);
}

#[tokio::test]
async fn the_finality_methods_read_the_registry_state() {
    let chain = fixture();
    let module = served(true, false);

    let head: u32 = call(&module, "cerulea_getFinalizedHead", rpc_params![])
        .await
        .unwrap();
    let genesis_final: bool = call(&module, "cerulea_isBlockFinalized", rpc_params![head])
        .await
        .unwrap();
    assert!(genesis_final);
    let tip_final: bool = call(&module, "cerulea_isBlockFinalized", rpc_params![head + 100])
        .await
        .unwrap();
    assert!(!tip_final);

    let hash: Result<String, ErrorObjectOwned> =
        call(&module, "cerulea_getFinalizedHash", rpc_params![]).await;
    match hash {
        Ok(text) => assert!(text.starts_with("0x")),
        Err(error) => {
            assert_eq!(error.code(), -32000);
            assert_eq!(error.message(), "No finalized block hash available");
        }
    }

    let round: u32 = call(&module, "cerulea_getCurrentRound", rpc_params![])
        .await
        .unwrap();
    let set: u32 = call(&module, "cerulea_getValidatorSetId", rpc_params![])
        .await
        .unwrap();
    assert!(round <= chain.client.info().best_number);
    assert_eq!(set, 0);

    let weights: Vec<(AccountId, u128)> =
        call(&module, "cerulea_getValidatorWeights", rpc_params![])
            .await
            .unwrap();
    assert_eq!(weights.len(), 1);
    assert_eq!(weights[0].0, alice());

    let tip = format!("{:?}", chain.client.info().best_hash);
    let tally: u128 = call(&module, "cerulea_getAccumulatedWeight", rpc_params![tip])
        .await
        .unwrap();
    assert_eq!(tally, 0);
}

#[tokio::test]
async fn the_accumulated_weight_refuses_a_malformed_block_hash() {
    let module = served(true, false);
    let error = refusal(&module, "cerulea_getAccumulatedWeight", rpc_params!["0xzz"]).await;
    assert_eq!(error.code(), -32602);
    assert!(error.message().starts_with("Invalid block hash format:"));

    let error = refusal(
        &module,
        "cerulea_getAccumulatedWeight",
        rpc_params!["0x1234"],
    )
    .await;
    assert_eq!(error.code(), -32602);
    assert_eq!(error.message(), "Block hash must be 32 bytes");
}

#[tokio::test]
async fn the_fork_status_reads_the_local_head() {
    let chain = fixture();
    let module = served(true, false);
    let status: std::collections::HashMap<String, serde_json::Value> =
        call(&module, "fork_getStatus", rpc_params![])
            .await
            .unwrap();
    assert_eq!(status["service"], serde_json::json!("fork-detection"));
    assert_eq!(
        status["version"],
        serde_json::json!(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        status["local_best_block"],
        serde_json::json!(chain.client.info().best_number)
    );
    assert_eq!(status["local_finalized_block"], serde_json::json!(2));
    assert_eq!(
        status["local_best_hash"],
        serde_json::json!(chain.client.info().best_hash.to_string())
    );
}

#[tokio::test]
async fn the_fork_check_compares_the_local_head_with_each_peer() {
    let chain = fixture();
    let (url, handle) = serve_fixture().await;
    let tip = chain.client.info().best_number;

    let error = refusal(
        &served(true, false),
        "fork_checkPeers",
        rpc_params![Vec::<String>::new(), 1u32],
    )
    .await;
    assert_eq!(error.code(), -32602);
    assert_eq!(
        error.message(),
        "At least one peer endpoint must be provided"
    );

    // The peer shares the head, so the divergence reads zero.
    let reports: Vec<ForkReport> = call(
        &served(true, false),
        "fork_checkPeers",
        rpc_params![
            vec![
                url.clone(),
                "http://127.0.0.1:1".to_string(),
                "not a url".to_string()
            ],
            1u32
        ],
    )
    .await
    .unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].peer_id, "peer-0");
    assert_eq!(reports[0].local_best, tip);
    assert_eq!(reports[0].peer_best, tip);
    assert_eq!(reports[0].divergence, 0);

    // A reader standing at block one trails the peer by the rest of the chain.
    let behind = cerulea_methods(ViewAt::block(1), open());
    let reports: Vec<ForkReport> = call(
        &behind,
        "fork_checkPeers",
        rpc_params![vec![url.clone()], 1u32],
    )
    .await
    .unwrap();
    assert_eq!(reports[0].local_best, 1);
    assert_eq!(reports[0].peer_best, tip);
    assert_eq!(reports[0].divergence, tip - 1);

    // The peer answers over a real HTTP connection.
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(&url)
        .unwrap();
    let name: String = client
        .request("chain_getChainName", rpc_params![])
        .await
        .unwrap();
    assert_eq!(name, "Cerulea");
    handle.stop().unwrap();
}

/// The first part of every message a failed runtime read carries.
fn runtime_failure(error: &ErrorObjectOwned, prefix: &str) {
    assert_eq!(error.code(), -32000, "{}", error.message());
    assert!(
        error.message().starts_with(prefix),
        "The message {} starts with {prefix}",
        error.message()
    );
}

#[tokio::test]
async fn every_method_reports_a_failed_runtime_read() {
    let module = cerulea_methods(ViewAt::unknown(), open());
    let hash = format!("{:?}", fixture().client.info().best_hash);
    for (method, params, prefix) in [
        (
            "pos_getValidatorScore",
            rpc_params![alice()],
            "Runtime API call failed: ",
        ),
        (
            "pos_getValidatorStake",
            rpc_params![alice()],
            "Runtime API call failed: ",
        ),
        (
            "pos_getSlashingCount",
            rpc_params![alice()],
            "Runtime API call failed: ",
        ),
        (
            "pos_getValidatorStatus",
            rpc_params![alice()],
            "Runtime API call failed: ",
        ),
        (
            "poi_getInferenceResult",
            rpc_params![alice()],
            "Runtime API call failed: ",
        ),
        (
            "poi_getInferenceConfidence",
            rpc_params![alice()],
            "Runtime API call failed: ",
        ),
        (
            "poi_getChallengeWindow",
            rpc_params![],
            "Runtime API call failed: ",
        ),
        (
            "poi_getInferenceStatus",
            rpc_params![alice()],
            "Runtime API call failed: ",
        ),
        (
            "dcf_getCurrentAuthor",
            rpc_params![],
            "Runtime API call failed: ",
        ),
        (
            "dcf_getExpectedAuthor",
            rpc_params![4u32],
            "Runtime API call failed: ",
        ),
        (
            "dcf_getValidatorScores",
            rpc_params![],
            "Runtime API call failed: ",
        ),
        (
            "dcf_getConsensusWeights",
            rpc_params![],
            "Runtime API call failed: ",
        ),
        (
            "cerulea_getCurrentEpoch",
            rpc_params![],
            "Runtime API call failed: ",
        ),
        (
            "cerulea_getValidatorProfile",
            rpc_params![alice()],
            "Failed to get validator stake: ",
        ),
        (
            "cerulea_getTrustScore",
            rpc_params![alice()],
            "Failed to get PoS score: ",
        ),
        (
            "cerulea_listValidators",
            rpc_params![],
            "Failed to get active validators: ",
        ),
        (
            "cerulea_getStatus",
            rpc_params![],
            "Failed to get current epoch: ",
        ),
        (
            "cerulea_getBlockAuthoringStats",
            rpc_params![alice()],
            "Failed to get validator participation: ",
        ),
        (
            "cerulea_getAllBlockAuthoringStats",
            rpc_params![],
            "Failed to get active validators: ",
        ),
        (
            "cerulea_getFinalizedHead",
            rpc_params![],
            "Runtime API call failed: ",
        ),
        (
            "cerulea_getFinalizedHash",
            rpc_params![],
            "Failed to get finalized block number: ",
        ),
        (
            "cerulea_isBlockFinalized",
            rpc_params![1u32],
            "Runtime API call failed: ",
        ),
        (
            "cerulea_getCurrentRound",
            rpc_params![],
            "Runtime API call failed: ",
        ),
        (
            "cerulea_getAccumulatedWeight",
            rpc_params![hash],
            "Runtime API call failed: ",
        ),
        (
            "cerulea_getValidatorSetId",
            rpc_params![],
            "Runtime API call failed: ",
        ),
        (
            "cerulea_getValidatorWeights",
            rpc_params![],
            "Runtime API call failed: ",
        ),
    ] {
        let error = refusal(&module, method, params).await;
        runtime_failure(&error, prefix);
    }

    // The health method gathers every failed read as an issue.
    let health: HealthStatus = call(&module, "cerulea_health", rpc_params![])
        .await
        .unwrap();
    assert!(!health.is_healthy);
    assert_eq!(health.issues.len(), 3);
    assert!(health.issues[0].starts_with("Cannot get current epoch: "));
    assert!(health.issues[1].starts_with("Cannot get active validators: "));
    assert!(health.issues[2].starts_with("Cannot get consensus weights: "));
}
