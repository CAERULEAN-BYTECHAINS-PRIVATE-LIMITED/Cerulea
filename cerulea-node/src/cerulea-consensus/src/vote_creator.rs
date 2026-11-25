//! Vote Creator Service
//!
//! The service watches imported blocks and signs Commit Engine votes when the
//! node holds the seal key of an active validator. Every vote passes the
//! Commit Engine lock first. See [`crate::engines::commit::lock`] for the two
//! rules and the argument that no two conflicting blocks both commit.
//!
//! The service writes each vote to the ledger on disk before it signs. A
//! restart reads the ledger back, so the node never signs a second, different
//! vote in a round it already voted in. When the chain opens a new round with
//! a checkpoint still uncommitted, the service signs a round timeout statement
//! naming the block it is locked on, and then votes in the new round under the
//! lock.

use codec::{Decode, Encode};
use futures::StreamExt;
use log::{debug, info, warn};
use sc_client_api::{BlockchainEvents, HeaderBackend};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderMetadata;
use sp_core::ed25519;
use sp_keystore::{Keystore, KeystorePtr};
use sp_runtime::traits::{Block as BlockT, Header, NumberFor, SaturatedConversion};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use crate::dvf_gossip::{
    hash_bytes, CommitMessage, DvfVoteMessage, DvfVotePool, RoundTimeoutMessage, VoteOffer,
};
use crate::engines::commit::lock::{
    blocking_locks, certificate_release, certificate_undecided, choose_vote, CastVote, LedgerStore,
    TimeoutStatement, VoteChoice, VoteLedger, STATEMENT_WINDOW_ROUNDS,
};
use crate::metrics::DvfMetrics;
use pallet_cerulea_dcf::DcfApi as RuntimeDcfApi;
use pallet_cerulea_dvf::CeruleaAuthoringApi;
use pallet_cerulea_dvf::DvfApi as RuntimeDvfApi;

/// Pick the seal key the registry names for this node.
///
/// A file keystore lists every key of one key type whatever its scheme, so the
/// list of local ed25519 keys may carry a draw key beside the seal key. The
/// registry names the seal key of every active validator. The node signs with
/// the first local key the registry names as a seal key, together with the
/// account that owns it.
pub fn registry_seal_key<A: Clone>(
    local_keys: &[ed25519::Public],
    registry: &[(A, pallet_cerulea_dvf::keys::ValidatorKeySet)],
) -> Option<(A, ed25519::Public)> {
    registry.iter().find_map(|(account, set)| {
        let seal = ed25519::Public::from_raw(set.seal);
        local_keys.contains(&seal).then(|| (account.clone(), seal))
    })
}

/// The longest a locked node waits in a new round for the statements of the
/// round of its lock before it votes for its lock again.
///
/// The statements normally arrive within a block. The wait ends earlier once
/// more than two thirds of the weight has stated.
pub const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

/// The chain facts one voting pass reads.
struct ChainView<Hash> {
    round: u32,
    interval: u32,
    finalized: u32,
    best_number: u32,
    epoch_id: u32,
    validator_set_id: u32,
    best_hash: Hash,
}

/// Vote Creator Service
///
/// Monitors imported blocks and creates votes for checkpoint blocks when the node
/// is an active validator.
pub struct VoteCreatorService<Block, Client, AccountId>
where
    Block: BlockT,
{
    client: Arc<Client>,
    keystore: KeystorePtr,
    gossip_engine: Arc<Mutex<sc_network_gossip::GossipEngine<Block>>>,
    vote_pool: Arc<DvfVotePool<Block::Hash, AccountId>>,
    /// The vote ledger on disk.
    ledger: LedgerStore,
    /// The round timeout statements this node already broadcast.
    announced: Mutex<BTreeSet<(u32, u32)>>,
    /// The finalized height and the round the node saw when that height
    /// committed. Statements start from that round.
    committed_at: Mutex<Option<(u32, u32)>>,
    /// The round this node last saw open, and when.
    round_opened: Mutex<Option<(u32, std::time::Instant)>>,
    metrics: Option<Arc<DvfMetrics>>,
}

