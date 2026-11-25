//! The Custody Engine answer path on the node.
//!
//! The chain issues a custody challenge every few blocks and publishes it in
//! the block header. The challenge names one validator and one record of one
//! historical block. The named validator answers inside the deadline, three
//! blocks by default, or the chain records the challenge as missed and its
//! custody share falls.
//!
//! This task answers for the validator this node runs. On each new best block
//! it reads the published challenge. Where the challenge names a validator
//! whose seal key the local keystore holds, it reads the entries of the
//! challenged block from the local store, checks them against the record root
//! the chain holds, builds the inclusion proof with the same code the chain
//! verifies against, signs the answer with the validator seal key and submits
//! it to the transaction pool.
//!
//! The runtime writes every custody entry to the local store of the node as
//! the block executes. The node therefore answers from its own copy of the
//! history, with no network fetch.

use cerulea_runtime::{self as runtime, opaque::Block, AccountId, BlockNumber, Hash, Nonce};
use codec::{Decode, Encode};
use pallet_cerulea_custody::{Challenge, ChallengeAnswer};
use sp_core::H256;
use std::collections::BTreeSet;

/// The name the custody module carries in the runtime.
pub const CUSTODY_PALLET: &[u8] = b"Custody";

/// The challenges one task remembers answering, so a block seen twice never
/// produces a second answer.
pub const ANSWERED_MEMORY: usize = 256;

/// The storage key of one plain storage value.
pub fn value_key(pallet: &[u8], item: &[u8]) -> Vec<u8> {
    let mut key = sp_core::hashing::twox_128(pallet).to_vec();
    key.extend_from_slice(&sp_core::hashing::twox_128(item));
    key
}

/// The storage key of one entry of a map hashed with the concatenating
/// blake2 128 hasher.
pub fn map_key(pallet: &[u8], item: &[u8], encoded_key: &[u8]) -> Vec<u8> {
    let mut key = value_key(pallet, item);
    key.extend_from_slice(&sp_core::hashing::blake2_128(encoded_key));
    key.extend_from_slice(encoded_key);
    key
}

/// The challenge this node answers, if the published one names a local key.
///
/// A challenge already answered, or one whose deadline passed, yields nothing.
pub fn challenge_to_answer(
    local: &[AccountId],
    published: Option<(u64, Challenge<AccountId, BlockNumber>)>,
    answered: &BTreeSet<u64>,
    now: BlockNumber,
) -> Option<(u64, Challenge<AccountId, BlockNumber>)> {
    let (id, challenge) = published?;
    if answered.contains(&id) || now > challenge.deadline {
        return None;
    }
    local
        .iter()
        .any(|account| *account == challenge.target)
        .then_some((id, challenge))
}

/// Read the entries of one block from the local store.
///
/// The block committed `count` entries. A missing entry yields nothing: the
/// node lacks the history the challenge reaches into.
pub fn local_entries(
    get: impl Fn(&[u8]) -> Option<Vec<u8>>,
    block: u64,
    parent: &[u8; 32],
    count: u32,
) -> Option<Vec<Vec<u8>>> {
    (0..count)
        .map(|index| {
            get(&pallet_cerulea_custody::primitives::local_entry_key(
                block, parent, index,
            ))
        })
        .collect()
}

/// Build the answer from the local entries of the challenged block.
///
/// The entries must reach the root the chain holds for that block. A copy
/// that differs yields the reason, and the node answers nothing rather than
/// earn a fault for a proof the chain refuses.
pub fn build_answer(
    entries: &[Vec<u8>],
    index: u32,
    root: H256,
) -> Result<ChallengeAnswer, String> {
    let local = pallet_cerulea_custody::respond::local_record_root(entries);
    if local != root {
        return Err(format!(
            "the local entries reach root {:?} and the chain holds {:?}. Fix: resync the history of this node from genesis.",
            local, root
        ));
    }
    pallet_cerulea_custody::respond::assemble_answer(entries, index)
        .map_err(|reason| reason.plain_language().to_string())
}

