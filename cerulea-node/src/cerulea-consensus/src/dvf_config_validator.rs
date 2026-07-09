// DVF Configuration Validation Module
//
// This module validates DVF configuration parameters at node startup to ensure
// they meet the requirements for correct operation of the finality system.

use sp_runtime::Perbill;
use std::fmt;

/// Configuration validation error
#[derive(Debug, Clone)]
pub struct ConfigValidationError {
    /// The name of the configuration parameter that failed validation
    pub parameter: String,
    /// The reason why the validation failed
    pub reason: String,
    /// Optional recommendation for fixing the configuration
    pub recommendation: Option<String>,
}

impl fmt::Display for ConfigValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid configuration for {}: {}", self.parameter, self.reason)?;
        if let Some(ref rec) = self.recommendation {
            write!(f, " Recommendation: {}", rec)?;
        }
        Ok(())
    }
}

impl std::error::Error for ConfigValidationError {}

/// DVF configuration parameters to validate
#[derive(Debug, Clone)]
pub struct DvfConfig {
    /// Interval between checkpoint blocks (in block numbers)
    pub finality_checkpoint_interval: u32,
    /// Threshold percentage of voting weight required for finality (e.g., 67% for 2/3)
    pub finality_threshold: Perbill,
    /// Weight factor applied to validator stake
    pub stake_weight_factor: u128,
    /// Weight factor applied to validator score
    pub score_weight_factor: u128,
    /// Number of rounds to retain votes in memory
    pub vote_retention_rounds: u32,
}