impl<Block, Client, AccountId> VoteCreatorService<Block, Client, AccountId>
where
    Block: BlockT,
    Client: ProvideRuntimeApi<Block>
        + HeaderBackend<Block>
        + BlockchainEvents<Block>
        + HeaderMetadata<Block, Error = sp_blockchain::Error>
        + Send
        + Sync
        + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + RuntimeDvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>
        + CeruleaAuthoringApi<Block, AccountId>,
    AccountId: Clone
        + Encode
        + codec::Codec
        + PartialEq
        + Eq
        + std::hash::Hash
        + std::fmt::Debug
        + Send
        + Sync
        + 'static,
{
    /// Creates a new Vote Creator Service.
    ///
    /// The ledger store names the file the node writes every vote to before
    /// it signs.
    pub fn new(
        client: Arc<Client>,
        keystore: KeystorePtr,
        gossip_engine: Arc<Mutex<sc_network_gossip::GossipEngine<Block>>>,
        vote_pool: Arc<DvfVotePool<Block::Hash, AccountId>>,
        validator_account: AccountId,
        ledger: LedgerStore,
    ) -> Self {
        info!(
            "Commit Engine Vote Creator: Initializing service for validator {:?}, vote ledger at {}",
            validator_account,
            ledger.path().display()
        );
        Self {
            client,
            keystore,
            gossip_engine,
            vote_pool,
            ledger,
            announced: Mutex::new(BTreeSet::new()),
            committed_at: Mutex::new(None),
            round_opened: Mutex::new(None),
            metrics: None,
        }
    }

    /// Sets the metrics for this service
    pub fn with_metrics(mut self, metrics: Arc<DvfMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Starts the service as a background task
    pub async fn run(self) {
        info!("Commit Engine Vote Creator: Starting service");

        let mut import_notifications = self.client.import_notification_stream();

        while let Some(notification) = import_notifications.next().await {
            let block_number = *notification.header.number();
            debug!(
                "Commit Engine Vote Creator: Processing imported block #{} ({:?})",
                block_number, notification.hash
            );
            if let Err(e) = self.process_block(block_number).await {
                warn!(
                    "Commit Engine Vote Creator: Failed to process block #{}: {:?}",
                    block_number, e
                );
            }
        }

        warn!("Commit Engine Vote Creator: Import notification stream ended");
    }

    /// Read the chain facts one voting pass needs.
    fn chain_view(&self) -> Result<ChainView<Block::Hash>, String> {
        let api = self.client.runtime_api();
        let info = self.client.info();
        let best_hash = info.best_hash;
        let round = api
            .get_current_round(best_hash)
            .map_err(|e| format!("Failed to get current round: {:?}", e))?;
        let interval: u32 = api
            .get_finality_checkpoint_interval(best_hash)
            .map_err(|e| format!("Failed to get FinalityCheckpointInterval: {:?}", e))?
            .saturated_into::<u32>();
        let epoch_id = <Client::Api as RuntimeDvfApi<
            Block,
            NumberFor<Block>,
            AccountId,
            Block::Hash,
        >>::get_current_epoch(&api, best_hash)
        .map_err(|e| format!("Failed to get current epoch: {:?}", e))?;
        let validator_set_id = api
            .get_validator_set_id(best_hash)
            .map_err(|e| format!("Failed to get validator set ID: {:?}", e))?;
        Ok(ChainView {
            round,
            interval,
            finalized: info.finalized_number.saturated_into::<u32>(),
            best_number: info.best_number.saturated_into::<u32>(),
            epoch_id,
            validator_set_id,
            best_hash,
        })
    }

    /// One voting pass.
    ///
    /// The pass announces the round timeouts the node owes, then asks the lock
    /// what to sign in the current round and signs it.
    async fn process_block(&self, block_number: NumberFor<Block>) -> Result<(), String> {
        let Some((account, seal)) = self.local_seal_key()? else {
            debug!("Commit Engine Vote Creator: Node is not an active validator, skipping");
            return Ok(());
        };
        let view = self.chain_view()?;
        if view.interval == 0 {
            return Ok(());
        }

        let mut ledger = self.ledger.load().map_err(|e| e.to_string())?;
        if ledger.votes().any(|vote| vote.height <= view.finalized) {
            ledger.prune_through(view.finalized);
            self.ledger.save(&ledger).map_err(|e| e.to_string())?;
        }

        let is_checkpoint = block_number.saturated_into::<u32>() % view.interval == 0;
        self.announce_timeouts(&view, &mut ledger, &account, &seal, is_checkpoint)?;

        // Every validator votes for the lowest checkpoint above the committed
        // head, so the target holds still while blocks keep arriving and every
        // vote of a round lands on one height.
        let target = commit_target(view.finalized, view.interval);
        if target > view.best_number {
            return Ok(());
        }
        let Some(candidate) = self
            .client
            .hash(target.into())
            .map_err(|e| format!("Failed to read the checkpoint hash: {:?}", e))?
        else {
            return Ok(());
        };

        let related = |lock: &CastVote| -> bool {
            self.client
                .hash(lock.height.into())
                .ok()
                .flatten()
                .map(|canonical| hash_bytes(&canonical) == lock.hash)
                .unwrap_or(false)
        };
        let weights = self.weights(view.best_hash)?;
        let total_weight: u128 = weights.iter().map(|(_, weight)| *weight).sum();
        let certificate = |lock: &CastVote| -> Option<Option<[u8; 32]>> {
            let statements: Vec<TimeoutStatement> = self
                .vote_pool
                .timeouts_for(lock.round, lock.height)
                .into_iter()
                .filter_map(|statement| {
                    let weight = weights
                        .iter()
                        .find(|(held, _)| *held == statement.validator_account_id)
                        .map(|(_, weight)| *weight)?;
                    Some(TimeoutStatement {
                        signer: hash_bytes(&statement.validator_account_id),
                        weight,
                        voted: statement.voted.as_ref().map(hash_bytes),
                    })
                })
                .collect();
            certificate_release(lock.hash, &statements, total_weight)
        };

        let choice = choose_vote(
            &ledger,
            view.round,
            target,
            hash_bytes(&candidate),
            related,
            certificate,
        );
        // A locked node gives the statements of the round of its lock a moment
        // to arrive, so a split round gathers behind one block.
        if !matches!(choice, VoteChoice::Repeat(_) | VoteChoice::Silent) {
            if let Some(lock) = ledger.lock_at(target) {
                // The node waits only while the statements heard so far leave
                // the answer of the certificate open: whether it forms, and
                // which block it stands behind.
                let statements: Vec<TimeoutStatement> = self
                    .vote_pool
                    .timeouts_for(lock.round, lock.height)
                    .into_iter()
                    .filter_map(|statement| {
                        let weight = weights
                            .iter()
                            .find(|(held, _)| *held == statement.validator_account_id)
                            .map(|(_, weight)| *weight)?;
                        Some(TimeoutStatement {
                            signer: hash_bytes(&statement.validator_account_id),
                            weight,
                            voted: statement.voted.as_ref().map(hash_bytes),
                        })
                    })
                    .collect();
                let waited = self.round_age(view.round)?;
                if certificate_undecided(lock.hash, &statements, total_weight) && waited < LOCK_WAIT
                {
                    debug!(
                        "Commit Engine Vote Creator: waiting for the statements of round {} before voting in round {}",
                        lock.round, view.round
                    );
                    return Ok(());
                }
            }
        }
        if matches!(choice, VoteChoice::Repeat(_)) && !is_checkpoint {
            // The vote of this round already went out. A checkpoint import
            // sends it again, byte for byte, for peers that missed it.
            return Ok(());
        }
        if let VoteChoice::Released { .. } = choice {
            for lock in blocking_locks(&ledger, related) {
                info!(
                    "Commit Engine Vote Creator: a round change certificate for round {} released the lock at #{}",
                    lock.round, lock.height
                );
                ledger.release(&lock);
            }
        }
        if let VoteChoice::Locked { height, .. } = choice {
            info!(
                "Commit Engine Vote Creator: the lock at #{} holds in round {}, voting for the locked block again",
                height, view.round
            );
        }

        let Some(cast) = choice.as_vote(view.round) else {
            debug!(
                "Commit Engine Vote Creator: this node stated it cast no vote in round {}",
                view.round
            );
            return Ok(());
        };
        self.sign_and_broadcast(&view, &mut ledger, cast, account, seal)
            .await
    }

    /// Write one vote to the ledger, sign it and broadcast it.
    async fn sign_and_broadcast(
        &self,
        view: &ChainView<Block::Hash>,
        ledger: &mut VoteLedger,
        cast: CastVote,
        account: AccountId,
        seal: ed25519::Public,
    ) -> Result<(), String> {
        ledger.record(cast).map_err(|e| e.to_string())?;
        // The ledger reaches the disk before the signature exists.
        self.ledger.save(ledger).map_err(|e| e.to_string())?;

        let block_hash = Block::Hash::decode(&mut &cast.hash[..])
            .map_err(|e| format!("Failed to read the block hash: {:?}", e))?;
        let mut vote = DvfVoteMessage {
            epoch_id: view.epoch_id,
            validator_set_id: view.validator_set_id,
            round_number: cast.round,
            block_number: cast.height,
            block_hash,
            validator_account_id: account,
            validator_public_key: seal,
            signature: ed25519::Signature::from_raw([0u8; 64]),
        };
        vote.signature = self.sign(&seal, &vote_payload(&vote))?;

        match self.vote_pool.offer_vote(vote.clone()) {
            VoteOffer::Inserted | VoteOffer::Repeated => {}
            VoteOffer::Conflicting => {
                // The ledger rules this out. A pool holding another vote of
                // ours in this round means an earlier release of this node
                // signed it. Nothing goes out.
                warn!(
                    "Commit Engine Vote Creator: the pool holds a different vote of this node in round {}. Nothing is broadcast.",
                    cast.round
                );
                return Ok(());
            }
        }
        info!(
            "Commit Engine Vote Creator: voting for #{} ({:?}) in round {}",
            vote.block_number, vote.block_hash, vote.round_number
        );
        self.broadcast(CommitMessage::Vote(vote))
    }

    /// Broadcast the round timeout statements this node owes.
    ///
    /// Every round below the current round since the last commitment the node
    /// saw, up to the statement window, closed. For each one the node states
    /// the block it is locked on at the height it voted at. For a round in
    /// which it cast no vote, it records the abstention in the ledger first
    /// and then states its lock at the lowest uncommitted checkpoint. A locked validator reads
    /// these statements as its round change certificate. A checkpoint import
    /// sends every statement again, for peers that restarted.
    fn announce_timeouts(
        &self,
        view: &ChainView<Block::Hash>,
        ledger: &mut VoteLedger,
        account: &AccountId,
        seal: &ed25519::Public,
        resend: bool,
    ) -> Result<(), String> {
        if view.round == 0 {
            return Ok(());
        }
        let first_round = {
            let mut committed_at = self
                .committed_at
                .lock()
                .map_err(|_| "The commitment record is unreadable".to_string())?;
            match *committed_at {
                Some((height, _)) if height >= view.finalized => {}
                _ => *committed_at = Some((view.finalized, view.round)),
            }
            committed_at
                .map(|(_, round)| round)
                .unwrap_or(view.round)
                .saturating_sub(1)
                .max(view.round.saturating_sub(STATEMENT_WINDOW_ROUNDS))
        };
        let pending = (view.finalized / view.interval + 1) * view.interval;

        let mut owed: Vec<(u32, u32, Option<[u8; 32]>)> = Vec::new();
        let mut abstained = false;
        for round in first_round..view.round {
            let height = match ledger.vote_in_round(round) {
                Some(vote) if vote.height > view.finalized => vote.height,
                Some(_) => continue,
                None => {
                    if !ledger.abstained_in(round) {
                        ledger.abstain(round).map_err(|e| e.to_string())?;
                        abstained = true;
                    }
                    pending
                }
            };
            // The statement names the lock at that height as of that round.
            owed.push((round, height, ledger.statement_for(round, height)));
        }
        if abstained {
            // The abstention reaches the disk before any statement is signed.
            self.ledger.save(ledger).map_err(|e| e.to_string())?;
        }

        for (round, height, voted) in owed {
            {
                let mut announced = self
                    .announced
                    .lock()
                    .map_err(|_| "The timeout record is unreadable".to_string())?;
                if !announced.insert((round, height)) && !resend {
                    continue;
                }
                announced.retain(|(held, _)| *held >= first_round);
            }
            let voted_hash = match voted {
                Some(bytes) => Some(
                    Block::Hash::decode(&mut &bytes[..])
                        .map_err(|e| format!("Failed to read the block hash: {:?}", e))?,
                ),
                None => None,
            };
            let mut statement = RoundTimeoutMessage {
                epoch_id: view.epoch_id,
                validator_set_id: view.validator_set_id,
                round_number: round,
                block_number: height,
                voted: voted_hash,
                validator_account_id: account.clone(),
                validator_public_key: *seal,
                signature: ed25519::Signature::from_raw([0u8; 64]),
            };
            statement.signature = self.sign(seal, &statement.payload())?;
            self.vote_pool.offer_timeout(statement.clone());
            debug!(
                "Commit Engine Vote Creator: round {} closed with no commitment at #{}, statement sent",
                round, height
            );
            self.broadcast(CommitMessage::RoundTimeout(statement))?;
        }
        Ok(())
    }

    /// How long ago this node first saw the current round open.
    fn round_age(&self, round: u32) -> Result<std::time::Duration, String> {
        let mut opened = self
            .round_opened
            .lock()
            .map_err(|_| "The round record is unreadable".to_string())?;
        match *opened {
            Some((held, since)) if held == round => Ok(since.elapsed()),
            _ => {
                *opened = Some((round, std::time::Instant::now()));
                Ok(std::time::Duration::ZERO)
            }
        }
    }

    /// The frozen commit weight of every validator.
    fn weights(&self, best_hash: Block::Hash) -> Result<Vec<(AccountId, u128)>, String> {
        self.client
            .runtime_api()
            .get_validator_weights(best_hash)
            .map_err(|e| format!("Failed to get validator weights: {:?}", e))
    }

    /// The account and the seal key this node signs with.
    ///
    /// The registry names the seal key of every active validator. The node
    /// signs with the local key the registry names, never with the first key
    /// the keystore lists.
    fn local_seal_key(&self) -> Result<Option<(AccountId, ed25519::Public)>, String> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        let registry = api
            .active_validator_keys(best_hash)
            .map_err(|e| format!("Failed to read the validator key records: {:?}", e))?;
        let local = Keystore::ed25519_public_keys(&*self.keystore, crate::CERULEA_KEY_TYPE);
        Ok(registry_seal_key(&local, &registry))
    }

    /// Sign one payload with the seal key.
    fn sign(&self, seal: &ed25519::Public, payload: &[u8]) -> Result<ed25519::Signature, String> {
        Keystore::ed25519_sign(&*self.keystore, crate::CERULEA_KEY_TYPE, seal, payload)
            .map_err(|e| format!("Failed to sign: {:?}", e))?
            .ok_or_else(|| {
                "The keystore holds no seal key the registry names. Fix: insert the validator ed25519 seal key under the key type cerk.".to_string()
            })
    }

    /// Broadcast one commit protocol message.
    fn broadcast(&self, message: CommitMessage<Block::Hash, AccountId>) -> Result<(), String> {
        let encoded = message.encode();
        let msg_hash = sp_core::hashing::blake2_256(&encoded);
        let topic = Block::Hash::decode(&mut &msg_hash[..])
            .map_err(|e| format!("Failed to decode message hash: {:?}", e))?;
        let mut gossip_engine = self
            .gossip_engine
            .lock()
            .map_err(|e| format!("Failed to lock gossip engine: {:?}", e))?;
        gossip_engine.gossip_message(topic, encoded, false);
        Ok(())
    }
}

