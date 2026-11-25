//! The bounded identifiers every platform module shares.
//!
//! The Blueprint carries names as text of any length. Blockchain storage
//! carries names as bounded byte strings. This module holds the bounded
//! forms and the conversions between the two.
//!
//! Every platform module keys its storage with the types here. The schema
//! module sits at the root of the module graph, so the shared types live
//! here and every dependant reads them from one place.

use alloc::{string::String, vec::Vec};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{traits::ConstU32, BoundedVec};
use scale_info::TypeInfo;

/// The largest identifier the Blockchain stores, in bytes.
///
/// The Blueprint limits an identifier to 128 characters. A byte limit of
/// 128 matches that limit for the ASCII identifiers the Blueprint admits.
pub const MAX_NAME_BYTES: u32 = 128;

/// The largest record key the Blockchain stores, in bytes.
///
/// A record key is the canonical encoding of the primary key value. A text
/// key of 256 bytes covers every realistic key.
pub const MAX_KEY_BYTES: u32 = 256;

/// The largest index value the Blockchain stores, in bytes.
pub const MAX_INDEX_VALUE_BYTES: u32 = 256;

/// A bounded identifier.
pub type BoundedName = BoundedVec<u8, ConstU32<MAX_NAME_BYTES>>;

/// A bounded record key, holding the canonical encoding of a key value.
pub type RecordKey = BoundedVec<u8, ConstU32<MAX_KEY_BYTES>>;

/// A bounded index value, holding the canonical encoding of the indexed
/// fields.
pub type IndexValue = BoundedVec<u8, ConstU32<MAX_INDEX_VALUE_BYTES>>;

/// The identifier of one deployment.
///
/// A deployment is one installed Blueprint on one chain. The identifier is
/// the content address of the Blueprint the deployment installed.
pub type DeploymentId = [u8; 32];

/// The content hash of one registered type definition.
pub type TypeHash = [u8; 32];

/// The outcome of a name that runs past the bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NameTooLong;

/// Turn text into a bounded identifier.
///
/// A name of 128 bytes or fewer converts. A longer name reports
/// [`NameTooLong`], and the caller names the fix in its own error.
pub fn name_of(text: &str) -> Result<BoundedName, NameTooLong> {
    BoundedVec::try_from(text.as_bytes().to_vec()).map_err(|_| NameTooLong)
}

/// Read a bounded identifier back as text.
///
/// Every identifier the Blueprint admits is ASCII, so the conversion keeps
/// the original characters. A byte string carrying other content converts
/// through the replacement character.
pub fn name_text(name: &BoundedName) -> String {
    String::from_utf8_lossy(name.as_slice()).into_owned()
}

/// Turn a record key value into its bounded canonical form.
pub fn record_key_of(bytes: Vec<u8>) -> Result<RecordKey, NameTooLong> {
    BoundedVec::try_from(bytes).map_err(|_| NameTooLong)
}

/// Turn an index value into its bounded canonical form.
///
/// An index value longer than the bound folds to its hash. Folding keeps
/// the index usable for equality lookups on long values.
pub fn index_value_of(bytes: Vec<u8>) -> IndexValue {
    match BoundedVec::try_from(bytes.clone()) {
        Ok(held) => held,
        Err(_) => {
            let digest = sp_crypto_hashing::blake2_256(&bytes);
            BoundedVec::try_from(digest.to_vec()).expect("a 32 byte digest sits inside the bound")
        }
    }
}

/// One collection inside one deployment.
///
/// The pair is the first key of every record map the state module holds.
#[derive(
    Clone,
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
pub struct CollectionKey {
    /// The deployment the collection belongs to.
    pub deployment: DeploymentId,
    /// The collection name.
    pub name: BoundedName,
}

impl CollectionKey {
    /// Build a collection key from a deployment and a bounded name.
    pub fn new(deployment: DeploymentId, name: BoundedName) -> Self {
        CollectionKey { deployment, name }
    }

    /// Build a collection key from a deployment and text.
    pub fn from_text(deployment: DeploymentId, name: &str) -> Result<Self, NameTooLong> {
        Ok(CollectionKey {
            deployment,
            name: name_of(name)?,
        })
    }

    /// Read the collection name as text.
    pub fn name_text(&self) -> String {
        name_text(&self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_name_converts_and_reads_back() {
        let name = name_of("parcel").expect("a six byte name fits");
        assert_eq!(name_text(&name), "parcel");
    }

    #[test]
    fn a_long_name_reports_the_bound() {
        let long = "p".repeat(MAX_NAME_BYTES as usize + 1);
        assert_eq!(name_of(&long), Err(NameTooLong));
    }

    #[test]
    fn a_long_index_value_folds_to_its_digest() {
        let long = alloc::vec![7u8; MAX_INDEX_VALUE_BYTES as usize + 1];
        let folded = index_value_of(long.clone());
        assert_eq!(folded.len(), 32);
        assert_eq!(folded.as_slice(), &sp_crypto_hashing::blake2_256(&long));
    }

    #[test]
    fn a_collection_key_carries_the_deployment_and_the_name() {
        let key = CollectionKey::from_text([3u8; 32], "parcel").expect("the name fits");
        assert_eq!(key.deployment, [3u8; 32]);
        assert_eq!(key.name_text(), "parcel");
    }

    #[test]
    fn a_record_key_inside_the_bound_converts_and_a_longer_one_reports_it() {
        let fits = alloc::vec![1u8; MAX_KEY_BYTES as usize];
        assert_eq!(
            record_key_of(fits.clone()).map(|held| held.into_inner()),
            Ok(fits)
        );
        let long = alloc::vec![1u8; MAX_KEY_BYTES as usize + 1];
        assert_eq!(record_key_of(long), Err(NameTooLong));
    }

    #[test]
    fn a_short_index_value_keeps_its_bytes() {
        let short = alloc::vec![4u8, 5, 6];
        assert_eq!(index_value_of(short.clone()).into_inner(), short);
    }

    #[test]
    fn a_collection_key_over_a_long_name_reports_the_bound() {
        let long = "c".repeat(MAX_NAME_BYTES as usize + 1);
        assert_eq!(CollectionKey::from_text([3u8; 32], &long), Err(NameTooLong));
    }

    #[test]
    fn a_name_carrying_bytes_outside_text_reads_through_the_replacement_character() {
        let name: BoundedName =
            BoundedVec::try_from(alloc::vec![b'a', 0xff, b'b']).expect("the name fits");
        assert_eq!(name_text(&name), "a\u{fffd}b");
    }
}
