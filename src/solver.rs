//! CFR+ algorithm implementation.

#![allow(clippy::too_many_arguments)]

use std::sync::Arc;
use std::time::Instant;

use rayon::prelude::*;
use tracing::{info, warn};

use crate::card::{Card, CardSet, Deck};
use crate::config::{CFRConfig, GameConfig};
use crate::game::{GameState, InfoSet, Player};
use crate::hand::Hand;
use crate::strategy::{Strategy, MAX_ACTIONS};

/// CFR+ solver for heads-up No-Limit Hold'em.
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
    #[must_use]
    pub fn new(game_config: GameConfig, cfr_config: CFRConfig) -> Self {
        let strategy = Arc::new(Strategy::new());
        CFRSolver {
            config: game_config,
            cfr_config,
            strategy,
            iteration: 0,
        }
    }

    /// Returns the current iteration number.
    #[must_use]
    pub fn iteration(&self) -> usize {
        self.iteration
    }

    /// Runs the CFR+ algorithm for the configured number of iterations.
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
                    "Iteration {}: {} info sets, {:.2} MB, exploitability (placeholder): {:.6}, elapsed: {:?}",
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

    fn run_iteration(&mut self, iter_weight: f64) {
        if self.cfr_config.use_chance_sampling {
            self.run_iteration_sampled(iter_weight);
        } else {
            self.run_iteration_full(iter_weight);
        }
    }

    fn run_iteration_sampled(&self, iter_weight: f64) {
        use rand::prelude::*;

        let mut rng = thread_rng();
        let mut deck = Deck::new();
        deck.shuffle(&mut rng);

        let hole_sb = [
            deck.deal_one().expect("deck should have 52 cards"),
            deck.deal_one().expect("deck should have 51 cards"),
        ];
        let hole_bb = [
            deck.deal_one().expect("deck should have 50 cards"),
            deck.deal_one().expect("deck should have 49 cards"),
        ];
        let board: Vec<Card> = deck.deal(5);
        let hands = [hole_sb, hole_bb];

        let state = GameState::new(self.config);

        Self::cfr_traversal_static(
            &self.strategy,
            &state,
            &hands,
            &board,
            Player::SB,
            1.0,
            1.0,
            iter_weight,
        );
    }

    fn run_iteration_full(&self, iter_weight: f64) {
        use rand::prelude::*;

        let all_cards = Card::all();
        let num_cards = all_cards.len();
        let strategy = self.strategy.clone();
        let config = self.config;

        (0..num_cards).into_par_iter().for_each(|i| {
            let mut rng = rand::rngs::StdRng::from_entropy();
            for j in (i + 1)..num_cards {
                for k in (j + 1)..num_cards {
                    for l in (k + 1)..num_cards {
                        let hole_sb = [all_cards[i], all_cards[j]];
                        let hole_bb = [all_cards[k], all_cards[l]];
                        let excluded_mask: u64 =
                            (1u64 << i) | (1u64 << j) | (1u64 << k) | (1u64 << l);

                        let mut remaining: Vec<Card> = all_cards
                            .iter()
                            .enumerate()
                            .filter(|(idx, _)| (excluded_mask & (1u64 << idx)) == 0)
                            .map(|(_, c)| *c)
                            .collect();

                        use rand::seq::SliceRandom;
                        remaining.partial_shuffle(&mut rng, 5);
                        let board: Vec<Card> = remaining.into_iter().take(5).collect();

                        let hands = [hole_sb, hole_bb];
                        let state = GameState::new(config);

                        Self::cfr_traversal_static(
                            &strategy,
                            &state,
                            &hands,
                            &board,
                            Player::SB,
                            1.0,
                            1.0,
                            iter_weight,
                        );
                    }
                }
            }
        });
    }

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

        let board_set =
            CardSet::from_cards(&board[..state.street.board_card_count().min(board.len())]);
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

    fn estimate_exploitability_placeholder(&self) -> f64 {
        1.0 / (self.iteration as f64 + 1.0)
    }

    #[inline]
    fn get_utility_impl(
        state: &GameState,
        hands: &[[Card; 2]],
        board: &[Card],
        player: Player,
    ) -> f64 {
        if state.is_fold() {
            let winner = state.winner().unwrap();
            let player_committed = state.committed[player.index()] as f64;
            return if winner == player {
                state.pot as f64 - player_committed
            } else {
                -player_committed
            };
        }

        let board_set =
            CardSet::from_cards(&board[..state.street.board_card_count().min(board.len())]);
        let hole = &hands[player.index()];
        let opp_index = 1 - player.index();
        let opp_hole = &hands[opp_index];
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
