//! Plain-language failure explanation.
//!
//! A rule carries two sentences the person who built the deployment wrote.
//! One sentence states the problem. One sentence states the fix. A rule
//! that declines a change answers with those two sentences rather than
//! with the formal expression.
//!
//! The sentences travel out of the module in [`RuleFailure`]. Every caller
//! keeps them and shows them unchanged, so the person reading the screen
//! sees the words written beside the rule in the Studio.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use cerulea_blueprint::{
    rules::{Invariant, Precondition},
    Diagnostic,
};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use pallet_cerulea_schema::primitives::BoundedName;

/// The largest sentence the module stores, in bytes.
pub const MAX_SENTENCE_BYTES: u32 = 512;

/// A bounded plain sentence.
pub type Sentence = BoundedVec<u8, ConstU32<MAX_SENTENCE_BYTES>>;

/// Which kind of rule declined the work.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum RuleKind {
    /// A condition evaluated before an action runs.
    Precondition,
    /// A condition evaluated after every state change.
    Invariant,
}

impl RuleKind {
    /// A word naming the kind for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            RuleKind::Precondition => "precondition",
            RuleKind::Invariant => "invariant",
        }
    }
}

/// The plain statement of one refused change.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct RuleFailure {
    /// The rule that declined the work.
    pub rule: BoundedName,
    /// Which kind of rule declined the work.
    pub kind: RuleKind,
    /// The sentence the person wrote stating the problem.
    pub problem: Sentence,
    /// The sentence the person wrote stating the fix.
    pub fix: Sentence,
}

impl RuleFailure {
    /// Build a failure from two sentences.
    pub fn new(rule: BoundedName, kind: RuleKind, problem: &str, fix: &str) -> Self {
        RuleFailure {
            rule,
            kind,
            problem: BoundedVec::truncate_from(problem.as_bytes().to_vec()),
            fix: BoundedVec::truncate_from(fix.as_bytes().to_vec()),
        }
    }

    /// Build the failure of one precondition from the rule itself.
    pub fn of_precondition(rule: BoundedName, held: &Precondition) -> Self {
        RuleFailure::new(
            rule,
            RuleKind::Precondition,
            held.problem.as_str(),
            held.fix.as_str(),
        )
    }

    /// Build the failure of one invariant from the rule itself.
    pub fn of_invariant(rule: BoundedName, held: &Invariant) -> Self {
        RuleFailure::new(
            rule,
            RuleKind::Invariant,
            held.problem.as_str(),
            held.fix.as_str(),
        )
    }

    /// Build the failure of a rule whose expression faulted.
    ///
    /// The evaluator already states the problem and the fix in plain
    /// language, so the failure carries those two sentences.
    pub fn of_diagnostic(rule: BoundedName, kind: RuleKind, diagnostic: &Diagnostic) -> Self {
        RuleFailure::new(
            rule,
            kind,
            diagnostic.problem.as_str(),
            diagnostic.fix.as_str(),
        )
    }

    /// The problem sentence as text.
    pub fn problem_text(&self) -> String {
        String::from_utf8_lossy(self.problem.as_slice()).into_owned()
    }

    /// The fix sentence as text.
    pub fn fix_text(&self) -> String {
        String::from_utf8_lossy(self.fix.as_slice()).into_owned()
    }

    /// The two sentences joined, ready for one line on screen.
    pub fn line(&self) -> String {
        let mut held = self.problem_text();
        held.push(' ');
        held.push_str(&self.fix_text());
        held
    }

    /// The two sentences as bytes, ready for the audit payload.
    pub fn bytes(&self) -> Vec<u8> {
        self.line().to_string().into_bytes()
    }
}
