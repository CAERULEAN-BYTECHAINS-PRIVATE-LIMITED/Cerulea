//! Test modules for DCF pallet
//!
//! This module organizes all test files into logical groups for better
//! maintainability and readability.

// Unit tests are integrated into integration_tests.rs

// Basic functionality and integration tests
pub mod integration_tests;
pub mod system_integration_tests;
pub mod runtime_api_integration_tests;

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

// Genesis and configuration tests
pub mod genesis_validation_tests;

// Storage migration tests
pub mod storage_migration_tests;

// Trust score and stability tests
pub mod trust_score_stability_tests;

// Property and fuzz testing
pub mod property_tests;
pub mod comprehensive_property_tests;

// CI readiness and production tests
pub mod ci_readiness_tests;
pub mod production_readiness_tests;

// Cross-pallet integration and workflow tests
pub mod cross_pallet_integration_tests;
pub mod consensus_invariant_tests;
pub mod end_to_end_workflow_tests;
pub mod stress_tests;
pub mod runtime_api_cross_pallet_tests;