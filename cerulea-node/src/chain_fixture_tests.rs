//! One development Blockchain on the real runtime, shared by the node tests.
//!
//! The fixture parses the node command line arguments, builds the service
//! settings the node itself builds and opens the real client, backend and
//! transaction pool through `service::new_partial`. The proposer factory of
//! the Dynamic Consensus Framework then builds each block from the pool. The
//! fixture seals every block with the development validator keys and imports
//! it into the client.
//!
//! The chain carries real work. Block one installs a land registry Blueprint
//! through the logic module, registers the registrar, grants the role and
//! opens one governance proposal. Block two runs the Blueprint action, moves
//! a balance, casts a vote, challenges an inference result and records one
//! slashing. Every later block carries no author digest, so the framework
//! pallet records a missed slot against the scheduled author.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use cerulea_blueprint::{
    actions::{Action, Effect, Param},
    collections::{Collection, KeySelector, PurposeDeclaration},
    expression::Expr,
    identities::RoleDef,
    types::{
        CollectionId, FieldDef, FieldName, ParamName, PurposeTag, RecordDef, RoleId, TypeDef,
        TypeExpr, TypeId, Value,
    },
    views::{Projection, View},
    Blueprint, BlueprintMeta,
};
use cerulea_consensus::engines::authoring::draw::{derive_slot_seed, draw, DrawInput};
use cerulea_consensus::engines::authoring::fallback::{AuthoringPolicy, SlotSchedule};
use cerulea_consensus::engines::authoring::seal::{seal_header, BlockSeal, SealClaim};
use cerulea_consensus::engines::{Rank, Slot};
use cerulea_consensus::metrics::ConsensusMetrics;
use cerulea_consensus::{ProposerFactory, CERULEA_ENGINE_ID, CERULEA_KEY_TYPE};
use cerulea_runtime::opaque::Block;
use cerulea_runtime::{AccountId, RuntimeCall};
use clap::Parser;
use codec::Encode;
use frame_support::BoundedVec;
use pallet_cerulea_dvf::CeruleaAuthoringApi;
use pallet_cerulea_permission::{CapabilityKey, GrantPayload};
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};
use sc_consensus::{BlockImport, BlockImportParams, ForkChoiceStrategy, ImportResult};
use sc_service::Configuration as ServiceSettings;
use sc_transaction_pool_api::{
    ChainEvent, MaintainedTransactionPool, TransactionPool, TransactionSource,
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::{ed25519, sr25519, Pair, H256};
use sp_keyring::Ed25519Keyring;
use sp_keystore::{testing::MemoryKeystore, Keystore, KeystorePtr};
use sp_runtime::traits::{Block as BlockT, Header as HeaderT};
use sp_runtime::{DigestItem, OpaqueExtrinsic};

use crate::node_command::NodeCommand;
use crate::rpc::RpcSecurityConfig;
use crate::service::{FullClient, NodeConfig};

/// The backend the fixture client writes to.
pub(crate) type Backend = sc_service::TFullBackend<Block>;

/// The transaction pool the fixture client feeds.
pub(crate) type Pool = sc_transaction_pool::TransactionPoolHandle<Block, FullClient>;

/// The action the land registry Blueprint declares.
pub(crate) const REGISTRY_ACTION: &str = "record_transfer";

/// The collection the registry action writes.
pub(crate) const PARCEL: &str = "parcel";

/// The survey number of the parcel the fixture records.
pub(crate) const RECORDED_SURVEY: &str = "12/3";

/// The blocks the fixture builds after the two working blocks.
pub(crate) const MISSED_BLOCKS: u32 = 9;

/// One block the fixture imported.
#[derive(Clone, Debug)]
pub(crate) struct BuiltBlock {
    /// The height of the block.
    pub number: u32,
    /// The hash of the block the client holds.
    pub hash: H256,
    /// The header the client holds, seal included.
    pub header: <Block as BlockT>::Header,
    /// The seal the fixture placed on the header.
    pub seal: BlockSeal,
    /// The block carried an author digest naming the development validator.
    pub authored: bool,
}

/// The shared development Blockchain.
pub(crate) struct Fixture {
    /// The runtime every service task runs on.
    pub _runtime: tokio::runtime::Runtime,
    /// The folder holding the database and the keystore.
    pub folder: tempfile::TempDir,
    /// The client over the real runtime.
    pub client: Arc<FullClient>,
    /// The backend under the client.
    pub backend: Arc<Backend>,
    /// The transaction pool of the client.
    pub pool: Arc<Pool>,
    /// The consensus metrics `new_partial` registered.
    pub metrics: Option<ConsensusMetrics>,
    /// The land registry deployment.
    pub deployment: DeploymentId,
    /// Every block the fixture imported, in height order.
    pub blocks: Vec<BuiltBlock>,
    /// The hash of the balance transfer inside block two.
    pub transfer_hash: String,
    /// The hash of the Blueprint action call inside block two.
    pub action_hash: String,
    /// The keystore holding the development validator seal key.
    pub keystore: KeystorePtr,
    /// The spawner of the service task manager.
    pub spawner: sc_service::SpawnTaskHandle,
}

/// The development validator, Alice.
pub(crate) fn alice() -> AccountId {
    Ed25519Keyring::Alice.to_account_id()
}

/// The second endowed account, Bob. Bob holds no validator seat.
pub(crate) fn bob() -> AccountId {
    Ed25519Keyring::Bob.to_account_id()
}

/// The ed25519 seal key of the development validator.
pub(crate) fn seal_key() -> ed25519::Public {
    Ed25519Keyring::Alice.public()
}

/// The sr25519 draw key pair of the development validator.
pub(crate) fn draw_pair() -> sr25519::Pair {
    sr25519::Pair::from_string("//Alice", None).expect("the well known seed yields a draw key")
}

/// Parse node arguments and build the service settings the node builds.
pub(crate) fn node_settings(
    arguments: &[&str],
    handle: tokio::runtime::Handle,
) -> (NodeCommand, ServiceSettings) {
    let mut full = vec!["cerulea-node"];
    full.extend_from_slice(arguments);
    let command = NodeCommand::try_parse_from(full).expect("the node arguments parse");
    let settings = sc_cli::CliConfiguration::create_configuration(&command.run, &command, handle)
        .expect("the node arguments yield service settings");
    (command, settings)
}

/// The node settings with every Cerulea extension open.
pub(crate) fn open_extensions() -> NodeConfig {
    NodeConfig {
        rpc_config: RpcSecurityConfig {
            enable_cerulea_extensions: true,
            ..RpcSecurityConfig::default()
        },
    }
}

fn name(text: &str) -> BoundedName {
    name_of(text).expect("the name fits the bound")
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

/// The land registry Blueprint the fixture installs.
pub(crate) fn registry_blueprint() -> Blueprint {
    let mut blueprint = Blueprint::new(BlueprintMeta::new(
        "node test land registry",
        "The registry records land parcels and their holders.",
        "Caerulean Bytechains test desk",
    ));
    blueprint.types.push(TypeDef::Record(RecordDef {
        id: TypeId::new("parcel"),
        fields: vec![
            FieldDef::new(
                "survey_number",
                TypeExpr::Text,
                "The survey number of the parcel.",
            ),
            FieldDef::new("holder", TypeExpr::Text, "The name of the present holder."),
            FieldDef::new("area", TypeExpr::Integer, "The area in square metres."),
        ],
        summary: "One land parcel in the register.".into(),
    }));
    let mut collection = Collection::new(
        PARCEL,
        "parcel",
        "survey_number",
        "The register of land parcels.",
    );
    collection.purpose = PurposeDeclaration::new(
        "The register records who holds each land parcel.",
        &["registration"],
    );
    blueprint.collections.push(collection);
    blueprint.identities.roles.push(RoleDef::new(
        "registrar",
        None,
        "Registrar",
        "The registrar records land transfers.",
    ));
    let mut action = Action::new(
        REGISTRY_ACTION,
        "The registrar records a transfer of one parcel.",
    );
    action.params = vec![Param::new(
        "parcel",
        TypeExpr::Record(TypeId::new("parcel")),
        "The parcel the registrar records.",
    )];
    action.effects = vec![Effect::Insert {
        collection: CollectionId::new(PARCEL),
        key: KeySelector::Exact(Expr::field(Expr::bound("parcel"), "survey_number")),
        record: Expr::bound("parcel"),
    }];
    action.roles.push(RoleId::new("registrar"));
    blueprint.actions.push(action);
    let mut view = View::new(
        "holders",
        PARCEL,
        "The view lists the holder of every parcel.",
    );
    view.purposes = vec![PurposeTag::new("registration")];
    view.projection = vec![
        Projection::computed(
            "survey_number",
            Expr::field(Expr::bound("record"), "survey_number"),
        ),
        Projection::computed("holder", Expr::field(Expr::bound("record"), "holder")),
    ];
    blueprint.views.push(view);
    blueprint
}

/// One parcel value in canonical form.
fn parcel(survey: &str, holder: &str, area: i128) -> Value {
    let mut value = Value::record(
        TypeId::new("parcel"),
        vec![
            (FieldName::new("survey_number"), Value::Text(survey.into())),
            (FieldName::new("holder"), Value::Text(holder.into())),
            (FieldName::new("area"), Value::Integer(area)),
        ],
    );
    value.canonicalise();
    value
}

/// The calls of block one.
///
/// The registrar installs the Blueprint, takes the role and every
/// capability, and opens one proposal.
fn setup_calls(blueprint: &Blueprint, deployment: DeploymentId, now_ms: u64) -> Vec<RuntimeCall> {
    let mut calls = vec![
        RuntimeCall::Logic(pallet_cerulea_logic::Call::install_blueprint {
            blueprint: bounded(blueprint.encode()),
        }),
        RuntimeCall::Identity(pallet_cerulea_identity::Call::register_account {
            who: alice(),
            identity: [7u8; 32],
        }),
        RuntimeCall::Identity(pallet_cerulea_identity::Call::define_role {
            deployment,
            role: name("registrar"),
            unit: None,
            requires: BoundedVec::default(),
            title: BoundedVec::truncate_from(b"Registrar".to_vec()),
        }),
        RuntimeCall::Identity(pallet_cerulea_identity::Call::assign_role {
            deployment,
            who: alice(),
            role: name("registrar"),
            expires_at: None,
        }),
    ];
    for capability in [
        CapabilityKey::Read,
        CapabilityKey::Insert,
        CapabilityKey::Update,
        CapabilityKey::Delete,
        CapabilityKey::Invoke(name(REGISTRY_ACTION)),
    ] {
        calls.push(RuntimeCall::Permission(
            pallet_cerulea_permission::Call::grant {
                deployment,
                role: name("registrar"),
                collection: name(PARCEL),
                capability,
                payload: bounded(GrantPayload::everything().encode()),
            },
        ));
    }
    calls.push(RuntimeCall::Governance(
        pallet_cerulea_governance::Call::set_voter_weight {
            deployment,
            voter: alice(),
            weight: 10,
        },
    ));
    calls.push(RuntimeCall::Governance(
        pallet_cerulea_governance::Call::propose {
            deployment,
            kind: pallet_cerulea_governance::ProposalKind::PolicyAmendment,
            payload: bounded(b"raise the transfer fee".to_vec()),
            deadline: now_ms + 86_400_000,
        },
    ));
    calls
}

/// The calls Alice signs in block two.
fn working_calls(deployment: DeploymentId) -> Vec<RuntimeCall> {
    let arguments: pallet_cerulea_logic::Bindings = vec![(
        ParamName::new("parcel"),
        parcel(RECORDED_SURVEY, "Anitha", 400),
    )];
    vec![
        RuntimeCall::Logic(pallet_cerulea_logic::Call::execute_action {
            deployment,
            action: name(REGISTRY_ACTION),
            arguments: bounded(arguments.encode()),
        }),
        RuntimeCall::Balances(cerulea_runtime::BalancesCall::transfer_keep_alive {
            dest: bob().into(),
            value: 5 * cerulea_runtime::EXISTENTIAL_DEPOSIT,
        }),
        RuntimeCall::Governance(pallet_cerulea_governance::Call::vote {
            deployment,
            proposal: 0,
            approve: true,
        }),
        RuntimeCall::System(cerulea_runtime::SystemCall::remark {
            remark: b"cerulea node test".to_vec(),
        }),
        // An offence count takes the root origin, which Alice holds through
        // the sudo key of the development chain.
        RuntimeCall::Sudo(pallet_sudo::Call::sudo {
            call: Box::new(RuntimeCall::Pos(
                cerulea_runtime::pallet_cerulea_pos::Call::slash_validator_call {
                    validator: alice(),
                },
            )),
        }),
    ]
}

/// The calls Bob signs in block two.
fn bob_calls() -> Vec<RuntimeCall> {
    vec![RuntimeCall::Poi(
        cerulea_runtime::pallet_cerulea_poi::Call::challenge_inference {
            challenged: alice(),
            result: 80,
        },
    )]
}

/// The moment of the wall clock, in milliseconds.
pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock runs after the epoch")
        .as_millis() as u64
}

/// Seal one header the way the Authoring Engine seals it.
///
/// The claim names a slot in the past, so the claimed moment never runs ahead
/// of the clock. The chain publishes the committee of that slot, and the
/// development validator holds a seat in it.
pub(crate) fn seal_for_slot(
    client: &FullClient,
    keystore: &KeystorePtr,
    header: &<Block as BlockT>::Header,
    slot: u64,
) -> BlockSeal {
    let parent = *header.parent_hash();
    let parent_bytes: [u8; 32] = parent.into();
    let seats = client
        .runtime_api()
        .slot_committee(parent, slot, parent_bytes)
        .expect("the chain publishes the slot committee");
    let draw_pair = draw_pair();
    let seat = seats
        .iter()
        .find(|seat| seat.keys.draw == draw_pair.public().0)
        .expect("the development validator holds a seat");
    let rank = Rank(seat.rank);
    let input = DrawInput::new(Slot(slot), derive_slot_seed(&parent_bytes, Slot(slot)));
    let window = SlotSchedule::for_slot(Slot(slot), AuthoringPolicy::standard())
        .window(rank)
        .expect("the rank holds a window");
    let claim = SealClaim::new(
        Slot(slot),
        rank,
        seal_key(),
        draw(&draw_pair, &input),
        window.opens_ms + 5,
    );
    seal_header(keystore, header, claim).expect("the keystore holds the seal key")
}

/// A keystore holding the development validator seal key.
pub(crate) fn validator_keystore() -> KeystorePtr {
    let keystore = MemoryKeystore::new();
    keystore
        .ed25519_generate_new(CERULEA_KEY_TYPE, Some("//Alice"))
        .expect("the keystore takes the seal key");
    Arc::new(keystore)
}

/// Sign one call with a development account at a nonce.
pub(crate) fn signed(
    client: &FullClient,
    signer: Ed25519Keyring,
    call: RuntimeCall,
    nonce: u32,
) -> OpaqueExtrinsic {
    crate::benchmarking::create_benchmark_extrinsic(client, signer.pair(), call, nonce).into()
}

/// The hash the node reports for one encoded transaction.
pub(crate) fn transaction_hash(extrinsic: &OpaqueExtrinsic) -> String {
    let hash =
        <sp_runtime::traits::BlakeTwo256 as sp_runtime::traits::Hash>::hash(&extrinsic.encode());
    format!("0x{}", hex::encode(hash.as_bytes()))
}

/// Seal one built block and import it into the client.
pub(crate) async fn seal_and_import(
    client: &Arc<FullClient>,
    keystore: &KeystorePtr,
    block: Block,
    slot: u64,
) -> (H256, BlockSeal) {
    let (header, body) = block.deconstruct();
    let seal = seal_for_slot(client, keystore, &header, slot);
    let number = *header.number();
    let mut params = BlockImportParams::new(sp_consensus::BlockOrigin::Own, header.clone());
    params.post_digests.push(seal.digest_item());
    params.body = Some(body);
    params.fork_choice = Some(ForkChoiceStrategy::LongestChain);
    let imported = client
        .import_block(params)
        .await
        .expect("the client imports the sealed block");
    assert!(
        matches!(imported, ImportResult::Imported(_)),
        "The client reported {imported:?} for block {number}."
    );
    let mut sealed = header;
    sealed.digest_mut().push(seal.digest_item());
    (sealed.hash(), seal)
}

/// Build, seal and import one block on top of the best block.
async fn produce(
    client: &Arc<FullClient>,
    pool: &Arc<Pool>,
    factory: &mut ProposerFactory<Block, FullClient, Pool>,
    keystore: &KeystorePtr,
    slot: u64,
    authored: bool,
    transactions: Vec<OpaqueExtrinsic>,
) -> BuiltBlock {
    let parent = client.info().best_hash;
    for transaction in transactions {
        pool.submit_one(parent, TransactionSource::External, transaction)
            .await
            .expect("the pool admits the transaction");
    }
    let digest = authored.then(|| DigestItem::PreRuntime(CERULEA_ENGINE_ID, seal_key().encode()));
    let (block, _) = factory
        .create_complete_block(parent, slot, digest, Some(seal_key()))
        .await
        .expect("the proposer builds the block");
    let number = *block.header().number();
    let (hash, seal) = seal_and_import(client, keystore, block, slot).await;
    assert_eq!(client.info().best_number, number);
    assert_eq!(client.info().best_hash, hash);
    pool.maintain(ChainEvent::NewBestBlock {
        hash,
        tree_route: None,
    })
    .await;
    let header = client
        .header(hash)
        .expect("the header index answers")
        .expect("the client holds the header");
    BuiltBlock {
        number,
        hash,
        header,
        seal,
        authored,
    }
}

/// Build the shared development Blockchain.
fn build() -> Fixture {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("the test runtime starts");
    let folder = tempfile::tempdir().expect("a temporary folder opens");
    let base = folder
        .path()
        .to_str()
        .expect("the folder path is text")
        .to_string();
    let (_, settings) = node_settings(
        &[
            "--dev",
            "--base-path",
            &base,
            "--no-telemetry",
            "--prometheus-port",
            "0",
        ],
        runtime.handle().clone(),
    );
    let entered = runtime.enter();
    let parts = crate::service::new_partial(&settings, &open_extensions())
        .expect("the partial service builds over the development chain");
    drop(entered);
    let client = parts.client.clone();
    let pool = parts.transaction_pool.clone();
    let backend = parts.backend.clone();
    let metrics = parts.other.clone();
    let spawner = parts.task_manager.spawn_handle();
    // The fixture keeps every service task alive for the life of the process.
    std::mem::forget(parts);
    std::mem::forget(settings);

    let keystore = validator_keystore();
    let blueprint = registry_blueprint();
    let deployment = blueprint.id().0;
    let first_slot = SlotSchedule::slot_at(now_ms(), AuthoringPolicy::standard()).0 - 1_000;

    let (blocks, transfer_hash, action_hash) = runtime.block_on(async {
        let mut factory = ProposerFactory::<Block, FullClient, Pool>::new(
            client.clone(),
            pool.clone(),
            Duration::ZERO,
            256,
        );
        let mut blocks = Vec::new();
        let mut alice_nonce = 0u32;

        let mut first = Vec::new();
        for call in setup_calls(&blueprint, deployment, now_ms()) {
            first.push(signed(&client, Ed25519Keyring::Alice, call, alice_nonce));
            alice_nonce += 1;
        }
        blocks.push(
            produce(
                &client,
                &pool,
                &mut factory,
                &keystore,
                first_slot,
                true,
                first,
            )
            .await,
        );

        tokio::time::sleep(Duration::from_millis(3_100)).await;
        let mut second = Vec::new();
        let mut hashes = Vec::new();
        for call in working_calls(deployment) {
            let extrinsic = signed(&client, Ed25519Keyring::Alice, call, alice_nonce);
            hashes.push(transaction_hash(&extrinsic));
            second.push(extrinsic);
            alice_nonce += 1;
        }
        for (nonce, call) in bob_calls().into_iter().enumerate() {
            second.push(signed(&client, Ed25519Keyring::Bob, call, nonce as u32));
        }
        blocks.push(
            produce(
                &client,
                &pool,
                &mut factory,
                &keystore,
                first_slot + 1,
                true,
                second,
            )
            .await,
        );

        for index in 0..MISSED_BLOCKS {
            tokio::time::sleep(Duration::from_millis(3_100)).await;
            blocks.push(
                produce(
                    &client,
                    &pool,
                    &mut factory,
                    &keystore,
                    first_slot + 2 + u64::from(index),
                    false,
                    Vec::new(),
                )
                .await,
            );
        }
        (blocks, hashes[1].clone(), hashes[0].clone())
    });

    // The finality engine marks block two final, as the node does once the
    // signers reach the threshold.
    sc_client_api::Finalizer::finalize_block(&*client, blocks[1].hash, None, true)
        .expect("the client finalises block two");

    Fixture {
        _runtime: runtime,
        folder,
        client,
        backend,
        pool,
        metrics,
        deployment,
        blocks,
        transfer_hash,
        action_hash,
        keystore,
        spawner,
    }
}

/// A reader of the fixture chain that sees one block as the best block.
///
/// The Cerulea interfaces read every answer at the best block. The reader
/// lets a test read the chain as it stood at an earlier block. A block hash
/// the chain never held makes every runtime read fail, which drives the
/// failure paths through the real runtime.
pub(crate) struct ViewAt {
    inner: Arc<FullClient>,
    position: parking_lot::Mutex<(H256, u32)>,
}

impl ViewAt {
    /// Read the fixture chain as it stood at one of its blocks.
    pub(crate) fn block(number: u32) -> Arc<Self> {
        let chain = fixture();
        let view = Arc::new(ViewAt {
            inner: chain.client.clone(),
            position: parking_lot::Mutex::new((H256::zero(), 0)),
        });
        view.move_to(number);
        view
    }

    /// Read the fixture chain at a block hash the chain never held.
    pub(crate) fn unknown() -> Arc<Self> {
        let view = Self::block(0);
        view.move_to_unknown();
        view
    }

    /// Stand the reader at one block of the fixture chain.
    pub(crate) fn move_to(&self, number: u32) {
        let hash = self
            .inner
            .hash(number)
            .expect("the block index answers")
            .expect("the chain holds the block");
        *self.position.lock() = (hash, number);
    }

    /// Stand the reader at the head height under a hash the chain never held.
    pub(crate) fn move_to_unknown(&self) {
        *self.position.lock() = (H256::repeat_byte(0xab), self.inner.info().best_number);
    }
}

impl ProvideRuntimeApi<Block> for ViewAt {
    type Api = <FullClient as ProvideRuntimeApi<Block>>::Api;

    fn runtime_api(&self) -> sp_api::ApiRef<'_, Self::Api> {
        self.inner.runtime_api()
    }
}