/// The checkpoint a round votes for: the lowest one above the committed head.
pub fn commit_target(finalized: u32, interval: u32) -> u32 {
    if interval == 0 {
        return finalized;
    }
    (finalized / interval + 1) * interval
}

/// The bytes one vote signature covers.
pub fn vote_payload<Hash: Encode, AccountId: Encode>(
    vote: &DvfVoteMessage<Hash, AccountId>,
) -> Vec<u8> {
    let mut payload = Vec::new();
    vote.epoch_id.encode_to(&mut payload);
    vote.validator_set_id.encode_to(&mut payload);
    vote.round_number.encode_to(&mut payload);
    vote.block_number.encode_to(&mut payload);
    vote.block_hash.encode_to(&mut payload);
    vote.validator_account_id.encode_to(&mut payload);
    payload
}

#[cfg(test)]
mod seal_key_tests {
    use super::registry_seal_key;
    use pallet_cerulea_dvf::keys::ValidatorKeySet;
    use sp_core::{ed25519, sr25519, Pair};
    use sp_keystore::Keystore;

    /// A node holding both keys in either file order signs votes that verify
    /// against the seal key the registry names.
    #[test]
    fn a_node_holding_both_keys_signs_with_the_registry_seal_key() {
        let mut orders = (false, false);
        for index in 0..24u32 {
            let folder = tempfile::tempdir().unwrap();
            let keystore = sc_keystore::LocalKeystore::open(folder.path(), None).unwrap();
            let seed = format!("//Cerulea//order//{}", index);
            let seal = ed25519::Pair::from_string(&seed, None).unwrap().public();
            let draw = sr25519::Pair::from_string(&seed, None).unwrap().public();

            // The earlier layout kept both keys under one key type. A file
            // keystore lists both as ed25519 keys, in file name order.
            keystore
                .insert(crate::CERULEA_KEY_TYPE, &seed, draw.as_ref())
                .unwrap();
            keystore
                .insert(crate::CERULEA_KEY_TYPE, &seed, seal.as_ref())
                .unwrap();
            let listed = keystore.ed25519_public_keys(crate::CERULEA_KEY_TYPE);
            assert_eq!(listed.len(), 2, "the file keystore lists both keys");
            if listed[0] == seal {
                orders.0 = true;
            } else {
                orders.1 = true;
            }

            let account = [index as u8; 32];
            let registry = vec![
                ([200u8; 32], ValidatorKeySet::new([7u8; 32], [8u8; 32])),
                (account, ValidatorKeySet::new(draw.0, seal.0)),
            ];
            let (named, key) =
                registry_seal_key(&listed, &registry).expect("the registry names the key");
            assert_eq!(named, account);
            assert_eq!(key, seal);

            let payload = b"Cerulea vote payload";
            let signature = keystore
                .ed25519_sign(crate::CERULEA_KEY_TYPE, &key, payload)
                .unwrap()
                .expect("the keystore signs with the seal key");
            assert!(ed25519::Pair::verify(&signature, payload, &seal));
        }
        assert!(
            orders.0 && orders.1,
            "both file orders appeared: {:?}",
            orders
        );
    }

