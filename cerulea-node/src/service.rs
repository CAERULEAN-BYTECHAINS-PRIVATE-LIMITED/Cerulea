#![allow(unused_variables, static_mut_refs)]
use crate::block_tracker::{BlockTracker, BlockTrackerConfig};
use cerulea_consensus::dvf_gossip::{DvfGossipValidator, DvfVotePool, DVF_PROTOCOL_NAME};
use cerulea_consensus::equivocation_gossip::{
    EquivocationGossipValidator, EQUIVOCATION_PROTOCOL_NAME, MAX_EQUIVOCATION_REPORT_BYTES,
};
use cerulea_consensus::import_queue::DcfImportQueue;
use cerulea_consensus::{AuthorSelectionMode, ConsensusParams};
use cerulea_runtime::{self, apis::RuntimeApi, opaque::Block, AccountId, Hash};
use futures::FutureExt;
use sc_client_api::{Backend, Finalizer};
use sc_service::{error::Error as ServiceError, Configuration as ServiceSettings, TaskManager};
use sc_telemetry::TelemetryWorker;
use sc_transaction_pool_api::OffchainTransactionPoolFactory;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use std::{sync::Arc, sync::Mutex};

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
    Option<cerulea_consensus::metrics::ConsensusMetrics>,
>;

#[derive(Clone)]
pub struct NodeConfig {
    pub rpc_config: crate::rpc::RpcSecurityConfig,
}

