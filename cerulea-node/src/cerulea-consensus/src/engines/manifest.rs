//! The Access Manifest.
//!
//! Every Cerulea transaction declares the state it reads and the state it
//! writes before execution begins. That declaration is the Access Manifest.
//! Cerulea derives the declaration from the Blueprint automatically. The person
//! who builds the deployment writes none of it.
//!
//! General Blockchains discover contract storage access by running the
//! contract. Blocks there execute in one serial pass and become irreversible as
//! one indivisible unit. Cerulea knows the access set ahead of execution. That
//! knowledge is what supports parallel execution and per-domain commitment.
//!
//! A manifest is a contract rather than a hint. Execution that reaches state
//! outside the manifest aborts that transaction.

use codec::{Decode, Encode};
use frame_support::traits::ConstU32;
use frame_support::BoundedVec;
use scale_info::TypeInfo;

use super::{CollectionId, EngineError, KeyRange, TransactionClass};

/// The highest number of collection accesses one side of a manifest carries.
///
/// An action that reaches past this limit splits into smaller actions.
pub const MAX_ACCESS: u32 = 64;

/// The highest number of bytes one key range carries.
pub const MAX_KEY_BYTES: usize = 128;

/// The bound the manifest lists carry.
pub type MaxAccess = ConstU32<MAX_ACCESS>;

/// One collection and the span of keys a transaction touches inside it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, TypeInfo, Default)]
pub struct CollectionAccess {
    /// The collection the access names.
    pub collection: CollectionId,
    /// The span of keys the access touches.
    pub keys: KeyRange,
}

impl CollectionAccess {
    /// Declare an access over one collection and one key span.
    pub fn new(collection: CollectionId, keys: KeyRange) -> Self {
        CollectionAccess { collection, keys }
    }

    /// Declare an access over one key of one collection.
    pub fn exact(collection: CollectionId, key: &[u8]) -> Self {
        CollectionAccess {
            collection,
            keys: KeyRange::Exact(key.to_vec()),
        }
    }

    /// Declare an access over every key beneath a prefix.
    pub fn prefix(collection: CollectionId, prefix: &[u8]) -> Self {
        CollectionAccess {
            collection,
            keys: KeyRange::Prefix(prefix.to_vec()),
        }
    }

    /// Declare an access over a whole collection.
    pub fn full(collection: CollectionId) -> Self {
        CollectionAccess {
            collection,
            keys: KeyRange::Full,
        }
    }

    /// Report whether this access holds the given key of the given collection.
    pub fn covers(&self, collection: &CollectionId, key: &[u8]) -> bool {
        &self.collection == collection && self.keys.covers(key)
    }

    /// Report whether this access and another access share at least one key.
    pub fn overlaps(&self, other: &CollectionAccess) -> bool {
        self.collection == other.collection && self.keys.overlaps(&other.keys)
    }

    /// Check the key span against the byte limit.
    pub fn validate(&self) -> Result<(), EngineError> {
        let length = self.keys.byte_len();
        if length > MAX_KEY_BYTES {
            return Err(EngineError::KeyRangeTooLong {
                length,
                limit: MAX_KEY_BYTES,
            });
        }
        Ok(())
    }
}

/// The state one transaction declares before execution begins.
///
/// The manifest carries the collections read, the collections written, the key
/// spans touched inside each collection, and the transaction class. The
/// Execution Engine reads the write side to group transactions into conflict
/// domains. The manifest guard reads both sides during execution. The Assurance
/// Engine reads the class to pick the threshold.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Default)]
pub struct AccessManifest {
    /// The collections and key spans the transaction reads.
    pub reads: BoundedVec<CollectionAccess, MaxAccess>,
    /// The collections and key spans the transaction writes.
    pub writes: BoundedVec<CollectionAccess, MaxAccess>,
    /// The class the Blueprint action carries.
    pub class: TransactionClass,
}

impl AccessManifest {
    /// Start an empty manifest for one transaction class.
    pub fn new(class: TransactionClass) -> Self {
        AccessManifest {
            reads: BoundedVec::default(),
            writes: BoundedVec::default(),
            class,
        }
    }

