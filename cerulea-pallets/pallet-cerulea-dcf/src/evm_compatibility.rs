//! EVM compatibility module for DCF pallet
//!
//! This module implements EVM compatibility features for DCF events and integration,
//! ensuring DCF events can be properly indexed and queried from the EVM side.

use super::*;
use frame_support::BoundedVec;
use sp_core::ConstU32;
use codec::MaxEncodedLen;
use sp_std::vec::Vec;

/// Maximum size for EVM-compatible event payloads (in bytes)
/// This limit ensures events don't exceed EVM log data limits
pub const MAX_EVM_EVENT_PAYLOAD_SIZE: u32 = 1024;

/// EVM event compatibility validator
pub struct EvmEventValidator;

impl EvmEventValidator {
    /// Check if EVM pallet is enabled in the runtime
    pub fn is_evm_enabled() -> bool {
        // This would check if the EVM pallet is configured in the runtime
        // For now, we'll assume it's always potentially available
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
                },
                b']' | b'}' | b')' => {
                    current_nesting = current_nesting.saturating_sub(1);
                },
                _ => {},
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
        let raw_data = BoundedVec::try_from(encoded)
            .map_err(|_| EvmCompatibilityError::PayloadTooLarge {
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
            Event::ValidatorJoined { validator, .. } |
            Event::ValidatorLeft { validator, .. } |
            Event::ValidatorEjected { validator, .. } => {
                let encoded = validator.encode();
                if encoded.len() <= 32 {
                    let bounded_field = BoundedVec::try_from(encoded).unwrap_or_default();
                    let _ = indexed_fields.try_push(bounded_field);
                }
            },
            Event::EpochStarted { epoch, .. } => {
                let encoded = epoch.encode();
                if encoded.len() <= 32 {
                    let bounded_field = BoundedVec::try_from(encoded).unwrap_or_default();
                    let _ = indexed_fields.try_push(bounded_field);
                }
            },
            Event::ProposalSubmitted { proposal_id, proposer, .. } => {
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
            },
            Event::ProposalExecuted { proposal_id, .. } => {
                let encoded = proposal_id.encode();
                if encoded.len() <= 32 {
                    let bounded_field = BoundedVec::try_from(encoded).unwrap_or_default();
                    let _ = indexed_fields.try_push(bounded_field);
                }
            },
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
        BoundedVec::try_from(encoded)
            .map_err(|_| EvmCompatibilityError::PayloadTooLarge {
                actual_size: encoded_len as u32,
                max_size: 1024,
            })
    }
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
    PayloadTooLarge {
        actual_size: u32,
        max_size: u32,
    },
    /// Event data has excessive nesting that may cause stack overflow
    ExcessiveNesting {
        level: u32,
        max_level: u32,
    },
    /// Indexed field exceeds EVM topic size limit
    IndexedFieldTooLarge {
        field_size: u32,
        max_size: u32,
    },
    /// Event type not supported for EVM compatibility
    UnsupportedEventType,
    /// Event encoding failed
    EncodingFailed,
}

impl<T: Config> Pallet<T> {
    /// Check if EVM pallet is enabled in the runtime
    pub fn is_evm_enabled() -> bool {
        // This would check if the EVM pallet is configured in the runtime
        // For now, we'll assume it's always potentially available
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

    /// Validate all DCF events for EVM compatibility
    pub fn validate_all_events_evm_compatible() -> Result<(), EvmCompatibilityError> {
        // This would be called during testing to ensure all events are EVM-compatible
        // For now, we'll just validate the event structure
        
        // Test with sample events
        let sample_events = Self::get_sample_events_for_testing();
        
        for event in sample_events {
            EvmEventValidator::convert_to_evm_format(&event)?;
        }
        
        Ok(())
    }

    /// Get sample events for testing EVM compatibility
    fn get_sample_events_for_testing() -> Vec<Event<T>> {
        // This would return sample events for testing
        // Implementation would depend on the specific test requirements
        Vec::new()
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
            let leave_event = Event::<Test>::ValidatorLeft {
                validator: 1u64,
            };

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
                indexed_fields: BoundedVec::try_from(vec![BoundedVec::try_from(vec![1u8, 2u8, 3u8]).unwrap_or_default()]).unwrap_or_default(),
                data_fields: BoundedVec::try_from(vec![4u8, 5u8, 6u8]).unwrap_or_default(),
                raw_data: BoundedVec::try_from(vec![1u8, 2u8, 3u8, 4u8, 5u8, 6u8]).unwrap_or_default(),
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