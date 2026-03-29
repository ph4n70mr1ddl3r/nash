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

    let mut solver = CFRSolver::new(game_config, cfr_config).unwrap_or_else(|e| {
        eprintln!("Failed to create solver: {e}");
        std::process::exit(1);
    });
    solver.solve();
}

#[cfg(test)]
mod tests {
    use nash::{
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
        };

        let mut solver = CFRSolver::new(game_config, cfr_config).unwrap();
        solver.solve();
        assert!(solver.strategy.len() > 0);
    }

    #[test]
    fn test_action_history_push_and_iter() {
        use nash::ActionHistory;
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
        use nash::ActionHistory;
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
}