/// The payload one transaction signature covers, with its extensions.
pub fn transaction_parts(
    call: runtime::RuntimeCall,
    nonce: Nonce,
    genesis: Hash,
    best: Hash,
    best_number: BlockNumber,
) -> (runtime::SignedPayload, runtime::TxExtension) {
    let period = runtime::configs::BlockHashCount::get()
        .checked_next_power_of_two()
        .map(|count| count / 2)
        .unwrap_or(2) as u64;
    let extension: runtime::TxExtension = (
        frame_system::CheckNonZeroSender::<runtime::Runtime>::new(),
        frame_system::CheckSpecVersion::<runtime::Runtime>::new(),
        frame_system::CheckTxVersion::<runtime::Runtime>::new(),
        frame_system::CheckGenesis::<runtime::Runtime>::new(),
        frame_system::CheckEra::<runtime::Runtime>::from(sp_runtime::generic::Era::mortal(
            period,
            best_number as u64,
        )),
        frame_system::CheckNonce::<runtime::Runtime>::from(nonce),
        frame_system::CheckWeight::<runtime::Runtime>::new(),
        pallet_transaction_payment::ChargeTransactionPayment::<runtime::Runtime>::from(0),
        frame_metadata_hash_extension::CheckMetadataHash::<runtime::Runtime>::new(false),
        frame_system::WeightReclaim::<runtime::Runtime>::new(),
    );
    let payload = runtime::SignedPayload::from_raw(
        call,
        extension.clone(),
        (
            (),
            runtime::VERSION.spec_version,
            runtime::VERSION.transaction_version,
            genesis,
            best,
            (),
            (),
            (),
            None,
            (),
        ),
    );
    (payload, extension)
}

/// The call that answers one challenge.
pub fn answer_call(id: u64, answer: ChallengeAnswer) -> runtime::RuntimeCall {
    runtime::RuntimeCall::Custody(pallet_cerulea_custody::Call::answer_challenge { id, answer })
}