impl DvfConfig {
    /// Validate all configuration parameters
    ///
    /// Returns Ok(()) if all parameters are valid, or Err with the first validation error found.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        self.validate_finality_checkpoint_interval()?;
        self.validate_finality_threshold()?;
        self.validate_weight_factors()?;
        self.validate_vote_retention_rounds()?;
        Ok(())
    }

    /// Validate FinalityCheckpointInterval
    ///
    // Verify value is greater than zero
    fn validate_finality_checkpoint_interval(&self) -> Result<(), ConfigValidationError> {
        if self.finality_checkpoint_interval == 0 {
            return Err(ConfigValidationError {
                parameter: "FinalityCheckpointInterval".to_string(),
                reason: "Value must be greater than zero".to_string(),
                recommendation: Some(
                    "Set FinalityCheckpointInterval to a positive value (e.g., 10 for finality every 10 blocks)"
                        .to_string(),
                ),
            });
        }
        Ok(())
    }

    /// Validate FinalityThreshold
    ///
    /// Verify value is between 50% and 100%, recommend 67% for BFT
    fn validate_finality_threshold(&self) -> Result<(), ConfigValidationError> {
        let threshold_percent = self.finality_threshold.deconstruct() as f64 / 10_000_000.0;

        if threshold_percent < 50.0 {
            return Err(ConfigValidationError {
                parameter: "FinalityThreshold".to_string(),
                reason: format!(
                    "Value is {}%, which is below the minimum of 50%",
                    threshold_percent
                ),
                recommendation: Some(
                    "Set FinalityThreshold to at least 50% (Perbill::from_percent(50)). \
                     For Byzantine fault tolerance, 67% is recommended (Perbill::from_percent(67))"
                        .to_string(),
                ),
            });
        }

        if threshold_percent > 100.0 {
            return Err(ConfigValidationError {
                parameter: "FinalityThreshold".to_string(),
                reason: format!(
                    "Value is {}%, which exceeds the maximum of 100%",
                    threshold_percent
                ),
                recommendation: Some(
                    "Set FinalityThreshold to at most 100% (Perbill::from_percent(100))"
                        .to_string(),
                ),
            });
        }

        // Log recommendation if not using 67%
        if threshold_percent < 66.0 || threshold_percent > 68.0 {
            log::warn!(
                "DVF Config: FinalityThreshold is set to {}%. \
                 For Byzantine fault tolerance (tolerating up to 1/3 Byzantine validators), \
                 67% (2/3 threshold) is recommended.",
                threshold_percent
            );
        }

        Ok(())
    }

    /// Validate weight factors
    ///
    /// Verify StakeWeightFactor and ScoreWeightFactor are greater than zero
    fn validate_weight_factors(&self) -> Result<(), ConfigValidationError> {
        if self.stake_weight_factor == 0 {
            return Err(ConfigValidationError {
                parameter: "StakeWeightFactor".to_string(),
                reason: "Value must be greater than zero".to_string(),
                recommendation: Some(
                    "Set StakeWeightFactor to a positive value (e.g., 1 for 1:1 stake-to-weight ratio)"
                        .to_string(),
                ),
            });
        }

        if self.score_weight_factor == 0 {
            return Err(ConfigValidationError {
                parameter: "ScoreWeightFactor".to_string(),
                reason: "Value must be greater than zero".to_string(),
                recommendation: Some(
                    "Set ScoreWeightFactor to a positive value (e.g., 1000 to give scores meaningful weight)"
                        .to_string(),
                ),
            });
        }

        Ok(())
    }

    /// Validate VoteRetentionRounds
    ///
    /// Verify value is at least 1, recommend at least 2 for debugging
    fn validate_vote_retention_rounds(&self) -> Result<(), ConfigValidationError> {
        if self.vote_retention_rounds == 0 {
            return Err(ConfigValidationError {
                parameter: "VoteRetentionRounds".to_string(),
                reason: "Value must be at least 1".to_string(),
                recommendation: Some(
                    "Set VoteRetentionRounds to at least 1. For debugging purposes, 2 or more is recommended."
                        .to_string(),
                ),
            });
        }

        // Log recommendation if set to 1
        if self.vote_retention_rounds == 1 {
            log::warn!(
                "DVF Config: VoteRetentionRounds is set to 1. \
                 For better debugging capabilities, a value of 2 or more is recommended."
            );
        }

        Ok(())
    }

    /// Log all configuration parameters at info level
    ///
    /// Log all DVF configuration values at startup
    pub fn log_configuration(&self) {
        let threshold_percent = self.finality_threshold.deconstruct() as f64 / 10_000_000.0;

        log::info!("=== DVF Configuration ===");
        log::info!(
            "  FinalityCheckpointInterval: {} blocks",
            self.finality_checkpoint_interval
        );
        log::info!("  FinalityThreshold: {:.2}%", threshold_percent);
        log::info!("  StakeWeightFactor: {}", self.stake_weight_factor);
        log::info!("  ScoreWeightFactor: {}", self.score_weight_factor);
        log::info!("  VoteRetentionRounds: {}", self.vote_retention_rounds);
        log::info!("=========================");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_configuration() {
        let config = DvfConfig {
            finality_checkpoint_interval: 10,
            finality_threshold: Perbill::from_percent(67),
            stake_weight_factor: 1,
            score_weight_factor: 1000,
            vote_retention_rounds: 2,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_checkpoint_interval_zero() {
        let config = DvfConfig {
            finality_checkpoint_interval: 0,
            finality_threshold: Perbill::from_percent(67),
            stake_weight_factor: 1,
            score_weight_factor: 1000,
            vote_retention_rounds: 2,
        };

        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.parameter, "FinalityCheckpointInterval");
    }

    #[test]
    fn test_invalid_threshold_too_low() {
        let config = DvfConfig {
            finality_checkpoint_interval: 10,
            finality_threshold: Perbill::from_percent(49),
            stake_weight_factor: 1,
            score_weight_factor: 1000,
            vote_retention_rounds: 2,
        };

        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.parameter, "FinalityThreshold");
    }

    #[test]
    fn test_invalid_stake_weight_factor_zero() {
        let config = DvfConfig {
            finality_checkpoint_interval: 10,
            finality_threshold: Perbill::from_percent(67),
            stake_weight_factor: 0,
            score_weight_factor: 1000,
            vote_retention_rounds: 2,
        };

        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.parameter, "StakeWeightFactor");
    }

    #[test]
    fn test_invalid_score_weight_factor_zero() {
        let config = DvfConfig {
            finality_checkpoint_interval: 10,
            finality_threshold: Perbill::from_percent(67),
            stake_weight_factor: 1,
            score_weight_factor: 0,
            vote_retention_rounds: 2,
        };

        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.parameter, "ScoreWeightFactor");
    }

    #[test]
    fn test_invalid_vote_retention_rounds_zero() {
        let config = DvfConfig {
            finality_checkpoint_interval: 10,
            finality_threshold: Perbill::from_percent(67),
            stake_weight_factor: 1,
            score_weight_factor: 1000,
            vote_retention_rounds: 0,
        };

        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.parameter, "VoteRetentionRounds");
    }

    #[test]
    fn test_threshold_at_boundary_50_percent() {
        let config = DvfConfig {
            finality_checkpoint_interval: 10,
            finality_threshold: Perbill::from_percent(50),
            stake_weight_factor: 1,
            score_weight_factor: 1000,
            vote_retention_rounds: 2,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_threshold_at_boundary_100_percent() {
        let config = DvfConfig {
            finality_checkpoint_interval: 10,
            finality_threshold: Perbill::from_percent(100),
            stake_weight_factor: 1,
            score_weight_factor: 1000,
            vote_retention_rounds: 2,
        };

        assert!(config.validate().is_ok());
    }
}
