//! Policy amendment under authority, quorum and delay.
//!
//! The Amendment Engine governs changes to the first seven policies. Three
//! conditions hold for every change.
//!
//! 1. **Authority.** The account opening the amendment holds the authority the
//!    Amendment policy names.
//! 2. **Quorum.** The approvals reach the share of the governing body the
//!    Amendment policy names.
//! 3. **Delay.** The approved amendment waits the blocks the Amendment policy
//!    names before it takes effect.
//!
//! The Amendment policy governs itself. A change to the Amendment policy runs
//! under the Amendment policy in force at the moment the change opens, so no
//! change lowers its own bar.

use super::policies::{Authority, PolicyKind, PolicySet};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;
use sp_runtime::Perbill;

/// The state one amendment reaches.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub enum AmendmentState {
    /// The amendment collects approvals.
    #[default]
    Collecting,
    /// The amendment reached the quorum and waits out the delay.
    Waiting,
    /// The amendment took effect.
    Enacted,
    /// The amendment failed to reach the quorum inside the voting window.
    Lapsed,
}

impl AmendmentState {
    /// A plain sentence stating where the amendment stands.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            AmendmentState::Collecting => "The amendment collects approvals.",
            AmendmentState::Waiting => "The amendment reached the quorum and waits out the delay.",
            AmendmentState::Enacted => "The amendment took effect.",
            AmendmentState::Lapsed => {
                "The amendment failed to reach the quorum inside the voting window."
            }
        }
    }
}

/// A reason an amendment fails.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum AmendmentError {
    /// The account opening the amendment lacks the authority.
    AuthorityAbsent,
    /// The approvals fall short of the quorum.
    QuorumShort,
    /// The delay window still holds.
    DelayWindowOpen,
    /// The amendment already took effect.
    AlreadyEnacted,
    /// The voting window closed with the quorum unmet.
    VotingWindowClosed,
    /// The account already approved this amendment.
    AlreadyApproved,
}

impl AmendmentError {
    /// The sentence a person reads, naming the fix.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            AmendmentError::AuthorityAbsent => {
                "The account lacks the authority this deployment names. Open the amendment through the named authority."
            }
            AmendmentError::QuorumShort => {
                "The approvals fall short of the quorum. Collect the remaining approvals, then enact the amendment."
            }
            AmendmentError::DelayWindowOpen => {
                "The delay window still holds. Wait for the remaining blocks, then enact the amendment."
            }
            AmendmentError::AlreadyEnacted => {
                "The amendment already took effect. Open a new amendment to change the policy again."
            }
            AmendmentError::VotingWindowClosed => {
                "The voting window closed with the quorum unmet. Open a new amendment."
            }
            AmendmentError::AlreadyApproved => {
                "The account already approved this amendment. One account approves once."
            }
        }
    }
}

/// One amendment under way.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct Amendment {
    /// The policy the amendment changes.
    pub policy: PolicyKind,
    /// The block at which the amendment opened.
    pub opened_at: u32,
    /// The last block at which an approval counts.
    pub voting_closes_at: u32,
    /// The first block at which the amendment takes effect.
    pub effective_at: u32,
    /// The approvals collected so far.
    pub approvals: u32,
    /// The size of the governing body at the moment the amendment opened.
    pub body_size: u32,
    /// The quorum the amendment reaches, taken at the moment it opened.
    pub quorum: Perbill,
    /// The state the amendment reached.
    pub state: AmendmentState,
}

impl Amendment {
    /// The approvals the amendment still needs.
    pub fn approvals_remaining(&self) -> u32 {
        approvals_for_quorum(self.body_size, self.quorum).saturating_sub(self.approvals)
    }

    /// Report whether the amendment reached the quorum.
    pub fn quorum_reached(&self) -> bool {
        self.approvals >= approvals_for_quorum(self.body_size, self.quorum)
    }

    /// The blocks remaining before the amendment takes effect.
    pub fn blocks_remaining(&self, now: u32) -> u32 {
        self.effective_at.saturating_sub(now)
    }
}

/// The approvals one quorum asks for.
///
/// The count rounds upward, so a quorum of two thirds over five members asks
/// for four rather than three.
pub fn approvals_for_quorum(body_size: u32, quorum: Perbill) -> u32 {
    if body_size == 0 {
        return 0;
    }
    let whole = Perbill::one().deconstruct() as u64;
    let parts = quorum.deconstruct() as u64;
    (((body_size as u64) * parts).div_ceil(whole)) as u32
}

