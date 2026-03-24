use nash::{CFRConfig, CFRSolver, GameConfig};

fn main() {
    tracing_subscriber::fmt::init();

    let game_config = GameConfig {
        initial_stacks: [1000, 1000],
        small_blind: 1,
        big_blind: 2,
        min_bet: 2,
    };

    let cfr_config = CFRConfig {
        num_iterations: 100,
        log_interval: 10,
        save_interval: 50,
        save_path: Some("strategy.bin".to_string()),
        use_chance_sampling: true,
    };

    let mut solver = CFRSolver::new(game_config, cfr_config);
    solver.solve();
}

#[cfg(test)]
mod tests {
    use nash::{
        Action, CFRConfig, Card, CardSet, Deck, GameConfig, GameState, Hand, Player, Strategy,
        StrategyEntry, Street,
    };

    fn card(rank: u8, suit: u8) -> Card {
        Card::new(rank, suit).expect("valid card")
    }

    #[test]
    fn test_hand_high_card() {
        let hole = [card(14, 0), card(12, 1)];
        let board = [card(10, 2), card(8, 3), card(5, 0), card(3, 1), card(2, 2)];
        let hand = Hand::evaluate(&hole, &board);
        let low_hand = Hand::evaluate(&[card(9, 0), card(7, 1)], &board);
        assert!(hand > low_hand);
    }

    #[test]
    fn test_hand_pair() {
        let hole = [card(14, 0), card(14, 1)];
        let board = [card(10, 2), card(8, 3), card(5, 0), card(3, 1), card(2, 2)];
        let hand = Hand::evaluate(&hole, &board);
        let high_card_hand = Hand::evaluate(&[card(13, 0), card(12, 1)], &board);
        assert!(hand > high_card_hand);
    }

    #[test]
    fn test_hand_two_pair() {
        let hole = [card(14, 0), card(14, 1)];
        let board = [card(10, 0), card(10, 1), card(5, 0), card(3, 1), card(2, 2)];
        let hand = Hand::evaluate(&hole, &board);
        let pair_hand = Hand::evaluate(&[card(14, 0), card(13, 1)], &board);
        assert!(hand > pair_hand);
    }

    #[test]
    fn test_hand_three_of_a_kind() {
        let hole = [card(14, 0), card(14, 1)];
        let board = [card(14, 2), card(8, 3), card(5, 0), card(3, 1), card(2, 2)];
        let hand = Hand::evaluate(&hole, &board);
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
        let three_kind_hand = Hand::evaluate(&[card(10, 0), card(10, 1)], &board);
        assert!(hand > three_kind_hand);
    }

    #[test]
    fn test_hand_wheel_straight() {
        let hole = [card(14, 0), card(2, 1)];
        let board = [card(5, 2), card(4, 3), card(3, 0), card(10, 1), card(9, 2)];
        let hand = Hand::evaluate(&hole, &board);
        let high_card_hand = Hand::evaluate(&[card(13, 0), card(12, 1)], &board);
        assert!(hand > high_card_hand);
    }

    #[test]
    fn test_hand_flush() {
        let hole = [card(14, 0), card(12, 0)];
        let board = [card(10, 0), card(8, 0), card(5, 1), card(3, 1), card(2, 2)];
        let hand = Hand::evaluate(&hole, &board);
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
        let flush_hand = Hand::evaluate(&[card(12, 0), card(11, 0)], &board);
        assert!(hand > flush_hand);
    }

    #[test]
    fn test_hand_four_of_a_kind() {
        let hole = [card(14, 0), card(14, 1)];
        let board = [card(14, 2), card(14, 3), card(5, 0), card(3, 1), card(2, 2)];
        let hand = Hand::evaluate(&hole, &board);
        let full_house_hand = Hand::evaluate(&[card(10, 0), card(10, 1)], &board);
        assert!(hand > full_house_hand);
    }

    #[test]
    fn test_hand_straight_flush() {
        let hole = [card(14, 0), card(13, 0)];
        let board = [
            card(12, 0),
            card(11, 0),
            card(10, 0),
            card(3, 1),
            card(2, 2),
        ];
        let hand = Hand::evaluate(&hole, &board);
        let four_kind_hand = Hand::evaluate(&[card(5, 0), card(5, 1)], &board);
        assert!(hand > four_kind_hand);
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
        let state = state.apply_action(Action::Call);
        assert_eq!(state.street, Street::Flop);
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
        let state = state.apply_action(Action::Raise(10));
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
        let state = state.apply_action(Action::Raise(10));
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
        assert_eq!(set.as_slice().len(), 3);
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
}
