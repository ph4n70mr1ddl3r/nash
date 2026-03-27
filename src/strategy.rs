//! CFR strategy storage with concurrent `DashMap` access.

use std::fs::File;
use std::io::{BufReader, BufWriter};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::game::{Action, InfoSet};

/// Maximum number of actions supported at any decision point.
pub const MAX_ACTIONS: usize = 8;

/// Statistics about the stored strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrategyStats {
    /// Number of information sets stored.
    pub info_sets: usize,
    /// Estimated memory usage in megabytes.
    pub memory_mb: u64,
}

/// Errors that can occur when saving/loading strategies.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StrategyError {
    /// I/O error during file operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl From<bincode::Error> for StrategyError {
    fn from(e: bincode::Error) -> Self {
        Self::Serialization(e.to_string())
    }
}

/// Strategy data for a single information set.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StrategyEntry {
    /// Cumulative regrets for each action.
    pub regrets: [f64; MAX_ACTIONS],
    /// Cumulative strategy sum for each action.
    pub strategy_sum: [f64; MAX_ACTIONS],
    /// Number of legal actions at this info set.
    pub num_actions: u8,
}

impl StrategyEntry {
    /// Creates a new strategy entry with zero regrets.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn new(num_actions: usize) -> Self {
        Self {
            regrets: [0.0; MAX_ACTIONS],
            strategy_sum: [0.0; MAX_ACTIONS],
            num_actions: num_actions.min(MAX_ACTIONS) as u8,
        }
    }

    /// Returns the number of actions for this entry.
    #[must_use]
    #[inline]
    pub const fn num_actions(&self) -> usize {
        self.num_actions as usize
    }

    /// Computes the current strategy from regrets (regret matching).
    #[inline]
    #[allow(clippy::cast_precision_loss)]
    pub fn get_strategy(&self, out: &mut [f64]) {
        let num_actions = self.num_actions as usize;
        if num_actions == 0 {
            return;
        }
        let len = out.len().min(num_actions);
        let mut sum = 0.0;
        let uniform = 1.0 / num_actions as f64;
        for (out_val, &regret) in out.iter_mut().zip(self.regrets.iter()).take(len) {
            let s = regret.max(0.0);
            *out_val = s;
            sum += s;
        }
        if sum > 0.0 {
            for s in &mut out[..len] {
                *s /= sum;
            }
        } else {
            out[..len].fill(uniform);
        }
    }

    /// Updates regrets and strategy sum for this entry.
    #[inline]
    pub fn update(&mut self, regrets: &[f64], strategy: &[f64], pi_o: f64, iter_weight: f64) {
        let len = self.num_actions as usize;
        for (i, &r) in regrets.iter().enumerate().take(len) {
            self.regrets[i] = (self.regrets[i] + r).max(0.0);
        }
        for (i, &s) in strategy.iter().enumerate().take(len) {
            self.strategy_sum[i] += pi_o * s * iter_weight;
        }
    }
}

/// Thread-safe strategy storage using `DashMap`.
#[derive(Debug)]
pub struct Strategy {
    entries: DashMap<InfoSet, StrategyEntry>,
}

impl Strategy {
    /// Creates a new empty strategy.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }

    /// Creates a new strategy with pre-allocated capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: DashMap::with_capacity(capacity),
        }
    }

    /// Returns the number of information sets stored.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no information sets are stored.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Gets or creates the strategy for an info set.
    #[inline]
    pub fn get_strategy(&self, info_set: &InfoSet, num_actions: usize, out: &mut [f64]) {
        use dashmap::mapref::entry::Entry;
        match self.entries.entry(info_set.clone()) {
            Entry::Occupied(e) => {
                e.get().get_strategy(out);
            }
            Entry::Vacant(e) => {
                let entry = StrategyEntry::new(num_actions);
                entry.get_strategy(out);
                e.insert(entry);
            }
        }
    }

    /// Updates the strategy entry for an info set.
    #[inline]
    pub fn update_entry(
        &self,
        info_set: &InfoSet,
        regrets: &[f64],
        strategy: &[f64],
        pi_o: f64,
        iter_weight: f64,
    ) {
        if let Some(mut entry) = self.entries.get_mut(info_set) {
            entry.update(regrets, strategy, pi_o, iter_weight);
        }
    }

    /// Returns statistics about the stored strategy.
    #[must_use]
    #[inline]
    pub fn stats(&self) -> StrategyStats {
        let info_sets = self.entries.len();
        let entry_size = std::mem::size_of::<InfoSet>() + std::mem::size_of::<StrategyEntry>();
        let dashmap_overhead = std::mem::size_of::<DashMap<InfoSet, StrategyEntry>>();
        let avg_history_len = 4;
        let history_overhead = avg_history_len * std::mem::size_of::<Action>();
        let total_memory = dashmap_overhead + info_sets * (entry_size + history_overhead);
        let memory_mb = (total_memory / 1_000_000) as u64;
        StrategyStats {
            info_sets,
            memory_mb,
        }
    }

    /// Saves the strategy to a binary file.
    ///
    /// The strategy is serialized using bincode for efficient storage.
    /// Note: This collects all entries into memory before serialization,
    /// which may be memory-intensive for very large strategies.
    ///
    /// # Errors
    ///
    /// Returns an error if file I/O or serialization fails.
    pub fn save(&self, path: &str) -> Result<(), StrategyError> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        let entries: Vec<_> = self
            .entries
            .iter()
            .map(|e| (e.key().clone(), *e.value()))
            .collect();
        bincode::serialize_into(writer, &entries)?;
        Ok(())
    }

    /// Loads a strategy from a binary file.
    ///
    /// # Errors
    ///
    /// Returns an error if file I/O or deserialization fails.
    pub fn load(path: &str) -> Result<Self, StrategyError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let entries: Vec<(InfoSet, StrategyEntry)> = bincode::deserialize_from(reader)?;
        let strategy = Self::new();
        for (key, value) in entries {
            strategy.entries.insert(key, value);
        }
        Ok(strategy)
    }
}

impl Default for Strategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for StrategyEntry {
    fn default() -> Self {
        Self::new(0)
    }
}