    /// A node the registry names no seal key for signs nothing.
    #[test]
    fn a_node_outside_the_registry_selects_no_key() {
        let seal = ed25519::Pair::from_string("//Cerulea//outside", None)
            .unwrap()
            .public();
        let registry = vec![([1u8; 32], ValidatorKeySet::new([2u8; 32], [3u8; 32]))];
        assert!(registry_seal_key(&[seal], &registry).is_none());
    }
}

#[cfg(test)]
mod service_tests {
    //! The vote creator, run against the test chain.
    //!
    //! The node holds one seal key in a memory keystore. The registry names
    //! that key and the key of one peer validator. The checkpoint interval is
    //! ten blocks. The service writes its ledger to a temporary folder and
    //! multicasts on a gossip engine with no peers, which keeps every message.

    use super::*;
    use crate::test_support::{
        capture_logs, gossip_holds, peerless_gossip, AccountId, Block, TestChain, Voter,
    };
    use pallet_cerulea_dvf::keys::ValidatorKeySet;
    use sc_network_gossip::GossipEngine;
    use sp_core::{Pair as _, H256};
    use sp_keystore::testing::MemoryKeystore;
    use std::time::Duration;

    type Pool = DvfVotePool<H256, AccountId>;
    type Service = VoteCreatorService<Block, TestChain, AccountId>;

