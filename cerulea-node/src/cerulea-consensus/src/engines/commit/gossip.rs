//! The capped propagation message set.
//!
//! The propagation layer remembers the messages it has already seen, so one
//! message crosses each peer once. Earlier releases let that memory grow with
//! no limit. A peer with no account reached the path through a network message,
//! so a stream of messages raised node memory without bound.
//!
//! The set here carries a capacity limit. A new message at capacity evicts the
//! message that went longest without a hit. That rule is least recently used
//! eviction. Memory therefore stays inside a stated ceiling whatever the peers
//! send.
//!
//! The set is a hash map beside a doubly linked order list held in one vector.
//! Insertion, lookup and eviction each cost constant time, so the cap adds no
//! cost to propagation.

use std::collections::HashMap;
use std::hash::Hash;

/// The number of messages the set remembers by default.
///
/// Each entry holds one message identifier. The default covers several slots
/// of traffic across a two hundred validator set.
pub const DEFAULT_MESSAGE_CAPACITY: usize = 8_192;

/// One slot in the order list.
#[derive(Clone, Copy, Debug)]
struct Slot {
    newer: usize,
    older: usize,
}

const NO_SLOT: usize = usize::MAX;

/// A set of seen message identifiers with a capacity limit.
///
/// The set answers whether a message arrived before. A message at capacity
/// evicts the entry that went longest without a hit.
#[derive(Debug)]
pub struct MessageSet<K: Eq + Hash + Clone> {
    capacity: usize,
    index: HashMap<K, usize>,
    keys: Vec<K>,
    order: Vec<Slot>,
    free: Vec<usize>,
    newest: usize,
    oldest: usize,
    evictions: u64,
}

impl<K: Eq + Hash + Clone> MessageSet<K> {
    /// Build a set holding the default capacity.
    pub fn new() -> Self {
        MessageSet::with_capacity(DEFAULT_MESSAGE_CAPACITY)
    }

    /// Build a set holding the given capacity.
    ///
    /// A capacity of zero rises to one, so the set always remembers the last
    /// message.
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        MessageSet {
            capacity,
            index: HashMap::with_capacity(capacity),
            keys: Vec::with_capacity(capacity),
            order: Vec::with_capacity(capacity),
            free: Vec::new(),
            newest: NO_SLOT,
            oldest: NO_SLOT,
            evictions: 0,
        }
    }

    /// The capacity this set holds.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The number of messages the set remembers.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Report whether the set remembers nothing.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// The number of messages the set has evicted.
    pub fn evictions(&self) -> u64 {
        self.evictions
    }

    /// Report whether the set remembers one message.
    ///
    /// The call leaves the order untouched.
    pub fn holds(&self, key: &K) -> bool {
        self.index.contains_key(key)
    }

    /// Record one message and report whether the set saw it before.
    ///
    /// A message the set already holds moves to the newest position. A new
    /// message at capacity evicts the oldest entry first.
    pub fn record(&mut self, key: K) -> bool {
        if let Some(slot) = self.index.get(&key).copied() {
            self.touch(slot);
            return true;
        }
        if self.len() >= self.capacity {
            self.evict_oldest();
        }
        let slot = self.take_slot(key.clone());
        self.index.insert(key, slot);
        self.link_newest(slot);
        false
    }

    /// The message that goes next under eviction.
    pub fn oldest(&self) -> Option<&K> {
        if self.oldest == NO_SLOT {
            None
        } else {
            self.keys.get(self.oldest)
        }
    }

    /// The message the set saw most recently.
    pub fn newest(&self) -> Option<&K> {
        if self.newest == NO_SLOT {
            None
        } else {
            self.keys.get(self.newest)
        }
    }

    /// Forget every message.
    pub fn clear(&mut self) {
        self.index.clear();
        self.keys.clear();
        self.order.clear();
        self.free.clear();
        self.newest = NO_SLOT;
        self.oldest = NO_SLOT;
    }

    fn take_slot(&mut self, key: K) -> usize {
        match self.free.pop() {
            Some(slot) => {
                self.keys[slot] = key;
                self.order[slot] = Slot {
                    newer: NO_SLOT,
                    older: NO_SLOT,
                };
                slot
            }
            None => {
                self.keys.push(key);
                self.order.push(Slot {
                    newer: NO_SLOT,
                    older: NO_SLOT,
                });
                self.keys.len() - 1
            }
        }
    }

    fn link_newest(&mut self, slot: usize) {
        self.order[slot].newer = NO_SLOT;
        self.order[slot].older = self.newest;
        if self.newest != NO_SLOT {
            self.order[self.newest].newer = slot;
        }
        self.newest = slot;
        if self.oldest == NO_SLOT {
            self.oldest = slot;
        }
    }

    fn unlink(&mut self, slot: usize) {
        let entry = self.order[slot];
        if entry.newer != NO_SLOT {
            self.order[entry.newer].older = entry.older;
        } else {
            self.newest = entry.older;
        }
        if entry.older != NO_SLOT {
            self.order[entry.older].newer = entry.newer;
        } else {
            self.oldest = entry.newer;
        }
        self.order[slot] = Slot {
            newer: NO_SLOT,
            older: NO_SLOT,
        };
    }

    fn touch(&mut self, slot: usize) {
        if self.newest == slot {
            return;
        }
        self.unlink(slot);
        self.link_newest(slot);
    }

    fn evict_oldest(&mut self) {
        let slot = self.oldest;
        if slot == NO_SLOT {
            return;
        }
        self.unlink(slot);
        let key = self.keys[slot].clone();
        self.index.remove(&key);
        self.free.push(slot);
        self.evictions = self.evictions.saturating_add(1);
    }
}