/// Report whether an account holds the authority to open an amendment.
///
/// The call takes what the chain knows about the account rather than the
/// account itself, so the same check runs on chain and off chain.
pub fn holds_authority(
    authority: Authority,
    is_operator: bool,
    is_seated_validator: bool,
    is_governing_member: bool,
) -> bool {
    match authority {
        Authority::Operator => is_operator,
        Authority::ValidatorSet => is_seated_validator,
        Authority::GoverningBody => is_governing_member,
    }
}

/// The blocks a voting window holds.
pub const DEFAULT_VOTING_WINDOW_BLOCKS: u32 = 100;

/// Open one amendment against the policy set in force.
///
/// The quorum and the delay come from the Amendment policy at the moment the
/// amendment opens, so a later change to that policy leaves this amendment
/// where it stands.
pub fn open(
    policies: &PolicySet,
    policy: PolicyKind,
    now: u32,
    body_size: u32,
    voting_window: u32,
) -> Amendment {
    Amendment {
        policy,
        opened_at: now,
        voting_closes_at: now.saturating_add(voting_window),
        effective_at: now
            .saturating_add(voting_window)
            .saturating_add(policies.amendment.delay_blocks),
        approvals: 0,
        body_size,
        quorum: policies.amendment.quorum,
        state: AmendmentState::Collecting,
    }
}

/// Record one approval against an amendment.
pub fn approve(amendment: &mut Amendment, now: u32) -> Result<(), AmendmentError> {
    match amendment.state {
        AmendmentState::Enacted => return Err(AmendmentError::AlreadyEnacted),
        AmendmentState::Lapsed => return Err(AmendmentError::VotingWindowClosed),
        _ => {}
    }
    if now > amendment.voting_closes_at {
        amendment.state = AmendmentState::Lapsed;
        return Err(AmendmentError::VotingWindowClosed);
    }
    amendment.approvals = amendment.approvals.saturating_add(1);
    if amendment.quorum_reached() {
        amendment.state = AmendmentState::Waiting;
    }
    Ok(())
}

/// Close the voting window on one amendment.
///
/// An amendment that failed to reach the quorum lapses.
pub fn close_voting(amendment: &mut Amendment, now: u32) {
    if amendment.state == AmendmentState::Collecting && now > amendment.voting_closes_at {
        amendment.state = AmendmentState::Lapsed;
    }
}