impl HeaderBackend<Block> for ViewAt {
    fn header(&self, hash: H256) -> sp_blockchain::Result<Option<<Block as BlockT>::Header>> {
        self.inner.header(hash)
    }

    fn info(&self) -> sp_blockchain::Info<Block> {
        let mut info = self.inner.info();
        let (hash, number) = *self.position.lock();
        info.best_hash = hash;
        info.best_number = number;
        info
    }

    fn status(&self, hash: H256) -> sp_blockchain::Result<sp_blockchain::BlockStatus> {
        self.inner.status(hash)
    }

    fn number(&self, hash: H256) -> sp_blockchain::Result<Option<u32>> {
        self.inner.number(hash)
    }

    fn hash(&self, number: u32) -> sp_blockchain::Result<Option<H256>> {
        self.inner.hash(number)
    }
}

impl sc_client_api::BlockBackend<Block> for ViewAt {
    fn block_body(
        &self,
        hash: H256,
    ) -> sp_blockchain::Result<Option<Vec<<Block as BlockT>::Extrinsic>>> {
        self.inner.block_body(hash)
    }

    fn block_indexed_body(&self, hash: H256) -> sp_blockchain::Result<Option<Vec<Vec<u8>>>> {
        self.inner.block_indexed_body(hash)
    }

