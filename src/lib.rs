//! Nash - A heads-up No-Limit Hold'em poker solver using CFR+
//!
//! This library implements the Counterfactual Regret Minimization with linear
//! weighting (CFR+) algorithm for solving heads-up No-Limit Hold'em poker.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, clippy::all)]

pub mod card;
pub mod config;
pub mod game;
pub mod hand;
pub mod solver;
pub mod strategy;

pub use card::{Card, CardSet, Deck};
pub use config::{CFRConfig, CFRConfigError, ConfigError, GameConfig};
pub use game::{Action, GameState, InfoSet, Player, Street};
pub use hand::{Hand, HandType};
pub use solver::CFRSolver;
pub use strategy::{Strategy, StrategyEntry, StrategyError, StrategyStats};
