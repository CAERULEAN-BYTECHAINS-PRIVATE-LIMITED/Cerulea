//! The Cerulea collections module.
//!
//! A collection declares how the Blockchain holds its records. Four
//! privacy classes cover the range.
//!
//! 1. **Open.** The record sits on the Blockchain in plain form. Every
//!    caller the permission module admits reads it.
//! 2. **Hash only.** The Blockchain holds a commitment while the payload
//!    lives elsewhere. A holder proves a payload by presenting it against
//!    the commitment.
//! 3. **Encrypted.** The Blockchain holds the record sealed under the
//!    content key of the collection. The content key reaches a role holder
//!    wrapped under the role key.
//! 4. **External.** The Blockchain holds a commitment and the payload
//!    lives in a declared external store.
//!
//! A class change rewrites every record of the collection into the new
//! form and rotates the key. The change runs in pages, so a collection of
//! any size moves inside the block weight the runtime allows. The class
//! takes effect when the last page lands.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod keys;
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;
pub use weights::WeightInfo;

use alloc::{vec, vec::Vec};
use cerulea_blueprint::{collections::PrivacyClass, types::CollectionId};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use pallet_cerulea_schema::{
    primitives::{name_of, BoundedName, CollectionKey, DeploymentId, RecordKey},
    registry::SchemaRegistry,
};
use pallet_cerulea_state::RecordStore;

/// The privacy class of one collection, as the Blockchain stores it.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum PrivacyLevel {
    /// The record sits on the Blockchain in plain form.
    Open,
    /// The Blockchain holds a commitment and the payload lives elsewhere.
    HashOnly,
    /// The Blockchain holds the record sealed under the content key.
    Encrypted,
    /// The Blockchain holds a commitment and the payload lives in a
    /// declared external store.
    External,
}

impl PrivacyLevel {
    /// A short label for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            PrivacyLevel::Open => "open",
            PrivacyLevel::HashOnly => "hash only",
            PrivacyLevel::Encrypted => "encrypted",
            PrivacyLevel::External => "external",
        }
    }

    /// Report whether the class keeps the record readable on the
    /// Blockchain.
    pub fn is_plain(&self) -> bool {
        matches!(self, PrivacyLevel::Open)
    }

    /// Report whether the class seals the record under a content key.
    pub fn is_sealed(&self) -> bool {
        matches!(self, PrivacyLevel::Encrypted)
    }

    /// Report whether the class holds a commitment while the payload lives
    /// elsewhere.
    pub fn is_commitment(&self) -> bool {
        matches!(self, PrivacyLevel::HashOnly | PrivacyLevel::External)
    }

    /// The class one Blueprint declaration names.
    pub fn of(declared: &PrivacyClass) -> PrivacyLevel {
        match declared {
            PrivacyClass::Open => PrivacyLevel::Open,
            PrivacyClass::HashOnly => PrivacyLevel::HashOnly,
            PrivacyClass::Encrypted { .. } => PrivacyLevel::Encrypted,
            PrivacyClass::External { .. } => PrivacyLevel::External,
        }
    }
}

/// A class change part way through its pages.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct PendingChange {
    /// The class the collection takes when the last page lands.
    pub target: PrivacyLevel,
    /// The class the collection held when the change started.
    pub from: PrivacyLevel,
    /// The key epoch the change moves to.
    pub epoch: u32,
    /// The last record key the change rewrote.
    pub cursor: Option<RecordKey>,
    /// The number of records the change has rewritten.
    pub rewritten: u32,
}

/// The read a dependant module performs over the privacy classes.
pub trait PrivacyRegistry {
    /// The class one collection carries.
    fn class(deployment: &DeploymentId, collection: &CollectionId) -> PrivacyLevel;

    /// The key epoch one collection sits at.
    fn key_epoch(deployment: &DeploymentId, collection: &CollectionId) -> u32;

    /// The roles that hold the content key of one collection.
    fn key_holders(deployment: &DeploymentId, collection: &CollectionId) -> Vec<BoundedName>;

    /// The commitment one record carries, where the class holds one.
    fn commitment(
        deployment: &DeploymentId,
        collection: &CollectionId,
        key: &RecordKey,
    ) -> Option<[u8; 32]>;

