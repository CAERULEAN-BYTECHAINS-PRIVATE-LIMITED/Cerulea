//! The recursive length prefix decoder.
//!
//! Networks running Ethereum Virtual Machine bytecode encode a block
//! header, a transaction receipt and every trie node in this form. The
//! decoder here reads that form. The module holds its own decoder, so the
//! bridge carries no dependency outside the pinned set.
//!
//! # The encoding in plain words
//!
//! An item is either a byte string or a list of items. A single byte below
//! 128 encodes itself. A short byte string of up to 55 bytes takes a
//! marker of 128 plus the length. A long byte string takes a marker of 183
//! plus the byte count of the length, then the length, then the bytes. A
//! short list of up to 55 payload bytes takes a marker of 192 plus the
//! payload length. A long list takes a marker of 247 plus the byte count
//! of the payload length, then the length, then the payload.
//!
//! The decoder rejects a length carrying a leading zero byte and rejects a
//! short form written in the long form. Both rules keep one encoding per
//! value, so two nodes agree on the hash of the same content.

use alloc::{vec, vec::Vec};

/// The deepest nesting the decoder walks.
///
/// A block header nests one level. A trie node nests two levels. A limit
/// of eight leaves room and stops a crafted input from exhausting the
/// stack.
pub const MAX_DEPTH: usize = 8;

/// What went wrong while bytes were decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RlpError {
    /// The bytes ran out before the item finished.
    Truncated,
    /// A length carries a leading zero byte.
    LeadingZero,
    /// A value sits in the long form while the short form holds it.
    NotCanonical,
    /// The nesting runs past the depth limit.
    TooDeep,
    /// Bytes follow the item the caller asked for.
    TrailingBytes,
    /// The item holds a list where the caller expects a byte string.
    ListFound,
    /// The item holds a byte string where the caller expects a list.
    BytesFound,
    /// The list holds a different count of items than the caller expects.
    WrongLength,
}

/// One decoded item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// A byte string.
    Bytes(Vec<u8>),
    /// A list of items.
    List(Vec<Item>),
}

impl Item {
    /// The byte string the item holds.
    pub fn bytes(&self) -> Result<&[u8], RlpError> {
        match self {
            Item::Bytes(held) => Ok(held.as_slice()),
            Item::List(_) => Err(RlpError::ListFound),
        }
    }

    /// The list the item holds.
    pub fn list(&self) -> Result<&[Item], RlpError> {
        match self {
            Item::List(held) => Ok(held.as_slice()),
            Item::Bytes(_) => Err(RlpError::BytesFound),
        }
    }

    /// The item at one position of the list the item holds.
    pub fn at(&self, position: usize) -> Result<&Item, RlpError> {
        self.list()?.get(position).ok_or(RlpError::WrongLength)
    }

    /// The byte string at one position of the list the item holds.
    pub fn bytes_at(&self, position: usize) -> Result<&[u8], RlpError> {
        self.at(position)?.bytes()
    }

    /// The thirty two byte value at one position of the list.
    pub fn hash_at(&self, position: usize) -> Result<[u8; 32], RlpError> {
        let held = self.bytes_at(position)?;
        if held.len() != 32 {
            return Err(RlpError::WrongLength);
        }
        let mut found = [0u8; 32];
        found.copy_from_slice(held);
        Ok(found)
    }

    /// The whole number at one position of the list.
    ///
    /// A number sits in the encoding as a big endian byte string with no
    /// leading zero byte. A value of zero sits as an empty byte string.
    pub fn number_at(&self, position: usize) -> Result<u64, RlpError> {
        let held = self.bytes_at(position)?;
        if held.len() > 8 {
            return Err(RlpError::WrongLength);
        }
        if held.first() == Some(&0) {
            return Err(RlpError::LeadingZero);
        }
        let mut found = 0u64;
        for byte in held.iter() {
            found = (found << 8) | (*byte as u64);
        }
        Ok(found)
    }

    /// The number of items the list holds.
    pub fn len(&self) -> usize {
        match self {
            Item::List(held) => held.len(),
            Item::Bytes(held) => held.len(),
        }
    }

