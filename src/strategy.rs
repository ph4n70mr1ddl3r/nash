//! CFR strategy storage with concurrent `DashMap` access.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::game::InfoSet;

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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

    /// Computes the average strategy from cumulative strategy sums.
    ///
    /// This is the converged strategy that should be used after CFR+ training.
    /// Returns the normalized strategy sum divided by the total sum.
    #[inline]
    #[allow(clippy::cast_precision_loss)]
    pub fn get_average_strategy(&self, out: &mut [f64]) {
        let num_actions = self.num_actions as usize;
        if num_actions == 0 {
            return;
        }
        let len = out.len().min(num_actions);
        let mut sum = 0.0;
        for (out_val, &s) in out.iter_mut().zip(self.strategy_sum.iter()).take(len) {
            *out_val = s;
            sum += s;
        }
        if sum > 0.0 {
            for s in &mut out[..len] {
                *s /= sum;
            }
        } else {
            let uniform = 1.0 / num_actions as f64;
            out[..len].fill(uniform);
        }
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
            let positive_regret = regret.max(0.0);
            *out_val = positive_regret;
            sum += positive_regret;
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
        for (i, &regret) in regrets.iter().enumerate().take(len) {
            self.regrets[i] = (self.regrets[i] + regret).max(0.0);
        }
        for (i, &strat) in strategy.iter().enumerate().take(len) {
            self.strategy_sum[i] = (pi_o * strat).mul_add(iter_weight, self.strategy_sum[i]);
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
    #[inline]
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }

    /// Creates a new strategy with pre-allocated capacity.
    #[must_use]
    #[inline]
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
        match self.entries.entry(info_set.clone()) {
            dashmap::mapref::entry::Entry::Occupied(e) => {
                e.get().get_strategy(out);
            }
            dashmap::mapref::entry::Entry::Vacant(e) => {
                let entry = StrategyEntry::new(num_actions);
                entry.get_strategy(out);
                e.insert(entry);
            }
        }
    }

    /// Gets the average strategy for an info set, if it exists.
    ///
    /// Returns `true` if the entry was found (and `out` is populated),
    /// or `false` if no entry exists (and `out` is filled with uniform).
    #[inline]
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn get_average_strategy(
        &self,
        info_set: &InfoSet,
        num_actions: usize,
        out: &mut [f64],
    ) -> bool {
        if let Some(entry) = self.entries.get(info_set) {
            entry.get_average_strategy(out);
            true
        } else {
            let len = num_actions.min(out.len());
            if len > 0 {
                let uniform = 1.0 / num_actions as f64;
                out[..len].fill(uniform);
            }
            false
        }
    }

    /// Updates the strategy entry for an info set.
    ///
    /// If the entry does not exist, a new one is created and updated in place
    /// using an atomic upsert to avoid the read-then-write race.
    #[inline]
    pub fn update_entry(
        &self,
        info_set: &InfoSet,
        regrets: &[f64],
        strategy: &[f64],
        pi_o: f64,
        iter_weight: f64,
    ) {
        use dashmap::mapref::entry::Entry;

        match self.entries.entry(info_set.clone()) {
            Entry::Occupied(mut e) => {
                e.get_mut().update(regrets, strategy, pi_o, iter_weight);
            }
            Entry::Vacant(e) => {
                let mut entry = StrategyEntry::new(regrets.len().max(1));
                entry.update(regrets, strategy, pi_o, iter_weight);
                e.insert(entry);
            }
        }
    }

    /// Returns statistics about the stored strategy.
    #[must_use]
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    pub fn stats(&self) -> StrategyStats {
        let info_sets = self.entries.len();
        let key_size = std::mem::size_of::<InfoSet>();
        let val_size = std::mem::size_of::<StrategyEntry>();
        let ptr_overhead = std::mem::size_of::<usize>() * 2;
        let bytes_per_entry = key_size + val_size + ptr_overhead;
        let total_bytes = (info_sets as u128) * (bytes_per_entry as u128);
        let memory_mb = (total_bytes / 1_000_000) as u64;
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
    #[cold]
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), StrategyError> {
        let file = File::create(path.as_ref())?;
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
    #[cold]
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, StrategyError> {
        let file = File::open(path.as_ref())?;
        let reader = BufReader::new(file);
        let entries: Vec<(InfoSet, StrategyEntry)> = bincode::deserialize_from(reader)?;
        let strategy = Self::with_capacity(entries.len());
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