    /// A block off the canonical chain, at the first checkpoint.
    const OFF_CHAIN: [u8; 32] = [0x42; 32];

    struct Scene {
        chain: Arc<TestChain>,
        pool: Arc<Pool>,
        gossip: Arc<Mutex<GossipEngine<Block>>>,
        keystore: KeystorePtr,
        seal: ed25519::Public,
        me: AccountId,
        peer: Voter,
        folder: tempfile::TempDir,
    }

    impl Scene {
        fn on(chain: Arc<TestChain>) -> Scene {
            let keystore: KeystorePtr = Arc::new(MemoryKeystore::new());
            let seal = keystore
                .ed25519_generate_new(crate::CERULEA_KEY_TYPE, Some("//Cerulea//voter"))
                .expect("the keystore makes the seal key");
            let me = AccountId::from(seal.0);
            let peer = Voter::from_seed(2);
            let answers = &chain.answers;
            answers.set(
                "CeruleaAuthoringApi_active_validator_keys",
                (),
                vec![
                    (me.clone(), ValidatorKeySet::new([9u8; 32], seal.0)),
                    (
                        peer.account.clone(),
                        ValidatorKeySet::new([8u8; 32], peer.public().0),
                    ),
                ],
            );
            answers.set("DvfApi_get_current_round", (), 0u32);
            answers.set("DvfApi_get_finality_checkpoint_interval", (), 10u32);
            answers.set("DvfApi_get_current_epoch", (), 1u32);
            answers.set("DvfApi_get_validator_set_id", (), 1u32);
            answers.set(
                "DvfApi_get_validator_weights",
                (),
                vec![(me.clone(), 100u128)],
            );
            Scene {
                chain,
                pool: Pool::new(),
                gossip: peerless_gossip(),
                keystore,
                seal,
                me,
                peer,
                folder: tempfile::tempdir().expect("the folder opens"),
            }
        }

        fn store(&self) -> LedgerStore {
            LedgerStore::under(self.folder.path())
        }

        fn ledger(&self) -> VoteLedger {
            self.store().load().expect("the ledger reads")
        }

