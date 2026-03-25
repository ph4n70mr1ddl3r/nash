use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GameConfig {
    pub initial_stacks: [u64; 2],
    pub small_blind: u64,
    pub big_blind: u64,
    pub min_bet: u64,
}

#[derive(Debug, Clone, Copy, Error)]
#[non_exhaustive]
pub enum ConfigError {
    #[error("Stacks must be positive")]
    InvalidStacks,
    #[error("Blinds must be positive")]
    InvalidBlinds,
    #[error("Big blind must be >= small blind")]
    InvalidBlindRatio,
    #[error("Min bet must be positive")]
    InvalidMinBet,
}

impl GameConfig {
    #[must_use = "validate() returns a Result that should be checked"]
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.initial_stacks.contains(&0) {
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
        GameConfig {
            initial_stacks: [1000, 1000],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFRConfig {
    pub num_iterations: usize,
    pub log_interval: usize,
    pub save_interval: usize,
    pub save_path: Option<String>,
    pub use_chance_sampling: bool,
}

impl Default for CFRConfig {
    fn default() -> Self {
        CFRConfig {
            num_iterations: 100,
            log_interval: 10,
            save_interval: 50,
            save_path: None,
            use_chance_sampling: true,
        }
    }
}