    fn block(
        &self,
        hash: H256,
    ) -> sp_blockchain::Result<Option<sp_runtime::generic::SignedBlock<Block>>> {
        self.inner.block(hash)
    }

    fn block_status(&self, hash: H256) -> sp_blockchain::Result<sp_consensus::BlockStatus> {
        self.inner.block_status(hash)
    }

    fn justifications(
        &self,
        hash: H256,
    ) -> sp_blockchain::Result<Option<sp_runtime::Justifications>> {
        self.inner.justifications(hash)
    }

    fn block_hash(&self, number: u32) -> sp_blockchain::Result<Option<H256>> {
        self.inner.block_hash(number)
    }

    fn indexed_transaction(&self, hash: H256) -> sp_blockchain::Result<Option<Vec<u8>>> {
        self.inner.indexed_transaction(hash)
    }

    fn requires_full_sync(&self) -> bool {
        self.inner.requires_full_sync()
    }
}

/// The storage fault a [`FaultyStore`] carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoreFault {
    /// The block index refuses every height.
    HashIndex,
    /// The header store refuses every read.
    HeaderRead,
    /// The header store holds no header.
    HeaderMissing,
    /// The body store refuses every read.
    BodyRead,
}

/// A reader over the fixture chain whose block store carries one fault.
///
/// The runtime answers as the fixture chain answers. The block store fails
/// the way a damaged or pruned database fails.
pub(crate) struct FaultyStore {
    inner: Arc<FullClient>,
    fault: StoreFault,
}

