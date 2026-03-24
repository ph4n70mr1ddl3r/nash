use std::fs::File;
use std::io::{BufReader, BufWriter};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::game::{Action, InfoSet};

pub const MAX_ACTIONS: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct StrategyStats {
    pub info_sets: usize,
    pub memory_mb: f64,
}

#[derive(Debug, Error)]
pub enum StrategyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl From<bincode::Error> for StrategyError {
    fn from(e: bincode::Error) -> Self {
        StrategyError::Serialization(e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyEntry {
    pub regrets: [f64; MAX_ACTIONS],
    pub strategy_sum: [f64; MAX_ACTIONS],
    pub num_actions: u8,
}

impl StrategyEntry {
    #[must_use]
    pub fn new(num_actions: usize) -> Self {
        StrategyEntry {
            regrets: [0.0; MAX_ACTIONS],
            strategy_sum: [0.0; MAX_ACTIONS],
            num_actions: num_actions.min(MAX_ACTIONS) as u8,
        }
    }

    #[inline]
    pub fn get_strategy(&self, out: &mut [f64]) {
        let len = out.len().min(self.num_actions as usize);
        assert!(len == self.num_actions as usize, "output buffer too small");
        let mut sum = 0.0;
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
            let uniform = 1.0 / len as f64;
            for s in &mut out[..len] {
                *s = uniform;
            }
        }
    }

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

pub struct Strategy {
    entries: DashMap<InfoSet, StrategyEntry>,
}

impl Strategy {
    #[must_use]
    pub fn new() -> Self {
        Strategy {
            entries: DashMap::new(),
        }
    }

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

    #[must_use]
    pub fn stats(&self) -> StrategyStats {
        let info_sets = self.entries.len();
        let base_size = std::mem::size_of::<InfoSet>()
            + std::mem::size_of::<StrategyEntry>()
            + std::mem::size_of::<DashMap<InfoSet, StrategyEntry>>();
        let avg_history_overhead = 3 * std::mem::size_of::<Action>();
        let total_memory = info_sets * (base_size + avg_history_overhead);
        let memory_mb = total_memory as f64 / 1_000_000.0;
        StrategyStats {
            info_sets,
            memory_mb,
        }
    }

    pub fn save(&self, path: &str) -> Result<(), StrategyError> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        let entries: Vec<_> = self
            .entries
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect();
        bincode::serialize_into(writer, &entries)?;
        Ok(())
    }

    pub fn load(path: &str) -> Result<Self, StrategyError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let entries: Vec<(InfoSet, StrategyEntry)> = bincode::deserialize_from(reader)?;
        let strategy = Strategy::new();
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
