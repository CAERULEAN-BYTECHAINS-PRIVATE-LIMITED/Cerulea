//! The rank timeout and the fallback to the next rank.
//!
//! Rank one authors the block for a slot. A slot still unclaimed after the rank
//! timeout passes to rank two. The slot walks down the committee one rank at a
//! time until a block lands or the committee runs out.
//!
//! The rank timeout is a policy value. The deployment profile carries it and
//! the Amendment Engine governs a change. The value here is the profile a
//! deployment starts from.
//!
//! Failover costs one timeout and no extra round of messages. The committee for
//! the slot is already settled, so the standby ranks need no election.

use crate::engines::{Rank, Slot, COMMITTEE_SIZE};

/// The rank timeout a deployment starts from, in milliseconds.
///
/// A rank holds the slot for this long. The slot passes to the next rank once
/// the window closes with no block.
pub const DEFAULT_RANK_TIMEOUT_MS: u64 = 1_500;

/// The authoring policy of one deployment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthoringPolicy {
    /// The span one rank holds the slot, in milliseconds.
    pub rank_timeout_ms: u64,
    /// The number of ranks the committee carries.
    pub committee_size: usize,
}

impl Default for AuthoringPolicy {
    fn default() -> Self {
        AuthoringPolicy::standard()
    }
}

impl AuthoringPolicy {
    /// The policy a deployment starts from.
    pub const fn standard() -> Self {
        AuthoringPolicy {
            rank_timeout_ms: DEFAULT_RANK_TIMEOUT_MS,
            committee_size: COMMITTEE_SIZE,
        }
    }

    /// Build a policy holding the two stated values.
    ///
    /// A timeout of zero rises to one millisecond and a committee of zero rises
    /// to one member, so the schedule always holds at least one window.
    pub const fn new(rank_timeout_ms: u64, committee_size: usize) -> Self {
        AuthoringPolicy {
            rank_timeout_ms: if rank_timeout_ms == 0 {
                1
            } else {
                rank_timeout_ms
            },
            committee_size: if committee_size == 0 {
                1
            } else {
                committee_size
            },
        }
    }

    /// The span the whole committee holds the slot, in milliseconds.
    pub const fn slot_span_ms(&self) -> u64 {
        self.rank_timeout_ms
            .saturating_mul(self.committee_size as u64)
    }
}

/// The window one rank holds the slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RankWindow {
    /// The rank the window belongs to.
    pub rank: Rank,
    /// The first moment the rank may author, in milliseconds.
    pub opens_ms: u64,
    /// The first moment the rank may no longer author, in milliseconds.
    pub closes_ms: u64,
}

impl RankWindow {
    /// Report whether the window holds the given moment.
    pub fn holds(&self, moment_ms: u64) -> bool {
        moment_ms >= self.opens_ms && moment_ms < self.closes_ms
    }
}

/// The schedule of one slot.
///
/// Every node builds the same schedule from the slot start and the policy, so
/// every node agrees on which rank holds the slot at any moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotSchedule {
    /// The slot the schedule serves.
    pub slot: Slot,
    /// The moment the slot opened, in milliseconds.
    pub opens_ms: u64,
    /// The policy the deployment carries.
    pub policy: AuthoringPolicy,
}

impl SlotSchedule {
    /// Build the schedule of one slot.
    pub fn new(slot: Slot, opens_ms: u64, policy: AuthoringPolicy) -> Self {
        SlotSchedule {
            slot,
            opens_ms,
            policy,
        }
    }

    /// Build the schedule of one slot from the slot number alone.
    ///
    /// Slots tile the clock from the epoch of the operating system. Slot number
    /// N opens at N times the slot span. Every node therefore builds the same
    /// schedule for a slot without asking any peer.
    pub fn for_slot(slot: Slot, policy: AuthoringPolicy) -> Self {
        SlotSchedule::new(slot, slot.0.saturating_mul(policy.slot_span_ms()), policy)
    }

    /// The slot holding one moment.
    pub fn slot_at(moment_ms: u64, policy: AuthoringPolicy) -> Slot {
        Slot(moment_ms / policy.slot_span_ms())
    }

