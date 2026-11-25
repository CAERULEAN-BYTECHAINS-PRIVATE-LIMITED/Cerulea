//! The bound and the price of every network-reachable message path.
//!
//! A network-reachable path is any place where a Cerulea node turns bytes a
//! stranger sent into a value. A stranger chooses those bytes. Two rules hold
//! at every one of those places.
//!
//! 1. **A hard size bound.** The path names the highest number of bytes one
//!    message carries. A larger message is refused on the length alone, before
//!    any decoding runs. Refusing an oversized message therefore costs one
//!    comparison.
//! 2. **A price that follows the size.** The path names a base price and a
//!    price for each byte. The caller pays the price before the work starts,
//!    so a large message costs what it costs to handle.
//!
//! Earlier releases priced several of these paths at a fixed figure whatever
//! the message carried. One message could then carry thousands of items at the
//! price of one. This table closes that surface across every path at once.
//!
//! The figures live in one place so the node, the runtime and the document at
//! `docs/NETWORK_BOUNDS.md` never drift apart.

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

/// One network-reachable decode path.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum MessagePath {
    /// The seal one arriving block header carries in its digest.
    BlockSeal,
    /// One commit vote a validator sends to the author of the slot.
    CommitVote,
    /// One aggregate signature the author submits for a block.
    CommitSubmission,
    /// One equivocation report a peer sends.
    EquivocationReport,
    /// One message on the Commit Engine propagation path.
    GossipMessage,
    /// One custody answer carrying a record and its inclusion proof.
    CustodyAnswer,
    /// One proof of a far network the bridge module verifies.
    BridgeProof,
    /// The body of one extrinsic the transaction pool admits.
    ExtrinsicBody,
    /// The body of one remote procedure call request.
    RpcBody,
    /// One Blueprint artefact arriving for deployment.
    BlueprintArtefact,
}

impl MessagePath {
    /// Every network-reachable path, in the order the document lists them.
    pub const ALL: [MessagePath; 10] = [
        MessagePath::BlockSeal,
        MessagePath::CommitVote,
        MessagePath::CommitSubmission,
        MessagePath::EquivocationReport,
        MessagePath::GossipMessage,
        MessagePath::CustodyAnswer,
        MessagePath::BridgeProof,
        MessagePath::ExtrinsicBody,
        MessagePath::RpcBody,
        MessagePath::BlueprintArtefact,
    ];

    /// The name this path carries in logs and in the bounds document.
    pub const fn label(&self) -> &'static str {
        match self {
            MessagePath::BlockSeal => "block seal",
            MessagePath::CommitVote => "commit vote",
            MessagePath::CommitSubmission => "commit submission",
            MessagePath::EquivocationReport => "equivocation report",
            MessagePath::GossipMessage => "propagation message",
            MessagePath::CustodyAnswer => "custody answer",
            MessagePath::BridgeProof => "bridge proof",
            MessagePath::ExtrinsicBody => "extrinsic body",
            MessagePath::RpcBody => "remote procedure call body",
            MessagePath::BlueprintArtefact => "Blueprint artefact",
        }
    }

    /// The highest number of bytes one message on this path carries.
    pub const fn max_bytes(&self) -> usize {
        match self {
            MessagePath::BlockSeal => 1_024,
            MessagePath::CommitVote => 256,
            MessagePath::CommitSubmission => 64 * 1_024,
            MessagePath::EquivocationReport => 768,
            MessagePath::GossipMessage => 8 * 1_024,
            MessagePath::CustodyAnswer => 4 * 1_024,
            MessagePath::BridgeProof => 256 * 1_024,
            MessagePath::ExtrinsicBody => 3_932_160,
            MessagePath::RpcBody => 10 * 1_024 * 1_024,
            MessagePath::BlueprintArtefact => 8 * 1_024 * 1_024,
        }
    }

    /// The price one message on this path carries before its size.
    ///
    /// The base covers the work every message costs whatever it carries. The
    /// figures are weight units, the same unit the Execution Engine meters.
    pub const fn base_cost(&self) -> u64 {
        match self {
            MessagePath::BlockSeal => 20_000,
            MessagePath::CommitVote => 10_000,
            MessagePath::CommitSubmission => 10_000,
            MessagePath::EquivocationReport => 40_000,
            MessagePath::GossipMessage => 5_000,
            MessagePath::CustodyAnswer => 30_000,
            MessagePath::BridgeProof => 50_000,
            MessagePath::ExtrinsicBody => 10_000,
            MessagePath::RpcBody => 5_000,
            MessagePath::BlueprintArtefact => 100_000,
        }
    }

    /// The price each byte of one message on this path adds.
    ///
    /// A path that runs signature work over its bytes prices a byte higher
    /// than a path that copies them.
    pub const fn cost_per_byte(&self) -> u64 {
        match self {
            MessagePath::BlockSeal => 100,
            MessagePath::CommitVote => 100,
            MessagePath::CommitSubmission => 80,
            MessagePath::EquivocationReport => 120,
            MessagePath::GossipMessage => 40,
            MessagePath::CustodyAnswer => 60,
            MessagePath::BridgeProof => 60,
            MessagePath::ExtrinsicBody => 40,
            MessagePath::RpcBody => 10,
            MessagePath::BlueprintArtefact => 20,
        }
    }

    /// The price one message of the stated size carries on this path.
    pub const fn cost(&self, bytes: usize) -> u64 {
        self.base_cost()
            .saturating_add(self.cost_per_byte().saturating_mul(bytes as u64))
    }

    /// The highest price any admitted message on this path carries.
    pub const fn ceiling_cost(&self) -> u64 {
        self.cost(self.max_bytes())
    }
}

