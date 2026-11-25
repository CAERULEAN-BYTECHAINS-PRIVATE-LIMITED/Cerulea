//! Test modules for DCF pallet
//!
//! This module organizes all test files into logical groups for better
//! maintainability and readability.

// Unit tests are integrated into integration_tests.rs

// Basic functionality and integration tests
pub mod integration_tests;
pub mod runtime_api_integration_tests;
pub mod system_integration_tests;

// Validator lifecycle and management tests
pub mod validator_lifecycle_edge_case_tests;

// Economic and accounting tests
pub mod accounting_tests;

// Governance and parameter tests
pub mod parameter_governance_tests;

// DoS protection and rate limiting tests
pub mod dos_protection_tests;

// Invariant and system health tests
pub mod invariant_tests;

// Performance and scalability tests
pub mod performance_tests;

// Deterministic processing tests
pub mod deterministic_tests;

// Finality and consensus tests
pub mod finality_tests;

// Genesis and deployment profile tests
pub mod genesis_validation_tests;

// Storage migration tests
pub mod storage_migration_tests;

// Trust score and stability tests
pub mod trust_score_stability_tests;

// Property and fuzz testing
pub mod comprehensive_property_tests;
pub mod property_tests;

// CI readiness and production tests
pub mod ci_readiness_tests;
pub mod production_readiness_tests;

// Cross-pallet integration and workflow tests
pub mod consensus_invariant_tests;
pub mod cross_pallet_integration_tests;
pub mod end_to_end_workflow_tests;
pub mod runtime_api_cross_pallet_tests;
pub mod stress_tests;
// Assurance Engine tests
pub mod assurance_engine_tests;

// Equivocation detection and penalty tests
pub mod equivocation_tests;

// Checkpoint anchoring tests
pub mod checkpoint_tests;

// Line coverage suites. Each file reaches one area of the pallet that the
// suites above leave unread.
pub mod coverage_calls_engines_tests;
pub mod coverage_calls_governance_tests;
pub mod coverage_epoch_helpers_tests;
pub mod coverage_hooks_genesis_tests;
pub mod coverage_runtime_api_tests;
pub mod coverage_support_modules_tests;
pub mod coverage_trust_parameter_tests;
pub mod coverage_weights_tests;

// ---------------------------------------------------------------------------
// Shared engine checks
// ---------------------------------------------------------------------------
//
// The rebuild moved validator standing from a stake derived score to the Trust
// Score. The Standing Engine composes the Trust Score from the components the
// network weighs. Every component is a share in parts per billion, so the
// total is a share as well. The helpers below read the engine through the
// pallet, so every suite in this module asserts the same rule the engine holds.

use crate::mock::{DcfPallet, Test};
use sp_runtime::Perbill;

/// Hold the Trust Score of one validator inside the bound the engine sets.
///
/// The total never passes one whole. Each component never passes one whole.
/// A component the network leaves out contributes nothing.
pub fn assert_trust_score_bounded(validator: &u64) {
    let score = DcfPallet::trust_score(validator);
    let whole = Perbill::one();

    assert!(
        score.total <= whole,
        "The Trust Score of validator {} passed one whole. Lower a component share before composing.",
        validator
    );
    for (name, share) in [
        ("standing", score.standing),
        ("performance", score.performance),
        ("custody", score.custody),
        ("sampling", score.sampling),
        ("role", score.role),
        ("attestation", score.attestation),
    ] {
        assert!(
            share <= whole,
            "The {} share of validator {} passed one whole. Clamp the component where it is recorded.",
            name,
            validator
        );
    }
}

/// Hold the Commit Engine weight of one validator inside the same bound.
///
/// The Commit Engine aggregates the Trust Score of every signer. The weight of
/// one validator therefore stays inside one whole share.
pub fn assert_trust_weight_bounded(validator: &u64) {
    let weight = DcfPallet::trust_weight(validator);
    assert!(
        weight <= Perbill::one().deconstruct() as u128,
        "The commit weight of validator {} passed one whole share. Lower the Trust Score that produced it.",
        validator
    );
}

/// The composition shares of the network this deployment runs.
///
/// The public-access chain weighs standing at 40 percent, performance at 30,
/// custody at 20 and sampling at 10. A private chain weighs role at 35,
/// attestation at 25, performance at 25 and custody at 15. Each set reaches one
/// hundred.
pub fn assert_composition_reaches_one_whole() {
    use crate::engines::standing::{
        ChainKind, PRIVATE_ATTESTATION_PERCENT, PRIVATE_CUSTODY_PERCENT,
        PRIVATE_PERFORMANCE_PERCENT, PRIVATE_ROLE_PERCENT, PUBLIC_CUSTODY_PERCENT,
        PUBLIC_PERFORMANCE_PERCENT, PUBLIC_SAMPLING_PERCENT, PUBLIC_STANDING_PERCENT,
    };

    assert_eq!(
        PUBLIC_STANDING_PERCENT
            + PUBLIC_PERFORMANCE_PERCENT
            + PUBLIC_CUSTODY_PERCENT
            + PUBLIC_SAMPLING_PERCENT,
        100
    );
    assert_eq!(
        PRIVATE_ROLE_PERCENT
            + PRIVATE_ATTESTATION_PERCENT
            + PRIVATE_PERFORMANCE_PERCENT
            + PRIVATE_CUSTODY_PERCENT,
        100
    );

    // The deployment reads one of the two networks.
    let kind = crate::DeploymentChainKind::<Test>::get();
    assert!(matches!(kind, ChainKind::PublicAccess | ChainKind::Private));
}