impl FaultyStore {
    /// A reader carrying one fault.
    pub(crate) fn with(fault: StoreFault) -> Arc<Self> {
        Arc::new(FaultyStore {
            inner: fixture().client.clone(),
            fault,
        })
    }

    fn refused(&self) -> sp_blockchain::Error {
        sp_blockchain::Error::Backend(format!("the store carries the fault {:?}", self.fault))
    }
}

impl ProvideRuntimeApi<Block> for FaultyStore {
    type Api = <FullClient as ProvideRuntimeApi<Block>>::Api;

    fn runtime_api(&self) -> sp_api::ApiRef<'_, Self::Api> {
        self.inner.runtime_api()
    }
}

impl HeaderBackend<Block> for FaultyStore {
    fn header(&self, hash: H256) -> sp_blockchain::Result<Option<<Block as BlockT>::Header>> {
        match self.fault {
            StoreFault::HeaderRead => Err(self.refused()),
            StoreFault::HeaderMissing => Ok(None),
            _ => self.inner.header(hash),
        }
    }

    fn info(&self) -> sp_blockchain::Info<Block> {
        self.inner.info()
    }

    fn status(&self, hash: H256) -> sp_blockchain::Result<sp_blockchain::BlockStatus> {
        self.inner.status(hash)
    }

