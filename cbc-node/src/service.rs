#![allow(unused_variables, static_mut_refs)]
use futures::FutureExt;
use sc_client_api::{Backend, Finalizer};
use sc_service::{error::Error as ServiceError, Configuration, TaskManager};
use sc_telemetry::TelemetryWorker;
use sc_transaction_pool_api::OffchainTransactionPoolFactory;
use cbc_runtime::{self, apis::RuntimeApi, opaque::Block};
use std::{sync::Arc};
use cbc_consensus::{ConsensusParams, AuthorSelectionMode};
use sp_blockchain::HeaderBackend;
use sp_api::ProvideRuntimeApi;
use crate::block_tracker::{BlockTracker, BlockTrackerConfig};
use cbc_consensus::import_queue::DcfImportQueue;

pub(crate) type FullClient = sc_service::TFullClient<
    Block,
    RuntimeApi,
    sc_executor::WasmExecutor<sp_io::SubstrateHostFunctions>,
>;

type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;

pub type Service = sc_service::PartialComponents<
    FullClient,
    FullBackend,
    FullSelectChain,
    sc_consensus::BasicQueue<Block>,
    sc_transaction_pool::TransactionPoolHandle<Block, FullClient>,
    (), 
>;

#[derive(Clone)]
pub struct NodeConfig {
    pub rpc_config: crate::rpc::RpcSecurityConfig,
}

pub fn new_partial(
    config: &Configuration,
    node_config: &NodeConfig,
) -> Result<Service, ServiceError> {
    let telemetry = config
        .telemetry_endpoints
        .clone()
        .filter(|x| !x.is_empty())
        .map(|endpoints| -> Result<_, sc_telemetry::Error> {
            let worker = TelemetryWorker::new(16)?;
            let telemetry = worker.handle().new_telemetry(endpoints);
            Ok((worker, telemetry))
        })
        .transpose()?;

    let executor = sc_service::new_wasm_executor::<sp_io::SubstrateHostFunctions>(&config.executor);

    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, _>(
            config,
            telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
            executor,
        )?;

    let client = Arc::new(client);

    let _telemetry = telemetry.map(|(worker, telemetry)| {
        task_manager.spawn_handle().spawn("telemetry", None, worker.run());
        telemetry
    });

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = Arc::from(
        sc_transaction_pool::Builder::new(
            task_manager.spawn_essential_handle(),
            client.clone(),
            config.role.is_authority().into(),
        )
        .with_options(config.transaction_pool.clone())
        .with_prometheus(config.prometheus_registry())
        .build(),
    );

    // Initialize Prometheus metrics for consensus monitoring
    let consensus_metrics = if let Some(registry) = config.prometheus_registry() {
        match cbc_consensus::metrics::ConsensusMetrics::new(registry) {
            Ok(metrics) => {
                log::info!("CBC: Consensus metrics initialized successfully in new_partial");
                Some(metrics)
            }
            Err(e) => {
                log::error!("CBC: Failed to initialize consensus metrics in new_partial: {:?}", e);
                None
            }
        }
    } else {
        log::warn!("CBC: No Prometheus registry available in new_partial, metrics disabled");
        None
    };

 
    let import_queue = {
        let dcf_verifier: DcfImportQueue<Block, FullClient, FullBackend> = if let Some(ref consensus_metrics) = consensus_metrics {
            DcfImportQueue::new_with_metrics(client.clone(), consensus_metrics.clone())
        } else {
            DcfImportQueue::new(client.clone())
        };
        
        sc_consensus::BasicQueue::new(
            dcf_verifier,
            Box::new(client.clone()),
            None,
            &task_manager.spawn_essential_handle(),
            None,
        )
    };

    Ok(sc_service::PartialComponents {
        client,
        backend,
        task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other: (), 
    })
}

pub fn new_full<
    N: sc_network::NetworkBackend<Block, <Block as sp_runtime::traits::Block>::Hash>,
