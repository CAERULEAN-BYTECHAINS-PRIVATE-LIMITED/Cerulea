//! The gate every network-reachable message passes.
//!
//! A network-reachable path is any place where a Cerulea node turns bytes a
//! stranger sent into a value. A stranger chooses those bytes. The shared
//! primitives crate names the highest number of bytes each path carries and the
//! price a message of a given size costs. This module applies that table on the
//! node side.
//!
//! # The order of the two checks
//!
//! The size check runs first and reads the byte count alone. Refusing an
//! oversized message therefore costs one comparison. The price follows the size
//! and the caller pays it before the work starts. Nothing decodes until both
//! answers are in.
//!
//! # Why the price follows the size
//!
//! Earlier releases priced several paths at a fixed figure whatever the message
//! carried. One message could then carry thousands of items at the price of
//! one. A price that rises with the byte count removes that gap.

use crate::engines::EngineError;

pub use cerulea_primitives::bounds::{
    admit as admit_bytes, price, Admitted, BoundFault, MessagePath,
};

/// Check one message against the bound of its path.
///
/// The call takes the byte count alone, so a caller runs it before any
/// decoding. The call reports the price the caller pays where the message fits
/// inside the bound.
pub fn admit(path: MessagePath, bytes: usize) -> Result<Admitted, EngineError> {
    admit_bytes(path, bytes).map_err(|fault| EngineError::MessageAboveBound {
        detail: fault.plain_language(),
    })
}

/// Check one message and read it back, in that order.
///
/// The size check runs before the decoding, so an oversized message never
/// reaches the decoder.
pub fn admit_and_decode<T: codec::Decode>(
    path: MessagePath,
    bytes: &[u8],
) -> Result<(T, Admitted), EngineError> {
    let admitted = admit(path, bytes.len())?;
    let value = T::decode(&mut &bytes[..]).map_err(|reason| EngineError::MessageUnreadable {
        path: path.label(),
        detail: format!("{}", reason),
    })?;
    Ok((value, admitted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codec::Encode;

    #[test]
    fn a_message_inside_the_bound_carries_its_price() {
        let admitted = admit(MessagePath::CommitVote, 100).expect("one hundred bytes fit");
        assert_eq!(admitted.bytes, 100);
        assert_eq!(admitted.cost, MessagePath::CommitVote.cost(100));
    }

    #[test]
    fn every_path_refuses_an_oversized_message() {
        for path in MessagePath::ALL {
            let error = admit(path, path.max_bytes() + 1)
                .expect_err("the message crosses the bound of its path");
            assert!(matches!(error, EngineError::MessageAboveBound { .. }));
            let text = error.to_string();
            assert!(text.contains("Fix:"), "{} names a fix", path.label());
            assert!(text.contains(path.label()));
        }
    }

    #[test]
    fn an_oversized_message_never_reaches_the_decoder() {
        let bytes = vec![0u8; MessagePath::CommitVote.max_bytes() + 1];
        let outcome = admit_and_decode::<u64>(MessagePath::CommitVote, &bytes);
        assert!(matches!(
            outcome,
            Err(EngineError::MessageAboveBound { .. })
        ));
    }

    #[test]
    fn a_message_inside_the_bound_reads_back() {
        let value = 42u64;
        let bytes = value.encode();
        let (read, admitted) =
            admit_and_decode::<u64>(MessagePath::CommitVote, &bytes).expect("the value reads back");
        assert_eq!(read, value);
        assert_eq!(admitted.bytes, bytes.len());
    }

    #[test]
    fn bytes_that_fail_to_decode_name_the_path() {
        let error = admit_and_decode::<u64>(MessagePath::CommitVote, &[1u8, 2])
            .expect_err("two bytes carry no value");
        assert!(matches!(error, EngineError::MessageUnreadable { .. }));
        assert!(error.to_string().contains("commit vote"));
    }

    #[test]
    fn the_price_rises_with_the_byte_count() {
        let small = admit(MessagePath::BridgeProof, 100).expect("the message fits");
        let large = admit(MessagePath::BridgeProof, 10_000).expect("the message fits");
        assert!(large.cost > small.cost);
        assert_eq!(price(MessagePath::BridgeProof, 100), Ok(small.cost));
    }
}