        fn service(&self) -> Service {
            Service::new(
                self.chain.clone(),
                self.keystore.clone(),
                self.gossip.clone(),
                self.pool.clone(),
                self.me.clone(),
                self.store(),
            )
        }

        fn round(&self, round: u32) {
            self.chain
                .answers
                .set("DvfApi_get_current_round", (), round);
        }

        /// The peer validator joins with three times the weight of this node.
        fn add_heavy_peer(&self) {
            self.chain.answers.set(
                "DvfApi_get_validator_weights",
                (),
                vec![
                    (self.me.clone(), 100u128),
                    (self.peer.account.clone(), 300u128),
                ],
            );
        }

        fn recorded(&self, vote: CastVote) {
            self.store()
                .record_before_signing(vote)
                .expect("the ledger takes the vote");
        }

        fn sent(&self, message: CommitMessage<H256, AccountId>) -> bool {
            gossip_holds(&self.gossip, &message.encode())
        }
    }

    fn cast(round: u32, height: u32, hash: [u8; 32]) -> CastVote {
        CastVote {
            round,
            height,
            hash,
        }
    }

    #[test]
    fn the_commit_target_is_the_lowest_checkpoint_above_the_committed_head() {
        assert_eq!(commit_target(0, 10), 10);
        assert_eq!(commit_target(9, 10), 10);
        assert_eq!(commit_target(10, 10), 20);
        assert_eq!(commit_target(19, 10), 20);
        // An interval of zero leaves the committed head as the target.
        assert_eq!(commit_target(7, 0), 7);
        assert_eq!(LOCK_WAIT, Duration::from_secs(30));
    }

    #[test]
    fn the_vote_payload_joins_the_fields_in_order() {
        let voter = Voter::from_seed(4);
        let vote = voter.vote(3, 2, 9, 40, H256::repeat_byte(1));
        let mut expected = Vec::new();
        expected.extend_from_slice(&3u32.encode());
        expected.extend_from_slice(&2u32.encode());
        expected.extend_from_slice(&9u32.encode());
        expected.extend_from_slice(&40u32.encode());
        expected.extend_from_slice(&[1u8; 32]);
        expected.extend_from_slice(&voter.account.encode());
        assert_eq!(vote_payload(&vote), expected);
        assert!(ed25519::Pair::verify(
            &vote.signature,
            &expected,
            &voter.public()
        ));
    }

    #[tokio::test]
    async fn a_validator_votes_for_the_lowest_open_checkpoint() {
        let scene = Scene::on(TestChain::with_blocks(12));
        let service = scene.service();
        let logs = capture_logs();

        service.process_block(10).await.expect("the pass completes");

        let target = scene.chain.hash_at(10);
        assert_eq!(scene.ledger().vote_in_round(0), Some(cast(0, 10, target.0)));
        let vote = scene
            .pool
            .vote_of(0, &scene.me)
            .expect("the pool holds the vote");
        assert_eq!(vote.block_hash, target);
        assert_eq!(vote.block_number, 10);
        assert_eq!((vote.epoch_id, vote.validator_set_id), (1, 1));
        assert_eq!(vote.validator_public_key, scene.seal);
        assert!(ed25519::Pair::verify(
            &vote.signature,
            vote_payload(&vote),
            &scene.seal
        ));
        assert!(scene.sent(CommitMessage::Vote(vote)));
        assert!(logs.has("voting for #10"));
    }

    #[tokio::test]
    async fn a_node_the_registry_leaves_out_signs_nothing() {
        let scene = Scene::on(TestChain::with_blocks(12));
        scene.chain.answers.set(
            "CeruleaAuthoringApi_active_validator_keys",
            (),
            vec![(
                scene.peer.account.clone(),
                ValidatorKeySet::new([8u8; 32], scene.peer.public().0),
            )],
        );
        let logs = capture_logs();

        scene.service().process_block(10).await.unwrap();

        assert!(scene.ledger().is_empty());
        assert_eq!(scene.pool.total_votes(), 0);
        assert!(logs.has("Node is not an active validator, skipping"));
    }

    #[tokio::test]
    async fn an_interval_of_zero_signs_nothing() {
        let scene = Scene::on(TestChain::with_blocks(12));
        scene
            .chain
            .answers
            .set("DvfApi_get_finality_checkpoint_interval", (), 0u32);

        scene.service().process_block(10).await.unwrap();

        assert!(scene.ledger().is_empty());
        assert_eq!(scene.pool.total_votes(), 0);
    }

    #[tokio::test]
    async fn a_checkpoint_above_the_best_block_waits() {
        let scene = Scene::on(TestChain::with_blocks(8));

        scene.service().process_block(8).await.unwrap();

        assert!(scene.ledger().is_empty());
        assert_eq!(scene.pool.total_votes(), 0);
    }

    #[tokio::test]
    async fn a_vote_already_cast_goes_out_again_only_at_a_checkpoint() {
        let scene = Scene::on(TestChain::with_blocks(12));
        let service = scene.service();
        let logs = capture_logs();

        service.process_block(10).await.unwrap();
        let first = scene.pool.vote_of(0, &scene.me).expect("the first vote");
        service.process_block(11).await.unwrap();
        assert_eq!(logs.count("voting for #10"), 1);
        service.process_block(20).await.unwrap();
        assert_eq!(logs.count("voting for #10"), 2);

        // The repeat carries the same vote, byte for byte.
        assert_eq!(scene.ledger().len(), 1);
        assert_eq!(scene.pool.vote_of(0, &scene.me), Some(first));
        assert_eq!(scene.pool.total_votes(), 1);
    }