>(
    config: Configuration,
    node_config: &NodeConfig,
) -> Result<TaskManager, ServiceError>
where
    N: sc_network::NetworkBackend<Block, <Block as sp_runtime::traits::Block>::Hash>,
{
    let sc_service::PartialComponents {
        client,
        backend,
        mut task_manager,
        import_queue,
        keystore_container,
        select_chain: _,
        transaction_pool,
        ..
    } = new_partial(&config, &node_config)?;

    let net_config = sc_network::config::FullNetworkConfiguration::<
        Block,
        <Block as sp_runtime::traits::Block>::Hash,
        N,
    >::new(&config.network, config.prometheus_registry().cloned());

    let metrics = N::register_notification_metrics(config.prometheus_registry());
    let _peer_store_handle = net_config.peer_store_handle();

    let (network, system_rpc_tx, tx_handler_controller, sync_service) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            net_config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            block_announce_validator_builder: None,
            warp_sync_config: None,
            block_relay: None,
            metrics,
        })?;

    if config.offchain_worker.enabled {
        let offchain_workers =
            sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
                runtime_api_provider: client.clone(),
                is_validator: config.role.is_authority(),
                keystore: Some(keystore_container.keystore()),
                offchain_db: backend.offchain_storage(),
                transaction_pool: Some(OffchainTransactionPoolFactory::new(
                    transaction_pool.clone(),
                )),
                network_provider: Arc::new(network.clone()),
                enable_http_requests: true,
                custom_extensions: |_| vec![],
            })?;

        task_manager.spawn_handle().spawn(
            "offchain-workers-runner",
            "offchain-worker",
            offchain_workers.run(client.clone(), task_manager.spawn_handle()).boxed(),
        );
    }


    // --- CBC Custom Consensus Integration ---
    // This integrates custom PoS/PoI consensus 
    // The DCF pallet provides validator selection based on PoS and PoI scores
    
    let consensus_params = ConsensusParams {
        author_selection_mode: AuthorSelectionMode::RoundRobin,
        finality_threshold: 10,
        block_time: 6,
        max_block_size: 2 * 1024 * 1024,
        max_transactions_per_block: 1000,
        slot_duration: std::time::Duration::from_secs(6),
        min_block_time: 1000,
        metrics_update_interval: 10,
        score_refresh_interval: 50,
        consensus_loop_interval: 1000,
        detailed_logging_interval: 100,
        health_check_interval: 10,
        min_performance_score: 30,
        high_performance_score: 80,
        min_participation_rate: 50,
        high_participation_rate: 90,
        max_missed_blocks: 10,
        max_missed_blocks_high: 2,
        healthy_validator_score: 50,
        healthy_participation_rate: 80,
        healthy_missed_blocks_max: 5,
        top_validators_display_count: 5,
        health_check_sample_size: 5,
    };

    // Initialize Prometheus metrics for consensus monitoring
    let consensus_metrics = if let Some(registry) = config.prometheus_registry() {
        match cbc_consensus::metrics::ConsensusMetrics::new(registry) {
            Ok(metrics) => {
                log::info!("CBC: Consensus metrics initialized successfully");
                Some(metrics)
            }
            Err(e) => {
                log::error!("CBC: Failed to initialize consensus metrics: {:?}", e);
                None
            }
        }
    } else {
        log::warn!("CBC: No Prometheus registry available, metrics disabled");
        None
    };
    
    // Use the client directly for block import - Substrate handles this internally
    let dcf_block_import_arc = client.clone();
    
    // Set up  PoS+PoI block production with DCF consensus
    if config.role.is_authority() {
        let mut dcf_consensus = if let Some(metrics) = consensus_metrics.clone() {
            cbc_consensus::DcfConsensus::<Block, FullClient, sp_core::sr25519::Pair, _>::new_with_metrics(
                client.clone(),
                transaction_pool.clone(),
                dcf_block_import_arc.clone(), // Use real import queue
                consensus_params.clone(),
                metrics,
            )
        } else {
            cbc_consensus::DcfConsensus::<Block, FullClient, sp_core::sr25519::Pair, _>::new(
                client.clone(),
                transaction_pool.clone(),
                dcf_block_import_arc.clone(), // Use real import queue
                consensus_params.clone(),
            )
        };
        
        task_manager.spawn_essential_handle().spawn(
            "cbc-pos-poi-consensus",
            None,
            async move {
                log::info!("CBC: Starting real PoS+PoI consensus engine for block production");
                // Run the consensus engine which will produce real blocks
                dcf_consensus.run().await;
                log::error!("CBC: PoS+PoI consensus engine unexpectedly stopped");
            },
        );
    }

    // Start metrics update task if metrics are available
    if let Some(metrics) = consensus_metrics.clone() {
        let metrics_client = client.clone();
        task_manager.spawn_handle().spawn(
            "consensus-metrics-updater",
            None,
            async move {
                log::info!("CBC: Starting consensus metrics updater");
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
                
                loop {
                    interval.tick().await;
                    
                    if let Err(e) = metrics.update_from_runtime(&metrics_client) {
                        log::warn!("CBC: Failed to update consensus metrics: {}", e);
                    }
                }
            },
        );
    }

    // Start block authoring and missed block tracking service
    {
        let tracker_client = client.clone();
        task_manager.spawn_handle().spawn(
            "block-authoring-tracker",
            None,
            async move {
                log::info!("CBC: Starting block authoring tracking service");
                
                let tracker_config = BlockTrackerConfig {
                    monitoring_interval: std::time::Duration::from_secs(10),
                    stats_reporting_interval: std::time::Duration::from_secs(120), // 2 minutes
                    underperformance_threshold: 80.0, // 80% participation threshold
                    max_consecutive_misses: 3,
                    enable_alerts: true,
                    enable_detailed_logging: true,
                };
                
                let mut tracker = BlockTracker::<Block, FullClient>::new(tracker_client, tracker_config);
                tracker.run().await;
            },
        );
    }

    // Start DCF finality sync service
    // This syncs DCF's internal finality state to Substrate's client finalized head
    {
        use sp_runtime::traits::SaturatedConversion;
        use pallet_cbc_dcf::DcfApi as RuntimeDcfApi;
        use cbc_runtime::AccountId;
        
        let finality_client = client.clone();
        task_manager.spawn_essential_handle().spawn(
            "dcf-finality-sync",
            None,
            async move {
                log::info!("DCF: Starting finality sync service");
                
                loop {
                    // Wait 6 seconds between checks (one block time)
                    tokio::time::sleep(std::time::Duration::from_secs(6)).await;
                    
                    // Get DCF's finalized block number
                    let api = finality_client.runtime_api();
                    let best_hash = finality_client.info().best_hash;
                    
                    match api.get_last_finalized_block(best_hash) {
                        Ok(dcf_finalized) => {
                            let client_info = finality_client.info();
                            let client_finalized: u32 = client_info.finalized_number.saturated_into();
                            
                            if dcf_finalized > client_finalized {
                                log::info!(
                                    "DCF Finality Sync: Client at block #{}, DCF finalized up to #{}, syncing...",
                                    client_finalized,
                                    dcf_finalized
                                );
                                
                                // Finalize all blocks from client_finalized+1 to dcf_finalized
                                for block_num in (client_finalized + 1)..=dcf_finalized {
                                    // Get block hash for this number
                                    match finality_client.hash(block_num.into()) {
                                        Ok(Some(block_hash)) => {
                                            // Finalize this block
                                            match finality_client.finalize_block(block_hash, None, true) {
                                                Ok(_) => {
                                                    log::debug!("DCF Finality Sync: Finalized block #{}", block_num);
                                                }
                                                Err(e) => {
                                                    log::error!(
                                                        "DCF Finality Sync: Failed to finalize block #{}: {:?}",
                                                        block_num,
                                                        e
                                                    );
                                                    // Don't break, try to continue with next blocks
                                                }
                                            }
                                        }
                                        Ok(None) => {
                                            log::warn!(
                                                "DCF Finality Sync: Block #{} hash not found, skipping",
                                                block_num
                                            );
                                        }
                                        Err(e) => {
                                            log::error!(
                                                "DCF Finality Sync: Failed to get hash for block #{}: {:?}",
                                                block_num,
                                                e
                                            );
                                        }
                                    }
                                }
                                
                                // Log final state
                                let new_client_finalized: u32 = finality_client.info().finalized_number.saturated_into();
                                log::info!(
                                    "DCF Finality Sync: Sync complete. Client finalized head now at block #{}",
                                    new_client_finalized
                                );
                            } else if dcf_finalized == client_finalized {
                                log::trace!(
                                    "DCF Finality Sync: In sync at block #{}",
                                    client_finalized
                                );
                            } else {
                                log::warn!(
                                    "DCF Finality Sync: Client ahead of DCF (client: #{}, DCF: #{})",
                                    client_finalized,
                                    dcf_finalized
                                );
                            }
                        }
                        Err(e) => {
                            log::error!("DCF Finality Sync: Failed to get DCF finalized block: {:?}", e);
                        }
                    }
                }
            },
        );
    }
    
    log::info!("DCF: Consensus monitoring active. Block production handled by Substrate's default mechanisms.");

    let rpc_extensions_builder = {
        let client = client.clone();
        let pool = transaction_pool.clone();
        let rpc_config = node_config.rpc_config.clone();
        let rpc_consensus_metrics = consensus_metrics.clone(); // Clone for RPC use
        let rpc_backend = backend.clone();

        Box::new(move |_| {
            let deps = crate::rpc::FullDeps {
                client: client.clone(),
                pool: pool.clone(),
                rpc_config: rpc_config.clone(),
                consensus_metrics: rpc_consensus_metrics.clone(),
                backend: rpc_backend.clone(),
            };
            crate::rpc::create_full(deps).map_err(Into::into)
        })
    };

    let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        network: Arc::new(network.clone()),
        client: client.clone(),
        keystore: keystore_container.keystore(),
        task_manager: &mut task_manager,
        transaction_pool: transaction_pool.clone(),
        rpc_builder: rpc_extensions_builder,
        backend,
        system_rpc_tx,
        tx_handler_controller,
        sync_service: sync_service.clone(),
        config,
        telemetry: None,
    })?;

    Ok(task_manager)
}