    /// The window one rank holds.
    ///
    /// The call reports nothing for a rank outside the committee.
    pub fn window(&self, rank: Rank) -> Option<RankWindow> {
        if rank.0 == 0 || rank.index() >= self.policy.committee_size {
            return None;
        }
        let offset = self
            .policy
            .rank_timeout_ms
            .saturating_mul(rank.index() as u64);
        let opens_ms = self.opens_ms.saturating_add(offset);
        Some(RankWindow {
            rank,
            opens_ms,
            closes_ms: opens_ms.saturating_add(self.policy.rank_timeout_ms),
        })
    }

    /// The rank holding the slot at the given moment.
    ///
    /// The call reports nothing before the slot opens and after the last rank
    /// window closes.
    pub fn rank_at(&self, moment_ms: u64) -> Option<Rank> {
        if moment_ms < self.opens_ms {
            return None;
        }
        let elapsed = moment_ms.saturating_sub(self.opens_ms);
        let position = elapsed / self.policy.rank_timeout_ms;
        if position >= self.policy.committee_size as u64 {
            return None;
        }
        Some(Rank::from_index(position as usize))
    }

    /// Report whether the given rank may author at the given moment.
    pub fn claim_open(&self, rank: Rank, moment_ms: u64) -> bool {
        match self.window(rank) {
            Some(window) => window.holds(moment_ms),
            None => false,
        }
    }

    /// The rank that takes the slot once the given rank lets it pass.
    ///
    /// The call reports nothing where the given rank is the last of the
    /// committee.
    pub fn next_rank(&self, rank: Rank) -> Option<Rank> {
        let next = rank.next();
        if next.index() < self.policy.committee_size {
            Some(next)
        } else {
            None
        }
    }

    /// The moment the whole committee lets the slot pass, in milliseconds.
    pub fn closes_ms(&self) -> u64 {
        self.opens_ms.saturating_add(self.policy.slot_span_ms())
    }
}

/// The state of one slot while the committee works through its ranks.
///
/// A node watching a slot records the block that lands. The tracker reports
/// which rank holds the slot and whether the slot passed to a later rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FallbackTracker {
    schedule: SlotSchedule,
    claimed_by: Option<Rank>,
}

impl FallbackTracker {
    /// Start watching one slot.
    pub fn new(schedule: SlotSchedule) -> Self {
        FallbackTracker {
            schedule,
            claimed_by: None,
        }
    }

    /// The schedule this tracker watches.
    pub fn schedule(&self) -> SlotSchedule {
        self.schedule
    }

    /// Record the block that claimed the slot.
    ///
    /// A second block for the same slot leaves the first claim standing.
    pub fn record_claim(&mut self, rank: Rank) -> bool {
        if self.claimed_by.is_some() {
            return false;
        }
        self.claimed_by = Some(rank);
        true
    }

    /// The rank that claimed the slot.
    pub fn claimed_by(&self) -> Option<Rank> {
        self.claimed_by
    }

    /// Report whether the slot still waits for a block.
    pub fn waiting(&self) -> bool {
        self.claimed_by.is_none()
    }

    /// The rank holding the slot at the given moment.
    ///
    /// A claimed slot holds no rank. An unclaimed slot holds the rank whose
    /// window covers the moment.
    pub fn holder_at(&self, moment_ms: u64) -> Option<Rank> {
        if self.claimed_by.is_some() {
            return None;
        }
        self.schedule.rank_at(moment_ms)
    }