    /// Report whether one payload matches the stored commitment.
    fn verify_commitment(
        deployment: &DeploymentId,
        collection: &CollectionId,
        key: &RecordKey,
        payload: &[u8],
    ) -> bool;

    /// Report whether a class change is part way through its pages.
    fn change_pending(deployment: &DeploymentId, collection: &CollectionId) -> bool;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The settings the runtime supplies to the collections module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The registry that holds the collection definitions.
        type Schema: SchemaRegistry;

        /// The store that holds the records.
        type Records: RecordStore<AccountId = Self::AccountId>;

        /// The largest number of records one class change page rewrites.
        #[pallet::constant]
        type MaxRecordsPerPage: Get<u32>;

        /// The largest number of roles that hold the content key of one
        /// collection.
        #[pallet::constant]
        type MaxKeyHolders: Get<u32>;

        /// The largest wrapped content key, in bytes.
        #[pallet::constant]
        type MaxWrappedKeyBytes: Get<u32>;

        /// The largest payload one commitment check carries, in bytes.
        #[pallet::constant]
        type MaxPayloadBytes: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The privacy class every collection carries.
    #[pallet::storage]
    pub type Classes<T: Config> =
        StorageMap<_, Blake2_128Concat, CollectionKey, PrivacyLevel, OptionQuery>;

    /// The class change part way through its pages, per collection.
    #[pallet::storage]
    pub type Changes<T: Config> =
        StorageMap<_, Blake2_128Concat, CollectionKey, PendingChange, OptionQuery>;

    /// The key epoch every collection sits at.
    #[pallet::storage]
    pub type KeyEpochs<T: Config> = StorageMap<_, Blake2_128Concat, CollectionKey, u32, ValueQuery>;

    /// The hash of the content key of every key epoch.
    #[pallet::storage]
    pub type ContentKeyHashes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        CollectionKey,
        Twox64Concat,
        u32,
        [u8; 32],
        OptionQuery,
    >;

    /// The roles that hold the content key of every collection.
    #[pallet::storage]
    pub type KeyHolders<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        CollectionKey,
        BoundedVec<BoundedName, T::MaxKeyHolders>,
        ValueQuery,
    >;

