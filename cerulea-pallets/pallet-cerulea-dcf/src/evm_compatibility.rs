//! EVM compatibility module for DCF pallet
//!
//! This module implements EVM compatibility features for DCF events and integration,
//! ensuring DCF events can be properly indexed and queried from the EVM side.

use super::*;
use codec::MaxEncodedLen;
use frame_support::BoundedVec;
use sp_core::ConstU32;
use sp_runtime::traits::TrailingZeroInput;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;

/// Maximum size for EVM-compatible event payloads (in bytes)
/// This limit ensures events don't exceed EVM log data limits
pub const MAX_EVM_EVENT_PAYLOAD_SIZE: u32 = 1024;

/// EVM event compatibility validator
pub struct EvmEventValidator;

impl EvmEventValidator {
    /// Check if EVM pallet is enabled in the runtime
    pub fn is_evm_enabled() -> bool {
        // Event conversion runs inside this pallet and depends on no other pallet.
        true
    }

    /// Validate that an event payload is compatible with EVM indexing
    pub fn validate_event_payload(payload: &[u8]) -> Result<(), EvmCompatibilityError> {
        // Check payload size limit
        if payload.len() > MAX_EVM_EVENT_PAYLOAD_SIZE as usize {
            return Err(EvmCompatibilityError::PayloadTooLarge {
                actual_size: payload.len() as u32,
                max_size: MAX_EVM_EVENT_PAYLOAD_SIZE,
            });
        }

        // Check for EVM-incompatible data structures
        Self::validate_data_structure(payload)?;

        Ok(())
    }

    /// Validate that the data structure is EVM-compatible
    fn validate_data_structure(payload: &[u8]) -> Result<(), EvmCompatibilityError> {
        // Check for extremely nested structures that might cause stack overflow
        let nesting_level = Self::calculate_nesting_level(payload);
        if nesting_level > 10 {
            return Err(EvmCompatibilityError::ExcessiveNesting {
                level: nesting_level,
                max_level: 10,
            });
        }

        Ok(())
    }

    /// Calculate the nesting level of encoded data
    fn calculate_nesting_level(payload: &[u8]) -> u32 {
        // Simple heuristic: count consecutive opening brackets/braces
        let mut max_nesting = 0;
        let mut current_nesting = 0;

        for &byte in payload {
            match byte {
                b'[' | b'{' | b'(' => {
                    current_nesting += 1;
                    max_nesting = max_nesting.max(current_nesting);
                }
                b']' | b'}' | b')' => {
                    current_nesting = current_nesting.saturating_sub(1);
                }
                _ => {}
            }
        }

        max_nesting
    }

    /// Convert DCF event to EVM-compatible format
    pub fn convert_to_evm_format<T: Config>(
        event: &Event<T>,
    ) -> Result<EvmCompatibleEvent, EvmCompatibilityError> {
        let encoded = event.encode();
        Self::validate_event_payload(&encoded)?;

        let event_type = Self::get_event_type_id(event);
        let indexed_fields = Self::extract_indexed_fields(event)?;
        let data_fields = Self::extract_data_fields(event)?;

        let encoded_len = encoded.len();
        let raw_data =
            BoundedVec::try_from(encoded).map_err(|_| EvmCompatibilityError::PayloadTooLarge {
                actual_size: encoded_len as u32,
                max_size: 2048,
            })?;

        Ok(EvmCompatibleEvent {
            event_type,
            indexed_fields,
            data_fields,
            raw_data,
        })
    }

    /// Get numeric event type ID for EVM indexing
    fn get_event_type_id<T: Config>(event: &Event<T>) -> u32 {
        match event {
            Event::ValidatorJoined { .. } => 1,
            Event::ValidatorLeft { .. } => 2,
            Event::ValidatorEjected { .. } => 5,
            Event::EpochStarted { .. } => 6,
            Event::ProposalSubmitted { .. } => 7,
            Event::ProposalExecuted { .. } => 8,
            Event::ValidatorScoreUpdated { .. } => 9,
            Event::ConsensusWeightsUpdated { .. } => 10,
            Event::GovernanceModeToggled { .. } => 11,
            Event::ValidatorMisbehaviorReported { .. } => 12,
            Event::InvariantViolationsDetected { .. } => 13,
            Event::PrivateChainModeEnabled { .. } => 14,
            Event::PrivateChainModeDisabled => 15,
            Event::ValidatorAddedToAllowlist { .. } => 16,
            Event::ValidatorRemovedFromAllowlist { .. } => 17,
            Event::ValidatorForcedToLeave { .. } => 18,
            _ => 999, // Unknown event type
        }
    }

