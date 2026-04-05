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

#[cfg(test)]
mod tests {
    use crate::{
        Action, CFRConfig, CFRSolver, Card, CardSet, Deck, GameConfig, GameState, Hand, HandType,
        InfoSet, Player, Strategy, StrategyEntry, Street,
    };

    fn card(rank: u8, suit: u8) -> Card {
        Card::new(rank, suit).expect("valid card")
    }

    #[test]
    fn test_hand_high_card() {
        let hole = [card(14, 0), card(12, 1)];
        let board = [card(10, 2), card(8, 3), card(5, 0), card(3, 1), card(2, 2)];
        let hand = Hand::evaluate(&hole, &board);
        assert_eq!(hand.hand_type(), HandType::HighCard);
        let low_hand = Hand::evaluate(&[card(9, 0), card(7, 1)], &board);
        assert!(hand > low_hand);
    }

    #[test]
    fn test_hand_pair() {
        let hole = [card(14, 0), card(14, 1)];
        let board = [card(10, 2), card(8, 3), card(5, 0), card(3, 1), card(2, 2)];
        let hand = Hand::evaluate(&hole, &board);
        assert_eq!(hand.hand_type(), HandType::Pair);
        let high_card_hand = Hand::evaluate(&[card(13, 0), card(12, 1)], &board);
        assert!(hand > high_card_hand);
    }

    #[test]
    fn test_hand_two_pair() {
        let hole = [card(14, 0), card(14, 1)];
        let board = [card(10, 0), card(10, 1), card(5, 0), card(3, 1), card(2, 2)];
        let hand = Hand::evaluate(&hole, &board);
        assert_eq!(hand.hand_type(), HandType::TwoPair);
        let pair_hand = Hand::evaluate(&[card(14, 0), card(13, 1)], &board);
        assert!(hand > pair_hand);
    }

    #[test]
    fn test_hand_three_of_a_kind() {
        let hole = [card(14, 0), card(14, 1)];
        let board = [card(14, 2), card(8, 3), card(5, 0), card(3, 1), card(2, 2)];
        let hand = Hand::evaluate(&hole, &board);
        assert_eq!(hand.hand_type(), HandType::ThreeOfAKind);
        let two_pair_hand = Hand::evaluate(&[card(10, 0), card(10, 1)], &board);
        assert!(hand > two_pair_hand);
    }

    #[test]
    fn test_hand_straight() {
        let hole = [card(14, 0), card(13, 1)];
        let board = [
            card(12, 2),
            card(11, 3),
            card(10, 0),
            card(3, 1),
            card(2, 2),
        ];
        let hand = Hand::evaluate(&hole, &board);
        assert_eq!(hand.hand_type(), HandType::Straight);
        let three_kind_hand = Hand::evaluate(&[card(10, 0), card(10, 1)], &board);
        assert!(hand > three_kind_hand);
    }

    #[test]
    fn test_hand_wheel_straight() {
        let hole = [card(14, 0), card(2, 1)];
        let board = [card(5, 2), card(4, 3), card(3, 0), card(10, 1), card(9, 2)];
        let hand = Hand::evaluate(&hole, &board);
        assert_eq!(hand.hand_type(), HandType::Straight);
        let high_card_hand = Hand::evaluate(&[card(13, 0), card(12, 1)], &board);
        assert!(hand > high_card_hand);
    }

    #[test]
    fn test_hand_flush() {
        let hole = [card(14, 0), card(12, 0)];
        let board = [card(10, 0), card(8, 0), card(5, 0), card(3, 1), card(2, 2)];
        let hand = Hand::evaluate(&hole, &board);
        assert_eq!(hand.hand_type(), HandType::Flush);
        let straight_hand = Hand::evaluate(&[card(11, 1), card(9, 2)], &board);
        assert!(hand > straight_hand);
    }

