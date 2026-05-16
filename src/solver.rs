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
use rand::seq::SliceRandom;
use rayon::prelude::*;
use tracing::{info, warn};

use thiserror::Error;

use crate::card::{Card, CardSet, Deck};
use crate::config::{CFRConfig, CFRConfigError, ConfigError, GameConfig};
use crate::game::{GameState, InfoSet, Player, Street};
use crate::hand::Hand;
use crate::strategy::{Strategy, MAX_ACTIONS};

/// Minimum reach probability product before pruning a subtree.
///
/// When both `pi_reach` and `pi_neg_reach` fall below this threshold,
/// the subtree contribution is negligible and traversal is skipped.
const CFR_PRUNE_THRESHOLD: f64 = 1e-10;

/// Immutable per-deal context shared across CFR and best-response traversals.
///
/// Groups the data that is fixed for a single card deal (hole cards, board)
/// so that traversal functions take a single `&DealContext` instead of
/// multiple separate parameters.
///
/// Hole cards are stored in canonical (sorted) order. The original deal order
/// is irrelevant because [`Hand::evaluate`] sorts internally.
#[derive(Debug, Clone)]
struct DealContext {
    holes: [[Card; 2]; 2],
    board_sets: BoardSets,
}

/// Precomputed board card sets indexed by street ordinal.
///
/// Avoids reconstructing [`CardSet`] on every CFR node visit by building
/// all four street views once from the shuffled board.
#[derive(Debug, Clone)]
pub(crate) struct BoardSets([CardSet; 4]);

impl BoardSets {
    pub(crate) fn from_board(board: &[Card]) -> Self {
        Self([
            CardSet::from_cards(&board[..Street::Preflop.board_card_count().min(board.len())]),
            CardSet::from_cards(&board[..Street::Flop.board_card_count().min(board.len())]),
            CardSet::from_cards(&board[..Street::Turn.board_card_count().min(board.len())]),
            CardSet::from_cards(&board[..Street::River.board_card_count().min(board.len())]),
        ])
    }

    #[inline]
    const fn get(&self, street: Street) -> &CardSet {
        &self.0[street.ordinal()]
    }
}

impl DealContext {
    /// Builds a deal context from hole cards and the full 5-card board.
    ///
    /// Hole cards are sorted into canonical order for consistent info-set
    /// construction during CFR traversal.
    #[inline]
    fn new(hands: [[Card; 2]; 2], board: &[Card]) -> Self {
        let board_sets = BoardSets::from_board(board);
        let mut holes = hands;
        holes[0].sort_unstable();
        holes[1].sort_unstable();
        Self { holes, board_sets }
    }
}

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
#[derive(Debug)]
pub struct CFRSolver {
    /// The computed strategy (shared for concurrent access).
    pub(crate) strategy: Arc<Strategy>,
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

    /// Returns a reference to the computed strategy.
    #[must_use]
    #[inline]
    pub fn strategy(&self) -> &Strategy {
        &self.strategy
    }

    /// Runs the CFR+ algorithm for the configured number of iterations.
    #[allow(clippy::cast_precision_loss)]
    pub fn solve(&mut self) {
        let start = Instant::now();

        info!(
            "Starting CFR+ solver with {} iterations",
            self.cfr_config.num_iterations
        );

        let mut converged_early = false;

        for iter in 1..=self.cfr_config.num_iterations {
            self.iteration = iter;

            let iter_weight = iter as f64;

            self.run_iteration(iter_weight);

            let mut current_exploitability = None;

            if self.cfr_config.exploitability_interval > 0
                && iter % self.cfr_config.exploitability_interval == 0
            {
                let exploitability = self.compute_exploitability(self.cfr_config.exploitability_samples);
                current_exploitability = Some(exploitability);

                if self.cfr_config.convergence_threshold > 0.0
                    && exploitability <= self.cfr_config.convergence_threshold
                {
                    let stats = self.strategy.stats();
                    info!(
                        "Converged at iteration {iter} (exploitability {exploitability:.6} <= threshold {}, {stats})",
                        self.cfr_config.convergence_threshold,
                    );

                    if let Some(ref path) = self.cfr_config.save_path {
                        if let Err(e) = self.strategy.save(path) {
                            warn!("Failed to save strategy: {}", e);
                        } else {
                            info!("Saved strategy to {}", path);
                        }
                    }
                    converged_early = true;
                    break;
                }
            }

            if iter % self.cfr_config.log_interval == 0 {
                let elapsed = start.elapsed();
                let stats = self.strategy.stats();

                if let Some(expl) = current_exploitability {
                    info!(
                        "Iteration {}: {stats}, exploitability: {expl:.6}, elapsed: {elapsed:?}",
                        iter
                    );
                } else {
                    info!(
                        "Iteration {}: {stats}, elapsed: {elapsed:?}",
                        iter
                    );
                }
            }

            if let Some(ref path) = self.cfr_config.save_path {
                if self.cfr_config.save_interval > 0
                    && iter < self.cfr_config.num_iterations
                    && iter % self.cfr_config.save_interval == 0
                {
                    if let Err(e) = self.strategy.save(path) {
                        warn!("Failed to save strategy: {}", e);
                    } else {
                        info!("Saved strategy to {}", path);
                    }
                }
            }
        }

        // Always save the final strategy, even if save_interval didn't land
        // on the last iteration. The final strategy is the best one.
        // Skip if convergence already saved it.
        if !converged_early {
            if let Some(ref path) = self.cfr_config.save_path {
                if let Err(e) = self.strategy.save(path) {
                    warn!("Failed to save final strategy: {}", e);
                } else {
                    info!("Saved final strategy to {}", path);
                }
            }
        }

        let total = start.elapsed();
        info!("CFR+ completed in {:?}", total);
    }