    /// Extract indexed fields for EVM event filtering
    fn extract_indexed_fields<T: Config>(
        event: &Event<T>,
    ) -> Result<BoundedVec<BoundedVec<u8, ConstU32<32>>, ConstU32<3>>, EvmCompatibilityError> {
        let mut indexed_fields = BoundedVec::new();

        match event {
            Event::ValidatorJoined { validator, .. }
            | Event::ValidatorLeft { validator, .. }
            | Event::ValidatorEjected { validator, .. } => {
                let encoded = validator.encode();
                if encoded.len() <= 32 {
                    let bounded_field = BoundedVec::try_from(encoded).unwrap_or_default();
                    let _ = indexed_fields.try_push(bounded_field);
                }
            }
            Event::EpochStarted { epoch, .. } => {
                let encoded = epoch.encode();
                if encoded.len() <= 32 {
                    let bounded_field = BoundedVec::try_from(encoded).unwrap_or_default();
                    let _ = indexed_fields.try_push(bounded_field);
                }
            }
            Event::ProposalSubmitted {
                proposal_id,
                proposer,
                ..
            } => {
                let id_encoded = proposal_id.encode();
                if id_encoded.len() <= 32 {
                    let bounded_field = BoundedVec::try_from(id_encoded).unwrap_or_default();
                    let _ = indexed_fields.try_push(bounded_field);
                }

                let proposer_encoded = proposer.encode();
                if proposer_encoded.len() <= 32 && indexed_fields.len() < 3 {
                    let bounded_field = BoundedVec::try_from(proposer_encoded).unwrap_or_default();
                    let _ = indexed_fields.try_push(bounded_field);
                }
            }
            Event::ProposalExecuted { proposal_id, .. } => {
                let encoded = proposal_id.encode();
                if encoded.len() <= 32 {
                    let bounded_field = BoundedVec::try_from(encoded).unwrap_or_default();
                    let _ = indexed_fields.try_push(bounded_field);
                }
            }
            _ => {
                // For other events, no specific indexed fields
            }
        }

        Ok(indexed_fields)
    }

    /// Extract data fields for EVM event data
    fn extract_data_fields<T: Config>(
        event: &Event<T>,
    ) -> Result<BoundedVec<u8, ConstU32<1024>>, EvmCompatibilityError> {
        // For simplicity, encode the entire event as data
        // In a real implementation, you might want to extract specific fields
        let encoded = event.encode();
        Self::validate_event_payload(&encoded)?;

        let encoded_len = encoded.len();
        BoundedVec::try_from(encoded).map_err(|_| EvmCompatibilityError::PayloadTooLarge {
            actual_size: encoded_len as u32,
            max_size: 1024,
        })
    }
}

/// Build a bounded byte field from a short text sample.
///
/// The bound comes from the event field the text fills. Text beyond the bound is cut off.
fn sample_text<S: frame_support::traits::Get<u32>>(text: &[u8]) -> BoundedVec<u8, S> {
    BoundedVec::truncate_from(text.to_vec())
}

/// EVM-compatible event structure
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct EvmCompatibleEvent {
    /// Numeric event type ID for filtering
    pub event_type: u32,
    /// Indexed fields for EVM event filtering (max 3 fields, 32 bytes each)
    pub indexed_fields: BoundedVec<BoundedVec<u8, ConstU32<32>>, ConstU32<3>>,
    /// Event data payload
    pub data_fields: BoundedVec<u8, ConstU32<1024>>,
    /// Raw encoded event data
    pub raw_data: BoundedVec<u8, ConstU32<2048>>,
}

/// EVM compatibility error types
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub enum EvmCompatibilityError {
    /// Event payload exceeds EVM size limits
    PayloadTooLarge { actual_size: u32, max_size: u32 },
    /// Event data has excessive nesting that may cause stack overflow
    ExcessiveNesting { level: u32, max_level: u32 },
    /// Indexed field exceeds EVM topic size limit
    IndexedFieldTooLarge { field_size: u32, max_size: u32 },
    /// Event type not supported for EVM compatibility
    UnsupportedEventType,
    /// Event encoding failed
    EncodingFailed,
}