    #[test]
    fn test_hand_full_house() {
        let hole = [card(14, 0), card(14, 1)];
        let board = [
            card(14, 2),
            card(10, 0),
            card(10, 1),
            card(3, 1),
            card(2, 2),
        ];
        let hand = Hand::evaluate(&hole, &board);
        assert_eq!(hand.hand_type(), HandType::FullHouse);
        let flush_hand = Hand::evaluate(&[card(12, 0), card(11, 0)], &board);
        assert!(hand > flush_hand);
    }

    #[test]
    fn test_hand_four_of_a_kind() {
        let hole = [card(14, 0), card(14, 1)];
        let board = [card(14, 2), card(14, 3), card(5, 0), card(3, 1), card(2, 2)];
        let hand = Hand::evaluate(&hole, &board);
        assert_eq!(hand.hand_type(), HandType::FourOfAKind);
        let full_house_hand = Hand::evaluate(&[card(10, 0), card(10, 1)], &board);
        assert!(hand > full_house_hand);
    }

    #[test]
    fn test_hand_straight_flush() {
        let hole = [card(9, 0), card(8, 0)];
        let board = [card(7, 0), card(6, 0), card(5, 0), card(3, 1), card(2, 2)];
        let hand = Hand::evaluate(&hole, &board);
        assert_eq!(hand.hand_type(), HandType::StraightFlush);
        let four_kind_hand = Hand::evaluate(&[card(5, 1), card(5, 2)], &board);
        assert!(hand > four_kind_hand);
    }

    #[test]
    fn test_hand_royal_flush() {
        let hole = [card(14, 0), card(13, 0)];
        let board = [
            card(12, 0),
            card(11, 0),
            card(10, 0),
            card(3, 1),
            card(2, 2),
        ];
        let hand = Hand::evaluate(&hole, &board);
        assert_eq!(hand.hand_type(), HandType::RoyalFlush);
        let straight_flush_hand = Hand::evaluate(&[card(9, 0), card(8, 0)], &board);
        assert!(hand > straight_flush_hand);
    }

    #[test]
    fn test_hand_type_display() {
        assert_eq!(format!("{}", HandType::RoyalFlush), "Royal Flush");
        assert_eq!(format!("{}", HandType::StraightFlush), "Straight Flush");
        assert_eq!(format!("{}", HandType::FourOfAKind), "Four of a Kind");
        assert_eq!(format!("{}", HandType::FullHouse), "Full House");
        assert_eq!(format!("{}", HandType::Flush), "Flush");
        assert_eq!(format!("{}", HandType::Straight), "Straight");
        assert_eq!(format!("{}", HandType::ThreeOfAKind), "Three of a Kind");
        assert_eq!(format!("{}", HandType::TwoPair), "Two Pair");
        assert_eq!(format!("{}", HandType::Pair), "Pair");
        assert_eq!(format!("{}", HandType::HighCard), "High Card");
    }