    /// Build a manifest from two lists of accesses.
    ///
    /// The build fails where either list runs past the access limit or a key
    /// span runs past the byte limit.
    pub fn build(
        class: TransactionClass,
        reads: Vec<CollectionAccess>,
        writes: Vec<CollectionAccess>,
    ) -> Result<Self, EngineError> {
        let mut manifest = AccessManifest::new(class);
        for access in reads {
            manifest.declare_read(access)?;
        }
        for access in writes {
            manifest.declare_write(access)?;
        }
        Ok(manifest)
    }

    /// Add one read declaration.
    pub fn declare_read(&mut self, access: CollectionAccess) -> Result<(), EngineError> {
        access.validate()?;
        let count = self.reads.len();
        self.reads
            .try_push(access)
            .map_err(|_| EngineError::ManifestTooLarge {
                count: count.saturating_add(1),
                limit: MAX_ACCESS as usize,
            })
    }

    /// Add one write declaration.
    pub fn declare_write(&mut self, access: CollectionAccess) -> Result<(), EngineError> {
        access.validate()?;
        let count = self.writes.len();
        self.writes
            .try_push(access)
            .map_err(|_| EngineError::ManifestTooLarge {
                count: count.saturating_add(1),
                limit: MAX_ACCESS as usize,
            })
    }

    /// Report whether the manifest permits a read of the given key.
    ///
    /// A declared write permits a read of the same key. Execution reads a
    /// record before it replaces the record, so the write declaration carries
    /// the read with it.
    pub fn permits_read(&self, collection: &CollectionId, key: &[u8]) -> bool {
        self.reads
            .iter()
            .any(|access| access.covers(collection, key))
            || self.permits_write(collection, key)
    }

    /// Report whether the manifest permits a write of the given key.
    pub fn permits_write(&self, collection: &CollectionId, key: &[u8]) -> bool {
        self.writes
            .iter()
            .any(|access| access.covers(collection, key))
    }

    /// Report whether the written state of two manifests overlaps.
    ///
    /// The conflict domain partitioner joins two transactions into one domain
    /// where this check holds.
    pub fn writes_intersect(&self, other: &AccessManifest) -> bool {
        self.writes
            .iter()
            .any(|mine| other.writes.iter().any(|theirs| mine.overlaps(theirs)))
    }

    /// Report whether the written state of one manifest meets the read state of
    /// another manifest.
    ///
    /// A read that follows a write in the same block sees the written value.
    /// Two such transactions share one domain so the order stays settled.
    pub fn touches(&self, other: &AccessManifest) -> bool {
        if self.writes_intersect(other) {
            return true;
        }
        let write_meets_read = |writer: &AccessManifest, reader: &AccessManifest| {
            writer
                .writes
                .iter()
                .any(|write| reader.reads.iter().any(|read| write.overlaps(read)))
        };
        write_meets_read(self, other) || write_meets_read(other, self)
    }

    /// The collections this manifest writes.
    pub fn written_collections(&self) -> Vec<CollectionId> {
        let mut collections: Vec<CollectionId> =
            self.writes.iter().map(|access| access.collection).collect();
        collections.sort_unstable();
        collections.dedup();
        collections
    }

    /// The number of accesses the manifest declares across both sides.
    pub fn len(&self) -> usize {
        self.reads.len().saturating_add(self.writes.len())
    }

    /// Report whether the manifest declares zero accesses.
    pub fn is_empty(&self) -> bool {
        self.reads.is_empty() && self.writes.is_empty()
    }

    /// Check every declaration against the limits.
    pub fn validate(&self) -> Result<(), EngineError> {
        for access in self.reads.iter().chain(self.writes.iter()) {
            access.validate()?;
        }
        Ok(())
    }
}

/// A pending transaction that declares its state access ahead of execution.
///
/// The Execution Engine reads this declaration to group transactions into
/// conflict domains and to settle the order inside each domain.
pub trait DeclaresAccess {
    /// The hash of the transaction.
    ///
    /// The ordering step compares this value last, so two transactions that
    /// match on class and arrival still take a settled order.
    fn transaction_hash(&self) -> [u8; 32];

    /// The position the transaction took in the arrival queue.
    fn arrival_sequence(&self) -> u64;

    /// The Access Manifest the admission step derived.
    fn manifest(&self) -> &AccessManifest;