impl<T: Config> Pallet<T> {
    /// Check if EVM pallet is enabled in the runtime
    pub fn is_evm_enabled() -> bool {
        // Event conversion runs inside this pallet and depends on no other pallet.
        true
    }

    /// Emit EVM-compatible event if EVM pallet is enabled
    pub fn emit_evm_compatible_event(event: &Event<T>) -> Result<(), EvmCompatibilityError> {
        if !Self::is_evm_enabled() {
            return Ok(()); // Skip if EVM not enabled
        }

        let evm_event = EvmEventValidator::convert_to_evm_format(event)?;

        // Store the EVM-compatible event for potential EVM queries
        Self::store_evm_event(evm_event)?;

        Ok(())
    }

    /// Store EVM-compatible event for later retrieval
    fn store_evm_event(evm_event: EvmCompatibleEvent) -> Result<(), EvmCompatibilityError> {
        let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();

        // Store in a bounded vec to prevent unbounded growth
        EvmCompatibleEvents::<T>::mutate(current_block, |events| {
            let _ = events.try_push(evm_event);
        });

        Ok(())
    }

    /// Get EVM-compatible events for a specific block
    pub fn get_evm_events_for_block(block_number: u32) -> Vec<EvmCompatibleEvent> {
        EvmCompatibleEvents::<T>::get(block_number).to_vec()
    }

    /// Query EVM events by type and block range
    pub fn query_evm_events(
        event_type: Option<u32>,
        from_block: u32,
        to_block: u32,
    ) -> Vec<(u32, EvmCompatibleEvent)> {
        let mut results = Vec::new();

        for block_num in from_block..=to_block {
            let events = Self::get_evm_events_for_block(block_num);

            for event in events {
                if let Some(filter_type) = event_type {
                    if event.event_type == filter_type {
                        results.push((block_num, event));
                    }
                } else {
                    results.push((block_num, event));
                }
            }
        }

        results
    }

    /// Validate every DCF event variant for EVM compatibility.
    ///
    /// The check converts one sample of each variant into the Ethereum Virtual Machine log
    /// format. A failed conversion returns its error.
    ///
    /// The check then proves coverage. The first encoded byte of each sample is its variant
    /// index. Each sample must carry a distinct index. The sampled indices must equal the
    /// indices the event metadata declares. Equal sets also prove equal counts. A missing or
    /// repeated variant returns `UnsupportedEventType`.
    pub fn validate_all_events_evm_compatible() -> Result<(), EvmCompatibilityError> {
        let samples = Self::sample_events()?;
        let mut sampled_indices = BTreeSet::new();

        for event in &samples {
            let evm_event = EvmEventValidator::convert_to_evm_format(event)?;
            let index = evm_event
                .raw_data
                .first()
                .copied()
                .ok_or(EvmCompatibilityError::EncodingFailed)?;
            if !sampled_indices.insert(index) {
                return Err(EvmCompatibilityError::UnsupportedEventType);
            }
        }

        let declared_indices = match <Event<T> as scale_info::TypeInfo>::type_info().type_def {
            scale_info::TypeDef::Variant(definition) => definition
                .variants
                .iter()
                .map(|variant| variant.index)
                .collect::<BTreeSet<u8>>(),
            _ => return Err(EvmCompatibilityError::UnsupportedEventType),
        };

        if sampled_indices != declared_indices {
            return Err(EvmCompatibilityError::UnsupportedEventType);
        }

        Ok(())
    }