    #[test]
    fn test_game_state_initial() {
        let config = GameConfig {
            initial_stacks: [1000, 1000],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        assert_eq!(state.pot, 3);
        assert_eq!(state.committed, [1, 2]);
        assert_eq!(state.current_player, Player::SB);
        assert!(!state.is_terminal());
    }

    #[test]
    fn test_legal_actions_preflop() {
        let config = GameConfig {
            initial_stacks: [1000, 1000],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        let actions = state.legal_actions();
        assert!(actions.contains(&Action::Fold));
        assert!(actions.contains(&Action::Call));
        assert!(!actions.contains(&Action::Check));
    }

    #[test]
    fn test_betting_round_closed_check_check() {
        let config = GameConfig {
            initial_stacks: [1000, 1000],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        // Preflop: SB calls, BB checks (option)
        let state = state.apply_action(Action::Call);
        assert!(!state.betting_round_closed());
        let state = state.apply_action(Action::Check);
        assert_eq!(state.street, Street::Flop);
        // Flop: SB checks, BB checks
        let state = state.apply_action(Action::Check);
        assert!(!state.betting_round_closed());
        let state = state.apply_action(Action::Check);
        assert_eq!(state.street, Street::Turn);
    }

    #[test]
    fn test_betting_round_closed_call() {
        let config = GameConfig {
            initial_stacks: [1000, 1000],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        // Legal raises from preflop with pot=3: Raise(2) and Raise(3)
        let state = state.apply_action(Action::Raise(2));
        assert_eq!(state.street, Street::Preflop);
        assert!(!state.betting_round_closed());
        let state = state.apply_action(Action::Call);
        assert_eq!(state.street, Street::Flop);
    }

    #[test]
    fn test_street_advancement() {
        let config = GameConfig {
            initial_stacks: [1000, 1000],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        let state = state.apply_action(Action::Call);
        let state = state.apply_action(Action::Check);
        assert_eq!(state.street, Street::Flop);
    }

    #[test]
    fn test_fold_terminal_sb_folds_bb_wins() {
        let config = GameConfig {
            initial_stacks: [1000, 1000],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        let state = state.apply_action(Action::Fold);
        assert!(state.is_terminal());
        assert!(state.is_fold());
        assert_eq!(state.winner(), Some(Player::BB));
    }

    #[test]
    fn test_fold_terminal_bb_folds_sb_wins() {
        let config = GameConfig {
            initial_stacks: [1000, 1000],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        let state = state.apply_action(Action::Raise(2));
        let state = state.apply_action(Action::Fold);
        assert!(state.is_terminal());
        assert!(state.is_fold());
        assert_eq!(state.winner(), Some(Player::SB));
    }

    #[test]
    fn test_player_opponent() {
        assert_eq!(Player::SB.opponent(), Player::BB);
        assert_eq!(Player::BB.opponent(), Player::SB);
    }

    #[test]
    fn test_player_from_index() {
        assert_eq!(Player::from_index(0), Some(Player::SB));
        assert_eq!(Player::from_index(1), Some(Player::BB));
        assert_eq!(Player::from_index(2), None);
    }

    #[test]
    fn test_street_next() {
        assert_eq!(Street::Preflop.next(), Some(Street::Flop));
        assert_eq!(Street::Flop.next(), Some(Street::Turn));
        assert_eq!(Street::Turn.next(), Some(Street::River));
        assert_eq!(Street::River.next(), None);
    }

    #[test]
    fn test_street_board_card_count() {
        assert_eq!(Street::Preflop.board_card_count(), 0);
        assert_eq!(Street::Flop.board_card_count(), 3);
        assert_eq!(Street::Turn.board_card_count(), 4);
        assert_eq!(Street::River.board_card_count(), 5);
    }

    #[test]
    fn test_deck_deal() {
        let mut deck = Deck::new();
        use rand::prelude::*;
        let mut rng = thread_rng();
        deck.shuffle(&mut rng);
        let cards = deck.deal(5);
        assert_eq!(cards.len(), 5);
    }

    #[test]
    fn test_card_set() {
        let cards = vec![card(14, 0), card(13, 1), card(12, 2)];
        let set = CardSet::from_cards(&cards);
        assert_eq!(set.len(), 3);
        assert_eq!(set.as_slice().len(), 3);
        assert!(!set.is_empty());
    }

    #[test]
    fn test_card_set_empty() {
        let set = CardSet::from_cards(&[]);
        assert_eq!(set.len(), 0);
        assert!(set.is_empty());
    }

    #[test]
    fn test_strategy_entry_get_strategy_uniform() {
        let entry = StrategyEntry::new(3);
        let mut strat = [0.0f64; 8];
        entry.get_strategy(&mut strat);
        assert!((strat[0] - 1.0 / 3.0).abs() < 1e-10);
        assert!((strat[1] - 1.0 / 3.0).abs() < 1e-10);
        assert!((strat[2] - 1.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_strategy_entry_get_average_strategy_uniform() {
        let entry = StrategyEntry::new(2);
        let mut avg = [0.0f64; 8];
        entry.get_average_strategy(&mut avg);
        assert!((avg[0] - 0.5).abs() < 1e-10);
        assert!((avg[1] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_strategy_entry_get_average_strategy_after_update() {
        let mut entry = StrategyEntry::new(2);
        entry.update(&[1.0, 3.0], &[0.25, 0.75], 1.0, 1.0);
        let mut avg = [0.0f64; 8];
        entry.get_average_strategy(&mut avg);
        let sum = avg[0] + avg[1];
        assert!((sum - 1.0).abs() < 1e-10);
        assert!(avg[1] > avg[0]);
    }

    #[test]
    fn test_strategy_get_average_strategy_missing_entry() {
        let strategy = Strategy::new();
        let hole = [card(14, 0), card(13, 1)];
        let board = CardSet::from_cards(&[card(10, 2), card(9, 3), card(8, 0)]);
        let info_set = InfoSet::from_cards(Player::SB, Street::Flop, &hole, board);

        let mut avg = [0.0f64; 8];
        let found = strategy.get_average_strategy(&info_set, 3, &mut avg);
        assert!(!found);
        assert!((avg[0] - 1.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_strategy_get_average_strategy_existing_entry() {
        let strategy = Strategy::new();
        let hole = [card(14, 0), card(13, 1)];
        let board = CardSet::from_cards(&[card(10, 2), card(9, 3), card(8, 0)]);
        let info_set = InfoSet::from_cards(Player::SB, Street::Flop, &hole, board);

        let mut strat = [0.0f64; 8];
        strategy.get_strategy(&info_set, 3, &mut strat);
        strategy.update_entry(&info_set, &[1.0, 2.0, 3.0], &[0.3, 0.4, 0.3], 1.0, 1.0);

        let mut avg = [0.0f64; 8];
        let found = strategy.get_average_strategy(&info_set, 3, &mut avg);
        assert!(found);
        let sum: f64 = avg[..3].iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_hand_rank_ordering() {
        let board = [card(10, 0), card(8, 1), card(5, 2), card(3, 3), card(2, 0)];
        let high_card = Hand::evaluate(&[card(14, 0), card(12, 1)], &board);
        let pair = Hand::evaluate(&[card(14, 0), card(14, 1)], &board);
        assert!(pair > high_card);
    }

    #[test]
    fn test_card_new_valid() {
        assert!(Card::new(2, 0).is_some());
        assert!(Card::new(14, 3).is_some());
        assert!(Card::new(7, 2).is_some());
    }

    #[test]
    fn test_card_new_invalid() {
        assert!(Card::new(1, 0).is_none());
        assert!(Card::new(15, 0).is_none());
        assert!(Card::new(7, 4).is_none());
    }

    #[test]
    fn test_card_is_valid() {
        assert!(card(2, 0).is_valid());
        assert!(card(14, 3).is_valid());
        assert!(!Card::default().is_valid());
    }

    #[test]
    fn test_strategy_default() {
        let strategy = Strategy::new();
        let stats = strategy.stats();
        assert_eq!(stats.info_sets, 0);
    }

    #[test]
    fn test_cfr_config_default() {
        let config = CFRConfig::default();
        assert_eq!(config.num_iterations, 100);
        assert!(config.use_chance_sampling);
    }

    #[test]
    fn test_card_display() {
        let ace_spades = card(14, 3);
        assert_eq!(format!("{ace_spades}"), "As");
        let two_clubs = card(2, 0);
        assert_eq!(format!("{two_clubs}"), "2c");
        let king_hearts = card(13, 2);
        assert_eq!(format!("{king_hearts}"), "Kh");
        let ten_diamonds = card(10, 1);
        assert_eq!(format!("{ten_diamonds}"), "Td");
    }

    #[test]
    fn test_player_display() {
        assert_eq!(format!("{}", Player::SB), "SB");
        assert_eq!(format!("{}", Player::BB), "BB");
    }

    #[test]
    fn test_street_display() {
        assert_eq!(format!("{}", Street::Preflop), "Preflop");
        assert_eq!(format!("{}", Street::Flop), "Flop");
        assert_eq!(format!("{}", Street::Turn), "Turn");
        assert_eq!(format!("{}", Street::River), "River");
    }

    #[test]
    fn test_action_display() {
        assert_eq!(format!("{}", Action::Fold), "Fold");
        assert_eq!(format!("{}", Action::Check), "Check");
        assert_eq!(format!("{}", Action::Call), "Call");
        assert_eq!(format!("{}", Action::Bet(100)), "Bet(100)");
        assert_eq!(format!("{}", Action::Raise(50)), "Raise(50)");
        assert_eq!(format!("{}", Action::AllIn), "AllIn");
    }

    #[test]
    fn test_game_config_validate() {
        let valid = GameConfig {
            initial_stacks: [1000, 1000],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        assert!(valid.validate().is_ok());

        let invalid_stacks = GameConfig {
            initial_stacks: [0, 1000],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        assert!(invalid_stacks.validate().is_err());

        let invalid_blinds = GameConfig {
            initial_stacks: [1000, 1000],
            small_blind: 0,
            big_blind: 2,
            min_bet: 2,
        };
        assert!(invalid_blinds.validate().is_err());

        let invalid_blind_ratio = GameConfig {
            initial_stacks: [1000, 1000],
            small_blind: 5,
            big_blind: 2,
            min_bet: 2,
        };
        assert!(invalid_blind_ratio.validate().is_err());
    }

    #[test]
    fn test_cfr_config_validate() {
        let valid = CFRConfig::default();
        assert!(valid.validate().is_ok());

        let invalid_iterations = CFRConfig {
            num_iterations: 0,
            ..CFRConfig::default()
        };
        assert!(invalid_iterations.validate().is_err());

        let invalid_threshold = CFRConfig {
            convergence_threshold: -1.0,
            ..CFRConfig::default()
        };
        assert!(invalid_threshold.validate().is_err());
    }

    #[test]
    fn test_legal_actions_iter() {
        let config = GameConfig {
            initial_stacks: [1000, 1000],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        let actions = state.legal_actions();
        let count = actions.iter().count();
        assert!(count >= 3);
    }

    #[test]
    fn test_info_set_display() {
        let hole = [card(14, 0), card(13, 1)];
        let board = CardSet::from_cards(&[card(10, 2), card(9, 3), card(8, 0)]);
        let info_set = InfoSet::from_cards(Player::SB, Street::Flop, &hole, board);
        let display = format!("{info_set}");
        assert!(display.contains("SB"));
        assert!(display.contains("Flop"));
    }

    #[test]
    fn test_card_set_contains() {
        let ace = card(14, 0);
        let king = card(13, 1);
        let queen = card(12, 2);
        let set = CardSet::from_cards(&[ace, king]);
        assert!(set.contains(&ace));
        assert!(set.contains(&king));
        assert!(!set.contains(&queen));
    }

    #[test]
    fn test_strategy_save_load_roundtrip() {
        let dir = std::env::temp_dir().join("nash_test_strategy.bin");
        let path = dir.to_str().unwrap();

        let strategy = Strategy::new();
        let hole = [card(14, 0), card(13, 1)];
        let board = CardSet::from_cards(&[card(10, 2), card(9, 3), card(8, 0)]);
        let info_set = InfoSet::from_cards(Player::SB, Street::Flop, &hole, board);

        let mut strat = [0.0f64; 8];
        strategy.get_strategy(&info_set, 3, &mut strat);
        strategy.update_entry(&info_set, &[1.0, 2.0, 3.0], &[0.3, 0.4, 0.3], 1.0, 1.0);

        let mut expected = [0.0f64; 8];
        strategy.get_strategy(&info_set, 3, &mut expected);

        strategy.save(path).unwrap();
        let loaded = Strategy::load(path).unwrap();

        assert_eq!(loaded.len(), 1);
        let mut loaded_strat = [0.0f64; 8];
        loaded.get_strategy(&info_set, 3, &mut loaded_strat);
        for (a, b) in expected.iter().zip(loaded_strat.iter()) {
            assert!((a - b).abs() < 1e-10);
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_solver_runs() {
        let game_config = GameConfig {
            initial_stacks: [100, 100],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let cfr_config = CFRConfig {
            num_iterations: 3,
            log_interval: 1,
            save_interval: 100,
            save_path: None,
            use_chance_sampling: true,
            samples_per_iteration: 2,
            exploitability_interval: 0,
            convergence_threshold: 0.0,
        };

        let mut solver = CFRSolver::new(game_config, cfr_config).unwrap();
        solver.solve();
        assert!(!solver.strategy.is_empty());
    }

    #[test]
    fn test_action_history_push_and_iter() {
        use crate::ActionHistory;
        let mut history = ActionHistory::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);

        history.push(Action::Call);
        history.push(Action::Raise(10));
        assert!(!history.is_empty());
        assert_eq!(history.len(), 2);

        let actions: Vec<&Action> = history.iter().collect();
        assert_eq!(actions[0], &Action::Call);
        assert_eq!(actions[1], &Action::Raise(10));
    }

    #[test]
    fn test_action_history_equality() {
        use crate::ActionHistory;
        let mut a = ActionHistory::new();
        let mut b = ActionHistory::new();
        a.push(Action::Check);
        a.push(Action::Bet(5));
        b.push(Action::Check);
        b.push(Action::Bet(5));
        assert_eq!(a, b);

        b.push(Action::Call);
        assert_ne!(a, b);
    }

    #[test]
    fn test_deck_fixed_size() {
        let mut deck = Deck::new();
        assert_eq!(deck.deal(52).len(), 52);
        assert!(deck.deal(1).is_empty());

        let mut deck2 = Deck::new();
        use rand::prelude::*;
        let mut rng = thread_rng();
        deck2.shuffle(&mut rng);
        let cards = deck2.deal(5);
        assert_eq!(cards.len(), 5);
    }

    #[test]
    fn test_all_in_showdown_skips_streets() {
        let config = GameConfig {
            initial_stacks: [10, 10],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        assert!(!state.is_all_in_showdown());

        let state = state.apply_action(Action::AllIn);
        let state = state.apply_action(Action::Call);

        assert!(state.is_terminal());
        assert!(state.is_all_in_showdown());
        assert!(!state.is_fold());
    }

    #[test]
    fn test_all_in_vs_all_in_immediate_showdown() {
        let config = GameConfig {
            initial_stacks: [10, 10],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        let state = state.apply_action(Action::AllIn);
        // BB's to_call (8) == remaining (8), so AllIn is deduped to Call.
        let state = state.apply_action(Action::Call);

        assert!(state.is_terminal());
        assert!(state.is_all_in_showdown());
        assert!(!state.is_fold());
    }

    #[test]
    fn test_both_all_in_from_blinds() {
        let config = GameConfig {
            initial_stacks: [1, 2],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        assert!(state.is_all_in_showdown());
        assert!(state.is_terminal());
    }

    #[test]
    fn test_cfr_config_validate_log_and_save_intervals() {
        let invalid_log = CFRConfig {
            log_interval: 0,
            ..CFRConfig::default()
        };
        assert!(invalid_log.validate().is_err());

        let invalid_save = CFRConfig {
            save_interval: 0,
            ..CFRConfig::default()
        };
        assert!(invalid_save.validate().is_err());
    }

    // --- Unequal-stack utility tests ---

    #[test]
    fn test_unequal_stack_showdown_short_stack_wins() {
        // Both forced all-in from blinds: SB=1, BB=2. contested=min(1,2)=1.
        // SB wins → nets +1, BB loses -1.
        let config = GameConfig {
            initial_stacks: [1, 2],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        assert!(state.is_terminal());

        // SB has pocket Aces, BB has pocket Kings
        let hole_sb = [card(14, 0), card(14, 1)];
        let hole_bb = [card(13, 2), card(13, 3)];
        let board = [card(2, 0), card(4, 1), card(6, 2), card(8, 3), card(9, 0)];

        let utility =
            CFRSolver::get_utility_impl(&state, &[hole_sb, hole_bb], &board, Player::SB);
        assert!(
            (utility - 1.0).abs() < 1e-10,
            "SB should win contested amount (1), got {utility}"
        );

        let utility_bb =
            CFRSolver::get_utility_impl(&state, &[hole_sb, hole_bb], &board, Player::BB);
        assert!(
            (utility_bb - (-1.0)).abs() < 1e-10,
            "BB should lose contested amount (-1), got {utility_bb}"
        );
    }

    #[test]
    fn test_unequal_stack_showdown_big_stack_wins() {
        // Both forced all-in from blinds: SB=1, BB=2. contested=1.
        // BB wins → nets +1, SB loses -1.
        let config = GameConfig {
            initial_stacks: [1, 2],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);

        let hole_sb = [card(13, 0), card(13, 1)];
        let hole_bb = [card(14, 2), card(14, 3)];
        let board = [card(2, 0), card(4, 1), card(6, 2), card(8, 3), card(9, 0)];

        let utility_sb =
            CFRSolver::get_utility_impl(&state, &[hole_sb, hole_bb], &board, Player::SB);
        assert!(
            (utility_sb - (-1.0)).abs() < 1e-10,
            "SB should lose contested amount (-1), got {utility_sb}"
        );

        let utility_bb =
            CFRSolver::get_utility_impl(&state, &[hole_sb, hole_bb], &board, Player::BB);
        assert!(
            (utility_bb - 1.0).abs() < 1e-10,
            "BB should win contested amount (+1), got {utility_bb}"
        );
    }

    #[test]
    fn test_equal_stack_showdown_unchanged() {
        // Verify the contested-pot fix doesn't change equal-stack behavior.
        let config = GameConfig {
            initial_stacks: [100, 100],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        let state = state.apply_action(Action::AllIn);
        let state = state.apply_action(Action::Call);

        let hole_sb = [card(14, 0), card(14, 1)];
        let hole_bb = [card(13, 2), card(13, 3)];
        let board = [card(2, 0), card(4, 1), card(6, 2), card(8, 3), card(9, 0)];

        let utility =
            CFRSolver::get_utility_impl(&state, &[hole_sb, hole_bb], &board, Player::SB);
        assert!(
            (utility - 100.0).abs() < 1e-10,
            "SB should win 100 with equal stacks, got {utility}"
        );
    }

    // --- All-in action restriction tests ---

    #[test]
    fn test_no_raises_against_all_in_opponent() {
        // SB goes all-in preflop. BB should only see Fold, Call, AllIn (short call).
        // No bet or raise options.
        let config = GameConfig {
            initial_stacks: [10, 100],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        let state = state.apply_action(Action::AllIn); // SB all-in for 10

        let actions = state.legal_actions();
        assert!(actions.contains(&Action::Fold));
        assert!(actions.contains(&Action::Call));
        // No Check (facing a bet)
        assert!(!actions.contains(&Action::Check));
        // No Bet (opponent all-in)
        for action in actions.iter() {
            assert!(
                !matches!(action, Action::Bet(_)),
                "Should not offer Bet against all-in opponent"
            );
            assert!(
                !matches!(action, Action::Raise(_)),
                "Should not offer Raise against all-in opponent"
            );
        }
    }

    #[test]
    fn test_postflop_no_bets_against_all_in() {
        // Both see a flop, then one player is all-in. Other should only check.
        let config = GameConfig {
            initial_stacks: [100, 10],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        // SB calls, BB checks → flop
        let state = state.apply_action(Action::Call);
        let state = state.apply_action(Action::Check);
        assert_eq!(state.street, Street::Flop);

        // BB goes all-in on flop (BB has 8 remaining)
        let state = state.apply_action(Action::Check); // SB checks
        let state = state.apply_action(Action::AllIn); // BB all-in

        // SB faces all-in opponent on flop
        let actions = state.legal_actions();
        assert!(actions.contains(&Action::Fold));
        assert!(actions.contains(&Action::Call));
        // No bets or raises
        for action in actions.iter() {
            assert!(
                !matches!(action, Action::Bet(_)),
                "Should not offer Bet against all-in opponent postflop"
            );
            assert!(
                !matches!(action, Action::Raise(_)),
                "Should not offer Raise against all-in opponent postflop"
            );
        }
    }

    #[test]
    fn test_legal_actions_owned_intoiter() {
        let config = GameConfig {
            initial_stacks: [1000, 1000],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        let actions = state.legal_actions();
        let slice_len = actions.len();
        assert_eq!(
            actions.into_iter().count(),
            slice_len,
            "Owned IntoIterator should yield exactly len() actions"
        );
    }

    #[test]
    fn test_all_in_player_cannot_fold() {
        // SB goes all-in preflop, BB calls. On the flop, SB (all-in) should
        // only see Check — not Fold. An all-in player has already committed
        // everything and cannot forfeit.
        let config = GameConfig {
            initial_stacks: [10, 100],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        let state = state.apply_action(Action::AllIn); // SB all-in for 10
        let state = state.apply_action(Action::Call); // BB calls

        // Now on the flop. SB is all-in (committed=10=stack).
        // current_player should be SB on postflop.
        assert_eq!(state.street, Street::Flop);
        assert_eq!(state.current_player, Player::SB);

        let actions = state.legal_actions();
        assert!(
            !actions.contains(&Action::Fold),
            "All-in player should not be offered Fold"
        );
        assert!(
            actions.contains(&Action::Check),
            "All-in player should be offered Check"
        );
        // No bets/raises/all-in (remaining == 0)
        for action in actions.iter() {
            assert!(
                !matches!(action, Action::Bet(_) | Action::Raise(_) | Action::AllIn),
                "All-in player should not see Bet/Raise/AllIn, got {action:?}"
            );
        }
    }

    #[test]
    fn test_one_player_all_in_skips_to_showdown() {
        // SB goes all-in preflop, BB calls (but BB has more chips).
        // After the preflop round closes, the game should skip straight
        // to all-in showdown instead of playing check-check on each street.
        let config = GameConfig {
            initial_stacks: [10, 100],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        let state = state.apply_action(Action::AllIn); // SB all-in for 10
        let state = state.apply_action(Action::Call); // BB calls (matching 10)

        // SB committed 10 = stack, BB committed 10 < 100. Exactly one all-in.
        assert!(
            state.is_terminal(),
            "Should be terminal (one player all-in, round closed)"
        );
        assert!(
            state.is_all_in_showdown(),
            "Should be all-in showdown, not fold or river showdown"
        );
        assert!(!state.is_fold());
    }

    #[test]
    fn test_call_allin_dedup_when_call_matches_stack() {
        // When to_call exactly equals remaining, Call and AllIn are identical.
        // AllIn should be deduplicated so the CFR solver doesn't branch on
        // two equivalent actions.
        let config = GameConfig {
            initial_stacks: [10, 10],
            small_blind: 1,
            big_blind: 2,
            min_bet: 2,
        };
        let state = GameState::new(config);
        // SB goes all-in for 10. BB has 10 total, committed 2, remaining 8.
        // to_call = 10 - 2 = 8 = remaining. Call == AllIn.
        let state = state.apply_action(Action::AllIn); // SB all-in

        let actions = state.legal_actions();
        assert!(
            actions.contains(&Action::Call),
            "Should offer Call when to_call == remaining"
        );
        assert!(
            !actions.contains(&Action::AllIn),
            "Should NOT offer AllIn when Call already commits everything"
        );
    }
}
