//! Game and solver configuration structs.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Configuration for a heads-up No-Limit Hold'em game.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GameConfig {
    /// Initial stack sizes for both players [SB, BB].
    pub initial_stacks: [u64; 2],
    /// Small blind amount.
    pub small_blind: u64,
    /// Big blind amount.
    pub big_blind: u64,
    /// Minimum bet/raise size.
    pub min_bet: u64,
}

/// Errors that can occur when validating game configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// Stack sizes must be positive.
    #[error("Stacks must be positive")]
    InvalidStacks,
    /// Blind amounts must be positive.
    #[error("Blinds must be positive")]
    InvalidBlinds,
    /// Big blind must be >= small blind.
    #[error("Big blind must be >= small blind")]
    InvalidBlindRatio,
    /// Minimum bet must be positive.
    #[error("Min bet must be positive")]
    InvalidMinBet,
}

impl GameConfig {
    /// Validates the game configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if any configuration value is invalid.
    #[must_use = "validate() returns a Result that should be checked"]
    pub const fn validate(&self) -> Result<(), ConfigError> {
        if self.initial_stacks[0] == 0 || self.initial_stacks[1] == 0 {
            return Err(ConfigError::InvalidStacks);
        }
        if self.small_blind == 0 || self.big_blind == 0 {
            return Err(ConfigError::InvalidBlinds);
        }
        if self.big_blind < self.small_blind {
            return Err(ConfigError::InvalidBlindRatio);
        }
        if self.min_bet == 0 {
            return Err(ConfigError::InvalidMinBet);
        }
        Ok(())
    }
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            initial_stacks: [1000, 1000],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        }
    }
}

/// Errors that can occur when validating CFR configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CFRConfigError {
    /// Number of iterations must be positive.
    #[error("num_iterations must be positive")]
    InvalidNumIterations,
    /// Log interval must be positive.
    #[error("log_interval must be positive")]
    InvalidLogInterval,
    /// Save interval must be positive.
    #[error("save_interval must be positive")]
    InvalidSaveInterval,
}

/// Configuration for the CFR+ solver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFRConfig {
    /// Number of CFR iterations to run.
    pub num_iterations: usize,
    /// How often to log progress (in iterations).
    pub log_interval: usize,
    /// How often to save the strategy (in iterations).
    pub save_interval: usize,
    /// Path to save the strategy file (optional).
    pub save_path: Option<String>,
    /// Whether to use chance sampling for card dealing.
    pub use_chance_sampling: bool,
}

impl CFRConfig {
    /// Validates the CFR configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if any configuration value is invalid.
    #[cold]
    #[must_use = "validate() returns a Result that should be checked"]
    pub const fn validate(&self) -> Result<(), CFRConfigError> {
        if self.num_iterations == 0 {
            return Err(CFRConfigError::InvalidNumIterations);
        }
        if self.log_interval == 0 {
            return Err(CFRConfigError::InvalidLogInterval);
        }
        if self.save_interval == 0 {
            return Err(CFRConfigError::InvalidSaveInterval);
        }
        Ok(())
    }
}

impl Default for CFRConfig {
    fn default() -> Self {
        Self {
            num_iterations: 100,
            log_interval: 10,
            save_interval: 50,
            save_path: None,
            use_chance_sampling: true,
        }
    }
}