    fn number(&self, hash: H256) -> sp_blockchain::Result<Option<u32>> {
        self.inner.number(hash)
    }

    fn hash(&self, number: u32) -> sp_blockchain::Result<Option<H256>> {
        match self.fault {
            StoreFault::HashIndex => Err(self.refused()),
            _ => self.inner.hash(number),
        }
    }
}

impl sc_client_api::BlockBackend<Block> for FaultyStore {
    fn block_body(
        &self,
        hash: H256,
    ) -> sp_blockchain::Result<Option<Vec<<Block as BlockT>::Extrinsic>>> {
        match self.fault {
            StoreFault::BodyRead => Err(self.refused()),
            _ => self.inner.block_body(hash),
        }
    }

    fn block_indexed_body(&self, hash: H256) -> sp_blockchain::Result<Option<Vec<Vec<u8>>>> {
        self.inner.block_indexed_body(hash)
    }

    fn block(
        &self,
        hash: H256,
    ) -> sp_blockchain::Result<Option<sp_runtime::generic::SignedBlock<Block>>> {
        self.inner.block(hash)
    }

    fn block_status(&self, hash: H256) -> sp_blockchain::Result<sp_consensus::BlockStatus> {
        self.inner.block_status(hash)
    }

    fn justifications(
        &self,
        hash: H256,
    ) -> sp_blockchain::Result<Option<sp_runtime::Justifications>> {
        self.inner.justifications(hash)
    }

