//! CFR+ algorithm implementation.
//!
//! This module implements Counterfactual Regret Minimization with linear weighting (CFR+)
//! for solving heads-up No-Limit Hold'em poker.
//!
//! # Algorithm Overview
//!
//! CFR+ works by iteratively traversing the game tree and updating regret values at each
//! information set. The algorithm converges to a Nash equilibrium strategy.
//!
//! Key features:
//! - Linear regret weighting (CFR+) for faster convergence
//! - Optional chance sampling for reduced computation
//! - Parallel iteration support using Rayon

use std::sync::Arc;
use std::time::Instant;

use rand::prelude::*;
use rayon::prelude::*;
use tracing::{info, warn};

use thiserror::Error;

use crate::card::{Card, CardSet, Deck};
use crate::config::{CFRConfig, CFRConfigError, ConfigError, GameConfig};
use crate::game::{GameState, InfoSet, Player};
use crate::hand::Hand;
use crate::strategy::{Strategy, MAX_ACTIONS};

/// Error type for solver operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SolverError {
    /// Invalid game configuration.
    #[error("Invalid game config: {0}")]
    InvalidGameConfig(#[from] ConfigError),
    /// Invalid CFR configuration.
    #[error("Invalid CFR config: {0}")]
    InvalidCFRConfig(#[from] CFRConfigError),
}

/// CFR+ solver for heads-up No-Limit Hold'em.
#[derive(Debug, Clone)]
pub struct CFRSolver {
    /// The computed strategy (shared for concurrent access).
    pub strategy: Arc<Strategy>,
    /// Game configuration.
    pub config: GameConfig,
    /// CFR solver configuration.
    pub cfr_config: CFRConfig,
    iteration: usize,
}

impl CFRSolver {
    /// Creates a new solver with the given configurations.
    ///
    /// # Errors
    ///
    /// Returns an error if either configuration is invalid.
    pub fn new(game_config: GameConfig, cfr_config: CFRConfig) -> Result<Self, SolverError> {
        game_config.validate()?;
        cfr_config.validate()?;

        let estimated_info_sets = if cfr_config.use_chance_sampling {
            10_000
        } else {
            100_000
        };
        let strategy = Arc::new(Strategy::with_capacity(estimated_info_sets));
        Ok(Self {
            config: game_config,
            cfr_config,
            strategy,
            iteration: 0,
        })
    }

    /// Returns the current iteration number.
    #[must_use]
    pub const fn iteration(&self) -> usize {
        self.iteration
    }

    /// Runs the CFR+ algorithm for the configured number of iterations.
    #[allow(clippy::cast_precision_loss)]
    pub fn solve(&mut self) {
        let start = Instant::now();

        info!(
            "Starting CFR+ solver with {} iterations",
            self.cfr_config.num_iterations
        );

        for iter in 1..=self.cfr_config.num_iterations {
            self.iteration = iter;

            let iter_weight = iter as f64;

            self.run_iteration(iter_weight);

            if iter % self.cfr_config.log_interval == 0 {
                let elapsed = start.elapsed();
                let stats = self.strategy.stats();
                let exploitability = self.estimate_exploitability_placeholder();

                info!(
                    "Iteration {}: {} info sets, {} MB, exploitability (placeholder): {:.6}, elapsed: {:?}",
                    iter, stats.info_sets, stats.memory_mb, exploitability, elapsed
                );
            }

            if let Some(ref path) = self.cfr_config.save_path {
                if iter % self.cfr_config.save_interval == 0 {
                    if let Err(e) = self.strategy.save(path) {
                        warn!("Failed to save strategy: {}", e);
                    } else {
                        info!("Saved strategy to {}", path);
                    }
                }
            }
        }

        let total = start.elapsed();
        info!("CFR+ completed in {:?}", total);
    }

    #[inline]
    fn run_iteration(&self, iter_weight: f64) {
        if self.cfr_config.use_chance_sampling {
            self.run_iteration_sampled(iter_weight);
        } else {
            self.run_iteration_full(iter_weight);
        }
    }

    #[inline]
    fn run_iteration_sampled(&self, iter_weight: f64) {
        let num_samples = if self.cfr_config.samples_per_iteration > 0 {
            self.cfr_config.samples_per_iteration
        } else {
            rayon::current_num_threads().max(1)
        };

        let strategy = self.strategy.clone();
        let config = self.config;

        (0..num_samples).into_par_iter().for_each(move |_| {
            let mut rng = thread_rng();
            let mut deck = Deck::new();
            deck.shuffle(&mut rng);

            let hole_cards = deck.deal_into::<4>();
            let hole_sb = [hole_cards[0], hole_cards[1]];
            let hole_bb = [hole_cards[2], hole_cards[3]];

            let board = deck.deal_into::<5>();
            let hands = [hole_sb, hole_bb];

            let state = GameState::new(config);

            for &player in &[Player::SB, Player::BB] {
                Self::cfr_traversal_static(
                    &strategy,
                    &state,
                    &hands,
                    &board,
                    player,
                    1.0,
                    1.0,
                    iter_weight,
                );
            }
        });
    }

    #[inline]
    fn run_iteration_full(&self, iter_weight: f64) {
        let all_cards = Card::all();
        let num_cards = all_cards.len();
        let strategy = self.strategy.clone();
        let config = self.config;

        (0..num_cards).into_par_iter().for_each(|i| {
            let mut rng = rand::rngs::StdRng::seed_from_u64(i as u64);
            for j in (i + 1)..num_cards {
                for k in (j + 1)..num_cards {
                    for l in (k + 1)..num_cards {
                        Self::process_card_combination(
                            &strategy,
                            config,
                            &mut rng,
                            all_cards,
                            i,
                            j,
                            k,
                            l,
                            iter_weight,
                        );
                    }
                }
            }
        });
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn process_card_combination(
        strategy: &Arc<Strategy>,
        config: GameConfig,
        rng: &mut rand::rngs::StdRng,
        all_cards: &[Card],
        i: usize,
        j: usize,
        k: usize,
        l: usize,
        iter_weight: f64,
    ) {
        use rand::seq::SliceRandom;

        let hole_sb = [all_cards[i], all_cards[j]];
        let hole_bb = [all_cards[k], all_cards[l]];
        let excluded_mask: u64 = (1u64 << i) | (1u64 << j) | (1u64 << k) | (1u64 << l);

        let mut remaining: [Card; 48] = [Card::placeholder(); 48];
        let mut remaining_len = 0;
        for (idx, &c) in all_cards.iter().enumerate() {
            if (excluded_mask & (1u64 << idx)) == 0 {
                remaining[remaining_len] = c;
                remaining_len += 1;
            }
        }

        remaining[..remaining_len].partial_shuffle(rng, 5);

        let hands = [hole_sb, hole_bb];
        let state = GameState::new(config);

        for &player in &[Player::SB, Player::BB] {
            Self::cfr_traversal_static(
                strategy,
                &state,
                &hands,
                &remaining[..5],
                player,
                1.0,
                1.0,
                iter_weight,
            );
        }
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn cfr_traversal_static(
        strategy: &Arc<Strategy>,
        state: &GameState,
        hands: &[[Card; 2]],
        board: &[Card],
        player: Player,
        pi_o: f64,
        pi_neg_o: f64,
        iter_weight: f64,
    ) -> f64 {
        if state.is_terminal() {
            return Self::get_utility_impl(state, hands, board, player);
        }

        let current = state.current_player;
        let actions = state.legal_actions();

        if actions.is_empty() {
            return Self::get_utility_impl(state, hands, board, player);
        }

        let board_set = CardSet::from_cards(&board[..state.visible_board_count(board.len())]);
        let hole = &hands[current.index()];

        let mut info_set = InfoSet::from_cards(current, state.street, hole, board_set);
        for action in &state.history {
            info_set.add_action(action);
        }

        let mut strat = [0.0f64; MAX_ACTIONS];
        strategy.get_strategy(&info_set, actions.len(), &mut strat[..actions.len()]);

        let mut action_values = [0.0f64; MAX_ACTIONS];
        let mut node_value = 0.0;
        for (i, &action) in actions.iter().enumerate() {
            let new_state = state.apply_action(action);

            let value = if current == player {
                Self::cfr_traversal_static(
                    strategy,
                    &new_state,
                    hands,
                    board,
                    player,
                    pi_o * strat[i],
                    pi_neg_o,
                    iter_weight,
                )
            } else {
                Self::cfr_traversal_static(
                    strategy,
                    &new_state,
                    hands,
                    board,
                    player,
                    pi_o,
                    pi_neg_o * strat[i],
                    iter_weight,
                )
            };

            action_values[i] = value;
            node_value += strat[i] * value;
        }

        if current == player {
            let mut regrets = [0.0f64; MAX_ACTIONS];
            for (i, &av) in action_values.iter().enumerate().take(actions.len()) {
                regrets[i] = pi_neg_o * (av - node_value);
            }

            strategy.update_entry(
                &info_set,
                &regrets[..actions.len()],
                &strat[..actions.len()],
                pi_o,
                iter_weight,
            );
        }

        node_value
    }

    /// Returns a placeholder estimate of strategy exploitability.
    ///
    /// **Note:** This is a stub implementation that returns `1/(iteration+1)` for
    /// progress tracking purposes only. A proper exploitability calculation requires
    /// computing best response values through a separate traversal, which is not
    /// yet implemented. The returned value decreases with iterations but does not
    /// represent actual exploitability in game units.
    ///
    /// For production use, implement a proper best response calculation that:
    /// 1. Computes the best response strategy for each player against the current strategy
    /// 2. Calculates the expected value of each best response
    /// 3. Returns the average of both players' best response values
    #[inline]
    #[allow(clippy::cast_precision_loss)]
    fn estimate_exploitability_placeholder(&self) -> f64 {
        1.0 / (self.iteration as f64 + 1.0)
    }

    #[inline]
    #[allow(clippy::cast_precision_loss)]
    fn get_utility_impl(
        state: &GameState,
        hands: &[[Card; 2]],
        board: &[Card],
        player: Player,
    ) -> f64 {
        if state.is_fold() {
            let Some(winner) = state.winner() else {
                return 0.0;
            };
            let player_committed = state.committed[player.index()] as f64;
            return if winner == player {
                state.pot as f64 - player_committed
            } else {
                -player_committed
            };
        }

        let visible = if state.is_all_in_showdown() {
            board.len().min(5)
        } else {
            state.visible_board_count(board.len())
        };
        let board_set = CardSet::from_cards(&board[..visible]);
        let hole = &hands[player.index()];
        let opp_hole = &hands[player.opponent().index()];
        let player_committed = state.committed[player.index()] as f64;

        let hand = Hand::evaluate(hole, board_set.as_slice());
        let opp_hand = Hand::evaluate(opp_hole, board_set.as_slice());

        match hand.cmp(&opp_hand) {
            std::cmp::Ordering::Greater => state.pot as f64 - player_committed,
            std::cmp::Ordering::Less => -player_committed,
            std::cmp::Ordering::Equal => (state.pot as f64 / 2.0) - player_committed,
        }
    }
}