impl<K: Eq + Hash + Clone> Default for MessageSet<K> {
    fn default() -> Self {
        MessageSet::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_message_reports_itself_as_new() {
        let mut set: MessageSet<u32> = MessageSet::with_capacity(4);
        assert!(!set.record(1));
        assert!(set.record(1));
        assert_eq!(set.len(), 1);
        assert!(set.holds(&1));
        assert!(!set.holds(&2));
        assert!(!set.is_empty());
    }

    #[test]
    fn the_set_evicts_at_capacity_and_memory_stays_bounded() {
        let mut set: MessageSet<u32> = MessageSet::with_capacity(16);
        for message in 0..10_000u32 {
            set.record(message);
            assert!(
                set.len() <= 16,
                "the set never holds more than its capacity"
            );
        }
        assert_eq!(set.len(), 16);
        assert_eq!(set.capacity(), 16);
        assert_eq!(set.evictions(), 10_000 - 16);
        // The last sixteen messages remain. Every earlier message is gone.
        for message in 9_984..10_000u32 {
            assert!(set.holds(&message));
        }
        for message in 0..9_984u32 {
            assert!(!set.holds(&message));
        }
    }

    #[test]
    fn the_message_that_went_longest_without_a_hit_goes_first() {
        let mut set: MessageSet<u32> = MessageSet::with_capacity(3);
        set.record(1);
        set.record(2);
        set.record(3);
        assert_eq!(set.oldest(), Some(&1));
        // A hit on one moves it to the newest position.
        assert!(set.record(1));
        assert_eq!(set.oldest(), Some(&2));
        assert_eq!(set.newest(), Some(&1));
        set.record(4);
        assert!(!set.holds(&2), "the oldest entry went first");
        assert!(set.holds(&1));
        assert!(set.holds(&3));
        assert!(set.holds(&4));
    }

    #[test]
    fn a_hit_leaves_the_length_unchanged() {
        let mut set: MessageSet<u32> = MessageSet::with_capacity(8);
        for message in 0..5u32 {
            set.record(message);
        }
        for _ in 0..100 {
            assert!(set.record(2));
        }
        assert_eq!(set.len(), 5);
        assert_eq!(set.evictions(), 0);
    }

    #[test]
    fn the_evicted_slots_return_to_use() {
        let mut set: MessageSet<u32> = MessageSet::with_capacity(4);
        for message in 0..40u32 {
            set.record(message);
        }
        // Four live slots serve forty messages, so the store holds four keys.
        assert_eq!(set.len(), 4);
        assert_eq!(set.keys.len(), 4);
    }

    #[test]
    fn a_capacity_of_zero_rises_to_one() {
        let mut set: MessageSet<u32> = MessageSet::with_capacity(0);
        assert_eq!(set.capacity(), 1);
        set.record(1);
        set.record(2);
        assert_eq!(set.len(), 1);
        assert!(set.holds(&2));
        assert!(!set.holds(&1));
    }

    #[test]
    fn the_set_forgets_everything_on_request() {
        let mut set: MessageSet<u32> = MessageSet::with_capacity(4);
        set.record(1);
        set.record(2);
        set.clear();
        assert!(set.is_empty());
        assert_eq!(set.oldest(), None);
        assert_eq!(set.newest(), None);
        assert!(!set.record(1), "a cleared set treats the message as new");
    }

    #[test]
    fn the_default_capacity_holds_its_stated_value() {
        let set: MessageSet<u32> = MessageSet::new();
        assert_eq!(set.capacity(), DEFAULT_MESSAGE_CAPACITY);
        let fallback: MessageSet<u32> = MessageSet::default();
        assert_eq!(fallback.capacity(), DEFAULT_MESSAGE_CAPACITY);
    }

    #[test]
    fn the_set_holds_a_hash_shaped_key() {
        let mut set: MessageSet<[u8; 32]> = MessageSet::with_capacity(2);
        assert!(!set.record([1u8; 32]));
        assert!(set.record([1u8; 32]));
        assert!(!set.record([2u8; 32]));
        assert!(!set.record([3u8; 32]));
        assert!(!set.holds(&[1u8; 32]));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn a_single_slot_set_holds_the_last_message() {
        let mut set: MessageSet<u32> = MessageSet::with_capacity(1);
        for message in 0..50u32 {
            set.record(message);
            assert_eq!(set.len(), 1);
            assert!(set.holds(&message));
        }
        assert_eq!(set.evictions(), 49);
    }
}