/// What the gate decided about one message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Admitted {
    /// The path the message arrived on.
    pub path: MessagePath,
    /// The bytes the message carried.
    pub bytes: usize,
    /// The price the message carries.
    pub cost: u64,
}

/// A message the gate refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundFault {
    /// The path the message arrived on.
    pub path: MessagePath,
    /// The bytes the message carried.
    pub bytes: usize,
    /// The highest number of bytes that path carries.
    pub limit: usize,
}

impl BoundFault {
    /// One plain sentence naming the problem and the fix.
    pub fn plain_language(&self) -> alloc::string::String {
        alloc::format!(
            "The {} carries {} bytes and the limit is {}. Fix: send a smaller message on that path.",
            self.path.label(),
            self.bytes,
            self.limit
        )
    }
}

/// Check one message against the bound of its path.
///
/// The call takes the byte count alone, so a caller runs it before any
/// decoding. A message above the bound is refused on the length. A message
/// inside the bound reports the price the caller pays before the work starts.
pub const fn admit(path: MessagePath, bytes: usize) -> Result<Admitted, BoundFault> {
    if bytes > path.max_bytes() {
        Err(BoundFault {
            path,
            bytes,
            limit: path.max_bytes(),
        })
    } else {
        Ok(Admitted {
            path,
            bytes,
            cost: path.cost(bytes),
        })
    }
}

/// Check one message and report its price alone.
pub const fn price(path: MessagePath, bytes: usize) -> Result<u64, BoundFault> {
    match admit(path, bytes) {
        Ok(admitted) => Ok(admitted.cost),
        Err(fault) => Err(fault),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_path_carries_a_bound_and_a_price() {
        for path in MessagePath::ALL {
            assert!(path.max_bytes() > 0, "{} carries a bound", path.label());
            assert!(
                path.base_cost() > 0,
                "{} carries a base price",
                path.label()
            );
            assert!(
                path.cost_per_byte() > 0,
                "{} prices each byte",
                path.label()
            );
            assert!(!path.label().is_empty());
        }
        assert_eq!(MessagePath::ALL.len(), 10);
    }

    #[test]
    fn the_price_rises_with_the_size() {
        for path in MessagePath::ALL {
            let small = path.cost(0);
            let large = path.cost(path.max_bytes());
            assert!(large > small, "{} prices its bytes", path.label());
            assert_eq!(small, path.base_cost());
            assert_eq!(large, path.ceiling_cost());
        }
    }

    #[test]
    fn a_message_inside_the_bound_carries_its_price() {
        let admitted = admit(MessagePath::CommitVote, 200).expect("two hundred bytes fit");
        assert_eq!(admitted.path, MessagePath::CommitVote);
        assert_eq!(admitted.bytes, 200);
        assert_eq!(admitted.cost, MessagePath::CommitVote.cost(200));
        assert_eq!(price(MessagePath::CommitVote, 200), Ok(admitted.cost));
    }

    #[test]
    fn a_message_above_the_bound_is_refused_on_its_length() {
        for path in MessagePath::ALL {
            let fault = admit(path, path.max_bytes() + 1)
                .expect_err("the message crosses the bound of its path");
            assert_eq!(fault.path, path);
            assert_eq!(fault.limit, path.max_bytes());
            assert!(fault.plain_language().contains("Fix:"));
            assert!(fault.plain_language().contains(path.label()));
        }
    }

    #[test]
    fn a_message_at_the_bound_passes() {
        for path in MessagePath::ALL {
            assert!(
                admit(path, path.max_bytes()).is_ok(),
                "{} admits a message at its bound",
                path.label()
            );
        }
    }

    #[test]
    fn two_paths_carry_two_bounds() {
        assert_ne!(
            MessagePath::CommitVote.max_bytes(),
            MessagePath::BridgeProof.max_bytes()
        );
        assert!(MessagePath::CommitVote.max_bytes() < MessagePath::RpcBody.max_bytes());
    }

    #[test]
    fn the_price_holds_inside_the_number_range() {
        let huge = admit(MessagePath::RpcBody, usize::MAX);
        assert!(huge.is_err());
        assert!(MessagePath::RpcBody.cost(usize::MAX) > 0);
    }

    #[test]
    fn a_path_round_trips_through_the_encoding() {
        for path in MessagePath::ALL {
            let bytes = path.encode();
            assert_eq!(
                MessagePath::decode(&mut &bytes[..]).expect("the encoding decodes"),
                path
            );
        }
    }
}