    /// The wrapped content key of every role at every key epoch.
    #[pallet::storage]
    pub type WrappedKeys<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, CollectionKey>,
            NMapKey<Blake2_128Concat, BoundedName>,
            NMapKey<Twox64Concat, u32>,
        ),
        BoundedVec<u8, T::MaxWrappedKeyBytes>,
        OptionQuery,
    >;

    /// The hash of the role key each role registered, per key epoch.
    #[pallet::storage]
    pub type RoleKeyHashes<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, CollectionKey>,
            NMapKey<Blake2_128Concat, BoundedName>,
            NMapKey<Twox64Concat, u32>,
        ),
        [u8; 32],
        OptionQuery,
    >;

    /// The commitment every record of a commitment class carries.
    #[pallet::storage]
    pub type Commitments<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        CollectionKey,
        Blake2_128Concat,
        RecordKey,
        [u8; 32],
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A class change started and rewrote its first page.
        ClassChangeStarted {
            /// The deployment.
            deployment: DeploymentId,
            /// The collection.
            collection: BoundedName,
            /// The class the collection held.
            from: PrivacyLevel,
            /// The class the collection takes.
            to: PrivacyLevel,
        },
        /// A class change rewrote a further page.
        ClassChangeAdvanced {
            /// The deployment.
            deployment: DeploymentId,
            /// The collection.
            collection: BoundedName,
            /// The number of records the change has rewritten.
            rewritten: u32,
        },
        /// A class change rewrote its last page and took effect.
        ClassChanged {
            /// The deployment.
            deployment: DeploymentId,
            /// The collection.
            collection: BoundedName,
            /// The class the collection now carries.
            class: PrivacyLevel,
            /// The key epoch the collection now sits at.
            epoch: u32,
            /// The number of records the change rewrote.
            rewritten: u32,
        },
        /// A role gained the content key of one collection.
        KeyDistributed {
            /// The deployment.
            deployment: DeploymentId,
            /// The collection.
            collection: BoundedName,
            /// The role.
            role: BoundedName,
            /// The key epoch.
            epoch: u32,
        },
        /// A role lost the content key of one collection.
        AccessRevoked {
            /// The deployment.
            deployment: DeploymentId,
            /// The collection.
            collection: BoundedName,
            /// The role.
            role: BoundedName,
            /// The key epoch.
            epoch: u32,
        },
        /// A payload matched the commitment the Blockchain holds.
        CommitmentVerified {
            /// The deployment.
            deployment: DeploymentId,
            /// The collection.
            collection: BoundedName,
            /// The record key.
            key: RecordKey,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The name runs past 128 bytes. Shorten the collection name or
        /// the role name.
        NameTooLong,
        /// The registry lacks that collection. Register the collection
        /// before setting its class.
        CollectionUnknown,
        /// A class change is already part way through its pages. Advance
        /// the change to its last page before starting another.
        ChangePending,
        /// No class change is part way through its pages. Start the change
        /// before advancing it.
        ChangeAbsent,
        /// The call supplies no content key for a sealed class. Supply the
        /// content key of the collection.
        ContentKeyMissing,
        /// The supplied content key disagrees with the key the Blockchain
        /// holds the hash of. Supply the content key of that key epoch.
        ContentKeyMismatch,
        /// The supplied role key disagrees with the key the role
        /// registered. Supply the role key of that key epoch.
        RoleKeyMismatch,
        /// The record failed to open under the supplied key. Supply the
        /// content key of the key epoch the record was sealed at.
        SealUnreadable,
        /// A commitment class holds no payload on the Blockchain, so the
        /// records cannot return to plain form. Load the payloads again
        /// through an import.
        PayloadAbsent,
        /// The collection names more key holder roles than the runtime
        /// admits. Remove a role, or raise the limit in the runtime.
        HolderLimitReached,
        /// The wrapped key runs past the wrapped key limit. Raise the
        /// limit in the runtime.
        WrappedKeyTooLarge,
        /// The Blockchain holds no commitment under that record key.
        /// Set the collection to a commitment class first.
        CommitmentAbsent,
        /// The payload disagrees with the commitment the Blockchain holds.
        /// Present the payload the commitment was taken over.
        CommitmentMismatch,
        /// The Blockchain holds no record under that key. Insert the
        /// record before sealing it.
        RecordAbsent,
        /// The role holds no wrapped content key at that key epoch. Ask
        /// the steward to distribute the key to the role.
        RoleWithoutKey,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Start a privacy class change and rewrite its first page.
        ///
        /// A change to a sealed class supplies the new content key. A
        /// change away from a sealed class supplies the present content
        /// key. A change between two sealed epochs supplies both, which
        /// rotates the key and re-seals every record.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_class())]
        pub fn set_class(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            collection: BoundedName,
            class: PrivacyLevel,
            holders: BoundedVec<BoundedName, T::MaxKeyHolders>,
            present_key: Option<[u8; 32]>,
            next_key: Option<[u8; 32]>,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let id = Self::collection_id(&collection)?;
            let slot = CollectionKey::new(deployment, collection.clone());
            ensure!(
                T::Schema::collection(&deployment, &id).is_some(),
                Error::<T>::CollectionUnknown
            );
            ensure!(
                Changes::<T>::get(&slot).is_none(),
                Error::<T>::ChangePending
            );

            let from = Self::level(&deployment, &id);
            ensure!(
                !(from.is_commitment() && class.is_plain()),
                Error::<T>::PayloadAbsent
            );
            if class.is_sealed() {
                ensure!(next_key.is_some(), Error::<T>::ContentKeyMissing);
            }
            if from.is_sealed() {
                ensure!(present_key.is_some(), Error::<T>::ContentKeyMissing);
                Self::check_content_key(&slot, KeyEpochs::<T>::get(&slot), present_key)?;
            }

            let epoch = KeyEpochs::<T>::get(&slot).saturating_add(1);
            KeyHolders::<T>::insert(&slot, holders);
            if let Some(held) = next_key {
                ContentKeyHashes::<T>::insert(&slot, epoch, keys::key_commitment(&held));
            }
            Changes::<T>::insert(
                &slot,
                PendingChange {
                    target: class,
                    from,
                    epoch,
                    cursor: None,
                    rewritten: 0,
                },
            );
            Self::deposit_event(Event::ClassChangeStarted {
                deployment,
                collection: collection.clone(),
                from,
                to: class,
            });
            Self::run_page(&deployment, &id, &slot, present_key, next_key)
        }

        /// Rewrite the next page of a class change.
        ///
        /// The class takes effect when the last page lands.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::advance_class_change())]
        pub fn advance_class_change(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            collection: BoundedName,
            present_key: Option<[u8; 32]>,
            next_key: Option<[u8; 32]>,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let id = Self::collection_id(&collection)?;
            let slot = CollectionKey::new(deployment, collection);
            ensure!(Changes::<T>::get(&slot).is_some(), Error::<T>::ChangeAbsent);
            Self::run_page(&deployment, &id, &slot, present_key, next_key)
        }

        /// Give one role the content key of one collection.
        ///
        /// The call supplies the role key and the content key. The
        /// Blockchain keeps the wrapped content key and the hash of each
        /// supplied key, and keeps neither key in plain form.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::distribute_key())]
        pub fn distribute_key(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            collection: BoundedName,
            role: BoundedName,
            role_key: [u8; 32],
            content_key: [u8; 32],
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let slot = CollectionKey::new(deployment, collection.clone());
            let epoch = KeyEpochs::<T>::get(&slot);
            Self::check_content_key(&slot, epoch, Some(content_key))?;
            if let Some(held) = RoleKeyHashes::<T>::get((slot.clone(), role.clone(), epoch)) {
                ensure!(
                    held == keys::key_commitment(&role_key),
                    Error::<T>::RoleKeyMismatch
                );
            }

            let wrapped = keys::wrap(&role_key, &slot, epoch, role.as_slice(), &content_key);
            let bounded: BoundedVec<u8, T::MaxWrappedKeyBytes> =
                BoundedVec::try_from(wrapped).map_err(|_| Error::<T>::WrappedKeyTooLarge)?;
            WrappedKeys::<T>::insert((slot.clone(), role.clone(), epoch), bounded);
            RoleKeyHashes::<T>::insert(
                (slot.clone(), role.clone(), epoch),
                keys::key_commitment(&role_key),
            );
            KeyHolders::<T>::try_mutate(&slot, |held| -> DispatchResult {
                if !held.contains(&role) {
                    held.try_push(role.clone())
                        .map_err(|_| Error::<T>::HolderLimitReached)?;
                }
                Ok(())
            })?;

            Self::deposit_event(Event::KeyDistributed {
                deployment,
                collection,
                role,
                epoch,
            });
            Ok(())
        }

        /// Take the content key of one collection away from one role.
        ///
        /// The wrapped key leaves the Blockchain at once. A rotation gives
        /// the remaining roles a fresh content key, so the revoked role
        /// reads nothing written after the rotation.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::revoke_access())]
        pub fn revoke_access(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            collection: BoundedName,
            role: BoundedName,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let slot = CollectionKey::new(deployment, collection.clone());
            let epoch = KeyEpochs::<T>::get(&slot);
            ensure!(
                WrappedKeys::<T>::contains_key((slot.clone(), role.clone(), epoch)),
                Error::<T>::RoleWithoutKey
            );
            WrappedKeys::<T>::remove((slot.clone(), role.clone(), epoch));
            RoleKeyHashes::<T>::remove((slot.clone(), role.clone(), epoch));
            KeyHolders::<T>::mutate(&slot, |held| {
                held.retain(|present| present != &role);
            });
            Self::deposit_event(Event::AccessRevoked {
                deployment,
                collection,
                role,
                epoch,
            });
            Ok(())
        }

        /// Check one payload against the commitment the Blockchain holds.
        ///
        /// A matching payload writes the check into the Blockchain record,
        /// so an auditor reads when a payload was last presented.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::verify_commitment())]
        pub fn verify_commitment(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            collection: BoundedName,
            key: RecordKey,
            payload: BoundedVec<u8, T::MaxPayloadBytes>,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let slot = CollectionKey::new(deployment, collection.clone());
            let held = Commitments::<T>::get(&slot, &key).ok_or(Error::<T>::CommitmentAbsent)?;
            ensure!(
                held == keys::commitment(payload.as_slice()),
                Error::<T>::CommitmentMismatch
            );
            Self::deposit_event(Event::CommitmentVerified {
                deployment,
                collection,
                key,
            });
            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    fn collection_id(name: &BoundedName) -> Result<CollectionId, DispatchError> {
        let text = alloc::string::String::from_utf8(name.to_vec())
            .map_err(|_| Error::<T>::CollectionUnknown)?;
        Ok(CollectionId::new(&text))
    }

    /// The class one collection carries.
    ///
    /// A collection that has taken no class change carries the class its
    /// Blueprint declares.
    pub fn level(deployment: &DeploymentId, collection: &CollectionId) -> PrivacyLevel {
        let name = match name_of(collection.as_str()) {
            Ok(held) => held,
            Err(_) => return PrivacyLevel::Open,
        };
        let slot = CollectionKey::new(*deployment, name);
        match Classes::<T>::get(&slot) {
            Some(held) => held,
            None => T::Schema::collection(deployment, collection)
                .map(|held| PrivacyLevel::of(&held.privacy))
                .unwrap_or(PrivacyLevel::Open),
        }
    }

    fn check_content_key(
        slot: &CollectionKey,
        epoch: u32,
        supplied: Option<[u8; 32]>,
    ) -> DispatchResult {
        let held = match ContentKeyHashes::<T>::get(slot, epoch) {
            Some(found) => found,
            None => return Ok(()),
        };
        let supplied = supplied.ok_or(Error::<T>::ContentKeyMissing)?;
        ensure!(
            held == keys::key_commitment(&supplied),
            Error::<T>::ContentKeyMismatch
        );
        Ok(())
    }

    fn run_page(
        deployment: &DeploymentId,
        id: &CollectionId,
        slot: &CollectionKey,
        present_key: Option<[u8; 32]>,
        next_key: Option<[u8; 32]>,
    ) -> DispatchResult {
        let mut change = Changes::<T>::get(slot).ok_or(Error::<T>::ChangeAbsent)?;
        if change.from.is_sealed() {
            Self::check_content_key(slot, KeyEpochs::<T>::get(slot), present_key)?;
        }
        if change.target.is_sealed() {
            Self::check_content_key(slot, change.epoch, next_key)?;
        }

        let page = T::Records::keys_after(
            deployment,
            id,
            change.cursor.as_ref(),
            T::MaxRecordsPerPage::get(),
        );
        let present_epoch = KeyEpochs::<T>::get(slot);

        for key in page.iter() {
            let stored =
                T::Records::read_bytes(deployment, id, key).ok_or(Error::<T>::RecordAbsent)?;
            let author = T::Records::meta(deployment, id, key)
                .ok_or(Error::<T>::RecordAbsent)?
                .author;
            let plain = Self::to_plain(slot, &change, present_epoch, key, &stored, present_key)?;
            let produced = Self::to_target(slot, &change, key, &plain, next_key)?;
            if change.target.is_commitment() {
                Commitments::<T>::insert(slot, key, keys::commitment(&plain));
            } else {
                Commitments::<T>::remove(slot, key);
            }
            T::Records::write_bytes(deployment, id, &author, key, produced)?;
            change.cursor = Some(key.clone());
            change.rewritten = change.rewritten.saturating_add(1);
        }

        let finished = page.len() < T::MaxRecordsPerPage::get() as usize;
        if finished {
            Classes::<T>::insert(slot, change.target);
            KeyEpochs::<T>::insert(slot, change.epoch);
            Changes::<T>::remove(slot);
            Self::deposit_event(Event::ClassChanged {
                deployment: *deployment,
                collection: slot.name.clone(),
                class: change.target,
                epoch: change.epoch,
                rewritten: change.rewritten,
            });
        } else {
            let rewritten = change.rewritten;
            Changes::<T>::insert(slot, change);
            Self::deposit_event(Event::ClassChangeAdvanced {
                deployment: *deployment,
                collection: slot.name.clone(),
                rewritten,
            });
        }
        Ok(())
    }

    fn to_plain(
        slot: &CollectionKey,
        change: &PendingChange,
        present_epoch: u32,
        key: &RecordKey,
        stored: &[u8],
        present_key: Option<[u8; 32]>,
    ) -> Result<Vec<u8>, DispatchError> {
        if !change.from.is_sealed() {
            return Ok(stored.to_vec());
        }
        let content = present_key.ok_or(Error::<T>::ContentKeyMissing)?;
        let nonce = keys::nonce_for(slot, present_epoch, key);
        keys::open(&content, &nonce, stored).ok_or_else(|| Error::<T>::SealUnreadable.into())
    }

    fn to_target(
        slot: &CollectionKey,
        change: &PendingChange,
        key: &RecordKey,
        plain: &[u8],
        next_key: Option<[u8; 32]>,
    ) -> Result<Vec<u8>, DispatchError> {
        match change.target {
            PrivacyLevel::Open => Ok(plain.to_vec()),
            PrivacyLevel::HashOnly | PrivacyLevel::External => Ok(keys::commitment(plain).to_vec()),
            PrivacyLevel::Encrypted => {
                let content = next_key.ok_or(Error::<T>::ContentKeyMissing)?;
                let nonce = keys::nonce_for(slot, change.epoch, key);
                Ok(keys::seal(&content, &nonce, plain))
            }
        }
    }

    /// Open one sealed record with one role key.
    ///
    /// The call unwraps the content key from the wrapped key the role
    /// holds and then opens the record. A caller without the role key
    /// reads the sealed bytes alone.
    pub fn open_record(
        deployment: &DeploymentId,
        collection: &CollectionId,
        key: &RecordKey,
        role: &BoundedName,
        role_key: &[u8; 32],
    ) -> Result<Vec<u8>, DispatchError> {
        let name = name_of(collection.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
        let slot = CollectionKey::new(*deployment, name);
        let epoch = KeyEpochs::<T>::get(&slot);
        let stored =
            T::Records::read_bytes(deployment, collection, key).ok_or(Error::<T>::RecordAbsent)?;
        if !Self::level(deployment, collection).is_sealed() {
            return Ok(stored);
        }
        let wrapped = WrappedKeys::<T>::get((slot.clone(), role.clone(), epoch))
            .ok_or(Error::<T>::RoleWithoutKey)?;
        let content = keys::unwrap(role_key, &slot, epoch, role.as_slice(), wrapped.as_slice())
            .ok_or(Error::<T>::RoleKeyMismatch)?;
        let nonce = keys::nonce_for(&slot, epoch, key);
        keys::open(&content, &nonce, &stored).ok_or_else(|| Error::<T>::SealUnreadable.into())
    }

    /// The wrapped content key one role holds at the live key epoch.
    pub fn wrapped_key(
        deployment: &DeploymentId,
        collection: &CollectionId,
        role: &BoundedName,
    ) -> Option<Vec<u8>> {
        let name = name_of(collection.as_str()).ok()?;
        let slot = CollectionKey::new(*deployment, name);
        let epoch = KeyEpochs::<T>::get(&slot);
        WrappedKeys::<T>::get((slot, role.clone(), epoch)).map(|held| held.into_inner())
    }
}

impl<T: Config> PrivacyRegistry for Pallet<T> {
    fn class(deployment: &DeploymentId, collection: &CollectionId) -> PrivacyLevel {
        Pallet::<T>::level(deployment, collection)
    }

    fn key_epoch(deployment: &DeploymentId, collection: &CollectionId) -> u32 {
        match name_of(collection.as_str()) {
            Ok(name) => KeyEpochs::<T>::get(CollectionKey::new(*deployment, name)),
            Err(_) => 0,
        }
    }

    fn key_holders(deployment: &DeploymentId, collection: &CollectionId) -> Vec<BoundedName> {
        match name_of(collection.as_str()) {
            Ok(name) => KeyHolders::<T>::get(CollectionKey::new(*deployment, name)).into_inner(),
            Err(_) => Vec::new(),
        }
    }

    fn commitment(
        deployment: &DeploymentId,
        collection: &CollectionId,
        key: &RecordKey,
    ) -> Option<[u8; 32]> {
        let name = name_of(collection.as_str()).ok()?;
        Commitments::<T>::get(CollectionKey::new(*deployment, name), key)
    }

    fn verify_commitment(
        deployment: &DeploymentId,
        collection: &CollectionId,
        key: &RecordKey,
        payload: &[u8],
    ) -> bool {
        match <Pallet<T> as PrivacyRegistry>::commitment(deployment, collection, key) {
            Some(held) => held == keys::commitment(payload),
            None => false,
        }
    }

    fn change_pending(deployment: &DeploymentId, collection: &CollectionId) -> bool {
        match name_of(collection.as_str()) {
            Ok(name) => Changes::<T>::contains_key(CollectionKey::new(*deployment, name)),
            Err(_) => false,
        }
    }
}
