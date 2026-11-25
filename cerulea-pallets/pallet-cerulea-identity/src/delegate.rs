//! Delegation chains with scope and expiry.
//!
//! A delegation passes a named scope from one account to another for a
//! declared span. A registrar who leaves for a week delegates the two
//! actions a clerk needs and nothing else.
//!
//! A delegation carries five facts.
//!
//! 1. **The grantor.** The account that passes the scope.
//! 2. **The holder.** The account that receives the scope.
//! 3. **The role.** The role the scope runs under. The holder acts as that
//!    role for the named actions alone.
//! 4. **The scope.** The actions the holder runs. A scope that names
//!    nothing passes nothing.
//! 5. **The expiry.** The moment the delegation lapses, in milliseconds.
//!    Every delegation carries an expiry.
//!
//! A holder passes a delegation onward, which builds a chain. Three rules
//! hold across a chain.
//!
//! 1. **The scope narrows.** A link passes a subset of the scope it
//!    received. A wider scope is refused when the link is made.
//! 2. **The expiry pulls in.** The effective expiry of a link is the
//!    earliest expiry on the chain above it.
//! 3. **A break stops the chain.** A withdrawn link, a lapsed link or a
//!    grantor that lost the role stops every link below it.
//!
//! A delegation past its expiry resolves to denied.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use pallet_cerulea_schema::primitives::BoundedName;
use scale_info::TypeInfo;

/// One delegation.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(AccountId, MaxScope))]
pub struct Delegation<AccountId, MaxScope: frame_support::traits::Get<u32>> {
    /// The account that passes the scope.
    pub grantor: AccountId,
    /// The account that receives the scope.
    pub holder: AccountId,
    /// The role the scope runs under.
    pub role: BoundedName,
    /// The actions the holder runs, in name order.
    pub scope: frame_support::BoundedVec<BoundedName, MaxScope>,
    /// The moment the delegation landed, in milliseconds.
    pub granted_at: u64,
    /// The moment the delegation lapses, in milliseconds.
    pub expires_at: u64,
    /// The delegation this one hangs from. A first link names none.
    pub parent: Option<u64>,
    /// The grantor withdrew the delegation.
    pub withdrawn: bool,
}

impl<AccountId, MaxScope: frame_support::traits::Get<u32>> Delegation<AccountId, MaxScope> {
    /// Report whether the delegation names one action.
    pub fn covers(&self, action: &BoundedName) -> bool {
        self.scope.contains(action)
    }

    /// Report whether the delegation holds at one moment.
    pub fn holds_at(&self, now: u64) -> bool {
        !self.withdrawn && now < self.expires_at
    }
}

/// Why one delegation fails to carry an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub enum DelegationFault {
    /// No delegation names that action for that account.
    Absent,
    /// A link on the chain lapsed.
    Expired,
    /// A link on the chain was withdrawn.
    Withdrawn,
    /// A link passes an action the link above it lacks.
    OutOfScope,
    /// A grantor on the chain lost the role the chain runs under.
    RoleLost,
    /// The chain runs past the declared depth.
    DepthExceeded,
    /// A link names a delegation the registry lacks.
    ChainBroken,
}

impl DelegationFault {
    /// One sentence stating the problem.
    pub fn problem(&self) -> &'static str {
        match self {
            DelegationFault::Absent => "No delegation passes that action to the account.",
            DelegationFault::Expired => "The delegation lapsed.",
            DelegationFault::Withdrawn => "The grantor withdrew the delegation.",
            DelegationFault::OutOfScope => {
                "The delegation passes an action the delegation above it lacks."
            }
            DelegationFault::RoleLost => {
                "A grantor on the delegation chain lost the role the chain runs under."
            }
            DelegationFault::DepthExceeded => {
                "The delegation chain runs past the depth the Blockchain admits."
            }
            DelegationFault::ChainBroken => "The delegation names a delegation the registry lacks.",
        }
    }

    /// One sentence naming the fix.
    pub fn fix(&self) -> &'static str {
        match self {
            DelegationFault::Absent => "Ask the role holder to delegate the action.",
            DelegationFault::Expired => "Ask the grantor to delegate the action again.",
            DelegationFault::Withdrawn => "Ask the grantor to delegate the action again.",
            DelegationFault::OutOfScope => {
                "Delegate an action the delegation above it already carries."
            }
            DelegationFault::RoleLost => "Ask a present role holder to delegate the action.",
            DelegationFault::DepthExceeded => "Take the delegation from a holder nearer the role.",
            DelegationFault::ChainBroken => "Ask a present role holder to delegate the action.",
        }
    }

    /// The problem and the fix as one line.
    pub fn line(&self) -> String {
        let mut held = self.problem().to_string();
        held.push(' ');
        held.push_str(self.fix());
        held
    }
}

/// The outcome of resolving one action against the delegations of one
/// account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DelegationOutcome<AccountId> {
    /// The chain carries the action.
    Allowed {
        /// The delegation the holder acts under.
        delegation: u64,
        /// The role the chain runs under.
        role: BoundedName,
        /// The account at the top of the chain.
        source: AccountId,
        /// The earliest expiry on the chain, in milliseconds.
        expires_at: u64,
        /// The number of links the chain holds.
        depth: u32,
    },
    /// The chain declines the action.
    Denied(DelegationFault),
}

impl<AccountId> DelegationOutcome<AccountId> {
    /// Report whether the outcome carries the action.
    pub fn is_allowed(&self) -> bool {
        matches!(self, DelegationOutcome::Allowed { .. })
    }

    /// The fault, where the outcome declines the action.
    pub fn fault(&self) -> Option<DelegationFault> {
        match self {
            DelegationOutcome::Denied(held) => Some(*held),
            DelegationOutcome::Allowed { .. } => None,
        }
    }
}

/// Place one action name into a scope list, keeping the list in name
/// order and dropping a repeat.
pub fn place_scope(scope: &mut Vec<BoundedName>, action: BoundedName) {
    match scope.binary_search(&action) {
        Ok(_) => {}
        Err(position) => scope.insert(position, action),
    }
}

/// Report whether one scope sits inside another.
pub fn scope_within(inner: &[BoundedName], outer: &[BoundedName]) -> bool {
    inner.iter().all(|held| outer.contains(held))
}