    #[tokio::test]
    async fn a_locked_node_waits_for_the_statements_of_its_lock_round() {
        let scene = Scene::on(TestChain::with_blocks(12));
        let target = scene.chain.hash_at(10);
        scene.recorded(cast(0, 10, target.0));
        scene.add_heavy_peer();
        scene.round(2);
        let service = scene.service();
        let logs = capture_logs();

        service.process_block(10).await.unwrap();

        // Round one closed with no vote from this node. The node recorded the
        // abstention and stated its lock.
        let ledger = scene.ledger();
        assert!(ledger.abstained_in(1));
        assert_eq!(ledger.vote_in_round(2), None);
        let statements = scene.pool.timeouts_for(1, 10);
        assert_eq!(statements.len(), 1);
        assert_eq!(statements[0].validator_account_id, scene.me);
        assert_eq!(statements[0].voted, Some(target));
        assert!(ed25519::Pair::verify(
            &statements[0].signature,
            statements[0].payload(),
            &scene.seal
        ));
        assert!(scene.sent(CommitMessage::RoundTimeout(statements[0].clone())));
        assert!(logs.has("waiting for the statements of round 0 before voting in round 2"));

        // The peer states the same lock. No unheard weight can now form a
        // certificate, so the node votes at once.
        assert!(scene
            .pool
            .offer_timeout(scene.peer.timeout(1, 1, 0, 10, Some(target))));
        service.process_block(11).await.unwrap();

        assert_eq!(scene.ledger().vote_in_round(2), Some(cast(2, 10, target.0)));
        assert_eq!(
            scene.pool.vote_of(2, &scene.me).map(|vote| vote.block_hash),
            Some(target)
        );
    }

    #[tokio::test]
    async fn statements_go_out_again_only_at_a_checkpoint() {
        let scene = Scene::on(TestChain::with_blocks(12));
        scene.recorded(cast(0, 10, scene.chain.hash_at(10).0));
        scene.add_heavy_peer();
        scene.round(2);
        let service = scene.service();
        let logs = capture_logs();
        let line = "round 1 closed with no commitment at #10, statement sent";

        service.process_block(10).await.unwrap();
        assert_eq!(logs.count(line), 1);
        service.process_block(11).await.unwrap();
        assert_eq!(logs.count(line), 1);
        service.process_block(20).await.unwrap();
        assert_eq!(logs.count(line), 2);
    }

    #[tokio::test]
    async fn a_lock_off_the_best_chain_holds_without_a_certificate() {
        let scene = Scene::on(TestChain::with_blocks(12));
        scene.recorded(cast(0, 10, OFF_CHAIN));
        scene.round(1);
        let logs = capture_logs();

        scene.service().process_block(10).await.unwrap();

        assert_eq!(
            scene.ledger().vote_in_round(1),
            Some(cast(1, 10, OFF_CHAIN))
        );
        assert_eq!(
            scene.pool.vote_of(1, &scene.me).map(|vote| vote.block_hash),
            Some(H256::from(OFF_CHAIN))
        );
        // The statement for round zero names the lock.
        let statements = scene.pool.timeouts_for(0, 10);
        assert_eq!(statements.len(), 1);
        assert_eq!(statements[0].voted, Some(H256::from(OFF_CHAIN)));
        assert!(logs.has("the lock at #10 holds in round 1, voting for the locked block again"));
    }

    #[tokio::test]
    async fn a_round_change_certificate_moves_the_lock_to_the_leading_block() {
        let scene = Scene::on(TestChain::with_blocks(12));
        let canonical = scene.chain.hash_at(10);
        scene.recorded(cast(0, 10, OFF_CHAIN));
        scene.add_heavy_peer();
        assert!(scene
            .pool
            .offer_timeout(scene.peer.timeout(1, 1, 0, 10, Some(canonical))));
        scene.round(1);
        let logs = capture_logs();

        scene.service().process_block(10).await.unwrap();

        let ledger = scene.ledger();
        assert_eq!(ledger.vote_in_round(1), Some(cast(1, 10, canonical.0)));
        assert_eq!(ledger.lock_at(10), Some(cast(1, 10, canonical.0)));
        assert!(logs.has("a round change certificate for round 0 released the lock at #10"));
    }

    #[tokio::test]
    async fn a_round_stated_as_silent_takes_no_vote() {
        let scene = Scene::on(TestChain::with_blocks(12));
        let mut ledger = VoteLedger::new();
        ledger.abstain(1).unwrap();
        scene.store().save(&ledger).unwrap();
        scene.round(1);
        let logs = capture_logs();

        scene.service().process_block(10).await.unwrap();

        let ledger = scene.ledger();
        assert_eq!(ledger.vote_in_round(1), None);
        assert!(ledger.abstained_in(0));
        let statements = scene.pool.timeouts_for(0, 10);
        assert_eq!(statements.len(), 1);
        assert_eq!(statements[0].voted, None);
        assert!(logs.has("this node stated it cast no vote in round 1"));
        assert!(scene.pool.vote_of(1, &scene.me).is_none());
    }