    /// Report whether the slot passed beyond rank one at the given moment.
    pub fn passed_on(&self, moment_ms: u64) -> bool {
        match self.holder_at(moment_ms) {
            Some(rank) => rank.0 > Rank::FIRST.0,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule() -> SlotSchedule {
        SlotSchedule::new(Slot(42), 10_000, AuthoringPolicy::standard())
    }

    #[test]
    fn the_standard_policy_holds_its_stated_values() {
        let policy = AuthoringPolicy::standard();
        assert_eq!(policy.rank_timeout_ms, DEFAULT_RANK_TIMEOUT_MS);
        assert_eq!(policy.committee_size, COMMITTEE_SIZE);
        assert_eq!(policy.slot_span_ms(), 1_500 * 5);
        assert_eq!(AuthoringPolicy::default(), policy);
    }

    #[test]
    fn a_policy_holds_at_least_one_rank_and_one_millisecond() {
        let policy = AuthoringPolicy::new(0, 0);
        assert_eq!(policy.rank_timeout_ms, 1);
        assert_eq!(policy.committee_size, 1);
    }

    #[test]
    fn rank_one_holds_the_slot_from_the_start() {
        let schedule = schedule();
        assert_eq!(schedule.rank_at(10_000), Some(Rank::FIRST));
        assert_eq!(schedule.rank_at(11_499), Some(Rank::FIRST));
        assert!(schedule.claim_open(Rank::FIRST, 10_000));
        assert!(schedule.claim_open(Rank::FIRST, 11_499));
    }

    #[test]
    fn an_unclaimed_slot_passes_to_rank_two_within_the_timeout() {
        let schedule = schedule();
        let mut tracker = FallbackTracker::new(schedule);
        assert_eq!(tracker.holder_at(10_000), Some(Rank::FIRST));
        assert!(!tracker.passed_on(10_000));
        // The rank one window closes one timeout after the slot opened.
        let handover = 10_000 + DEFAULT_RANK_TIMEOUT_MS;
        assert_eq!(tracker.holder_at(handover), Some(Rank(2)));
        assert!(tracker.passed_on(handover));
        assert!(schedule.claim_open(Rank(2), handover));
        assert!(!schedule.claim_open(Rank::FIRST, handover));
        assert!(tracker.waiting());
        assert!(tracker.record_claim(Rank(2)));
        assert!(!tracker.waiting());
        assert_eq!(tracker.claimed_by(), Some(Rank(2)));
        assert_eq!(tracker.holder_at(handover), None);
    }

    #[test]
    fn a_claimed_slot_stays_with_the_rank_that_claimed_it() {
        let mut tracker = FallbackTracker::new(schedule());
        assert!(tracker.record_claim(Rank::FIRST));
        assert!(!tracker.record_claim(Rank(2)));
        assert_eq!(tracker.claimed_by(), Some(Rank::FIRST));
        assert!(!tracker.passed_on(20_000));
    }

    #[test]
    fn the_slot_walks_down_every_rank_of_the_committee() {
        let schedule = schedule();
        for position in 0..COMMITTEE_SIZE {
            let moment = 10_000 + DEFAULT_RANK_TIMEOUT_MS * position as u64;
            assert_eq!(schedule.rank_at(moment), Some(Rank::from_index(position)));
        }
        assert_eq!(schedule.rank_at(schedule.closes_ms()), None);
        assert_eq!(schedule.closes_ms(), 10_000 + 1_500 * 5);
    }

    #[test]
    fn a_moment_before_the_slot_opens_holds_no_rank() {
        let schedule = schedule();
        assert_eq!(schedule.rank_at(9_999), None);
        assert!(!schedule.claim_open(Rank::FIRST, 9_999));
    }

    #[test]
    fn a_rank_outside_the_committee_holds_no_window() {
        let schedule = schedule();
        assert!(schedule.window(Rank(0)).is_none());
        assert!(schedule.window(Rank(6)).is_none());
        assert!(schedule.window(Rank(5)).is_some());
        assert!(!schedule.claim_open(Rank(6), 10_000));
    }

    #[test]
    fn each_rank_window_opens_where_the_one_before_it_closes() {
        let schedule = schedule();
        let first = schedule
            .window(Rank::FIRST)
            .expect("rank one holds a window");
        let second = schedule.window(Rank(2)).expect("rank two holds a window");
        assert_eq!(first.closes_ms, second.opens_ms);
        assert!(first.holds(first.opens_ms));
        assert!(!first.holds(first.closes_ms));
    }

    #[test]
    fn the_last_rank_lets_the_slot_pass_to_nothing() {
        let schedule = schedule();
        assert_eq!(schedule.next_rank(Rank::FIRST), Some(Rank(2)));
        assert_eq!(schedule.next_rank(Rank(4)), Some(Rank(5)));
        assert_eq!(schedule.next_rank(Rank(5)), None);
    }

    #[test]
    fn a_shorter_timeout_hands_over_sooner() {
        let quick = SlotSchedule::new(Slot(1), 0, AuthoringPolicy::new(200, 3));
        assert_eq!(quick.rank_at(0), Some(Rank::FIRST));
        assert_eq!(quick.rank_at(199), Some(Rank::FIRST));
        assert_eq!(quick.rank_at(200), Some(Rank(2)));
        assert_eq!(quick.rank_at(400), Some(Rank(3)));
        assert_eq!(quick.rank_at(600), None);
        assert_eq!(quick.closes_ms(), 600);
    }

    #[test]
    fn a_tracker_reads_back_its_schedule() {
        let tracker = FallbackTracker::new(schedule());
        assert_eq!(tracker.schedule(), schedule());
    }
}