    /// The class the Blueprint action carries.
    fn class(&self) -> TransactionClass {
        self.manifest().class
    }
}

/// One pending transaction held for execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingTransaction {
    /// The hash of the transaction.
    pub hash: [u8; 32],
    /// The position the transaction took in the arrival queue.
    pub arrival: u64,
    /// The declaration the admission step derived.
    pub manifest: AccessManifest,
    /// The encoded transaction body the Execution Engine runs.
    pub body: Vec<u8>,
}

impl PendingTransaction {
    /// Hold one transaction for execution.
    pub fn new(hash: [u8; 32], arrival: u64, manifest: AccessManifest, body: Vec<u8>) -> Self {
        PendingTransaction {
            hash,
            arrival,
            manifest,
            body,
        }
    }
}

impl DeclaresAccess for PendingTransaction {
    fn transaction_hash(&self) -> [u8; 32] {
        self.hash
    }

    fn arrival_sequence(&self) -> u64 {
        self.arrival
    }

    fn manifest(&self) -> &AccessManifest {
        &self.manifest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parcel() -> CollectionId {
        CollectionId::from_name("parcel")
    }

    fn holder() -> CollectionId {
        CollectionId::from_name("holder")
    }

    fn transfer_manifest() -> AccessManifest {
        AccessManifest::build(
            TransactionClass::Financial,
            vec![CollectionAccess::exact(holder(), b"holder-1")],
            vec![CollectionAccess::exact(parcel(), b"parcel-1")],
        )
        .expect("the manifest sits inside every limit")
    }

    #[test]
    fn a_manifest_carries_reads_writes_and_a_class() {
        let manifest = transfer_manifest();
        assert_eq!(manifest.class, TransactionClass::Financial);
        assert_eq!(manifest.reads.len(), 1);
        assert_eq!(manifest.writes.len(), 1);
        assert_eq!(manifest.len(), 2);
        assert!(!manifest.is_empty());
        assert!(AccessManifest::new(TransactionClass::Routine).is_empty());
    }

    #[test]
    fn a_manifest_round_trips_through_scale() {
        let manifest = transfer_manifest();
        let bytes = manifest.encode();
        let back = AccessManifest::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(manifest, back);
    }

    #[test]
    fn scale_decoding_rejects_a_list_past_the_access_limit() {
        // The encoding carries a length prefix. A length past the bound fails
        // to decode, so an oversized manifest never reaches the pool.
        let oversized: Vec<CollectionAccess> = (0..(MAX_ACCESS as usize + 1))
            .map(|index| CollectionAccess::exact(parcel(), format!("key-{}", index).as_bytes()))
            .collect();
        let mut bytes = Vec::new();
        oversized.encode_to(&mut bytes);
        Vec::<CollectionAccess>::new().encode_to(&mut bytes);
        TransactionClass::Routine.encode_to(&mut bytes);
        assert!(AccessManifest::decode(&mut &bytes[..]).is_err());
    }

    #[test]
    fn a_manifest_past_the_access_limit_names_the_fix() {
        let mut manifest = AccessManifest::new(TransactionClass::Routine);
        for index in 0..MAX_ACCESS {
            manifest
                .declare_write(CollectionAccess::exact(
                    parcel(),
                    format!("key-{}", index).as_bytes(),
                ))
                .expect("the list holds this many accesses");
        }
        let outcome = manifest.declare_write(CollectionAccess::exact(parcel(), b"one-too-many"));
        let error = outcome.expect_err("the list rejects the extra access");
        assert_eq!(
            error,
            EngineError::ManifestTooLarge {
                count: MAX_ACCESS as usize + 1,
                limit: MAX_ACCESS as usize,
            }
        );
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn a_key_span_past_the_byte_limit_names_the_fix() {
        let long = vec![7u8; MAX_KEY_BYTES + 1];
        let mut manifest = AccessManifest::new(TransactionClass::Routine);
        let error = manifest
            .declare_read(CollectionAccess::new(parcel(), KeyRange::Exact(long)))
            .expect_err("the byte limit rejects the span");
        assert_eq!(
            error,
            EngineError::KeyRangeTooLong {
                length: MAX_KEY_BYTES + 1,
                limit: MAX_KEY_BYTES,
            }
        );
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn a_manifest_permits_the_state_it_declares() {
        let manifest = transfer_manifest();
        assert!(manifest.permits_write(&parcel(), b"parcel-1"));
        assert!(!manifest.permits_write(&parcel(), b"parcel-2"));
        assert!(manifest.permits_read(&holder(), b"holder-1"));
        assert!(!manifest.permits_read(&holder(), b"holder-2"));
        assert!(!manifest.permits_write(&holder(), b"holder-1"));
    }

    #[test]
    fn a_declared_write_permits_a_read_of_the_same_key() {
        let manifest = AccessManifest::build(
            TransactionClass::Routine,
            vec![],
            vec![CollectionAccess::exact(parcel(), b"parcel-1")],
        )
        .expect("the manifest builds");
        assert!(manifest.permits_read(&parcel(), b"parcel-1"));
        assert!(!manifest.permits_read(&parcel(), b"parcel-2"));
    }

    #[test]
    fn a_prefix_declaration_permits_every_key_beneath_it() {
        let manifest = AccessManifest::build(
            TransactionClass::Routine,
            vec![],
            vec![CollectionAccess::prefix(parcel(), b"north-")],
        )
        .expect("the manifest builds");
        assert!(manifest.permits_write(&parcel(), b"north-1"));
        assert!(manifest.permits_write(&parcel(), b"north-2"));
        assert!(!manifest.permits_write(&parcel(), b"south-1"));
    }

    #[test]
    fn two_manifests_writing_one_key_intersect() {
        let left = transfer_manifest();
        let right = AccessManifest::build(
            TransactionClass::Routine,
            vec![],
            vec![CollectionAccess::exact(parcel(), b"parcel-1")],
        )
        .expect("the manifest builds");
        let apart = AccessManifest::build(
            TransactionClass::Routine,
            vec![],
            vec![CollectionAccess::exact(parcel(), b"parcel-9")],
        )
        .expect("the manifest builds");
        assert!(left.writes_intersect(&right));
        assert!(right.writes_intersect(&left));
        assert!(!left.writes_intersect(&apart));
    }

    #[test]
    fn a_write_that_meets_a_read_counts_as_a_touch() {
        let writer = AccessManifest::build(
            TransactionClass::Routine,
            vec![],
            vec![CollectionAccess::exact(parcel(), b"parcel-1")],
        )
        .expect("the manifest builds");
        let reader = AccessManifest::build(
            TransactionClass::Routine,
            vec![CollectionAccess::exact(parcel(), b"parcel-1")],
            vec![CollectionAccess::exact(holder(), b"holder-4")],
        )
        .expect("the manifest builds");
        assert!(!writer.writes_intersect(&reader));
        assert!(writer.touches(&reader));
        assert!(reader.touches(&writer));
    }

    #[test]
    fn a_full_declaration_meets_every_key_of_that_collection() {
        let sweep = AccessManifest::build(
            TransactionClass::Structural,
            vec![],
            vec![CollectionAccess::full(parcel())],
        )
        .expect("the manifest builds");
        assert!(sweep.permits_write(&parcel(), b"anything"));
        assert!(!sweep.permits_write(&holder(), b"anything"));
        assert!(sweep.writes_intersect(&transfer_manifest()));
    }

    #[test]
    fn the_written_collections_read_back_once_each() {
        let manifest = AccessManifest::build(
            TransactionClass::Routine,
            vec![],
            vec![
                CollectionAccess::exact(parcel(), b"a"),
                CollectionAccess::exact(parcel(), b"b"),
                CollectionAccess::exact(holder(), b"c"),
            ],
        )
        .expect("the manifest builds");
        assert_eq!(manifest.written_collections().len(), 2);
    }

    #[test]
    fn a_pending_transaction_reads_back_its_declaration() {
        let pending = PendingTransaction::new([3u8; 32], 17, transfer_manifest(), vec![1, 2, 3]);
        assert_eq!(pending.transaction_hash(), [3u8; 32]);
        assert_eq!(pending.arrival_sequence(), 17);
        assert_eq!(pending.class(), TransactionClass::Financial);
        assert!(pending.manifest().permits_write(&parcel(), b"parcel-1"));
    }

    #[test]
    fn a_valid_manifest_passes_the_check() {
        assert!(transfer_manifest().validate().is_ok());
    }
}
