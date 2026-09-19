//! Challenge publication in the block header.
//!
//! The challenge appears in the block header as a digest item. Every node sees
//! the challenge at the same moment, and a node that joins later reads the
//! challenge from the header alone.
//!
//! The digest identifier is [`crate::primitives::CUSTODY_DIGEST_ID`]. The
//! node-side engines read the same identifier. The value lives in this pallet
//! so one crate owns it.

use crate::primitives::{Challenge, CUSTODY_DIGEST_ID};
use codec::{Decode, Encode};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;
use sp_runtime::generic::DigestItem;
use sp_std::vec::Vec;

/// A reason a digest item fails to read.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum PublishError {
    /// The item carries another engine identifier.
    OtherEngine,
    /// The item carries the custody identifier and bytes that fail to decode.
    PayloadUnreadable,
}

impl PublishError {
    /// The sentence a person reads, naming the fix.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            PublishError::OtherEngine => {
                "The header item belongs to another engine. Read the item carrying the custody identifier."
            }
            PublishError::PayloadUnreadable => {
                "The custody header item failed to decode. Rebuild the block with the current challenge encoding."
            }
        }
    }
}

/// Build the digest item carrying one challenge.
pub fn digest_item<ValidatorId, BlockNumber>(
    challenge: &Challenge<ValidatorId, BlockNumber>,
) -> DigestItem
where
    ValidatorId: Encode,
    BlockNumber: Encode,
{
    DigestItem::Consensus(CUSTODY_DIGEST_ID, challenge.encode())
}

/// Read the challenge one digest item carries.
pub fn read_digest_item<ValidatorId, BlockNumber>(
    item: &DigestItem,
) -> Result<Challenge<ValidatorId, BlockNumber>, PublishError>
where
    ValidatorId: Decode,
    BlockNumber: Decode,
{
    match item {
        DigestItem::Consensus(id, payload) if *id == CUSTODY_DIGEST_ID => {
            Challenge::decode(&mut &payload[..]).map_err(|_| PublishError::PayloadUnreadable)
        }
        _ => Err(PublishError::OtherEngine),
    }
}

/// Find the challenge inside a list of digest items.
///
/// A header carrying no custody item yields nothing. A header carrying an
/// unreadable custody item yields the failure.
pub fn find_challenge<ValidatorId, BlockNumber>(
    items: &[DigestItem],
) -> Option<Result<Challenge<ValidatorId, BlockNumber>, PublishError>>
where
    ValidatorId: Decode,
    BlockNumber: Decode,
{
    for item in items.iter() {
        match read_digest_item::<ValidatorId, BlockNumber>(item) {
            Err(PublishError::OtherEngine) => continue,
            outcome => return Some(outcome),
        }
    }
    None
}

/// The raw bytes one challenge encodes to.
///
/// The node side reads these bytes from the header without depending on this
/// pallet.
pub fn encoded_payload<ValidatorId, BlockNumber>(
    challenge: &Challenge<ValidatorId, BlockNumber>,
) -> Vec<u8>
where
    ValidatorId: Encode,
    BlockNumber: Encode,
{
    challenge.encode()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::RecordRef;
    use frame_support::BoundedVec;

    fn challenge() -> Challenge<u64, u64> {
        Challenge {
            target: 7,
            block: 12,
            record: RecordRef {
                block: 12,
                index: 3,
                collection: [9u8; 16],
                key: BoundedVec::try_from(sp_std::vec![1u8, 2, 3]).expect("three bytes fit"),
            },
            issued: 40,
            deadline: 43,
        }
    }

    #[test]
    fn a_challenge_survives_the_header_round_trip() {
        let item = digest_item(&challenge());
        let read: Challenge<u64, u64> = read_digest_item(&item).expect("reads");
        assert_eq!(read, challenge());
    }

    #[test]
    fn an_item_from_another_engine_is_passed_over() {
        let item = DigestItem::Consensus(*b"xxxx", sp_std::vec![1, 2, 3]);
        assert_eq!(
            read_digest_item::<u64, u64>(&item),
            Err(PublishError::OtherEngine)
        );
    }

    #[test]
    fn an_unreadable_custody_item_is_named() {
        let item = DigestItem::Consensus(CUSTODY_DIGEST_ID, sp_std::vec![0xff, 0xff]);
        assert_eq!(
            read_digest_item::<u64, u64>(&item),
            Err(PublishError::PayloadUnreadable)
        );
    }

    #[test]
    fn the_challenge_is_found_among_other_items() {
        let items = sp_std::vec![
            DigestItem::Other(sp_std::vec![1]),
            DigestItem::Consensus(*b"aaaa", sp_std::vec![2]),
            digest_item(&challenge()),
        ];
        let found: Challenge<u64, u64> = find_challenge(&items).expect("present").expect("reads");
        assert_eq!(found, challenge());
    }

    #[test]
    fn a_header_carrying_no_custody_item_yields_nothing() {
        let items = sp_std::vec![DigestItem::Other(sp_std::vec![1])];
        assert!(find_challenge::<u64, u64>(&items).is_none());
    }

    #[test]
    fn the_payload_matches_the_challenge_encoding() {
        let held = challenge();
        assert_eq!(encoded_payload(&held), held.encode());
    }

    #[test]
    fn every_publication_failure_names_a_fix() {
        assert!(PublishError::OtherEngine.plain_language().len() > 20);
        assert!(PublishError::PayloadUnreadable.plain_language().len() > 20);
    }
}