/// Start the task that answers custody challenges for this node.
pub fn spawn<C, BE, P>(
    spawner: &sc_service::SpawnTaskHandle,
    client: std::sync::Arc<C>,
    backend: std::sync::Arc<BE>,
    pool: std::sync::Arc<P>,
    keystore: sp_keystore::KeystorePtr,
) where
    C: sc_client_api::BlockchainEvents<Block>
        + sc_client_api::StorageProvider<Block, BE>
        + sp_blockchain::HeaderBackend<Block>
        + sp_api::ProvideRuntimeApi<Block>
        + Send
        + Sync
        + 'static,
    C::Api: frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce>,
    BE: sc_client_api::Backend<Block> + Send + Sync + 'static,
    P: sc_transaction_pool_api::TransactionPool<Block = Block> + 'static,
{
    use frame_system_rpc_runtime_api::AccountNonceApi;
    use futures::StreamExt;
    use sc_transaction_pool_api::TransactionSource;
    use sp_core::offchain::OffchainStorage;
    use sp_runtime::traits::Header as _;

    let mut imports = client.import_notification_stream();
    spawner.spawn("custody-answer", None, async move {
        let mut answered: BTreeSet<u64> = BTreeSet::new();
        while let Some(notification) = imports.next().await {
            if !notification.is_new_best {
                continue;
            }
            let at = notification.hash;
            let now: BlockNumber = *notification.header.number();
            let read = |key: Vec<u8>| -> Option<Vec<u8>> {
                client
                    .storage(at, &sc_client_api::StorageKey(key))
                    .ok()
                    .flatten()
                    .map(|data| data.0)
            };
            let published = read(value_key(CUSTODY_PALLET, b"PublishedChallenge")).and_then(
                |bytes| {
                    <(u64, Challenge<AccountId, BlockNumber>)>::decode(&mut &bytes[..]).ok()
                },
            );
            let local: Vec<AccountId> = keystore
                .ed25519_public_keys(cerulea_consensus::CERULEA_KEY_TYPE)
                .into_iter()
                .map(|key| AccountId::from(key.0))
                .collect();
            let Some((id, challenge)) = challenge_to_answer(&local, published, &answered, now)
            else {
                continue;
            };
            log::info!(
                "Custody Engine: challenge {} names this node for block #{} record {}, issued at #{} with deadline #{}, seen at #{}",
                id,
                challenge.record.block,
                challenge.record.index,
                challenge.issued,
                challenge.deadline,
                now
            );
            let block = challenge.record.block;
            let encoded_block = block.encode();
            let count: u32 = read(map_key(CUSTODY_PALLET, b"RecordCounts", &encoded_block))
                .and_then(|bytes| u32::decode(&mut &bytes[..]).ok())
                .unwrap_or(0);
            let root = read(map_key(CUSTODY_PALLET, b"RecordRoots", &encoded_block))
                .and_then(|bytes| H256::decode(&mut &bytes[..]).ok());
            let parent = client
                .hash(block)
                .ok()
                .flatten()
                .and_then(|hash| client.header(hash).ok().flatten())
                .map(|header| *header.parent_hash());
            let (Some(root), Some(parent)) = (root, parent) else {
                log::warn!(
                    "Custody Engine: challenge {} reaches block #{} and this node holds no root or header for it.",
                    id,
                    block
                );
                continue;
            };
            let mut parent_bytes = [0u8; 32];
            parent_bytes.copy_from_slice(parent.as_ref());
            let Some(store) = backend.offchain_storage() else {
                log::warn!(
                    "Custody Engine: this node keeps no local store. Fix: start the node with the local store enabled."
                );
                continue;
            };
            let entries = local_entries(
                |key| store.get(sp_core::offchain::STORAGE_PREFIX, key),
                block as u64,
                &parent_bytes,
                count,
            );
            let Some(entries) = entries else {
                log::warn!(
                    "Custody Engine: challenge {} reaches block #{} and the local store lacks its entries.",
                    id,
                    block
                );
                continue;
            };
            let answer = match build_answer(&entries, challenge.record.index, root) {
                Ok(answer) => answer,
                Err(reason) => {
                    log::warn!("Custody Engine: challenge {} left unanswered: {}", id, reason);
                    continue;
                }
            };

            let signer = sp_core::ed25519::Public::from_raw(
                <AccountId as AsRef<[u8; 32]>>::as_ref(&challenge.target).to_owned(),
            );
            let info = client.info();
            let nonce = match client.runtime_api().account_nonce(info.best_hash, challenge.target.clone()) {
                Ok(nonce) => nonce,
                Err(reason) => {
                    log::warn!("Custody Engine: the account nonce failed to read: {:?}", reason);
                    continue;
                }
            };
            let call = answer_call(id, answer);
            let (payload, extension) =
                transaction_parts(call.clone(), nonce, info.genesis_hash, info.best_hash, info.best_number);
            let signature = payload.using_encoded(|bytes| {
                keystore.ed25519_sign(cerulea_consensus::CERULEA_KEY_TYPE, &signer, bytes)
            });
            let Ok(Some(signature)) = signature else {
                log::warn!(
                    "Custody Engine: the keystore declined to sign the answer to challenge {}.",
                    id
                );
                continue;
            };
            let extrinsic = runtime::UncheckedExtrinsic::new_signed(
                call,
                challenge.target.clone().into(),
                runtime::Signature::Ed25519(signature),
                extension,
            );
            let opaque: <Block as sp_runtime::traits::Block>::Extrinsic =
                match Decode::decode(&mut &extrinsic.encode()[..]) {
                    Ok(opaque) => opaque,
                    Err(_) => continue,
                };
            match pool.submit_one(info.best_hash, TransactionSource::Local, opaque).await {
                Ok(_) => {
                    log::info!(
                        "Custody Engine: answered challenge {} for block #{} record {}; the answer entered the pool at best block #{} with deadline #{}",
                        id,
                        block,
                        challenge.record.index,
                        info.best_number,
                        challenge.deadline
                    );
                    answered.insert(id);
                    while answered.len() > ANSWERED_MEMORY {
                        if let Some(oldest) = answered.iter().next().copied() {
                            answered.remove(&oldest);
                        }
                    }
                }
                Err(reason) => log::warn!(
                    "Custody Engine: the answer to challenge {} was refused by the pool: {:?}",
                    id,
                    reason
                ),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use pallet_cerulea_custody::RecordRef;

    fn account(byte: u8) -> AccountId {
        AccountId::from([byte; 32])
    }

    fn challenge(target: u8, deadline: BlockNumber) -> Challenge<AccountId, BlockNumber> {
        Challenge {
            target: account(target),
            block: 4,
            record: RecordRef {
                block: 4,
                index: 1,
                collection: [0u8; 16],
                key: Default::default(),
            },
            issued: deadline - 3,
            deadline,
        }
    }

    #[test]
    fn the_node_answers_only_its_own_open_challenges() {
        let local = vec![account(1)];
        let mut answered = BTreeSet::new();
        assert!(challenge_to_answer(&local, None, &answered, 10).is_none());
        let mine = Some((7, challenge(1, 13)));
        assert_eq!(
            challenge_to_answer(&local, mine.clone(), &answered, 10).map(|(id, _)| id),
            Some(7)
        );
        // Another validator's challenge.
        assert!(challenge_to_answer(&local, Some((8, challenge(2, 13))), &answered, 10).is_none());
        // Past the deadline.
        assert!(challenge_to_answer(&local, mine.clone(), &answered, 14).is_none());
        // Already answered.
        answered.insert(7);
        assert!(challenge_to_answer(&local, mine, &answered, 10).is_none());
    }

    #[test]
    fn the_answer_comes_from_the_local_entries_and_verifies() {
        let entries: Vec<Vec<u8>> = (0..5u8).map(|byte| vec![byte; 40]).collect();
        let parent = [9u8; 32];
        let store: std::collections::HashMap<Vec<u8>, Vec<u8>> = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                (
                    pallet_cerulea_custody::primitives::local_entry_key(4, &parent, index as u32),
                    entry.clone(),
                )
            })
            .collect();
        let read = local_entries(|key| store.get(key).cloned(), 4, &parent, 5)
            .expect("every entry sits in the store");
        assert_eq!(read, entries);
        // A missing entry: the node lacks the history.
        assert!(local_entries(|key| store.get(key).cloned(), 4, &parent, 6).is_none());
        // Another fork at the same height holds nothing here.
        assert!(local_entries(|key| store.get(key).cloned(), 4, &[8u8; 32], 5).is_none());

        let root = pallet_cerulea_custody::respond::local_record_root(&entries);
        let answer = build_answer(&read, 3, root).expect("the answer builds");
        assert!(pallet_cerulea_custody::verify::verify_against_root(
            root,
            3,
            &answer.value,
            &answer.proof
        )
        .is_ok());
        // A copy that differs from the chain answers nothing.
        assert!(build_answer(&read, 3, H256::repeat_byte(1)).is_err());
        // A position past the block.
        assert!(build_answer(&read, 9, root).is_err());
    }

    #[test]
    fn storage_keys_follow_the_runtime_layout() {
        let plain = value_key(b"Custody", b"PublishedChallenge");
        assert_eq!(plain.len(), 32);
        let entry = map_key(b"Custody", b"RecordCounts", &4u32.encode());
        assert_eq!(&entry[..32], &value_key(b"Custody", b"RecordCounts")[..]);
        assert_eq!(&entry[48..], &4u32.encode()[..]);
    }

    #[test]
    fn the_answer_call_names_the_challenge() {
        let entries: Vec<Vec<u8>> = (0..2u8).map(|byte| vec![byte; 8]).collect();
        let root = pallet_cerulea_custody::respond::local_record_root(&entries);
        let answer = build_answer(&entries, 0, root).expect("the answer builds");
        match answer_call(11, answer.clone()) {
            runtime::RuntimeCall::Custody(pallet_cerulea_custody::Call::answer_challenge {
                id,
                answer: carried,
            }) => {
                assert_eq!(id, 11);
                assert_eq!(carried, answer);
            }
            _ => panic!("the call answers a custody challenge"),
        }
    }
}
