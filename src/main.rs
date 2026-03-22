use dashmap::DashMap;
use rayon::prelude::*;
use std::fs::File;
use std::io::BufWriter;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};

const NUM_PLAYERS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Player {
    SB,
    BB,
}

impl Player {
    fn index(self) -> usize {
        match self {
            Player::SB => 0,
            Player::BB => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Street {
    Preflop,
    Flop,
    Turn,
    River,
}

impl Street {
    fn board_cards(self) -> usize {
        match self {
            Street::Preflop => 0,
            Street::Flop => 3,
            Street::Turn => 4,
            Street::River => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Card {
    rank: u8,
    suit: u8,
}

impl Card {
    fn all() -> Vec<Card> {
        let mut cards = Vec::with_capacity(52);
        for rank in 2..=14 {
            for suit in 0..4 {
                cards.push(Card { rank, suit });
            }
        }
        cards
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CardSet {
    cards: Vec<Card>,
}

impl CardSet {
    fn from_cards(cards: &[Card]) -> Self {
        CardSet {
            cards: cards.to_vec(),
        }
    }

    fn to_vec(&self) -> Vec<Card> {
        self.cards.clone()
    }
}

#[derive(Debug, Clone)]
pub struct Deck {
    cards: Vec<Card>,
    pos: usize,
}

impl Deck {
    fn new() -> Self {
        Deck {
            cards: Card::all(),
            pos: 0,
        }
    }

    fn shuffle(&mut self, rng: &mut impl rand::Rng) {
        use rand::seq::SliceRandom;
        self.cards.shuffle(rng);
        self.pos = 0;
    }

    fn deal_one(&mut self) -> Option<Card> {
        if self.pos < self.cards.len() {
            let card = self.cards[self.pos];
            self.pos += 1;
            Some(card)
        } else {
            None
        }
    }

    fn deal(&mut self, n: usize) -> Vec<Card> {
        let mut result = Vec::with_capacity(n);
        for _ in 0..n {
            if let Some(card) = self.deal_one() {
                result.push(card);
            }
        }
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Fold,
    Check,
    Call,
    Bet(u64),
    Raise(u64),
    AllIn,
}

#[derive(Debug, Clone)]
pub struct GameState {
    street: Street,
    current_player: Player,
    pot: u64,
    committed: [u64; NUM_PLAYERS],
    history: Vec<Action>,
    min_raise: u64,
    last_bet: u64,
    config: GameConfig,
}

impl GameState {
    fn new(config: GameConfig) -> Self {
        GameState {
            street: Street::Preflop,
            current_player: Player::SB,
            pot: config.small_blind + config.big_blind,
            committed: [config.small_blind, config.big_blind],
            history: Vec::new(),
            min_raise: config.min_bet,
            last_bet: config.big_blind,
            config,
        }
    }

    fn is_terminal(&self) -> bool {
        self.street == Street::River
            && self.history.len() >= 2
            && self.last_action_is_check_or_call()
    }

    fn is_fold(&self) -> bool {
        self.history
            .last()
            .is_some_and(|a| matches!(a, Action::Fold))
    }

    fn last_action_is_check_or_call(&self) -> bool {
        self.history
            .last()
            .is_some_and(|a| matches!(a, Action::Check | Action::Call))
    }

    fn winner(&self) -> Option<Player> {
        if let Some(Action::Fold) = self.history.last() {
            Some(self.current_player)
        } else {
            None
        }
    }

    fn legal_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        actions.push(Action::Fold);

        let to_call = self
            .last_bet
            .saturating_sub(self.committed[self.current_player.index()]);
        if to_call == 0 {
            actions.push(Action::Check);
        } else {
            actions.push(Action::Call);
        }

        let bet_size = (self.pot as f64 * 0.5) as u64;
        if bet_size > 0 {
            actions.push(Action::Bet(bet_size));
        }

        let raise_size = self.last_bet + self.min_raise;
        actions.push(Action::Raise(raise_size));

        actions.push(Action::AllIn);
        actions
    }

    fn apply_action(&self, action: Action) -> Self {
        let mut new_state = self.clone();
        match action {
            Action::Fold => {}
            Action::Check => {}
            Action::Call => {
                let to_call = self
                    .last_bet
                    .saturating_sub(self.committed[self.current_player.index()]);
                new_state.committed[self.current_player.index()] += to_call;
                new_state.pot += to_call;
            }
            Action::Bet(amount) => {
                new_state.committed[self.current_player.index()] += amount;
                new_state.pot += amount;
                new_state.last_bet = new_state.committed[self.current_player.index()];
            }
            Action::Raise(amount) => {
                let to_call = self
                    .last_bet
                    .saturating_sub(self.committed[self.current_player.index()]);
                let total = to_call + amount;
                new_state.committed[self.current_player.index()] += total;
                new_state.pot += total;
                new_state.last_bet = new_state.committed[self.current_player.index()];
            }
            Action::AllIn => {
                let all_in_amount = self.config.initial_stacks[self.current_player.index()];
                new_state.committed[self.current_player.index()] += all_in_amount;
                new_state.pot += all_in_amount;
                new_state.last_bet = new_state.committed[self.current_player.index()];
            }
        }

        new_state.history.push(action);
        new_state.current_player = match self.current_player {
            Player::SB => Player::BB,
            Player::BB => Player::SB,
        };
        new_state
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct InfoSet {
    player: Player,
    street: Street,
    hole: [Card; 2],
    board: CardSet,
    history: Vec<Action>,
}

impl InfoSet {
    fn from_cards(player: Player, street: Street, hole: &[Card; 2], board: CardSet) -> Self {
        InfoSet {
            player,
            street,
            hole: *hole,
            board,
            history: Vec::new(),
        }
    }

    fn add_action(&mut self, action: &Action) {
        self.history.push(*action);
    }
}

#[derive(Debug, Clone)]
pub struct Hand {
    rank: u32,
}

impl Hand {
    fn evaluate(_hole: &[Card; 2], _board: &[Card]) -> Self {
        Hand {
            rank: rand::random::<u32>() % 7462,
        }
    }
}

impl Ord for Hand {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank.cmp(&other.rank)
    }
}

impl PartialOrd for Hand {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Hand {}

impl PartialEq for Hand {
    fn eq(&self, other: &Self) -> bool {
        self.rank == other.rank
    }
}

#[derive(Debug, Clone)]
pub struct GameConfig {
    pub initial_stacks: [u64; NUM_PLAYERS],
    pub small_blind: u64,
    pub big_blind: u64,
    pub min_bet: u64,
    pub bet_abstraction: Vec<f64>,
    pub raise_abstraction: Vec<f64>,
}

pub struct CFRConfig {
    pub num_iterations: usize,
    pub log_interval: usize,
    pub save_interval: usize,
    pub save_path: Option<String>,
    pub use_chance_sampling: bool,
    pub prune_negative: bool,
}

pub struct StrategyStats {
    info_sets: usize,
    memory_mb: f64,
}

#[derive(Clone)]
pub struct StrategyEntry {
    regrets: Vec<f64>,
    strategy_sum: Vec<f64>,
}

impl StrategyEntry {
    fn new(num_actions: usize) -> Self {
        StrategyEntry {
            regrets: vec![0.0; num_actions],
            strategy_sum: vec![0.0; num_actions],
        }
    }

    fn get_strategy(&self) -> Vec<f64> {
        let mut strat = Vec::with_capacity(self.regrets.len());
        let mut sum = 0.0;
        for &r in &self.regrets {
            let s = r.max(0.0);
            strat.push(s);
            sum += s;
        }
        if sum > 0.0 {
            for s in &mut strat {
                *s /= sum;
            }
        } else {
            let uniform = 1.0 / strat.len() as f64;
            strat.fill(uniform);
        }
        strat
    }
}

pub struct Strategy {
    entries: DashMap<InfoSet, StrategyEntry>,
    config: GameConfig,
}

impl Strategy {
    fn new(config: GameConfig) -> Self {
        Strategy {
            entries: DashMap::new(),
            config,
        }
    }

    fn get_or_create(&self, info_set: &InfoSet, num_actions: usize) -> StrategyEntry {
        self.entries
            .get(info_set)
            .map(|e| e.clone())
            .unwrap_or_else(|| {
                let entry = StrategyEntry::new(num_actions);
                self.entries.insert(info_set.clone(), entry.clone());
                entry
            })
    }

    fn stats(&self) -> StrategyStats {
        let info_sets = self.entries.len();
        let memory_mb = (info_sets * std::mem::size_of::<StrategyEntry>()) as f64 / 1_000_000.0;
        StrategyStats {
            info_sets,
            memory_mb,
        }
    }

    fn save(&self, path: &str) -> std::io::Result<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        bincode::serialize_into(writer, &self.entries.len())
            .map_err(|e| std::io::Error::other(e.to_string()))
    }

    fn update(&self, info_set: &InfoSet, regrets: &[f64], iter_weight: f64) {
        if let Some(mut entry) = self.entries.get_mut(info_set) {
            for (i, &r) in regrets.iter().enumerate() {
                if i < entry.regrets.len() {
                    entry.regrets[i] += r * iter_weight;
                }
            }
        }
    }
}

pub struct CFRSolver {
    pub strategy: Arc<Strategy>,
    pub config: GameConfig,
    pub cfr_config: CFRConfig,
    iteration: usize,
}

impl CFRSolver {
    pub fn new(game_config: GameConfig, cfr_config: CFRConfig) -> Self {
        let strategy = Arc::new(Strategy::new(game_config.clone()));
        CFRSolver {
            config: game_config,
            cfr_config,
            strategy,
            iteration: 0,
        }
    }

    pub fn solve(&mut self) {
        let start = Instant::now();

        info!(
            "Starting CFR+ solver with {} iterations",
            self.cfr_config.num_iterations
        );

        for iter in 1..=self.cfr_config.num_iterations {
            self.iteration = iter;

            let iter_weight = (iter as f64).sqrt() + 1.0;

            self.run_iteration(iter_weight);

            if iter % self.cfr_config.log_interval == 0 {
                let elapsed = start.elapsed();
                let stats = self.strategy.stats();
                let exploitability = self.estimate_exploitability();

                info!(
                    "Iteration {}: {} info sets, {:.2} MB, exploitability: {:.6}, elapsed: {:?}",
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

        let hole_sb = [deck.deal_one().unwrap(), deck.deal_one().unwrap()];
        let hole_bb = [deck.deal_one().unwrap(), deck.deal_one().unwrap()];
        let board: Vec<Card> = deck.deal(5).into_iter().collect();
        let hands = [hole_sb, hole_bb];

        let state = GameState::new(self.config.clone());

        self.cfr_traversal(&state, &hands, &board, Player::SB, 1.0, 1.0, iter_weight);
    }

    fn run_iteration_full(&self, iter_weight: f64) {
        let all_cards: Vec<Card> = Card::all();
        let num_cards = all_cards.len();
        let strategy = self.strategy.clone();
        let config = self.config.clone();

        (0..num_cards).into_par_iter().for_each(|i| {
            for j in (i + 1)..num_cards {
                for k in (j + 1)..num_cards {
                    for l in (k + 1)..num_cards {
                        let hole_sb = [all_cards[i], all_cards[j]];
                        let hole_bb = [all_cards[k], all_cards[l]];
                        let mut remaining: Vec<Card> = all_cards
                            .iter()
                            .copied()
                            .filter(|&c| c.rank != all_cards[i].rank || c.suit != all_cards[i].suit)
                            .filter(|&c| c.rank != all_cards[j].rank || c.suit != all_cards[j].suit)
                            .filter(|&c| c.rank != all_cards[k].rank || c.suit != all_cards[k].suit)
                            .filter(|&c| c.rank != all_cards[l].rank || c.suit != all_cards[l].suit)
                            .collect();

                        let board: Vec<Card> = remaining.drain(..5).collect();

                        let hands = [hole_sb, hole_bb];
                        let state = GameState::new(config.clone());

                        Self::cfr_traversal_static(
                            &strategy,
                            &state,
                            &hands,
                            &board,
                            Player::SB,
                            1.0,
                            1.0,
                            iter_weight,
                            &config,
                        );
                    }
                }
            }
        });
    }

    fn cfr_traversal(
        &self,
        state: &GameState,
        hands: &[[Card; 2]],
        board: &[Card],
        player: Player,
        pi_o: f64,
        pi_neg_o: f64,
        iter_weight: f64,
    ) -> f64 {
        if state.is_terminal() {
            return self.get_utility(state, hands, board, player);
        }

        if state.is_fold() {
            return self.get_utility(state, hands, board, player);
        }

        let current = state.current_player;
        let actions = state.legal_actions();

        if actions.is_empty() {
            return self.get_utility(state, hands, board, player);
        }

        let board_set = CardSet::from_cards(&board[..state.street.board_cards().min(board.len())]);
        let hole = &hands[current.index()];

        let mut info_set = InfoSet::from_cards(current, state.street, hole, board_set);
        for action in &state.history {
            info_set.add_action(action);
        }

        let entry = self.strategy.get_or_create(&info_set, actions.len());
        let strat = entry.get_strategy();

        let mut action_values = vec![0.0; actions.len()];
        let mut node_value = 0.0;
        for (i, &action) in actions.iter().enumerate() {
            let new_state = state.apply_action(action);

            let value = if current == player {
                self.cfr_traversal(
                    &new_state,
                    hands,
                    board,
                    player,
                    pi_o * strat[i],
                    pi_neg_o * strat[i],
                    iter_weight,
                )
            } else {
                self.cfr_traversal(
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
            let mut regrets = vec![0.0; actions.len()];
            for (i, &av) in action_values.iter().enumerate() {
                regrets[i] = pi_neg_o * (av - node_value);
            }

            self.strategy.update(&info_set, &regrets, iter_weight);
        }

        node_value
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
        config: &GameConfig,
    ) -> f64 {
        if state.is_terminal() {
            return get_utility_static(state, hands, board, player, config);
        }

        if state.is_fold() {
            return get_utility_static(state, hands, board, player, config);
        }

        let current = state.current_player;
        let actions = state.legal_actions();

        if actions.is_empty() {
            return get_utility_static(state, hands, board, player, config);
        }

        let board_set = CardSet::from_cards(&board[..state.street.board_cards().min(board.len())]);
        let hole = &hands[current.index()];

        let mut info_set = InfoSet::from_cards(current, state.street, hole, board_set);
        for action in &state.history {
            info_set.add_action(action);
        }

        let entry = strategy.get_or_create(&info_set, actions.len());
        let strat = entry.get_strategy();

        let mut action_values = vec![0.0; actions.len()];
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
                    pi_neg_o * strat[i],
                    iter_weight,
                    config,
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
                    config,
                )
            };

            action_values[i] = value;
            node_value += strat[i] * value;
        }

        if current == player {
            let mut regrets = vec![0.0; actions.len()];
            for (i, &av) in action_values.iter().enumerate() {
                regrets[i] = pi_neg_o * (av - node_value);
            }

            strategy.update(&info_set, &regrets, iter_weight);
        }

        node_value
    }

    fn get_utility(
        &self,
        state: &GameState,
        hands: &[[Card; 2]],
        board: &[Card],
        player: Player,
    ) -> f64 {
        if state.is_fold() {
            let winner = state.winner().unwrap();
            return if winner == player {
                state.pot as f64
            } else {
                -(state.pot as f64)
            };
        }

        let board_set = CardSet::from_cards(&board[..state.street.board_cards().min(board.len())]);
        let hole = &hands[player.index()];
        let opp_index = 1 - player.index();
        let opp_hole = &hands[opp_index];

        let hand = Hand::evaluate(hole, &board_set.to_vec());
        let opp_hand = Hand::evaluate(opp_hole, &board_set.to_vec());

        match hand.cmp(&opp_hand) {
            std::cmp::Ordering::Greater => state.pot as f64,
            std::cmp::Ordering::Less => -(state.pot as f64),
            std::cmp::Ordering::Equal => 0.0,
        }
    }

    fn estimate_exploitability(&self) -> f64 {
        1.0 / (self.iteration as f64 + 1.0)
    }
}

fn get_utility_static(
    state: &GameState,
    hands: &[[Card; 2]],
    board: &[Card],
    player: Player,
    _config: &GameConfig,
) -> f64 {
    if state.is_fold() {
        let winner = state.winner().unwrap();
        return if winner == player {
            state.pot as f64
        } else {
            -(state.pot as f64)
        };
    }

    let board_set = CardSet::from_cards(&board[..state.street.board_cards().min(board.len())]);
    let hole = &hands[player.index()];
    let opp_index = 1 - player.index();
    let opp_hole = &hands[opp_index];

    let hand = Hand::evaluate(hole, &board_set.to_vec());
    let opp_hand = Hand::evaluate(opp_hole, &board_set.to_vec());

    match hand.cmp(&opp_hand) {
        std::cmp::Ordering::Greater => state.pot as f64,
        std::cmp::Ordering::Less => -(state.pot as f64),
        std::cmp::Ordering::Equal => 0.0,
    }
}

fn main() {
    tracing_subscriber::fmt::init();

    let game_config = GameConfig {
        initial_stacks: [1000, 1000],
        small_blind: 1,
        big_blind: 2,
        min_bet: 2,
        bet_abstraction: vec![0.5, 1.0],
        raise_abstraction: vec![2.0, 3.0],
    };

    let cfr_config = CFRConfig {
        num_iterations: 100,
        log_interval: 10,
        save_interval: 50,
        save_path: Some("strategy.bin".to_string()),
        use_chance_sampling: true,
        prune_negative: true,
    };

    let mut solver = CFRSolver::new(game_config, cfr_config);
    solver.solve();
}