    fn block_hash(&self, number: u32) -> sp_blockchain::Result<Option<H256>> {
        self.inner.block_hash(number)
    }

    fn indexed_transaction(&self, hash: H256) -> sp_blockchain::Result<Option<Vec<u8>>> {
        self.inner.indexed_transaction(hash)
    }

    fn requires_full_sync(&self) -> bool {
        self.inner.requires_full_sync()
    }
}

/// Serve the fixture chain over HTTP the way a node serves it.
///
/// The server answers the standard chain methods and every Cerulea method,
/// so a fork check and an author query read a live endpoint.
pub(crate) async fn serve_fixture() -> (String, jsonrpsee::server::ServerHandle) {
    use sc_rpc_api::chain::ChainApiServer;

    let chain = fixture();
    let mut module = crate::rpc::create_full(crate::rpc::FullDeps {
        client: chain.client.clone(),
        pool: chain.pool.clone(),
        rpc_config: open_extensions().rpc_config,
        consensus_metrics: chain.metrics.clone(),
        backend: chain.backend.clone(),
    })
    .expect("the interface builds over the fixture chain");
    let executor: sc_rpc::SubscriptionTaskExecutor = Arc::new(chain.spawner.clone());
    module
        .merge(sc_rpc::chain::new_full::<Block, _>(chain.client.clone(), executor).into_rpc())
        .expect("the chain methods join the interface");
    let server = jsonrpsee::server::Server::builder()
        .build("127.0.0.1:0")
        .await
        .expect("the server binds a free port");
    let address = server.local_addr().expect("the server reports its port");
    let handle = server.start(module);
    (format!("http://{address}"), handle)
}