pub fn new_partial(
    config: &ServiceSettings,
    node_config: &NodeConfig,
) -> Result<Service, ServiceError> {
    use crate::lifecycle_tracer::{LifecycleTracer, TraceMetadata};

    // STEP 7: Partial node components initialization started
    LifecycleTracer::global().trace_step(
        7,
        "service.rs::new_partial",
        "Partial node components initialization started",
        None,
    );

    let telemetry = config
        .telemetry_endpoints
        .clone()
        .filter(|x| !x.is_empty())
        .map(|endpoints| -> Result<_, sc_telemetry::Error> {
            let worker = TelemetryWorker::new(16)?;
            let telemetry = worker.handle().new_telemetry(endpoints);
            Ok((worker, telemetry))
        })
        .transpose()
        .inspect_err(|e| {
            LifecycleTracer::global().trace_error(
                8,
                "service.rs::new_partial",
                e,
                "Telemetry initialization failed",
            );
        })?;

    // STEP 8: Telemetry endpoints set
    if telemetry.is_some() {
        LifecycleTracer::global().trace_step(
            8,
            "service.rs::new_partial",
            "Telemetry endpoints set",
            Some(
                TraceMetadata::new()
                    .with_custom("telemetry_enabled".to_string(), "true".to_string()),
            ),
        );
    } else {
        LifecycleTracer::global().trace_step(
            8,
            "service.rs::new_partial",
            "Telemetry endpoints set (none)",
            None,
        );
    }

    let executor = sc_service::new_wasm_executor::<sp_io::SubstrateHostFunctions>(&config.executor);

    // STEP 9: WASM executor initialized
    LifecycleTracer::global().trace_step(
        9,
        "service.rs::new_partial",
        "WASM executor initialized",
        None,
    );

    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, _>(
            config,
            telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
            executor,
        )
        .inspect_err(|e| {
            LifecycleTracer::global().trace_error(
                10,
                "service.rs::new_partial",
                e,
                "Client and backend initialization failed",
            );
        })?;

    // STEP 10: Client and backend initialized
    let db_path = config
        .database
        .path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "in-memory".to_string());
    LifecycleTracer::global().trace_step(
        10,
        "service.rs::new_partial",
        "Client and backend initialized",
        Some(TraceMetadata::new().with_custom("database_path".to_string(), db_path)),
    );

    let client = Arc::new(client);

    // STEP 11: Keystore container created
    LifecycleTracer::global().trace_step(
        11,
        "service.rs::new_partial",
        "Keystore container created",
        None,
    );

    let _telemetry = telemetry.map(|(worker, telemetry)| {
        task_manager
            .spawn_handle()
            .spawn("telemetry", None, worker.run());
        telemetry
    });

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    // STEP 12: Chain selection strategy initialized
    LifecycleTracer::global().trace_step(
        12,
        "service.rs::new_partial",
        "Chain selection strategy initialized",
        Some(TraceMetadata::new().with_custom("strategy".to_string(), "LongestChain".to_string())),
    );

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

    // STEP 13: Transaction pool initialized
    LifecycleTracer::global().trace_step(
        13,
        "service.rs::new_partial",
        "Transaction pool initialized",
        Some(TraceMetadata::new().with_custom(
            "is_authority".to_string(),
            config.role.is_authority().to_string(),
        )),
    );

    // Initialize Prometheus metrics for consensus monitoring
    let consensus_metrics = if let Some(registry) = config.prometheus_registry() {
        match cerulea_consensus::metrics::ConsensusMetrics::new(registry) {
            Ok(metrics) => {
                log::info!("Cerulea: Consensus metrics initialized successfully in new_partial");
                Some(metrics)
            }
            Err(e) => {
                let error_msg = format!("Failed to initialize consensus metrics: {:?}", e);
                LifecycleTracer::global().trace_step(
                    14,
                    "service.rs::new_partial",
                    &error_msg,
                    Some(TraceMetadata::new().with_custom("error".to_string(), "true".to_string())),
                );
                log::error!(
                    "Cerulea: Failed to initialize consensus metrics in new_partial: {:?}",
                    e
                );
                None
            }
        }
    } else {
        log::warn!("Cerulea: No Prometheus registry available in new_partial, metrics disabled");
        None
    };

    // STEP 14: Prometheus consensus metrics registered
    LifecycleTracer::global().trace_step(
        14,
        "service.rs::new_partial",
        "Prometheus consensus metrics registered",
        Some(TraceMetadata::new().with_custom(
            "metrics_enabled".to_string(),
            consensus_metrics.is_some().to_string(),
        )),
    );

    let import_queue = {
        let dcf_verifier: DcfImportQueue<Block, FullClient, FullBackend> =
            if let Some(ref consensus_metrics) = consensus_metrics {
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

    // STEP 15: DCF import queue initialized
    LifecycleTracer::global().trace_step(
        15,
        "service.rs::new_partial",
        "DCF import queue initialized",
        Some(
            TraceMetadata::new().with_custom("verifier".to_string(), "DcfImportQueue".to_string()),
        ),
    );

    // STEP 16: Partial node components ready
    LifecycleTracer::global().trace_step(
        16,
        "service.rs::new_partial",
        "Partial node components ready",
        None,
    );

    // Flush the tracer to ensure all traces are written
    LifecycleTracer::global().flush();

    Ok(sc_service::PartialComponents {
        client,
        backend,
        task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other: consensus_metrics,
    })
}

pub fn new_full<N>(
    mut config: ServiceSettings,
    node_config: &NodeConfig,
) -> Result<TaskManager, ServiceError>
where
    N: sc_network::NetworkBackend<Block, <Block as sp_runtime::traits::Block>::Hash>,
{
    use crate::lifecycle_tracer::{LifecycleTracer, TraceMetadata};

    // STEP 6: Full service initialization started
    LifecycleTracer::global().trace_step(
        6,
        "service.rs::new_full",
        "Full service initialization started",
        None,
    );

    let sc_service::PartialComponents {
        client,
        backend,
        mut task_manager,
        import_queue,
        keystore_container,
        select_chain: _,
        transaction_pool,
        other: consensus_metrics,
    } = new_partial(&config, node_config).inspect_err(|e| {
        LifecycleTracer::global().trace_error(
            7,
            "service.rs::new_full",
            e,
            "Partial components failed to initialize",
        );
    })?;

    // Validate Registry settings parameters
    // This ensures all parameters meet requirements for correct operation
    {
        use pallet_cerulea_dvf::DvfApi as RuntimeDvfApi;
        use sp_api::ProvideRuntimeApi;

        log::info!("Registry: Validating settings parameters...");

        // Query runtime for Registry settings values
        let api = client.runtime_api();
        let best_hash = client.info().best_hash;

        let finality_checkpoint_interval = api
            .get_finality_checkpoint_interval(best_hash)
            .map_err(|e| {
                ServiceError::Other(format!(
                    "Failed to query FinalityCheckpointInterval: {:?}",
                    e
                ))
            })?;
        let finality_threshold = api.get_finality_threshold_perbill(best_hash).map_err(|e| {
            ServiceError::Other(format!("Failed to query FinalityThreshold: {:?}", e))
        })?;
        let vote_retention_rounds = api.get_vote_retention_rounds(best_hash).map_err(|e| {
            ServiceError::Other(format!("Failed to query VoteRetentionRounds: {:?}", e))
        })?;

        // For weight factors, we need to use the compile-time constants since they're not in the runtime API
        // These are the same values used by the runtime
        let stake_weight_factor = 1u128; // From runtime parameter_types
        let score_weight_factor = 1000u128; // From runtime parameter_types

        let dvf_config = cerulea_consensus::DvfConfig {
            finality_checkpoint_interval,
            finality_threshold,
            stake_weight_factor,
            score_weight_factor,
            vote_retention_rounds,
        };

        // Validate settings
        if let Err(e) = dvf_config.validate() {
            log::error!("Registry Settings Validation Failed: {}", e);
            log::error!("Node startup aborted due to invalid Registry settings.");
            return Err(ServiceError::Other(format!(
                "Invalid Registry settings: {}",
                e
            )));
        }

        // Log all settings parameters
        dvf_config.log_configuration();

        log::info!("Registry: Settings validation passed successfully");
    }

    // Bound the interface request body before the server starts. The shared
    // table names the highest number of bytes one request carries. A request
    // above that figure is refused before any decoding runs.
    let interface_body_limit = (cerulea_consensus::engines::bounds::MessagePath::RpcBody
        .max_bytes()
        / (1024 * 1024)) as u32;
    if config.rpc.max_request_size > interface_body_limit {
        log::info!(
            "Cerulea: the interface request body limit falls to {} megabytes",
            interface_body_limit
        );
        config.rpc.max_request_size = interface_body_limit;
    }

    let mut net_config = sc_network::config::FullNetworkConfiguration::<
        Block,
        <Block as sp_runtime::traits::Block>::Hash,
        N,
    >::new(&config.network, config.prometheus_registry().cloned());

    // STEP 17: Network settings prepared
    LifecycleTracer::global().trace_step(
        17,
        "service.rs::new_full",
        "Network settings prepared",
        Some(
            TraceMetadata::new()
                .with_custom(
                    "listen_addresses".to_string(),
                    format!("{:?}", config.network.listen_addresses),
                )
                .with_custom(
                    "public_addresses".to_string(),
                    format!("{:?}", config.network.public_addresses),
                ),
        ),
    );

    let metrics = N::register_notification_metrics(config.prometheus_registry());
    let _peer_store_handle = net_config.peer_store_handle();

    // Register Registry gossip protocol — backend-agnostic API (works for both libp2p & litep2p).
    let (dvf_notification_config, dvf_notification_service) = N::notification_config(
        cerulea_consensus::dvf_gossip::DVF_PROTOCOL_NAME.into(),
        Vec::new(),
        cerulea_consensus::engines::bounds::MessagePath::GossipMessage.max_bytes() as u64,
        None,
        sc_network::config::SetConfig {
            in_peers: 0,
            out_peers: 0,
            reserved_nodes: Vec::new(),
            non_reserved_mode: sc_network::config::NonReservedPeerMode::Accept,
        },
        metrics.clone(),
        _peer_store_handle.clone(),
    );
    net_config.add_notification_protocol(dvf_notification_config);

    // Register the equivocation report path. Equivocation means one validator
    // sealing two different headers for one slot at one rank. A node holding both
    // headers sends the evidence here. The message limit holds one report, so an
    // oversized message is refused before any decoding runs.
    let (equivocation_notification_config, equivocation_notification_service) =
        N::notification_config(
            EQUIVOCATION_PROTOCOL_NAME.into(),
            Vec::new(),
            MAX_EQUIVOCATION_REPORT_BYTES as u64,
            None,
            sc_network::config::SetConfig {
                in_peers: 0,
                out_peers: 0,
                reserved_nodes: Vec::new(),
                non_reserved_mode: sc_network::config::NonReservedPeerMode::Accept,
            },
            metrics.clone(),
            _peer_store_handle.clone(),
        );
    net_config.add_notification_protocol(equivocation_notification_config);

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
        })
        .inspect_err(|e| {
            LifecycleTracer::global().trace_error(
                18,
                "service.rs::new_full",
                e,
                "P2P network layer initialization failed",
            );
        })?;

    // Reserved validator peers banned by the sync layer are restored after
    // a backoff, so the links the Commit Engine relies on come back.
    crate::reserved_peers::spawn(&task_manager.spawn_handle(), network.clone());

    // Initialize Registry Gossip Components
    let dvf_gossip_pool = DvfVotePool::<Hash, AccountId>::new();
    let dvf_validator = Arc::new(DvfGossipValidator::new(
        dvf_gossip_pool.clone(),
        client.clone(),
    ));
    let dvf_gossip_validator: Arc<dyn sc_network_gossip::Validator<Block>> = dvf_validator.clone();

    // After every block import the validator learns the committed head and
    // the current round, which bound what the gossip store keeps and
    // repeats, and tries the messages it held again.
    {
        use futures::StreamExt;
        use pallet_cerulea_dvf::DvfApi as RuntimeDvfApi;
        use sc_client_api::BlockchainEvents;
        use sp_runtime::traits::SaturatedConversion;
        let dvf_validator = dvf_validator.clone();
        let window_client = client.clone();
        let mut imports = client.import_notification_stream();
        task_manager
            .spawn_handle()
            .spawn("dvf-gossip-held-votes", None, async move {
                while imports.next().await.is_some() {
                    let info = window_client.info();
                    let committed: u32 = info.finalized_number.saturated_into();
                    let round = window_client
                        .runtime_api()
                        .get_current_round(info.best_hash)
                        .unwrap_or_else(|_| dvf_validator.window().1);
                    dvf_validator.set_window(committed, round);

                    let decided = dvf_validator.retry_held();
                    if decided > 0 {
                        log::debug!(
                            "Commit Engine Gossip Validator: {} held messages decided after an import, {} still held",
                            decided,
                            dvf_validator.held_count()
                        );
                    }

                    // A peer sent the commitment of a block this node has yet
                    // to commit: the node commits it with that justification.
                    if let Some(heard) = dvf_validator.take_commitment() {
                        let justification = Some((
                            cerulea_consensus::CERULEA_COMMIT_ENGINE_ID,
                            heard.justification.clone(),
                        ));
                        match window_client.finalize_block(heard.block_hash, justification, true) {
                            Ok(()) => log::info!(
                                "Commit Engine: committed block #{} from the commitment a peer sent",
                                heard.block_number
                            ),
                            Err(reason) => log::warn!(
                                "Commit Engine: the commitment of block #{} a peer sent could not be applied: {:?}",
                                heard.block_number,
                                reason
                            ),
                        }
                    }
                }
            });
    }
    let dvf_gossip_engine = sc_network_gossip::GossipEngine::new(
        network.clone(),
        sync_service.clone(),
        dvf_notification_service, // Box<dyn NotificationService>
        DVF_PROTOCOL_NAME,
        dvf_gossip_validator,
        None,
    );
    let dvf_gossip_engine = Arc::new(Mutex::new(dvf_gossip_engine));

    // Every block the node commits with a justification goes out as one
    // commitment message. The gossip store keeps the newest one, so a peer
    // that connects or reconnects receives it at once and commits the block.
    {
        use codec::Decode;
        use futures::StreamExt;
        use sc_client_api::{BlockBackend, BlockchainEvents};
        use sp_runtime::traits::{Header as _, SaturatedConversion};
        let commitment_client = client.clone();
        let commitment_engine = dvf_gossip_engine.clone();
        let commitment_validator = dvf_validator.clone();
        let mut finality = client.finality_notification_stream();
        task_manager
            .spawn_handle()
            .spawn("dvf-commitment-relay", None, async move {
                let mut last_sent: u32 = 0;
                while let Some(notification) = finality.next().await {
                    let number: u32 = (*notification.header.number()).saturated_into();
                    let window = commitment_validator.window();
                    commitment_validator.set_window(number.max(window.0), window.1);
                    let Some(bytes) = cerulea_consensus::dvf_gossip::commitment_bytes::<
                        Hash,
                        AccountId,
                    >(
                        number,
                        last_sent,
                        commitment_client
                            .justifications(notification.hash)
                            .ok()
                            .flatten()
                            .as_ref()
                            .and_then(|held| held.get(cerulea_consensus::CERULEA_COMMIT_ENGINE_ID)),
                    ) else {
                        continue;
                    };
                    let topic =
                        <Hash as Decode>::decode(&mut &sp_core::hashing::blake2_256(&bytes)[..])
                            .unwrap_or_default();
                    if let Ok(mut engine) = commitment_engine.lock() {
                        engine.gossip_message(topic, bytes, false);
                        last_sent = number;
                        log::info!(
                            "Commit Engine: sent the commitment of block #{} to every peer",
                            number
                        );
                    }
                }
            });
    }

    // The equivocation report engine. The validator checks every arriving report
    // itself, so the node trusts the evidence rather than the peer.
    let equivocation_queue = std::sync::Arc::new(parking_lot::RwLock::new(
        cerulea_consensus::engines::authoring::equivocation::ReportQueue::new(),
    ));
    let equivocation_validator: Arc<dyn sc_network_gossip::Validator<Block>> = Arc::new(
        EquivocationGossipValidator::<Block>::new(equivocation_queue.clone()),
    );
    let equivocation_gossip_engine = sc_network_gossip::GossipEngine::new(
        network.clone(),
        sync_service.clone(),
        equivocation_notification_service,
        EQUIVOCATION_PROTOCOL_NAME,
        equivocation_validator,
        None,
    );
    let equivocation_gossip_engine = Arc::new(Mutex::new(equivocation_gossip_engine));

    {
        let equivocation_gossip_engine = equivocation_gossip_engine.clone();
        task_manager
            .spawn_handle()
            .spawn("equivocation-gossip-engine", None, async move {
                loop {
                    futures::future::poll_fn(|cx| {
                        let mut engine = equivocation_gossip_engine.lock().unwrap();
                        engine.poll_unpin(cx)
                    })
                    .await;
                }
            });
    }

    // Spawn Registry Gossip Service Task
    {
        let dvf_gossip_engine = dvf_gossip_engine.clone();
        task_manager
            .spawn_handle()
            .spawn("dvf-gossip-engine", None, async move {
                loop {
                    futures::future::poll_fn(|cx| {
                        let mut engine = dvf_gossip_engine.lock().unwrap();
                        engine.poll_unpin(cx)
                    })
                    .await;
                }
            });
    }

    // Start Registry Vote Creator Service (if node is authority)
    if config.role.is_authority() {
        // Get validator account from keystore
        let keystore = keystore_container.keystore();
        if let sc_service::config::KeystoreConfig::Path { path, .. } = &config.keystore {
            migrate_draw_keys(path, &keystore);
        }
        let mut public_keys = keystore.ed25519_public_keys(cerulea_consensus::CERULEA_KEY_TYPE);

        // The node generates and inserts one ed25519 key where the keystore holds
        // none. The key sits under the Cerulea key type on first start.
        if public_keys.is_empty() {
            use sp_core::crypto::{Pair, Ss58Codec};
            use sp_core::ed25519;

            // For well-known dev accounts (--alice, --bob, etc.) use deterministic seeds
            // so the validator set in genesis always matches. For any other node, generate
            // a fresh random keypair. The private key sits in the keystore on disk,
            // so the key survives a restart.
            let node_name = config.network.node_name.to_lowercase();

            let deterministic_seed: Option<&str> =
                if config.chain_spec.chain_type() == sc_service::ChainType::Development {
                    log::info!("Cerulea Registry: Dev mode — using //Alice deterministic key");
                    Some("//Alice")
                } else if node_name.contains("alice") {
                    log::info!("Cerulea Registry: Using deterministic key for Alice");
                    Some("//Alice")
                } else if node_name.contains("bob") {
                    log::info!("Cerulea Registry: Using deterministic key for Bob");
                    Some("//Bob")
                } else if node_name.contains("charlie") {
                    log::info!("Cerulea Registry: Using deterministic key for Charlie");
                    Some("//Charlie")
                } else if node_name.contains("dave") {
                    log::info!("Cerulea Registry: Using deterministic key for Dave");
                    Some("//Dave")
                } else if node_name.contains("eve") {
                    log::info!("Cerulea Registry: Using deterministic key for Eve");
                    Some("//Eve")
                } else if node_name.contains("ferdie") {
                    log::info!("Cerulea Registry: Using deterministic key for Ferdie");
                    Some("//Ferdie")
                } else {
                    None
                };

            match deterministic_seed {
                Some(seed) => {
                    let pair = ed25519::Pair::from_string(seed, None)
                        .expect("Cerulea: the well known seed yields a seal key");
                    keystore
                        .insert(
                            cerulea_consensus::CERULEA_KEY_TYPE,
                            seed,
                            pair.public().as_ref(),
                        )
                        .expect("Cerulea: the keystore takes the seal key");
                    // The Authoring Engine draws the slot committee with the sr25519
                    // draw key of the same well known name.
                    let draw = sp_core::sr25519::Pair::from_string(seed, None)
                        .expect("Cerulea: the well known seed yields a draw key");
                    keystore
                        .insert(
                            cerulea_consensus::CERULEA_DRAW_KEY_TYPE,
                            seed,
                            draw.public().as_ref(),
                        )
                        .expect("Cerulea: the keystore takes the draw key");
                    log::info!(
                        "Cerulea: seal key {} and draw key recorded",
                        pair.public().to_ss58check()
                    );
                }
                None => {
                    // A node name outside the well known list takes a fresh random
                    // ed25519 keypair. The keystore writes the key to disk under the
                    // chain base path, so the key survives a restart.
                    let (pair, seed_phrase, _) = ed25519::Pair::generate_with_phrase(None);
                    keystore
                        .insert(
                            cerulea_consensus::CERULEA_KEY_TYPE,
                            &seed_phrase,
                            pair.public().as_ref(),
                        )
                        .expect("Cerulea: the keystore takes the seal key");
                    let draw = sp_core::sr25519::Pair::from_string(&seed_phrase, None)
                        .expect("Cerulea: the generated phrase yields a draw key");
                    keystore
                        .insert(
                            cerulea_consensus::CERULEA_DRAW_KEY_TYPE,
                            &seed_phrase,
                            draw.public().as_ref(),
                        )
                        .expect("Cerulea: the keystore takes the draw key");
                    log::info!(
                        "Cerulea Registry: Auto-generated ed25519 key for new validator: {}",
                        pair.public().to_ss58check()
                    );
                    log::warn!(
						"Cerulea Registry: IMPORTANT — back up this seed phrase to recover your validator key: {}",
						seed_phrase
					);
                }
            }

            // Refresh after insertion
            public_keys = keystore.ed25519_public_keys(cerulea_consensus::CERULEA_KEY_TYPE);
        }

        if !public_keys.is_empty() {
            // The registry names the seal key of every active validator. The
            // node votes as the account that owns the local key the registry
            // names. A node the registry names no key for starts under its
            // first seal key, and the vote creator reselects before every vote.
            let named = {
                use pallet_cerulea_dvf::CeruleaAuthoringApi;
                let best = client.info().best_hash;
                client
                    .runtime_api()
                    .active_validator_keys(best)
                    .ok()
                    .and_then(|registry| {
                        cerulea_consensus::vote_creator::registry_seal_key(&public_keys, &registry)
                    })
            };
            let validator_account = match named {
                Some((account, _)) => account,
                None => AccountId::from(public_keys[0].0),
            };

            // Create Registry Finality Notifier
            let finality_notifier = Arc::new(cerulea_consensus::FinalityNotifier::new());

            // Every vote reaches this file before the node signs it.
            let ledger = cerulea_consensus::engines::commit::lock::LedgerStore::under(
                &config.base_path.config_dir(config.chain_spec.id()),
            );
            let vote_creator = cerulea_consensus::VoteCreatorService::new(
                client.clone(),
                keystore.clone(),
                dvf_gossip_engine.clone(),
                dvf_gossip_pool.clone(),
                validator_account.clone(),
                ledger,
            );

            task_manager
                .spawn_essential_handle()
                .spawn("dvf-vote-creator", None, async move {
                    vote_creator.run().await;
                });

            log::info!(
                "Registry: Vote Creator Service started for validator {:?}",
                validator_account
            );

            // Create Registry Justification Builder
            let justification_builder = Arc::new(
                cerulea_consensus::justification_builder::JustificationBuilder::new(
                    client.clone(),
                    dvf_gossip_pool.clone(),
                ),
            );

            // Start Registry Vote Aggregator Service
            let vote_aggregator = cerulea_consensus::VoteAggregatorService::new(
                client.clone(),
                dvf_gossip_pool.clone(),
                justification_builder,
                transaction_pool.clone(),
                std::time::Duration::from_secs(1), // Check every 1 second
            );

            task_manager
                .spawn_essential_handle()
                .spawn("dvf-vote-aggregator", None, async move {
                    vote_aggregator.run().await;
                });

            log::info!("Registry: Vote Aggregator Service started");

            // Start Registry Vote Pool Pruning Service
            let pruning_service = cerulea_consensus::VotePoolPruningService::new(
                client.clone(),
                dvf_gossip_pool.clone(),
                finality_notifier.clone(),
            );

            task_manager.spawn_essential_handle().spawn(
                "dvf-vote-pool-pruning",
                None,
                async move {
                    pruning_service.run().await;
                },
            );

            log::info!("Registry: Vote Pool Pruning Service started");
        } else {
            log::warn!(
                "Registry: No ed25519 keys found in keystore, Vote Creator Service not started"
            );
        }
    }

    // STEP 18: P2P network layer initialized
    LifecycleTracer::global().trace_step(
        18,
        "service.rs::new_full",
        "P2P network layer initialized",
        Some(TraceMetadata::new().with_custom(
            "local_peer_id".to_string(),
            network.local_peer_id().to_string(),
        )),
    );

    // STEP 19: Block synchronization service started
    LifecycleTracer::global().trace_step(
        19,
        "service.rs::new_full",
        "Block synchronization service started",
        None,
    );

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
            offchain_workers
                .run(client.clone(), task_manager.spawn_handle())
                .boxed(),
        );

        // STEP 20: Offchain workers spawned
        LifecycleTracer::global().trace_step(
            20,
            "service.rs::new_full",
            "Offchain workers spawned",
            Some(
                TraceMetadata::new()
                    .with_custom("http_requests_enabled".to_string(), "true".to_string()),
            ),
        );
    }

    // --- Cerulea Custom Consensus Integration ---
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

    // STEP 21: DCF consensus parameters set
    LifecycleTracer::global().trace_step(
        21,
        "service.rs::new_full",
        "DCF consensus parameters set",
        Some(
            TraceMetadata::new()
                .with_custom(
                    "author_selection_mode".to_string(),
                    format!("{:?}", consensus_params.author_selection_mode),
                )
                .with_custom(
                    "finality_threshold".to_string(),
                    consensus_params.finality_threshold.to_string(),
                )
                .with_custom(
                    "block_time".to_string(),
                    consensus_params.block_time.to_string(),
                )
                .with_custom(
                    "max_block_size".to_string(),
                    consensus_params.max_block_size.to_string(),
                )
                .with_custom(
                    "max_transactions_per_block".to_string(),
                    consensus_params.max_transactions_per_block.to_string(),
                ),
        ),
    );

    // We use the consensus_metrics already initialized in new_partial

    // Use the client directly for block import - Substrate handles this internally
    let dcf_block_import_arc = client.clone();

    // Set up  PoS+PoI block production with DCF consensus
    if config.role.is_authority() {
        let mut dcf_consensus = if let Some(metrics) = consensus_metrics.clone() {
            cerulea_consensus::DcfConsensus::<Block, FullClient, sp_core::ed25519::Pair, _>::new_with_metrics(
                client.clone(),
                transaction_pool.clone(),
                dcf_block_import_arc.clone(), // Use real import queue
                consensus_params.clone(),
                keystore_container.keystore(),
                metrics,
            )
        } else {
            cerulea_consensus::DcfConsensus::<Block, FullClient, sp_core::ed25519::Pair, _>::new(
                client.clone(),
                transaction_pool.clone(),
                dcf_block_import_arc.clone(), // Use real import queue
                consensus_params.clone(),
                keystore_container.keystore(),
            )
        };

        // STEP 22: DCF consensus engine initialized
        LifecycleTracer::global().trace_step(
            22,
            "service.rs::new_full",
            "DCF consensus engine initialized",
            Some(
                TraceMetadata::new()
                    .with_custom("is_authority".to_string(), "true".to_string())
                    .with_custom(
                        "metrics_enabled".to_string(),
                        consensus_metrics.is_some().to_string(),
                    ),
            ),
        );

        task_manager.spawn_essential_handle().spawn(
            "cerulea-pos-poi-consensus",
            None,
            async move {
                log::info!("Cerulea: Starting real PoS+PoI consensus engine for block production");
                // Run the consensus engine which will produce real blocks
                dcf_consensus.run().await;

                let err = std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Consensus engine unexpectedly stopped",
                );
                LifecycleTracer::global().trace_error(
                    23,
                    "service.rs::new_full",
                    &err,
                    "PoS+PoI consensus engine unexpectedly stopped",
                );
                log::error!("Cerulea: PoS+PoI consensus engine unexpectedly stopped");
            },
        );

        // STEP 23: Consensus engine task started
        LifecycleTracer::global().trace_step(
            23,
            "service.rs::new_full",
            "Consensus engine task started",
            Some(TraceMetadata::new().with_custom(
                "task_name".to_string(),
                "cerulea-pos-poi-consensus".to_string(),
            )),
        );
    }

    // Start metrics update task if metrics are available
    if let Some(metrics) = consensus_metrics.clone() {
        let metrics_client = client.clone();
        task_manager
            .spawn_handle()
            .spawn("consensus-metrics-updater", None, async move {
                log::info!("Cerulea: Starting consensus metrics updater");
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));

                loop {
                    interval.tick().await;

                    if let Err(e) = metrics.update_from_runtime(&metrics_client) {
                        log::warn!("Cerulea: Failed to update consensus metrics: {}", e);
                    }
                }
            });

        // STEP 24: Metrics updater task started
        LifecycleTracer::global().trace_step(
            24,
            "service.rs::new_full",
            "Metrics updater task started",
            Some(
                TraceMetadata::new()
                    .with_custom("update_interval_secs".to_string(), "5".to_string()),
            ),
        );
    }

    // Start block authoring and missed block tracking service
    {
        let tracker_client = client.clone();
        task_manager
            .spawn_handle()
            .spawn("block-authoring-tracker", None, async move {
                log::info!("Cerulea: Starting block authoring tracking service");

                let tracker_config = BlockTrackerConfig {
                    monitoring_interval: std::time::Duration::from_secs(10),
                    stats_reporting_interval: std::time::Duration::from_secs(120), // 2 minutes
                    underperformance_threshold: 80.0, // 80% participation threshold
                    max_consecutive_misses: 3,
                    enable_alerts: true,
                    enable_detailed_logging: true,
                };

                let mut tracker =
                    BlockTracker::<Block, FullClient>::new(tracker_client, tracker_config);
                tracker.run().await;
            });

        // STEP 25: Block authoring tracker started
        LifecycleTracer::global().trace_step(
            25,
            "service.rs::new_full",
            "Block authoring tracker started",
            Some(
                TraceMetadata::new()
                    .with_custom("monitoring_interval_secs".to_string(), "10".to_string())
                    .with_custom(
                        "stats_reporting_interval_secs".to_string(),
                        "120".to_string(),
                    ),
            ),
        );
    }

    // Start DCF finality sync service
    // This syncs DCF's internal finality state to Substrate's client finalized head
    {
        use pallet_cerulea_dvf::DvfApi as RuntimeDvfApi;
        use sp_runtime::traits::SaturatedConversion;
        let finality_client = client.clone();
        let finality_gossip_pool = dvf_gossip_pool.clone();
        task_manager.spawn_essential_handle().spawn(
            "dvf-finality-sync",
            None,
            async move {
                log::info!("Registry: Starting finality sync service");

                // Perform initial sync on startup to restore finalized head
                log::info!("Registry: Performing initial finality sync on startup");
                let api = finality_client.runtime_api();
                let best_hash = finality_client.info().best_hash;

                match api.get_dvf_finalized_block(best_hash) {
                    Ok(dvf_finalized) => {
                        let client_info = finality_client.info();
                        let client_finalized: u32 = client_info.finalized_number.saturated_into();

                        if dvf_finalized > client_finalized {
                            log::info!(
                                "Registry Startup Sync: Client at block #{}, Registry finalized up to #{}, restoring...",
                                client_finalized,
                                dvf_finalized
                            );

                            // Finalize all blocks from client_finalized+1 to dvf_finalized
                            for block_num in (client_finalized + 1)..=dvf_finalized {
                                match finality_client.hash(block_num) {
                                    Ok(Some(block_hash)) => {
                                        match finality_client.finalize_block(block_hash, None, true) {
                                            Ok(_) => {
                                                log::debug!("Registry Startup Sync: Restored finalized block #{}", block_num);
                                            }
                                            Err(e) => {
                                                log::error!(
                                                    "Registry Startup Sync: Failed to restore block #{}: {:?}",
                                                    block_num,
                                                    e
                                                );
                                            }
                                        }
                                    }
                                    Ok(None) => {
                                        log::warn!(
                                            "Registry Startup Sync: Block #{} hash not found, skipping",
                                            block_num
                                        );
                                    }
                                    Err(e) => {
                                        log::error!(
                                            "Registry Startup Sync: Failed to get hash for block #{}: {:?}",
                                            block_num,
                                            e
                                        );
                                    }
                                }
                            }

                            log::info!("Registry Startup Sync: Finalized head restored to block #{}", dvf_finalized);
                        } else {
                            log::info!("Registry Startup Sync: Client finalized head is up to date at block #{}", client_finalized);
                        }
                    }
                    Err(e) => {
                        log::error!("Registry Startup Sync: Failed to get Registry finalized block: {:?}", e);
                    }
                }

                loop {
                    // Wait 6 seconds between checks (one block time)
                    tokio::time::sleep(std::time::Duration::from_secs(6)).await;

					// Prune gossip pool
					let finalized_head = finality_client.info().finalized_number.saturated_into::<u32>();
					// The finalized head is a block number. Pruning by height
					// keeps the one-vote-per-round record, so a second vote in a
					// round stays refused.
					finality_gossip_pool.prune_by_finalized_block(finalized_head);

                    // STEP 65: Finality check iteration started
                    crate::lifecycle_tracer::LifecycleTracer::global().trace_step(
                        65,
                        "service.rs::dvf-finality-sync",
                        "Finality check iteration started",
                        Some(crate::lifecycle_tracer::TraceMetadata::new()),
                    );

                    // Get Registry's finalized block number
                    let api = finality_client.runtime_api();
                    let best_hash = finality_client.info().best_hash;

                    match api.get_dvf_finalized_block(best_hash) {
                        Ok(dvf_finalized) => {
                            // STEP 66: Registry finalized block retrieved
                            crate::lifecycle_tracer::LifecycleTracer::global().trace_step(
                                66,
                                "service.rs::dvf-finality-sync",
                                &format!("Registry finalized block retrieved (block {})", dvf_finalized),
                                Some(crate::lifecycle_tracer::TraceMetadata::new().with_block_number(dvf_finalized)),
                            );

                            let client_info = finality_client.info();
                            let client_finalized: u32 = client_info.finalized_number.saturated_into();

                            // STEP 67: Client finalized block retrieved
                            crate::lifecycle_tracer::LifecycleTracer::global().trace_step(
                                67,
                                "service.rs::dvf-finality-sync",
                                &format!("Client finalized block retrieved (block {})", client_finalized),
                                Some(crate::lifecycle_tracer::TraceMetadata::new().with_block_number(client_finalized)),
                            );

                            match dvf_finalized.cmp(&client_finalized) {
                                core::cmp::Ordering::Greater => {
                                let gap = dvf_finalized - client_finalized;

                                // STEP 68: Finality gap detected
                                crate::lifecycle_tracer::LifecycleTracer::global().trace_step(
                                    68,
                                    "service.rs::dvf-finality-sync",
                                    &format!("Finality gap detected (gap size {})", gap),
                                    Some(crate::lifecycle_tracer::TraceMetadata::new().with_custom("gap".to_string(), gap.to_string())),
                                );

                                log::info!(
                                    "Registry Finality Sync: Client at block #{}, Registry finalized up to #{}, syncing...",
                                    client_finalized,
                                    dvf_finalized
                                );

                                // Finalize all blocks from client_finalized+1 to dvf_finalized
                                for block_num in (client_finalized + 1)..=dvf_finalized {
                                    // STEP 69: Finalizing block N
                                    crate::lifecycle_tracer::LifecycleTracer::global().trace_step(
                                        69,
                                        "service.rs::dvf-finality-sync",
                                        &format!("Finalizing block {}", block_num),
                                        Some(crate::lifecycle_tracer::TraceMetadata::new().with_block_number(block_num)),
                                    );

                                    // Get block hash for this number
                                    match finality_client.hash(block_num) {
                                        Ok(Some(block_hash)) => {
                                            // Finalize this block
                                            match finality_client.finalize_block(block_hash, None, true) {
                                                Ok(_) => {
                                                    // STEP 70: Block N finalized successfully
                                                    crate::lifecycle_tracer::LifecycleTracer::global().trace_step(
                                                        70,
                                                        "service.rs::dvf-finality-sync",
                                                        &format!("Block {} finalized successfully", block_num),
                                                        Some(crate::lifecycle_tracer::TraceMetadata::new().with_block_number(block_num)),
                                                    );
                                                    log::debug!("Registry Finality Sync: Finalized block #{}", block_num);
                                                }
                                                Err(e) => {
                                                    log::error!(
                                                        "Registry Finality Sync: Failed to finalize block #{}: {:?}",
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

                                // STEP 71: Finality synchronization completed
                                crate::lifecycle_tracer::LifecycleTracer::global().trace_step(
                                    71,
                                    "service.rs::dvf-finality-sync",
                                    "Finality synchronization completed",
                                    Some(crate::lifecycle_tracer::TraceMetadata::new()),
                                );
                                }
                                core::cmp::Ordering::Equal => {
                                // Already in sync, just log occasionally
                                log::trace!("Registry and Client finality are in sync at #{}", dvf_finalized);
                                }
                                core::cmp::Ordering::Less => {
                                // This is normal: DCF (Deterministic Finality) is faster than Registry (Vote-based Finality)
                                log::info!(
                                    "Registry Sync Status: Client head (#{}) is ahead of Registry head (#{}). Normal behavior.",
                                    client_finalized,
                                    dvf_finalized
                                );
                                }
                            }
                        },
                        Err(e) => {
                            log::error!("DCF Finality Sync: Failed to get DCF finalized block: {:?}", e);
                        }
                    }
                }
            },
        );

        // STEP 26: DCF finality sync service started
        LifecycleTracer::global().trace_step(
            26,
            "service.rs::new_full",
            "DCF finality sync service started",
            Some(
                TraceMetadata::new()
                    .with_custom("check_interval_secs".to_string(), "6".to_string()),
            ),
        );
    }

    log::info!("DCF: Consensus monitoring active. Block production handled by the Dynamic Consensus Framework.");

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

    // STEP 27: RPC extensions set
    LifecycleTracer::global().trace_step(27, "service.rs::new_full", "RPC extensions set", None);

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

    // STEP 28: All service tasks spawned successfully
    LifecycleTracer::global().trace_step(
        28,
        "service.rs::new_full",
        "All service tasks spawned successfully",
        None,
    );

    // STEP 29: Full node service operational
    LifecycleTracer::global().trace_step(
        29,
        "service.rs::new_full",
        "Full node service operational",
        None,
    );

    // Flush the tracer to ensure all traces are written
    LifecycleTracer::global().flush();

    Ok(task_manager)
}

/// Move every draw key the keystore holds under the seal key type.
///
/// Earlier releases recorded the sr25519 draw key under the seal key type. A
/// file keystore lists every key of one type whatever its scheme, so the draw
/// key passed for a seal key. The call reads each key file of the seal type,
/// derives the sr25519 key from the recorded phrase and records the key again
/// under the draw key type where the two publics agree. The earlier file stays
/// in place, so no key leaves the keystore.
pub fn migrate_draw_keys(path: &std::path::Path, keystore: &sp_keystore::KeystorePtr) -> usize {
    use sp_core::crypto::Pair;
    let prefix = hex::encode(cerulea_consensus::CERULEA_KEY_TYPE.0);
    let held = keystore.sr25519_public_keys(cerulea_consensus::CERULEA_DRAW_KEY_TYPE);
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    let mut moved = 0;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(public_hex) = name.strip_prefix(&prefix) else {
            continue;
        };
        let Ok(public) = hex::decode(public_hex) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let Ok(phrase) = serde_json::from_str::<String>(&text) else {
            continue;
        };
        let Ok(draw) = sp_core::sr25519::Pair::from_string(&phrase, None) else {
            continue;
        };
        if draw.public().0[..] != public[..] || held.contains(&draw.public()) {
            continue;
        }
        if keystore
            .insert(cerulea_consensus::CERULEA_DRAW_KEY_TYPE, &phrase, &public)
            .is_ok()
        {
            moved += 1;
            log::info!("Cerulea: the draw key moved to its own key type");
        }
    }
    moved
}