/// Enact one amendment.
///
/// The call checks the quorum and the delay. An amendment that meets both takes
/// effect and the policy changes.
pub fn enact(amendment: &mut Amendment, now: u32) -> Result<PolicyKind, AmendmentError> {
    match amendment.state {
        AmendmentState::Enacted => return Err(AmendmentError::AlreadyEnacted),
        AmendmentState::Lapsed => return Err(AmendmentError::VotingWindowClosed),
        AmendmentState::Collecting => {
            if now > amendment.voting_closes_at {
                amendment.state = AmendmentState::Lapsed;
                return Err(AmendmentError::VotingWindowClosed);
            }
            return Err(AmendmentError::QuorumShort);
        }
        AmendmentState::Waiting => {}
    }
    if now < amendment.effective_at {
        return Err(AmendmentError::DelayWindowOpen);
    }
    amendment.state = AmendmentState::Enacted;
    Ok(amendment.policy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policies() -> PolicySet {
        PolicySet::starting_profile()
    }

    fn amendment() -> Amendment {
        open(
            &policies(),
            PolicyKind::Assurance,
            10,
            9,
            DEFAULT_VOTING_WINDOW_BLOCKS,
        )
    }

    #[test]
    fn the_quorum_rounds_upward() {
        assert_eq!(approvals_for_quorum(9, Perbill::from_percent(67)), 7);
        assert_eq!(approvals_for_quorum(5, Perbill::from_percent(67)), 4);
        assert_eq!(approvals_for_quorum(3, Perbill::from_percent(67)), 3);
        assert_eq!(approvals_for_quorum(0, Perbill::from_percent(67)), 0);
        assert_eq!(approvals_for_quorum(100, Perbill::from_percent(50)), 50);
    }

    #[test]
    fn the_authority_governs_who_opens_an_amendment() {
        assert!(holds_authority(Authority::Operator, true, false, false));
        assert!(!holds_authority(Authority::Operator, false, true, true));

        assert!(holds_authority(Authority::ValidatorSet, false, true, false));
        assert!(!holds_authority(Authority::ValidatorSet, true, false, true));

        assert!(holds_authority(
            Authority::GoverningBody,
            false,
            false,
            true
        ));
        assert!(!holds_authority(
            Authority::GoverningBody,
            true,
            true,
            false
        ));
    }

    #[test]
    fn an_amendment_below_the_quorum_fails_to_enact() {
        let mut held = amendment();
        for _ in 0..6 {
            assert_eq!(approve(&mut held, 20), Ok(()));
        }
        assert!(!held.quorum_reached());
        assert_eq!(held.approvals_remaining(), 1);
        // Block 100 sits inside the voting window, which closes at block 110.
        assert_eq!(enact(&mut held, 100), Err(AmendmentError::QuorumShort));
        // A call after the window reports the closed window instead.
        assert_eq!(
            enact(&mut held, 300),
            Err(AmendmentError::VotingWindowClosed)
        );
    }

    #[test]
    fn an_amendment_reaching_the_quorum_waits_out_the_delay() {
        let mut held = amendment();
        for _ in 0..7 {
            approve(&mut held, 20).expect("the approval counts");
        }
        assert!(held.quorum_reached());
        assert_eq!(held.state, AmendmentState::Waiting);

        let effective_at = held.effective_at;
        assert_eq!(effective_at, 10 + 100 + 100);
        assert_eq!(
            enact(&mut held, effective_at - 1),
            Err(AmendmentError::DelayWindowOpen)
        );
        assert_eq!(enact(&mut held, effective_at), Ok(PolicyKind::Assurance));
        assert_eq!(held.state, AmendmentState::Enacted);
    }

    #[test]
    fn an_enacted_amendment_enacts_once() {
        let mut held = amendment();
        for _ in 0..7 {
            approve(&mut held, 20).expect("the approval counts");
        }
        let effective_at = held.effective_at;
        enact(&mut held, effective_at).expect("the amendment takes effect");
        assert_eq!(
            enact(&mut held, effective_at),
            Err(AmendmentError::AlreadyEnacted)
        );
        assert_eq!(
            approve(&mut held, effective_at),
            Err(AmendmentError::AlreadyEnacted)
        );
    }

    #[test]
    fn an_amendment_that_misses_the_window_lapses() {
        let mut held = amendment();
        approve(&mut held, 20).expect("the approval counts");
        let closes_at = held.voting_closes_at;
        let effective_at = held.effective_at;
        close_voting(&mut held, closes_at + 1);
        assert_eq!(held.state, AmendmentState::Lapsed);
        assert_eq!(
            approve(&mut held, closes_at + 2),
            Err(AmendmentError::VotingWindowClosed)
        );
        assert_eq!(
            enact(&mut held, effective_at),
            Err(AmendmentError::VotingWindowClosed)
        );
    }

    #[test]
    fn an_approval_after_the_window_fails() {
        let mut held = amendment();
        let closes_at = held.voting_closes_at;
        assert_eq!(
            approve(&mut held, closes_at + 1),
            Err(AmendmentError::VotingWindowClosed)
        );
        assert_eq!(held.state, AmendmentState::Lapsed);
    }

    #[test]
    fn the_amendment_carries_the_quorum_in_force_when_it_opened() {
        let mut later = policies();
        later.amendment.quorum = Perbill::from_percent(51);
        let held = amendment();
        // The amendment holds the quorum it opened under.
        assert_eq!(held.quorum, Perbill::from_percent(67));
        let fresh = open(&later, PolicyKind::Reward, 10, 9, 100);
        assert_eq!(fresh.quorum, Perbill::from_percent(51));
    }

    #[test]
    fn the_engine_reports_the_blocks_remaining() {
        let mut held = amendment();
        for _ in 0..7 {
            approve(&mut held, 20).expect("the approval counts");
        }
        assert_eq!(held.blocks_remaining(held.effective_at - 30), 30);
        assert_eq!(held.blocks_remaining(held.effective_at), 0);
    }

    #[test]
    fn the_amendment_engine_governs_the_seven_policies() {
        for policy in PolicyKind::GOVERNED.iter() {
            let mut held = open(&policies(), *policy, 0, 4, 100);
            for _ in 0..3 {
                approve(&mut held, 10).expect("the approval counts");
            }
            assert!(held.quorum_reached(), "{} reaches quorum", policy.label());
            let effective_at = held.effective_at;
            assert_eq!(enact(&mut held, effective_at), Ok(*policy));
        }
    }

    #[test]
    fn every_amendment_state_carries_a_plain_sentence() {
        for state in [
            AmendmentState::Collecting,
            AmendmentState::Waiting,
            AmendmentState::Enacted,
            AmendmentState::Lapsed,
        ] {
            assert!(state.plain_language().len() > 20);
        }
    }

    #[test]
    fn every_amendment_failure_names_a_fix() {
        for failure in [
            AmendmentError::AuthorityAbsent,
            AmendmentError::QuorumShort,
            AmendmentError::DelayWindowOpen,
            AmendmentError::AlreadyEnacted,
            AmendmentError::VotingWindowClosed,
            AmendmentError::AlreadyApproved,
        ] {
            assert!(failure.plain_language().len() > 30);
        }
    }
}