    fn run_iteration(&self, iter_weight: f64) {
        if self.cfr_config.use_chance_sampling {
            self.run_iteration_sampled(iter_weight);
        } else {
            self.run_iteration_full(iter_weight);
        }
    }

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
            let hands = [
                [hole_cards[0], hole_cards[1]],
                [hole_cards[2], hole_cards[3]],
            ];

            let board = deck.deal_into::<5>();
            let deal = DealContext::new(hands, &board);
            let state = GameState::new(config);

            for &player in &Player::ALL {
                Self::cfr_traversal_static(
                    &strategy,
                    &state,
                    &deal,
                    player,
                    1.0,
                    1.0,
                    iter_weight,
                );
            }
        });
    }

    fn run_iteration_full(&self, iter_weight: f64) {
        let all_cards = Card::all();
        let num_cards = all_cards.len();
        let strategy = self.strategy.clone();
        let config = self.config;

        (0..num_cards).into_par_iter().for_each(|i| {
            let mut rng = rand::rngs::StdRng::seed_from_u64(
                (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ iter_weight.to_bits(),
            );
            for j in (i + 1)..num_cards {
                for k in (j + 1)..num_cards {
                    for l in (k + 1)..num_cards {
                        // All 6 ordered splits of {i,j,k,l} into SB/BB hole cards.
                        // Each 4-card set has C(4,2) = 6 ways to assign two cards
                        // to SB (rest to BB). Previously only one split was used,
                        // missing 5/6 of all deals and biasing regret updates.
                        for (a, b, c, d) in [
                            (i, j, k, l),
                            (i, k, j, l),
                            (i, l, j, k),
                            (j, k, i, l),
                            (j, l, i, k),
                            (k, l, i, j),
                        ] {
                            Self::process_card_combination(
                                &strategy,
                                config,
                                &mut rng,
                                all_cards,
                                a,
                                b,
                                c,
                                d,
                                iter_weight,
                            );
                        }
                    }
                }
            }
        });
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn process_card_combination(
        strategy: &Strategy,
        config: GameConfig,
        rng: &mut rand::rngs::StdRng,
        all_cards: &[Card],
        i: usize,
        j: usize,
        k: usize,
        l: usize,
        iter_weight: f64,
    ) {
        let hands = [[all_cards[i], all_cards[j]], [all_cards[k], all_cards[l]]];
        let excluded_mask: u64 = (1u64 << i) | (1u64 << j) | (1u64 << k) | (1u64 << l);

        let mut remaining: [Card; 48] = [Card::placeholder(); 48];
        let mut remaining_len = 0;
        for (idx, &c) in all_cards.iter().enumerate() {
            if (excluded_mask & (1u64 << idx)) == 0 {
                remaining[remaining_len] = c;
                remaining_len += 1;
            }
        }

        debug_assert!(remaining_len >= 5, "not enough remaining cards for board");
        remaining[..remaining_len].partial_shuffle(rng, 5);

        // partial_shuffle places the randomly selected elements at the END
        // of the slice (indices [len-amount..len]), not the beginning.
        let board_start = remaining_len - 5;
        let deal = DealContext::new(hands, &remaining[board_start..remaining_len]);
        let state = GameState::new(config);

        for &player in &Player::ALL {
            Self::cfr_traversal_static(
                strategy,
                &state,
                &deal,
                player,
                1.0,
                1.0,
                iter_weight,
            );
        }
    }

    #[inline]
    fn cfr_traversal_static(
        strategy: &Strategy,
        state: &GameState,
        deal: &DealContext,
        player: Player,
        pi_reach: f64,
        pi_neg_reach: f64,
        iter_weight: f64,
    ) -> f64 {
        if state.is_terminal() {
            return Self::get_utility_impl(state, &deal.holes, &deal.board_sets, player);
        }

        if pi_reach < CFR_PRUNE_THRESHOLD && pi_neg_reach < CFR_PRUNE_THRESHOLD {
            return 0.0;
        }

        let actions = state.legal_actions();

        if actions.is_empty() {
            return Self::get_utility_impl(state, &deal.holes, &deal.board_sets, player);
        }

        let current = state.current_player;

        let board_set = deal.board_sets.get(state.street);
        let hole = &deal.holes[current.index()];

        let info_set = InfoSet::from_cards_with_history(
            current,
            state.street,
            hole,
            board_set.clone(),
            state.history.clone(),
        );

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
                    deal,
                    player,
                    pi_reach * strat[i],
                    pi_neg_reach,
                    iter_weight,
                )
            } else {
                Self::cfr_traversal_static(
                    strategy,
                    &new_state,
                    deal,
                    player,
                    pi_reach,
                    pi_neg_reach * strat[i],
                    iter_weight,
                )
            };

            action_values[i] = value;
            node_value = strat[i].mul_add(value, node_value);
        }

        if current == player {
            let mut regrets = [0.0f64; MAX_ACTIONS];
            for (i, &av) in action_values.iter().enumerate().take(actions.len()) {
                regrets[i] = pi_neg_reach * (av - node_value);
            }

            strategy.update_entry(
                &info_set,
                &regrets[..actions.len()],
                &strat[..actions.len()],
                pi_reach,
                iter_weight,
            );
        }

        node_value
    }

    /// Computes the exploitability of the current average strategy via
    /// Monte Carlo best-response estimation.
    ///
    /// For each sampled deal, computes the best-response value for both
    /// players against the opponent's average strategy. The exploitability
    /// is the average of both players' best-response values.
    ///
    /// Higher `num_samples` gives a more accurate estimate but takes longer.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn compute_exploitability(&self, num_samples: usize) -> f64 {
        let strategy = self.strategy.clone();
        let config = self.config;

        let total: f64 = (0..num_samples)
            .into_par_iter()
            .map(|_| {
                let mut rng = thread_rng();
                let mut deck = Deck::new();
                deck.shuffle(&mut rng);

                let hole_cards = deck.deal_into::<4>();
                let hands = [
                    [hole_cards[0], hole_cards[1]],
                    [hole_cards[2], hole_cards[3]],
                ];
                let board = deck.deal_into::<5>();
                let deal = DealContext::new(hands, &board);

                let mut br_sum = 0.0f64;
                for &player in &Player::ALL {
                    let state = GameState::new(config);
                    br_sum +=
                        Self::best_response_traversal(&strategy, &state, &deal, player);
                }
                br_sum
            })
            .sum();

        total / (2.0 * num_samples as f64)
    }

    #[inline]
    fn best_response_traversal(
        strategy: &Strategy,
        state: &GameState,
        deal: &DealContext,
        br_player: Player,
    ) -> f64 {
        if state.is_terminal() {
            return Self::get_utility_impl(state, &deal.holes, &deal.board_sets, br_player);
        }

        let actions = state.legal_actions();
        if actions.is_empty() {
            return Self::get_utility_impl(state, &deal.holes, &deal.board_sets, br_player);
        }

        let current = state.current_player;

        let board_set = deal.board_sets.get(state.street);
        let hole = &deal.holes[current.index()];

        let info_set = InfoSet::from_cards_with_history(
            current,
            state.street,
            hole,
            board_set.clone(),
            state.history.clone(),
        );

        if current == br_player {
            let mut best_value = f64::NEG_INFINITY;
            for &action in &actions {
                let new_state = state.apply_action(action);
                let value =
                    Self::best_response_traversal(strategy, &new_state, deal, br_player);
                if value > best_value {
                    best_value = value;
                }
            }
            best_value
        } else {
            let mut strat = [0.0f64; MAX_ACTIONS];
            let _ = strategy.get_average_strategy(
                &info_set,
                actions.len(),
                &mut strat[..actions.len()],
            );

            let mut node_value = 0.0f64;
            for (i, &action) in actions.iter().enumerate() {
                let new_state = state.apply_action(action);
                let value =
                    Self::best_response_traversal(strategy, &new_state, deal, br_player);
                node_value = strat[i].mul_add(value, node_value);
            }
            node_value
        }
    }

    #[inline]
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    pub(crate) fn get_utility_impl(
        state: &GameState,
        hands: &[[Card; 2]],
        board_sets: &BoardSets,
        player: Player,
    ) -> f64 {
        if state.is_fold() {
            // Invariant: is_fold() is true implies the last action is Fold,
            // which means current_player (opponent of folder) is the winner.
            // The `else` branch is unreachable but avoids unwrap/expect.
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

        // All-in showdown always uses the full board (River entry);
        // normal showdown uses the board visible on the current street.
        let board_set = if state.is_all_in_showdown() {
            board_sets.get(Street::River)
        } else {
            board_sets.get(state.street)
        };
        let hole = &hands[player.index()];
        let opp_hole = &hands[player.opponent().index()];

        // Cap utility at the contested amount (min of both commitments).
        // When stacks are unequal, only the matching portion is at risk;
        // excess is returned to the bigger-stack player.
        let contested = state.committed[player.index()]
            .min(state.committed[player.opponent().index()]) as f64;

        let hand = Hand::evaluate(hole, board_set.as_slice());
        let opp_hand = Hand::evaluate(opp_hole, board_set.as_slice());

        match hand.cmp(&opp_hand) {
            std::cmp::Ordering::Greater => contested,
            std::cmp::Ordering::Less => -contested,
            std::cmp::Ordering::Equal => 0.0,
        }
    }
}