/// The shared development Blockchain, built once per test process.
pub(crate) fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    // The build drives its own runtime, so it runs on a thread of its own.
    // A test running inside an asynchronous runtime then shares it safely.
    FIXTURE.get_or_init(|| {
        std::thread::spawn(build)
            .join()
            .expect("the fixture chain builds")
    })
}

#[test]
fn the_fixture_chain_reaches_every_planned_height() {
    let chain = fixture();
    let expected = 2 + MISSED_BLOCKS;
    assert_eq!(chain.client.info().best_number, expected);
    assert_eq!(chain.blocks.len() as u32, expected);
    for (index, block) in chain.blocks.iter().enumerate() {
        assert_eq!(block.number, index as u32 + 1);
        assert_eq!(chain.client.hash(block.number).unwrap(), Some(block.hash));
        assert_eq!(block.authored, index < 2);
    }
    assert!(chain.folder.path().exists());
}

#[test]
fn every_fixture_block_carries_the_seal_of_the_development_validator() {
    let chain = fixture();
    for block in chain.blocks.iter() {
        let seal = cerulea_consensus::engines::authoring::seal::extract_seal(&block.header)
            .expect("the header carries a seal");
        assert_eq!(seal, block.seal);
        assert_eq!(seal.claim.author, seal_key());
        assert_eq!(seal.claim.draw.validator, draw_pair().public());
    }
}