    #[tokio::test]
    async fn a_pool_holding_another_vote_of_this_node_stops_the_broadcast() {
        let scene = Scene::on(TestChain::with_blocks(12));
        let mut earlier = DvfVoteMessage {
            epoch_id: 1,
            validator_set_id: 1,
            round_number: 0,
            block_number: 10,
            block_hash: H256::from(OFF_CHAIN),
            validator_account_id: scene.me.clone(),
            validator_public_key: scene.seal,
            signature: ed25519::Signature::from_raw([0u8; 64]),
        };
        earlier.signature = scene
            .keystore
            .ed25519_sign(
                crate::CERULEA_KEY_TYPE,
                &scene.seal,
                &vote_payload(&earlier),
            )
            .unwrap()
            .unwrap();
        assert!(scene.pool.insert_vote(earlier));
        let logs = capture_logs();

        scene.service().process_block(10).await.unwrap();

        // The ledger took the vote before the signature existed.
        assert_eq!(
            scene.ledger().vote_in_round(0),
            Some(cast(0, 10, scene.chain.hash_at(10).0))
        );
        let evidence = scene.pool.take_evidence();
        assert_eq!(evidence.len(), 1);
        assert!(!scene.sent(CommitMessage::Vote(evidence[0].1.clone())));
        assert!(logs
            .has("the pool holds a different vote of this node in round 0. Nothing is broadcast."));
    }

    #[tokio::test]
    async fn a_committed_height_leaves_the_ledger() {
        let scene = Scene::on(TestChain::build(10, 12));
        scene.recorded(cast(0, 10, scene.chain.hash_at(10).0));
        scene.round(1);

        scene.service().process_block(12).await.unwrap();

        let ledger = scene.ledger();
        assert!(ledger.is_empty());
        assert!(ledger.below_floor(0));
        // The statement for round zero names the next open checkpoint.
        let statements = scene.pool.timeouts_for(0, 20);
        assert_eq!(statements.len(), 1);
        assert_eq!(statements[0].voted, None);
        assert_eq!(scene.pool.total_votes(), 0);
    }

    #[tokio::test]
    async fn every_missing_chain_fact_names_its_query() {
        let cases = [
            (
                "CeruleaAuthoringApi_active_validator_keys",
                "Failed to read the validator key records: ",
            ),
            ("DvfApi_get_current_round", "Failed to get current round: "),
            (
                "DvfApi_get_finality_checkpoint_interval",
                "Failed to get FinalityCheckpointInterval: ",
            ),
            ("DvfApi_get_current_epoch", "Failed to get current epoch: "),
            (
                "DvfApi_get_validator_set_id",
                "Failed to get validator set ID: ",
            ),
            (
                "DvfApi_get_validator_weights",
                "Failed to get validator weights: ",
            ),
        ];
        for (call, opening) in cases {
            let scene = Scene::on(TestChain::with_blocks(12));
            scene.chain.answers.forget(call);
            let reason = scene
                .service()
                .process_block(10)
                .await
                .expect_err("the pass stops at the missing answer");
            assert!(reason.starts_with(opening), "{call}: {reason}");
        }
    }

    #[tokio::test]
    async fn an_unreadable_ledger_stops_the_pass_and_names_the_fix() {
        let scene = Scene::on(TestChain::with_blocks(12));
        std::fs::write(scene.store().path(), [0xffu8, 0xff, 0xff]).unwrap();

        let reason = scene
            .service()
            .process_block(10)
            .await
            .expect_err("the ledger fails to read");

        assert!(
            reason.starts_with("The vote ledger failed to read or write ("),
            "{reason}"
        );
        assert!(reason.ends_with("Fix: check that the node base path is writable."));
        assert_eq!(scene.pool.total_votes(), 0);
    }

    #[tokio::test]
    async fn the_running_service_votes_on_each_import() {
        let scene = Scene::on(TestChain::with_blocks(12));
        let service = scene.service().with_metrics(Arc::new(
            DvfMetrics::new(&prometheus::Registry::new()).expect("the metrics register"),
        ));
        assert!(service.metrics.is_some());
        let logs = capture_logs();
        let chain = scene.chain.clone();
        let driver = async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            chain.announce_import(10);
            tokio::time::sleep(Duration::from_millis(10)).await;
            chain.close_imports();
        };

        tokio::join!(service.run(), driver);

        assert!(scene.pool.vote_of(0, &scene.me).is_some());
        assert!(logs.has("Import notification stream ended"));
    }

    #[tokio::test]
    async fn the_running_service_reports_a_failed_pass() {
        let scene = Scene::on(TestChain::with_blocks(12));
        scene.chain.answers.forget("DvfApi_get_current_round");
        let service = scene.service();
        let logs = capture_logs();
        let chain = scene.chain.clone();
        let driver = async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            chain.announce_import(10);
            tokio::time::sleep(Duration::from_millis(10)).await;
            chain.close_imports();
        };

        tokio::join!(service.run(), driver);

        assert!(logs.has("Failed to process block #10"));
        assert_eq!(scene.pool.total_votes(), 0);
    }

    #[test]
    fn the_round_clock_restarts_with_each_new_round() {
        let scene = Scene::on(TestChain::with_blocks(2));
        let service = scene.service();
        assert_eq!(service.round_age(4).unwrap(), Duration::ZERO);
        std::thread::sleep(Duration::from_millis(5));
        assert!(service.round_age(4).unwrap() >= Duration::from_millis(5));
        assert_eq!(service.round_age(5).unwrap(), Duration::ZERO);
    }
}
