//! Nash - A heads-up No-Limit Hold'em poker solver using CFR+
//!
//! This library implements the Counterfactual Regret Minimization with linear
//! weighting (CFR+) algorithm for solving heads-up No-Limit Hold'em poker.

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::unwrap_used,
    clippy::expect_used
)]
#![allow(
    clippy::module_name_repetitions,
    clippy::redundant_pub_crate,
    clippy::option_if_let_else,
    clippy::similar_names
)]

pub mod card;
pub mod config;
pub mod game;
pub mod hand;
pub mod solver;
pub mod strategy;

pub use card::{Card, CardSet, Deck};
pub use config::{CFRConfig, CFRConfigError, ConfigError, GameConfig};
pub use game::{Action, ActionHistory, GameState, InfoSet, LegalActions, Player, Street};
pub use hand::{Hand, HandType};
pub use solver::{CFRSolver, SolverError};
pub use strategy::{Strategy, StrategyEntry, StrategyError, StrategyStats};
