//! Private chain compatibility module for DCF pallet
//!
//! This module implements support for private chains with fixed validator allowlists,
//! enabling controlled validator sets for permissioned networks.

use super::*;
use frame_support::{
    BoundedBTreeSet,
};
use sp_core::ConstU32;
use codec::MaxEncodedLen;

/// Private chain configuration and allowlist management
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct PrivateChainConfig<AccountId: MaxEncodedLen + Ord> {
    /// Whether private chain mode is enabled
    pub enabled: bool,
    /// Fixed allowlist of permitted validators (using BoundedBTreeSet for MaxEncodedLen)
    pub validator_allowlist: BoundedBTreeSet<AccountId, ConstU32<100>>,
    /// Whether to allow dynamic allowlist updates
    pub allow_allowlist_updates: bool,
    /// Maximum size of the allowlist
    pub max_allowlist_size: u32,
}

impl<AccountId: MaxEncodedLen + Ord> Default for PrivateChainConfig<AccountId> {
    fn default() -> Self {
        Self {
            enabled: false,
            validator_allowlist: BoundedBTreeSet::new(),
            allow_allowlist_updates: false,
            max_allowlist_size: 100,
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Check if private chain mode is enabled
    pub fn is_private_chain_mode() -> bool {
        PrivateChainConfigStorage::<T>::get().map(|config| config.enabled).unwrap_or(false)
    }
    
    /// Validate if an account is allowed to join as validator in private mode
    pub fn is_validator_allowed(account: &T::AccountId) -> bool {
        if let Some(config) = PrivateChainConfigStorage::<T>::get() {
            if !config.enabled {
                // Public mode - all accounts allowed
                return true;
            }
            
            // Private mode - check allowlist
            config.validator_allowlist.contains(account)
        } else {
            // No config means public mode
            true
        }
    }
    
    /// Add validator to private chain allowlist (root only)
    pub fn add_to_validator_allowlist(
        account: T::AccountId,
    ) -> Result<(), Error<T>> {
        let mut config = PrivateChainConfigStorage::<T>::get().ok_or(Error::<T>::PrivateChainModeDisabled)?;
        
        if !config.enabled {
            return Err(Error::<T>::PrivateChainModeDisabled);
        }
        
        if !config.allow_allowlist_updates {
            return Err(Error::<T>::AllowlistUpdatesDisabled);
        }
        
        if config.validator_allowlist.len() >= config.max_allowlist_size as usize {
            return Err(Error::<T>::AllowlistFull);
        }
        
        if config.validator_allowlist.try_insert(account.clone()).is_ok() {
            PrivateChainConfigStorage::<T>::put(config);
            
            Self::deposit_event(Event::ValidatorAddedToAllowlist {
                validator: account,
            });
        }
        
        Ok(())
    }
    
    /// Remove validator from private chain allowlist (root only)
    pub fn remove_from_validator_allowlist(
        account: &T::AccountId,
    ) -> Result<(), Error<T>> {
        let mut config = PrivateChainConfigStorage::<T>::get().ok_or(Error::<T>::PrivateChainModeDisabled)?;
        
        if !config.enabled {
            return Err(Error::<T>::PrivateChainModeDisabled);
        }
        
        if !config.allow_allowlist_updates {
            return Err(Error::<T>::AllowlistUpdatesDisabled);
        }
        
        if config.validator_allowlist.remove(account) {
            PrivateChainConfigStorage::<T>::put(config);
            
            Self::deposit_event(Event::ValidatorRemovedFromAllowlist {
                validator: account.clone(),
            });
            
            // If validator is currently active and no longer allowed, initiate leave
            if Self::is_validator_active(account) {
                let _ = Self::force_validator_leave(account);
            }
        }
        
        Ok(())
    }
    
    /// Enable private chain mode with initial allowlist
    pub fn enable_private_chain_mode(
        initial_allowlist: Vec<T::AccountId>,
        allow_updates: bool,
    ) -> Result<(), Error<T>> {
        if initial_allowlist.len() > <T as pos::Config>::MaxValidators::get() as usize {
            return Err(Error::<T>::TooManyValidators);
        }
        
        let mut validator_allowlist = BoundedBTreeSet::new();
        for validator in initial_allowlist.iter() {
            let _ = validator_allowlist.try_insert(validator.clone());
        }
        
        let config = PrivateChainConfig {
            enabled: true,
            validator_allowlist,
            allow_allowlist_updates: allow_updates,
            max_allowlist_size: <T as pos::Config>::MaxValidators::get(),
        };
        
        PrivateChainConfigStorage::<T>::put(config);
        
        Self::deposit_event(Event::PrivateChainModeEnabled {
            allowlist_size: initial_allowlist.len() as u32,
            allow_updates,
        });
        
        // Remove any active validators not in allowlist
        let active_validators = Self::active_validators();
        for validator in active_validators {
            if !Self::is_validator_allowed(&validator) {
                let _ = Self::force_validator_leave(&validator);
            }
        }
        
        Ok(())
    }
    
    /// Disable private chain mode (return to public mode)
    pub fn disable_private_chain_mode() -> Result<(), Error<T>> {
        let mut config = PrivateChainConfigStorage::<T>::get().ok_or(Error::<T>::PrivateChainModeDisabled)?;
        
        if !config.enabled {
            return Err(Error::<T>::PrivateChainModeDisabled);
        }
        
        config.enabled = false;
        config.validator_allowlist.clear();
        PrivateChainConfigStorage::<T>::put(config);
        
        Self::deposit_event(Event::PrivateChainModeDisabled);
        
        Ok(())
    }
    
    /// Get current private chain configuration
    pub fn get_private_chain_config() -> PrivateChainConfig<T::AccountId> {
        PrivateChainConfigStorage::<T>::get().unwrap_or_default()
    }
    
    /// Get validator allowlist
    pub fn get_validator_allowlist() -> Vec<T::AccountId> {
        let config = PrivateChainConfigStorage::<T>::get().unwrap_or_default();
        config.validator_allowlist.into_iter().collect()
    }
    
    /// Check if allowlist updates are allowed
    pub fn are_allowlist_updates_allowed() -> bool {
        let config = PrivateChainConfigStorage::<T>::get().unwrap_or_default();
        config.enabled && config.allow_allowlist_updates
    }
    
    /// Force a validator to leave (used when removed from allowlist)
    fn force_validator_leave(validator: &T::AccountId) -> Result<(), Error<T>> {
        if !Self::is_validator_active(validator) {
            return Ok(());
        }
        
        // Remove from active set immediately
        ValidatorSet::<T>::mutate(|validators| {
            validators.retain(|v| v != validator);
        });
        
        // Unreserve stake
        let reserved = T::Currency::reserved_balance(validator);
        T::Currency::unreserve(validator, reserved);
        
        // Clear validator data
        ValidatorTrustScores::<T>::remove(validator);
        pallet_cbc_poi::ValidatorInferenceCount::<T>::remove(validator);
        ValidatorLastSeen::<T>::remove(validator);
        ValidatorStates::<T>::remove(validator);
        
        Self::deposit_event(Event::ValidatorForcedToLeave {
            validator: validator.clone(),
            reason: b"Removed from private chain allowlist".to_vec(),
        });
        
        Ok(())
    }
    
    /// Validate proposal submission in private chain mode
    pub fn validate_proposal_in_private_mode(
        proposer: &T::AccountId,
        target: &T::AccountId,
    ) -> Result<(), Error<T>> {
        let config = PrivateChainConfigStorage::<T>::get().unwrap_or_default();
        
        if !config.enabled {
            return Ok(()); // Public mode - no restrictions
        }
        
        // In private mode, both proposer and target must be in allowlist
        if !config.validator_allowlist.contains(proposer) {
            return Err(Error::<T>::ProposerNotInAllowlist);
        }
        
        if !config.validator_allowlist.contains(target) {
            return Err(Error::<T>::TargetNotInAllowlist);
        }
        
        Ok(())
    }
    
    /// Validate governance operations in private chain mode
    pub fn validate_governance_in_private_mode(
        account: &T::AccountId,
    ) -> Result<(), Error<T>> {
        let config = PrivateChainConfigStorage::<T>::get().unwrap_or_default();
        
        if !config.enabled {
            return Ok(()); // Public mode - no restrictions
        }
        
        // In private mode, only allowlisted validators can participate in governance
        if !config.validator_allowlist.contains(account) {
            return Err(Error::<T>::NotInAllowlist);
        }
        
        Ok(())
    }
}

// Storage is defined in the main lib.rs file

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::*;
    use frame_support::{assert_ok, assert_noop};

    #[test]
    fn test_private_chain_mode_enable_disable() {
        new_test_ext().execute_with(|| {
            // Initially in public mode
            assert!(!DcfPallet::is_private_chain_mode());
            
            // Enable private mode with allowlist
            let allowlist = vec![1u64, 2u64, 3u64];
            assert_ok!(DcfPallet::enable_private_chain_mode(allowlist.clone(), true));
            
            assert!(DcfPallet::is_private_chain_mode());
            assert_eq!(DcfPallet::get_validator_allowlist(), allowlist);
            
            // Disable private mode
            assert_ok!(DcfPallet::disable_private_chain_mode());
            assert!(!DcfPallet::is_private_chain_mode());
        });
    }

    #[test]
    fn test_validator_allowlist_management() {
        new_test_ext().execute_with(|| {
            // Enable private mode
            assert_ok!(DcfPallet::enable_private_chain_mode(vec![1u64, 2u64], true));
            
            // Add validator to allowlist
            assert_ok!(DcfPallet::add_to_validator_allowlist(3u64));
            assert!(DcfPallet::is_validator_allowed(&3u64));
            
            // Remove validator from allowlist
            assert_ok!(DcfPallet::remove_from_validator_allowlist(&3u64));
            assert!(!DcfPallet::is_validator_allowed(&3u64));
        });
    }

    #[test]
    fn test_private_mode_validator_restrictions() {
        new_test_ext().execute_with(|| {
            // Setup balances
            let _ = Balances::make_free_balance_be(&1, 100_000_000);
            let _ = Balances::make_free_balance_be(&2, 100_000_000);
            
            // Enable private mode with limited allowlist
            assert_ok!(DcfPallet::enable_private_chain_mode(vec![1u64], false));
            
            // Allowed validator can join
            assert_ok!(DcfPallet::join_validators(RuntimeOrigin::signed(1), None));
            
            // Non-allowed validator cannot join
            assert_noop!(
                DcfPallet::join_validators(RuntimeOrigin::signed(2), None),
                Error::<Test>::NotInAllowlist
            );
        });
    }

    #[test]
    fn test_private_mode_proposal_restrictions() {
        new_test_ext().execute_with(|| {
            // Setup balances and validators
            let _ = Balances::make_free_balance_be(&1, 100_000_000);
            let _ = Balances::make_free_balance_be(&2, 100_000_000);
            let _ = Balances::make_free_balance_be(&3, 100_000_000);
            
            assert_ok!(DcfPallet::join_validators(RuntimeOrigin::signed(1), None));
            assert_ok!(DcfPallet::join_validators(RuntimeOrigin::signed(2), None));
            
            // Enable private mode
            assert_ok!(DcfPallet::enable_private_chain_mode(vec![1u64, 2u64], true));
            
            // Allowlisted validator can make proposals
            assert_ok!(DcfPallet::validate_proposal_in_private_mode(&1u64, &2u64));
            
            // Non-allowlisted validator cannot make proposals
            assert!(matches!(
                DcfPallet::validate_proposal_in_private_mode(&3u64, &1u64),
                Err(Error::<Test>::ProposerNotInAllowlist)
            ));
        });
    }

    #[test]
    fn test_allowlist_size_limits() {
        new_test_ext().execute_with(|| {
            // Try to enable with too many validators
            let large_allowlist: Vec<u64> = (1..=200).collect();
            assert!(matches!(
                DcfPallet::enable_private_chain_mode(large_allowlist, true),
                Err(Error::<Test>::TooManyValidators)
            ));
        });
    }
}