    /// Report whether the item holds nothing.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Decode one item and require the bytes to end with it.
pub fn decode(bytes: &[u8]) -> Result<Item, RlpError> {
    let (item, used) = decode_at(bytes, 0)?;
    if used == bytes.len() {
        Ok(item)
    } else {
        Err(RlpError::TrailingBytes)
    }
}

/// Decode one item from the front of the bytes.
///
/// The answer carries the item and the count of bytes the item took.
pub fn decode_at(bytes: &[u8], depth: usize) -> Result<(Item, usize), RlpError> {
    if depth > MAX_DEPTH {
        return Err(RlpError::TooDeep);
    }
    let marker = *bytes.first().ok_or(RlpError::Truncated)?;
    if marker < 0x80 {
        return Ok((Item::Bytes(vec![marker]), 1));
    }
    if marker < 0xb8 {
        let length = (marker - 0x80) as usize;
        let end = 1usize.checked_add(length).ok_or(RlpError::Truncated)?;
        let payload = bytes.get(1..end).ok_or(RlpError::Truncated)?;
        if length == 1 && payload[0] < 0x80 {
            return Err(RlpError::NotCanonical);
        }
        return Ok((Item::Bytes(payload.to_vec()), end));
    }
    if marker < 0xc0 {
        let count = (marker - 0xb7) as usize;
        let (length, start) = read_length(bytes, count)?;
        if length < 56 {
            return Err(RlpError::NotCanonical);
        }
        let end = start.checked_add(length).ok_or(RlpError::Truncated)?;
        let payload = bytes.get(start..end).ok_or(RlpError::Truncated)?;
        return Ok((Item::Bytes(payload.to_vec()), end));
    }
    if marker < 0xf8 {
        let length = (marker - 0xc0) as usize;
        let end = 1usize.checked_add(length).ok_or(RlpError::Truncated)?;
        let payload = bytes.get(1..end).ok_or(RlpError::Truncated)?;
        return Ok((Item::List(decode_list(payload, depth)?), end));
    }
    let count = (marker - 0xf7) as usize;
    let (length, start) = read_length(bytes, count)?;
    if length < 56 {
        return Err(RlpError::NotCanonical);
    }
    let end = start.checked_add(length).ok_or(RlpError::Truncated)?;
    let payload = bytes.get(start..end).ok_or(RlpError::Truncated)?;
    Ok((Item::List(decode_list(payload, depth)?), end))
}

/// Read a long form length and answer with the length and the offset the
/// payload starts at.
fn read_length(bytes: &[u8], count: usize) -> Result<(usize, usize), RlpError> {
    if count == 0 || count > 8 {
        return Err(RlpError::NotCanonical);
    }
    let end = 1usize.checked_add(count).ok_or(RlpError::Truncated)?;
    let held = bytes.get(1..end).ok_or(RlpError::Truncated)?;
    if held[0] == 0 {
        return Err(RlpError::LeadingZero);
    }
    let mut length = 0usize;
    for byte in held.iter() {
        length = length
            .checked_mul(256)
            .and_then(|held| held.checked_add(*byte as usize))
            .ok_or(RlpError::Truncated)?;
    }
    Ok((length, end))
}

/// Decode every item of one list payload.
fn decode_list(payload: &[u8], depth: usize) -> Result<Vec<Item>, RlpError> {
    let mut found: Vec<Item> = Vec::new();
    let mut cursor = 0usize;
    while cursor < payload.len() {
        let (item, used) = decode_at(&payload[cursor..], depth + 1)?;
        cursor = cursor.checked_add(used).ok_or(RlpError::Truncated)?;
        found.push(item);
    }
    Ok(found)
}

/// Encode one byte string.
pub fn encode_bytes(bytes: &[u8]) -> Vec<u8> {
    if bytes.len() == 1 && bytes[0] < 0x80 {
        return vec![bytes[0]];
    }
    let mut found = header(bytes.len(), 0x80, 0xb7);
    found.extend_from_slice(bytes);
    found
}

/// Encode a whole number as the encoding writes it.
pub fn encode_number(value: u64) -> Vec<u8> {
    if value == 0 {
        return vec![0x80];
    }
    let held = value.to_be_bytes();
    let start = held.iter().position(|byte| *byte != 0).unwrap_or(7);
    encode_bytes(&held[start..])
}

/// Encode a list from the encodings of its items.
pub fn encode_list(items: &[Vec<u8>]) -> Vec<u8> {
    let length = items.iter().map(|held| held.len()).sum::<usize>();
    let mut found = header(length, 0xc0, 0xf7);
    for item in items.iter() {
        found.extend_from_slice(item);
    }
    found
}

/// Write the marker and the length ahead of a payload.
fn header(length: usize, short: u8, long: u8) -> Vec<u8> {
    if length < 56 {
        return vec![short.saturating_add(length as u8)];
    }
    let held = (length as u64).to_be_bytes();
    let start = held.iter().position(|byte| *byte != 0).unwrap_or(7);
    let count = 8 - start;
    let mut found = vec![long.saturating_add(count as u8)];
    found.extend_from_slice(&held[start..]);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_low_byte_encodes_itself() {
        assert_eq!(decode(&[0x2a]), Ok(Item::Bytes(vec![0x2a])));
        assert_eq!(encode_bytes(&[0x2a]), vec![0x2a]);
    }

    #[test]
    fn an_empty_byte_string_takes_the_empty_marker() {
        assert_eq!(decode(&[0x80]), Ok(Item::Bytes(Vec::new())));
        assert_eq!(encode_bytes(&[]), vec![0x80]);
    }

    #[test]
    fn a_short_byte_string_round_trips() {
        let held = b"cerulea".to_vec();
        let encoded = encode_bytes(&held);
        assert_eq!(decode(&encoded), Ok(Item::Bytes(held)));
    }

    #[test]
    fn a_long_byte_string_round_trips() {
        let held = vec![7u8; 300];
        let encoded = encode_bytes(&held);
        assert_eq!(encoded[0], 0xb9);
        assert_eq!(decode(&encoded), Ok(Item::Bytes(held)));
    }

    #[test]
    fn a_list_round_trips() {
        let encoded = encode_list(&[encode_bytes(b"ab"), encode_number(1_024)]);
        let item = decode(&encoded).expect("the list decodes");
        assert_eq!(item.bytes_at(0), Ok(b"ab".as_slice()));
        assert_eq!(item.number_at(1), Ok(1_024));
    }

    #[test]
    fn a_long_list_round_trips() {
        let items: Vec<Vec<u8>> = (0..40).map(|_| encode_bytes(&[9u8; 4])).collect();
        let encoded = encode_list(&items);
        let item = decode(&encoded).expect("the list decodes");
        assert_eq!(item.list().map(|held| held.len()), Ok(40));
    }

    #[test]
    fn a_length_carrying_a_leading_zero_declines() {
        assert_eq!(decode(&[0xb8, 0x00]), Err(RlpError::LeadingZero));
    }

    #[test]
    fn a_short_value_in_the_long_form_declines() {
        assert_eq!(
            decode(&[0xb8, 0x02, 0x01, 0x02]),
            Err(RlpError::NotCanonical)
        );
    }

    #[test]
    fn a_single_byte_written_long_declines() {
        assert_eq!(decode(&[0x81, 0x2a]), Err(RlpError::NotCanonical));
    }

    #[test]
    fn truncated_bytes_decline() {
        assert_eq!(decode(&[0x83, 0x01, 0x02]), Err(RlpError::Truncated));
    }

    #[test]
    fn trailing_bytes_decline() {
        assert_eq!(decode(&[0x2a, 0x2b]), Err(RlpError::TrailingBytes));
    }

    #[test]
    fn a_number_reads_back_from_its_encoding() {
        for value in [0u64, 1, 127, 128, 255, 256, 65_535, u64::MAX] {
            let encoded = encode_list(&[encode_number(value)]);
            let item = decode(&encoded).expect("the list decodes");
            assert_eq!(item.number_at(0), Ok(value));
        }
    }

    #[test]
    fn a_number_carrying_a_leading_zero_declines() {
        let encoded = encode_list(&[encode_bytes(&[0x00, 0x01])]);
        let item = decode(&encoded).expect("the list decodes");
        assert_eq!(item.number_at(0), Err(RlpError::LeadingZero));
    }
}