    /// One sample of every event variant, in declaration order.
    ///
    /// Each sample carries realistic values. The values include a validator account, a block
    /// number, a score of 75 and a proposal identifier of 1. Every encoded sample stays well
    /// inside the payload limit.
    ///
    /// A new event variant needs a sample here. The coverage check in
    /// `validate_all_events_evm_compatible` fails until the sample exists.
    pub fn sample_events() -> Result<Vec<Event<T>>, EvmCompatibilityError> {
        let account = <T::AccountId as codec::Decode>::decode(&mut TrailingZeroInput::zeroes())
            .map_err(|_| EvmCompatibilityError::EncodingFailed)?;
        let peer = <T::AccountId as codec::Decode>::decode(&mut TrailingZeroInput::new(&[1u8; 32]))
            .map_err(|_| EvmCompatibilityError::EncodingFailed)?;

        let stake: <T as Config>::Balance = 1_000u32.into();
        let low_stake: <T as Config>::Balance = 500u32.into();
        let total_stake: <T as Config>::Balance = 2_000u32.into();
        let reward: <T as Config>::Balance = 100u32.into();
        let slashed: <T as Config>::Balance = 50u32.into();

        let output_hash = [7u8; 32];
        let randomness_seed = [3u8; 32];
        let commitment = [9u8; 32];

        let violation = InvariantViolation::Temporal {
            violation_type: TemporalViolationType::EpochRegression {
                previous_epoch: 2,
                current_epoch: 1,
            },
            context: sample_text(b"epoch counter moved back"),
        };

        Ok(sp_std::vec![
            Event::ValidatorScoreUpdated {
                validator: account.clone(),
                stake_score: 75,
                inference_score: 70,
                final_score: 73,
            },
            Event::ConsensusWeightsUpdated {
                pos_weight: 6_000,
                poi_weight: 4_000,
            },
            Event::EpochStarted {
                epoch: 1,
                validators: sp_std::vec![account.clone(), peer.clone()],
            },
            Event::EpochEnded { epoch: 1 },
            Event::ValidatorScoreDecayed {
                validator: account.clone(),
                old_score: 80,
                new_score: 75,
            },
            Event::ValidatorScoreBoosted {
                validator: account.clone(),
                old_score: 75,
                new_score: 80,
                reason: ScoreBoostReason::ValidBlockAuthored,
            },
            Event::ValidatorEjected {
                validator: peer.clone(),
                reason: EjectionReason::ScoreBelowThreshold,
            },
            Event::ValidatorReEntered {
                validator: peer.clone(),
                score: 75,
            },
            Event::AuthorMismatch {
                block_number: 10,
                expected: Some(account.clone()),
                actual: peer.clone(),
            },
            Event::AuthorValidationFailed {
                block_number: 10,
                expected: Some(account.clone()),
                actual: peer.clone(),
                reason: sample_text(b"author holds no seat in this slot"),
            },
            Event::InsufficientStake {
                validator: peer.clone(),
                required: stake,
                available: low_stake,
            },
            Event::InvalidAuthor {
                block_number: 10,
                author: peer.clone(),
            },
            Event::GovernanceModeToggled { enabled: true },
            Event::ProposalSubmitted {
                proposal_id: 1,
                proposer: account.clone(),
                action: ProposalAction::AddValidator {
                    validator: peer.clone(),
                },
            },
            Event::ProposalExecuted {
                proposal_id: 1,
                status: ProposalStatus::Executed,
            },
            Event::ProposalVoted {
                proposal_id: 1,
                voter: peer.clone(),
                approve: true,
            },
            Event::ProposalPassed { proposal_id: 1 },
            Event::ProposalRejected { proposal_id: 2 },
            Event::ValidatorJoinRequested {
                validator: account.clone(),
                stake_amount: stake,
            },
            Event::ValidatorJoined {
                validator: account.clone(),
                stake_amount: stake,
            },
            Event::ValidatorLeft {
                validator: account.clone(),
            },
            Event::ValidatorLeaveRequested {
                validator: account.clone(),
                cooldown_expires_at: 110,
            },
            Event::ValidatorLeaveCancelled {
                validator: account.clone(),
            },
            Event::ValidatorPoiScoreUpdated {
                validator: account.clone(),
                poi_score: 70,
            },
            Event::ValidatorMetadataUpdated {
                validator: account.clone(),
                name: sample_text(b"validator one"),
            },
            Event::ValidatorActivityUpdated {
                validator: account.clone(),
                blocks_authored: 19,
                blocks_missed: 1,
                uptime_percentage: 9_500,
            },
            Event::AutomaticValidatorProposal {
                validator: peer.clone(),
                action: ValidatorAction::Join,
                score: 75,
                reason: sample_text(b"score above the entry floor"),
            },
            Event::ValidatorAdded {
                validator: peer.clone(),
            },
            Event::ValidatorRemoved {
                validator: peer.clone(),
            },
            Event::ValidatorMisbehaviorReported {
                reported_validator: peer.clone(),
                reporter: account.clone(),
                evidence: sample_text(b"two headers for one slot"),
                total_reports: 1,
            },
            Event::ValidatorAutoSlashed {
                validator: peer.clone(),
                amount: slashed,
                report_count: 3,
            },
            Event::InferenceSimulated {
                validator: account.clone(),
                triggered_by: peer.clone(),
            },
            Event::EpochTransition {
                old_epoch: 1,
                new_epoch: 2,
                active_validators: sp_std::vec![account.clone(), peer.clone()],
                removed_validators: Vec::new(),
                added_validators: sp_std::vec![peer.clone()],
                total_validators: 2,
                average_performance_score: 75,
                total_rewards_distributed: reward,
                total_slashed_amount: slashed,
            },
            Event::EpochBoundaryDetected {
                block_number: 10,
                epoch: 1,
                governance_mode: false,
            },
            Event::ValidatorNameRegistered {
                validator: account.clone(),
                name: sample_text(b"validator one"),
            },
            Event::ProposalCreated {
                proposal_id: 1,
                proposer: account.clone(),
                action: ProposalAction::Reward {
                    validator: peer.clone(),
                    amount: reward,
                },
                description: sample_text(b"reward the validator for steady service"),
            },
            Event::ValidatorScoreUpdatedDetailed {
                validator: account.clone(),
                old_stake_score: 70,
                new_stake_score: 75,
                old_inference_score: 65,
                new_inference_score: 70,
                old_final_score: 68,
                new_final_score: 73,
                epoch: 1,
            },
            Event::TrustScoreUpdated {
                validator: account.clone(),
                old_score: 70,
                new_score: 75,
                uptime_component: 30,
                inference_component: 25,
                slashing_component: 0,
            },
            Event::CooldownExpired {
                validator: account.clone(),
                cooldown_type: sample_text(b"Leave"),
                expired_at_block: 110,
            },
            Event::StakeOperationFailed {
                validator: peer.clone(),
                operation: sample_text(b"Reserve"),
                reason: sample_text(b"free balance below the reserve amount"),
            },
            Event::TrustScoreDecayed {
                validator: account.clone(),
                old_score: 80,
                new_score: 75,
                decay_factor: 5,
            },
            Event::DcfParameterUpdated {
                parameter: ParameterType::EpochLength,
                old_value: sample_text(&100u32.encode()),
                new_value: sample_text(&200u32.encode()),
            },
            Event::InvariantViolationsDetected {
                epoch: 2,
                violations: BoundedVec::truncate_from(sp_std::vec![violation]),
                severity: InvariantSeverity::High,
            },
            Event::InvariantReportGenerated {
                epoch: 2,
                block_number: 20,
                violations_count: 1,
                severity: InvariantSeverity::High,
            },
            Event::StorageMigrationCompleted {
                from_version: 1,
                to_version: 2,
            },
            Event::StorageMigrationFailed {
                from_version: 1,
                to_version: 2,
                error: sample_text(b"validator record failed to decode"),
            },
            Event::StorageValidationFailed {
                version: 2,
                error: sample_text(b"stake total differs from the reserved sum"),
            },
            Event::RateLimitViolation {
                account: account.clone(),
                operation: 0,
                violation_type: 0,
                current_count: 6,
                limit: 5,
            },
            Event::RateLimitConfigUpdated {
                max_proposals_per_block: 5,
                max_joins_per_block: 3,
                max_leaves_per_block: 3,
            },
            Event::BlockRateLimitCountersReset { block_number: 10 },
            Event::DeterministicEpochProcessed {
                epoch: 2,
                block_number: 20,
                randomness_seed,
                author_sequence_length: 4,
                output_hash,
            },
            Event::EpochReplayValidated {
                epoch: 2,
                validation_passed: true,
                original_hash: output_hash,
                replay_hash: output_hash,
            },
            Event::DeterministicAuthorSequenceGenerated {
                epoch: 2,
                sequence_length: 4,
                randomness_seed,
                first_author: account.clone(),
            },
            Event::GenesisValidationSucceeded {
                validator_count: 2,
                total_stake,
            },
            Event::GenesisValidationFailed {
                error: sample_text(b"duplicate validator in the genesis list"),
            },
            Event::GenesisDryRunCompleted {
                validator_count: 2,
                total_stake,
                invariant_checks_passed: 12,
                warnings_count: 0,
            },
            Event::GenesisDryRunFailed {
                error: sample_text(b"active set below the minimum size"),
            },
            Event::ApiVersionChanged {
                old_version: 1,
                new_version: 2,
                breaking_changes: sample_text(b"score query returns the Trust Score parts"),
            },
            Event::PrivateChainModeEnabled {
                allowlist_size: 2,
                allow_updates: true,
            },
            Event::PrivateChainModeDisabled,
            Event::ValidatorAddedToAllowlist {
                validator: account.clone(),
            },
            Event::ValidatorRemovedFromAllowlist {
                validator: peer.clone(),
            },
            Event::ValidatorForcedToLeave {
                validator: peer.clone(),
                reason: b"removed from the allowlist".to_vec(),
            },
            Event::EvmCompatibilityTested { success: true },
            Event::EvmEventsQueried {
                from_block: 1,
                to_block: 10,
                event_type: Some(1),
                result_count: 3,
            },
            Event::BlockAuthored {
                block_number: 10,
                author: account.clone(),
            },
            Event::MissedBlock {
                block_number: 11,
                validator: peer.clone(),
            },
            Event::ValidatorStatusChanged {
                validator: account.clone(),
                old_status: ValidatorStatus::Active,
                new_status: ValidatorStatus::Leaving,
                block_number: 10,
            },
            Event::ValidatorUptimeUpdated {
                validator: account.clone(),
                epoch: 1,
                blocks_expected: 20,
                blocks_authored: 19,
                blocks_missed: 1,
                uptime_percentage: 9_500,
            },
            Event::AssuranceClassTableAmended {
                class: crate::engines::assurance::TransactionClass::Financial,
                previous_percent: 67,
                current_percent: 70,
                delay_blocks: 0,
            },
            Event::ScheduledChangeOpened {
                id: 1,
                class: crate::engines::assurance::TransactionClass::Constitutional,
                committed_at: 10,
                effective_at: 110,
            },
            Event::ScheduledChangeReleased {
                id: 1,
                class: crate::engines::assurance::TransactionClass::Constitutional,
                released_at: 110,
            },
            Event::CustodyShareRecorded {
                validator: account.clone(),
                share: 950_000,
            },
            Event::CustodyFaultRecorded {
                validator: peer.clone(),
                faults: 1,
            },
            Event::ServiceEpochCounted {
                validator: account.clone(),
                epoch: 2,
                epochs_served: 2,
                standing: sp_runtime::Perbill::from_percent(75),
            },
            Event::SamplingReportSettled {
                validator: account.clone(),
                outcome: crate::engines::standing::SampleOutcome::Agreed,
                sampling: sp_runtime::Perbill::from_percent(75),
            },
            Event::StandingDecayed {
                validator: account.clone(),
                idle_epochs: 3,
                standing: sp_runtime::Perbill::from_percent(70),
            },
            Event::StandingReset {
                validator: peer.clone(),
                misconduct: crate::engines::standing::Misconduct::FalseCustodyProof,
                epochs_destroyed: 2,
            },
            Event::CheckpointAnchored {
                index: 0,
                block: 10,
                hash: output_hash,
                commitment,
            },
            Event::EquivocationProven {
                validator: peer.clone(),
                slot: 12,
                rank: 0,
                epochs_destroyed: 2,
            },
            Event::AmendmentOpened {
                id: 1,
                policy: crate::engines::amendment::PolicyKind::Assurance,
                approvals_required: 3,
                effective_at: 110,
            },
            Event::AmendmentApproved {
                id: 1,
                approver: account.clone(),
                approvals: 1,
            },
            Event::AmendmentEnacted {
                id: 1,
                policy: crate::engines::amendment::PolicyKind::Assurance,
            },
        ])
    }

