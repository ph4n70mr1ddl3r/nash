use dashmap::DashMap;
use lazy_static::lazy_static;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};

const NUM_PLAYERS: usize = 2;
const MAX_ACTIONS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Street {
    Preflop,
    Flop,
    Turn,
    River,
}

impl Street {
    fn board_card_count(self) -> usize {
        match self {
            Street::Preflop => 0,
            Street::Flop => 3,
            Street::Turn => 4,
            Street::River => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    rank: u8,
    suit: u8,
}

impl Card {
    fn all() -> &'static Vec<Card> {
        &ALL_CARDS
    }
}

lazy_static! {
    static ref ALL_CARDS: Vec<Card> = {
        let mut cards = Vec::with_capacity(52);
        for rank in 2..=14 {
            for suit in 0..4 {
                cards.push(Card { rank, suit });
            }
        }
        cards
    };
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardSet {
    cards: [Card; 5],
    len: u8,
}

impl CardSet {
    fn from_cards(cards: &[Card]) -> Self {
        let mut arr = [Card { rank: 0, suit: 0 }; 5];
        let len = cards.len().min(5);
        arr[..len].copy_from_slice(&cards[..len]);
        CardSet {
            cards: arr,
            len: len as u8,
        }
    }

    fn to_vec(&self) -> Vec<Card> {
        self.cards[..self.len as usize].to_vec()
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
            cards: Card::all().clone(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
        self.is_fold() || self.is_showdown()
    }

    fn is_fold(&self) -> bool {
        self.history
            .last()
            .is_some_and(|a| matches!(a, Action::Fold))
    }

    fn is_showdown(&self) -> bool {
        self.street == Street::River && self.betting_round_closed()
    }

    fn betting_round_closed(&self) -> bool {
        if self.history.len() < 2 {
            return false;
        }
        let last = &self.history[self.history.len() - 1];
        match last {
            Action::Call => true,
            Action::Check => {
                let prev = &self.history[self.history.len() - 2];
                matches!(prev, Action::Check)
            }
            _ => false,
        }
    }

    fn winner(&self) -> Option<Player> {
        if let Some(Action::Fold) = self.history.last() {
            Some(self.opponent())
        } else {
            None
        }
    }

    fn opponent(&self) -> Player {
        match self.current_player {
            Player::SB => Player::BB,
            Player::BB => Player::SB,
        }
    }

    fn legal_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        actions.push(Action::Fold);

        let remaining = self.config.initial_stacks[self.current_player.index()]
            .saturating_sub(self.committed[self.current_player.index()]);

        let to_call = self
            .last_bet
            .saturating_sub(self.committed[self.current_player.index()]);
        if to_call == 0 {
            actions.push(Action::Check);
        } else if to_call <= remaining {
            actions.push(Action::Call);
        }

        let bet_size = ((self.pot as f64 * 0.5) as u64).min(remaining);
        if bet_size > 0 && bet_size < remaining {
            actions.push(Action::Bet(bet_size));
        }

        let raise_size = self.min_raise.min(remaining.saturating_sub(to_call));
        if raise_size > 0 && to_call + raise_size < remaining {
            actions.push(Action::Raise(raise_size));
        }

        if remaining > 0 && !actions.contains(&Action::AllIn) {
            actions.push(Action::AllIn);
        }
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
                new_state.committed[new_state.current_player.index()] += amount;
                new_state.pot += amount;
                new_state.last_bet = new_state.committed[new_state.current_player.index()];
                new_state.min_raise = amount;
            }
            Action::Raise(amount) => {
                let to_call = self
                    .last_bet
                    .saturating_sub(self.committed[self.current_player.index()]);
                let total = to_call + amount;
                new_state.committed[new_state.current_player.index()] += total;
                new_state.pot += total;
                new_state.last_bet = new_state.committed[new_state.current_player.index()];
                new_state.min_raise = amount;
            }
            Action::AllIn => {
                let remaining = self.config.initial_stacks[self.current_player.index()]
                    .saturating_sub(self.committed[self.current_player.index()]);
                new_state.committed[self.current_player.index()] += remaining;
                new_state.pot += remaining;
                new_state.last_bet = new_state.committed[self.current_player.index()];
            }
        }

        new_state.history.push(action);

        if !new_state.is_fold()
            && new_state.betting_round_closed()
            && new_state.street != Street::River
        {
            new_state.street = match new_state.street {
                Street::Preflop => Street::Flop,
                Street::Flop => Street::Turn,
                Street::Turn => Street::River,
                Street::River => Street::River,
            };
            new_state.last_bet = 0;
            new_state.current_player = Player::SB;
        } else {
            new_state.current_player = match self.current_player {
                Player::SB => Player::BB,
                Player::BB => Player::SB,
            };
        }
        new_state
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hand {
    rank: u32,
}

impl Hand {
    fn evaluate(hole: &[Card; 2], board: &[Card]) -> Self {
        let mut all_cards: Vec<Card> = hole.iter().copied().chain(board.iter().copied()).collect();
        all_cards.sort_by(|a, b| b.rank.cmp(&a.rank));

        let rank = Self::evaluate_hand_rank(&all_cards);
        Hand { rank }
    }

    fn evaluate_hand_rank(cards: &[Card]) -> u32 {
        if cards.len() < 5 {
            let kickers: Vec<u8> = cards.iter().map(|c| c.rank).collect();
            return Self::hand_rank(0, &kickers);
        }

        let flush = Self::find_flush(cards);
        let straight = Self::find_straight(cards);

        if let (Some(flush_cards), Some(straight_high)) = (&flush, straight) {
            if Self::is_straight_flush(flush_cards, straight_high) {
                if straight_high == 14 {
                    return Self::hand_rank(9, &[14]);
                }
                return Self::hand_rank(8, &[straight_high]);
            }
        }

        let ranks: Vec<u8> = cards.iter().map(|c| c.rank).collect();
        let counts = Self::count_ranks(&ranks);

        if let Some(rank) = Self::find_four_of_a_kind(&counts) {
            let kicker = Self::best_kicker(&counts, &[rank]);
            return Self::hand_rank(7, &[rank, kicker]);
        }

        if let Some((trips, pair)) = Self::find_full_house(&counts) {
            return Self::hand_rank(6, &[trips, pair]);
        }

        if let Some(flush_cards) = flush {
            let kickers: Vec<u8> = flush_cards.iter().take(5).map(|c| c.rank).collect();
            return Self::hand_rank(5, &kickers);
        }

        if let Some(high) = straight {
            return Self::hand_rank(4, &[high]);
        }

        if let Some(rank) = Self::find_three_of_a_kind(&counts) {
            let kickers = Self::best_kickers(&counts, &[rank], 2);
            return Self::hand_rank(3, &[rank, kickers[0], kickers[1]]);
        }

        if let Some((high, low)) = Self::find_two_pair(&counts) {
            let kicker = Self::best_kicker(&counts, &[high, low]);
            return Self::hand_rank(2, &[high, low, kicker]);
        }

        if let Some(rank) = Self::find_pair(&counts) {
            let kickers = Self::best_kickers(&counts, &[rank], 3);
            return Self::hand_rank(1, &[rank, kickers[0], kickers[1], kickers[2]]);
        }

        let kickers: Vec<u8> = cards.iter().take(5).map(|c| c.rank).collect();
        Self::hand_rank(0, &kickers)
    }

    fn hand_rank(hand_type: u32, values: &[u8]) -> u32 {
        let mut rank = hand_type * 100_000_000;
        for (i, &v) in values.iter().enumerate() {
            rank += (v as u32) << (24 - i * 5);
        }
        rank
    }

    fn best_kicker(counts: &[u8; 15], excluded: &[u8]) -> u8 {
        for (rank, &count) in counts.iter().enumerate().rev() {
            if count > 0 && !excluded.contains(&(rank as u8)) {
                return rank as u8;
            }
        }
        0
    }

    fn best_kickers(counts: &[u8; 15], excluded: &[u8], n: usize) -> Vec<u8> {
        let mut kickers = Vec::with_capacity(n);
        for (rank, &count) in counts.iter().enumerate().rev() {
            if count > 0 && !excluded.contains(&(rank as u8)) {
                kickers.push(rank as u8);
                if kickers.len() >= n {
                    break;
                }
            }
        }
        kickers
    }

    fn find_flush(cards: &[Card]) -> Option<Vec<Card>> {
        let mut suit_counts = [0usize; 4];
        for card in cards {
            suit_counts[card.suit as usize] += 1;
        }
        for (suit, &count) in suit_counts.iter().enumerate() {
            if count >= 5 {
                let flush_cards: Vec<Card> = cards
                    .iter()
                    .filter(|c| c.suit as usize == suit)
                    .copied()
                    .collect();
                return Some(flush_cards);
            }
        }
        None
    }

    fn find_straight(cards: &[Card]) -> Option<u8> {
        let mut rank_mask: u32 = 0;
        for card in cards {
            rank_mask |= 1 << card.rank;
        }

        for high in (5..=14).rev() {
            let straight_mask = ((1u32 << 5) - 1) << (high - 4);
            if rank_mask & straight_mask == straight_mask {
                return Some(high);
            }
        }

        if rank_mask & 0x1003F == 0x1003F {
            return Some(5);
        }
        None
    }

    fn is_straight_flush(flush_cards: &[Card], straight_high: u8) -> bool {
        Self::find_straight(flush_cards) == Some(straight_high)
    }

    fn count_ranks(ranks: &[u8]) -> [u8; 15] {
        let mut counts = [0u8; 15];
        for &rank in ranks {
            counts[rank as usize] += 1;
        }
        counts
    }

    fn find_four_of_a_kind(counts: &[u8; 15]) -> Option<u8> {
        for (rank, &count) in counts.iter().enumerate() {
            if count == 4 {
                return Some(rank as u8);
            }
        }
        None
    }

    fn find_full_house(counts: &[u8; 15]) -> Option<(u8, u8)> {
        let mut trips = None;
        let mut pair = None;
        for (rank, &count) in counts.iter().enumerate().rev() {
            if count >= 3 && trips.is_none() {
                trips = Some(rank as u8);
            } else if count >= 2 && pair.is_none() {
                pair = Some(rank as u8);
            }
        }
        trips.zip(pair)
    }

    fn find_three_of_a_kind(counts: &[u8; 15]) -> Option<u8> {
        for (rank, &count) in counts.iter().enumerate().rev() {
            if count == 3 {
                return Some(rank as u8);
            }
        }
        None
    }

    fn find_two_pair(counts: &[u8; 15]) -> Option<(u8, u8)> {
        let mut pairs: Vec<u8> = Vec::new();
        for (rank, &count) in counts.iter().enumerate().rev() {
            if count == 2 {
                pairs.push(rank as u8);
            }
        }
        if pairs.len() >= 2 {
            Some((pairs[0], pairs[1]))
        } else {
            None
        }
    }

    fn find_pair(counts: &[u8; 15]) -> Option<u8> {
        for (rank, &count) in counts.iter().enumerate().rev() {
            if count == 2 {
                return Some(rank as u8);
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GameConfig {
    pub initial_stacks: [u64; NUM_PLAYERS],
    pub small_blind: u64,
    pub big_blind: u64,
    pub min_bet: u64,
}

#[derive(Debug, Clone)]
pub struct CFRConfig {
    pub num_iterations: usize,
    pub log_interval: usize,
    pub save_interval: usize,
    pub save_path: Option<String>,
    pub use_chance_sampling: bool,
}

#[derive(Debug, Clone)]
pub struct StrategyStats {
    info_sets: usize,
    memory_mb: f64,
}

#[derive(Clone, Serialize, Deserialize)]
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

    fn get_strategy(&self, out: &mut [f64]) {
        let len = out.len().min(self.regrets.len());
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
}

pub struct Strategy {
    entries: DashMap<InfoSet, StrategyEntry>,
}

impl Strategy {
    fn new() -> Self {
        Strategy {
            entries: DashMap::new(),
        }
    }

    fn get_or_create(&self, info_set: &InfoSet, num_actions: usize) -> StrategyEntry {
        use dashmap::mapref::entry::Entry;
        match self.entries.entry(info_set.clone()) {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let entry = StrategyEntry::new(num_actions);
                e.insert(entry.clone());
                entry
            }
        }
    }

    fn stats(&self) -> StrategyStats {
        let info_sets = self.entries.len();
        let mut total_memory = 0usize;
        for entry in self.entries.iter() {
            total_memory += std::mem::size_of::<InfoSet>();
            total_memory += entry.key().hole.len() * std::mem::size_of::<Card>();
            total_memory += entry.key().board.cards.len() * std::mem::size_of::<Card>();
            total_memory += entry.key().history.capacity() * std::mem::size_of::<Action>();
            total_memory += std::mem::size_of::<StrategyEntry>();
            total_memory += entry.value().regrets.capacity() * std::mem::size_of::<f64>();
            total_memory += entry.value().strategy_sum.capacity() * std::mem::size_of::<f64>();
        }
        let memory_mb = total_memory as f64 / 1_000_000.0;
        StrategyStats {
            info_sets,
            memory_mb,
        }
    }

    fn save(&self, path: &str) -> std::io::Result<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        let entries: Vec<_> = self
            .entries
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect();
        bincode::serialize_into(writer, &entries).map_err(|e| std::io::Error::other(e.to_string()))
    }

    #[allow(dead_code)]
    fn load(path: &str) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let entries: Vec<(InfoSet, StrategyEntry)> =
            bincode::deserialize_from(reader).map_err(|e| std::io::Error::other(e.to_string()))?;
        let strategy = Strategy::new();
        for (key, value) in entries {
            strategy.entries.insert(key, value);
        }
        Ok(strategy)
    }

    fn update(
        &self,
        info_set: &InfoSet,
        regrets: &[f64],
        strategy: &[f64],
        pi_o: f64,
        iter_weight: f64,
    ) {
        if let Some(mut entry) = self.entries.get_mut(info_set) {
            for (i, &r) in regrets.iter().enumerate() {
                if i < entry.regrets.len() {
                    entry.regrets[i] += r * iter_weight;
                }
            }
            for (i, &s) in strategy.iter().enumerate() {
                if i < entry.strategy_sum.len() {
                    entry.strategy_sum[i] += pi_o * s * iter_weight;
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
        let strategy = Arc::new(Strategy::new());
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

            let iter_weight = iter as f64;

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
        let all_cards = Card::all();
        let num_cards = all_cards.len();
        let strategy = self.strategy.clone();
        let config = self.config;

        (0..num_cards).into_par_iter().for_each(|i| {
            for j in (i + 1)..num_cards {
                for k in (j + 1)..num_cards {
                    for l in (k + 1)..num_cards {
                        let hole_sb = [all_cards[i], all_cards[j]];
                        let hole_bb = [all_cards[k], all_cards[l]];
                        let excluded = [
                            (hole_sb[0].rank as u16) << 8 | hole_sb[0].suit as u16,
                            (hole_sb[1].rank as u16) << 8 | hole_sb[1].suit as u16,
                            (hole_bb[0].rank as u16) << 8 | hole_bb[0].suit as u16,
                            (hole_bb[1].rank as u16) << 8 | hole_bb[1].suit as u16,
                        ];
                        let board: Vec<Card> = all_cards
                            .iter()
                            .copied()
                            .filter(|c| {
                                let key = (c.rank as u16) << 8 | c.suit as u16;
                                !excluded.contains(&key)
                            })
                            .take(5)
                            .collect();

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

        let board_set =
            CardSet::from_cards(&board[..state.street.board_card_count().min(board.len())]);
        let hole = &hands[current.index()];

        let mut info_set = InfoSet::from_cards(current, state.street, hole, board_set);
        for action in &state.history {
            info_set.add_action(action);
        }

        let entry = strategy.get_or_create(&info_set, actions.len());
        let mut strat = [0.0f64; MAX_ACTIONS];
        entry.get_strategy(&mut strat[..actions.len()]);

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

            strategy.update(
                &info_set,
                &regrets[..actions.len()],
                &strat[..actions.len()],
                pi_o,
                iter_weight,
            );
        }

        node_value
    }

    /// Placeholder: returns inverse iteration count instead of true exploitability.
    fn estimate_exploitability(&self) -> f64 {
        1.0 / (self.iteration as f64 + 1.0)
    }

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

        let hand = Hand::evaluate(hole, &board_set.to_vec());
        let opp_hand = Hand::evaluate(opp_hole, &board_set.to_vec());

        match hand.cmp(&opp_hand) {
            std::cmp::Ordering::Greater => state.pot as f64 - player_committed,
            std::cmp::Ordering::Less => -player_committed,
            std::cmp::Ordering::Equal => (state.pot as f64 / 2.0) - player_committed,
        }
    }
}

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