    /// Clean up old EVM events to prevent storage bloat
    pub fn cleanup_old_evm_events(keep_blocks: u32) {
        let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
        let cutoff_block = current_block.saturating_sub(keep_blocks);

        // Remove events older than cutoff by iterating and removing individual entries
        for block_num in 0..cutoff_block {
            EvmCompatibleEvents::<T>::remove(block_num);
        }
    }
}

// Storage is defined in the main lib.rs file

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::*;
    use frame_support::assert_ok;

    #[test]
    fn test_event_payload_size_validation() {
        // Test payload within limits
        let small_payload = vec![1u8; 100];
        assert_ok!(EvmEventValidator::validate_event_payload(&small_payload));

        // Test payload exceeding limits
        let large_payload = vec![1u8; MAX_EVM_EVENT_PAYLOAD_SIZE as usize + 1];
        assert!(EvmEventValidator::validate_event_payload(&large_payload).is_err());
    }

    #[test]
    fn test_nesting_level_calculation() {
        // Test simple nesting
        let simple_data = b"[1,2,3]";
        assert_eq!(EvmEventValidator::calculate_nesting_level(simple_data), 1);

        // Test nested structures
        let nested_data = b"[[1,[2,3]],4]";
        assert_eq!(EvmEventValidator::calculate_nesting_level(nested_data), 3);

        // Test excessive nesting
        let excessive_nesting = b"[[[[[[[[[[[1]]]]]]]]]]]";
        assert!(EvmEventValidator::calculate_nesting_level(excessive_nesting) > 10);
    }

    #[test]
    fn test_event_type_id_assignment() {
        new_test_ext().execute_with(|| {
            // Test that different events get different type IDs
            let join_event = Event::<Test>::ValidatorJoined {
                validator: 1u64,
                stake_amount: 1000,
            };
            let leave_event = Event::<Test>::ValidatorLeft { validator: 1u64 };

            let join_id = EvmEventValidator::get_event_type_id(&join_event);
            let leave_id = EvmEventValidator::get_event_type_id(&leave_event);

            assert_ne!(join_id, leave_id);
            assert!(join_id > 0);
            assert!(leave_id > 0);
        });
    }

    #[test]
    fn test_evm_event_storage_and_retrieval() {
        new_test_ext().execute_with(|| {
            let block_number = 100u32;
            let sample_event = EvmCompatibleEvent {
                event_type: 1,
                indexed_fields: BoundedVec::try_from(vec![BoundedVec::try_from(vec![
                    1u8, 2u8, 3u8,
                ])
                .unwrap_or_default()])
                .unwrap_or_default(),
                data_fields: BoundedVec::try_from(vec![4u8, 5u8, 6u8]).unwrap_or_default(),
                raw_data: BoundedVec::try_from(vec![1u8, 2u8, 3u8, 4u8, 5u8, 6u8])
                    .unwrap_or_default(),
            };

            // Store event
            EvmCompatibleEvents::<Test>::mutate(block_number, |events| {
                let _ = events.try_push(sample_event.clone());
            });

            // Retrieve event
            let retrieved_events = DcfPallet::get_evm_events_for_block(block_number);
            assert_eq!(retrieved_events.len(), 1);
            assert_eq!(retrieved_events[0], sample_event);
        });
    }

    #[test]
    fn test_evm_event_querying() {
        new_test_ext().execute_with(|| {
            // Store events in multiple blocks
            for block in 100..=102 {
                let event = EvmCompatibleEvent {
                    event_type: if block % 2 == 0 { 1 } else { 2 },
                    indexed_fields: BoundedVec::new(),
                    data_fields: BoundedVec::new(),
                    raw_data: BoundedVec::new(),
                };

                EvmCompatibleEvents::<Test>::mutate(block, |events| {
                    let _ = events.try_push(event);
                });
            }

            // Query all events
            let all_events = DcfPallet::query_evm_events(None, 100, 102);
            assert_eq!(all_events.len(), 3);

            // Query events by type
            let type1_events = DcfPallet::query_evm_events(Some(1), 100, 102);
            assert_eq!(type1_events.len(), 2); // blocks 100 and 102

            let type2_events = DcfPallet::query_evm_events(Some(2), 100, 102);
            assert_eq!(type2_events.len(), 1); // block 101
        });
    }

    #[test]
    fn test_evm_event_cleanup() {
        new_test_ext().execute_with(|| {
            // Store events in old blocks
            for block in 1..=10 {
                let event = EvmCompatibleEvent {
                    event_type: 1,
                    indexed_fields: BoundedVec::new(),
                    data_fields: BoundedVec::new(),
                    raw_data: BoundedVec::new(),
                };

                EvmCompatibleEvents::<Test>::mutate(block, |events| {
                    let _ = events.try_push(event);
                });
            }

            // Verify events exist
            assert!(!DcfPallet::get_evm_events_for_block(5).is_empty());

            // Clean up old events (keep only last 3 blocks)
            frame_system::Pallet::<Test>::set_block_number(10);
            DcfPallet::cleanup_old_evm_events(3);

            // Verify old events are cleaned up
            assert!(DcfPallet::get_evm_events_for_block(5).is_empty());

            // Verify recent events are kept
            assert!(!DcfPallet::get_evm_events_for_block(9).is_empty());
        });
    }
}
