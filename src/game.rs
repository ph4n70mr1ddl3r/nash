//! Game state machine, actions, player positions, and streets.

use std::fmt;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::card::{Card, CardSet};
use crate::config::GameConfig;
use crate::strategy::MAX_ACTIONS;

/// Number of players in heads-up poker.
pub const NUM_PLAYERS: usize = 2;

/// Pot fractions used to generate bet sizing options (1/3-pot, 2/3-pot, full pot).
const BET_FRACTIONS: &[u64] = &[1, 2, 3];
/// Denominator for [`BET_FRACTIONS`] (divides pot into thirds).
const BET_DENOM: u64 = 3;
/// Pot fractions used to generate raise sizing options (1/2-pot, full pot over the call).
const RAISE_FRACTIONS: &[u64] = &[1, 2];
/// Denominator for [`RAISE_FRACTIONS`] (divides pot into halves).
const RAISE_DENOM: u64 = 2;

/// Player position in heads-up poker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Player {
    /// Small blind position (acts first preflop).
    SB,
    /// Big blind position (acts second preflop).
    BB,
}

impl Player {
    /// Returns the array index for this player (0 for SB, 1 for BB).
    #[must_use]
    #[inline]
    pub const fn index(self) -> usize {
        match self {
            Self::SB => 0,
            Self::BB => 1,
        }
    }

    /// Returns the player corresponding to the given array index (0 → SB, 1 → BB).
    #[must_use]
    #[inline]
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::SB),
            1 => Some(Self::BB),
            _ => None,
        }
    }

    /// Returns the opponent of this player.
    #[must_use]
    #[inline]
    pub const fn opponent(self) -> Self {
        match self {
            Self::SB => Self::BB,
            Self::BB => Self::SB,
        }
    }
}

impl fmt::Display for Player {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SB => write!(f, "SB"),
            Self::BB => write!(f, "BB"),
        }
    }
}

/// Betting street in a poker hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Street {
    /// Preflop (no community cards).
    Preflop,
    /// Flop (3 community cards).
    Flop,
    /// Turn (4 community cards).
    Turn,
    /// River (5 community cards).
    River,
}

impl Street {
    /// Returns the number of community cards on this street.
    #[must_use]
    #[inline]
    pub const fn board_card_count(self) -> usize {
        match self {
            Self::Preflop => 0,
            Self::Flop => 3,
            Self::Turn => 4,
            Self::River => 5,
        }
    }

    /// Returns the next street, or `None` if already on the river.
    #[must_use]
    #[inline]
    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Preflop => Some(Self::Flop),
            Self::Flop => Some(Self::Turn),
            Self::Turn => Some(Self::River),
            Self::River => None,
        }
    }
}

impl fmt::Display for Street {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preflop => write!(f, "Preflop"),
            Self::Flop => write!(f, "Flop"),
            Self::Turn => write!(f, "Turn"),
            Self::River => write!(f, "River"),
        }
    }
}

/// A player action in a poker hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Action {
    /// Fold and forfeit the hand.
    Fold,
    /// Check (pass when no bet to call).
    Check,
    /// Call the current bet.
    Call,
    /// Make a bet of the specified amount.
    Bet(u64),
    /// Raise by the specified amount over the current bet.
    Raise(u64),
    /// Go all-in with remaining stack.
    AllIn,
}

impl fmt::Display for Action {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fold => write!(f, "Fold"),
            Self::Check => write!(f, "Check"),
            Self::Call => write!(f, "Call"),
            Self::Bet(amount) => write!(f, "Bet({amount})"),
            Self::Raise(amount) => write!(f, "Raise({amount})"),
            Self::AllIn => write!(f, "AllIn"),
        }
    }
}

/// Maximum number of actions tracked in an `ActionHistory`.
const MAX_HISTORY_LEN: usize = 24;

/// Fixed-size action history that avoids heap allocation.
///
/// Used as part of [`InfoSet`] which serves as a `DashMap` key in the
/// CFR strategy table. By keeping the history inline, cloning an info set
/// is a single `memcpy` instead of a heap allocation.
#[derive(Debug, Clone)]
pub struct ActionHistory {
    actions: [Action; MAX_HISTORY_LEN],
    len: u8,
}

impl ActionHistory {
    /// Creates an empty action history.
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            actions: [Action::Fold; MAX_HISTORY_LEN],
            len: 0,
        }
    }

    /// Appends an action.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if at capacity.
    #[inline]
    pub fn push(&mut self, action: Action) {
        debug_assert!(
            (self.len as usize) < MAX_HISTORY_LEN,
            "ActionHistory overflow"
        );
        if (self.len as usize) < MAX_HISTORY_LEN {
            self.actions[self.len as usize] = action;
            self.len += 1;
        }
    }

    /// Returns the number of recorded actions.
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns `true` if no actions have been recorded.
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the recorded actions as a slice.
    #[must_use]
    #[inline]
    pub fn as_slice(&self) -> &[Action] {
        &self.actions[..self.len as usize]
    }

    /// Returns an iterator over the recorded actions.
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Action> {
        self.as_slice().iter()
    }
}

impl Default for ActionHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl Hash for ActionHistory {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len.hash(state);
        for action in &self.actions[..self.len as usize] {
            action.hash(state);
        }
    }
}

impl PartialEq for ActionHistory {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.as_slice() == other.as_slice()
    }
}

impl Eq for ActionHistory {}

impl Serialize for ActionHistory {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_slice().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ActionHistory {
    #[allow(clippy::cast_possible_truncation)]
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let actions: Vec<Action> = Vec::deserialize(deserializer)?;
        let mut arr = [Action::Fold; MAX_HISTORY_LEN];
        let len = actions.len().min(MAX_HISTORY_LEN);
        arr[..len].copy_from_slice(&actions[..len]);
        Ok(Self {
            actions: arr,
            len: len as u8,
        })
    }
}

impl<'a> IntoIterator for &'a ActionHistory {
    type Item = &'a Action;
    type IntoIter = std::slice::Iter<'a, Action>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

/// Legal actions result with stack allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegalActions {
    actions: [Action; MAX_ACTIONS],
    len: u8,
}

impl LegalActions {
    /// Returns the number of legal actions.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns `true` if there are no legal actions.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the actions as a slice.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[Action] {
        &self.actions[..self.len as usize]
    }

    /// Returns `true` if the action is in the legal actions.
    #[inline]
    #[must_use]
    pub fn contains(&self, action: &Action) -> bool {
        self.as_slice().contains(action)
    }

    /// Returns an iterator over the legal actions.
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Action> {
        self.as_slice().iter()
    }
}

impl<'a> IntoIterator for &'a LegalActions {
    type Item = &'a Action;
    type IntoIter = std::slice::Iter<'a, Action>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

/// Stack-allocated owned iterator over `LegalActions`.
/// Avoids heap allocation compared to `Vec::into_iter`.
#[derive(Debug, Clone)]
pub struct LegalActionsIter {
    actions: [Action; MAX_ACTIONS],
    len: u8,
    pos: u8,
}

impl Iterator for LegalActionsIter {
    type Item = Action;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.len {
            let action = self.actions[self.pos as usize];
            self.pos += 1;
            Some(action)
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.len - self.pos) as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for LegalActionsIter {
    #[inline]
    fn len(&self) -> usize {
        (self.len - self.pos) as usize
    }
}

impl std::iter::FusedIterator for LegalActionsIter {}

impl IntoIterator for LegalActions {
    type Item = Action;
    type IntoIter = LegalActionsIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        LegalActionsIter {
            actions: self.actions,
            len: self.len,
            pos: 0,
        }
    }
}

/// Complete state of a poker hand.
#[derive(Debug, Clone)]
pub struct GameState {
    /// Current betting street.
    pub street: Street,
    /// Player whose turn it is to act.
    pub current_player: Player,
    /// Current pot size.
    pub pot: u64,
    /// Amount each player has committed to the pot.
    pub committed: [u64; NUM_PLAYERS],
    /// History of actions taken in this hand.
    pub history: ActionHistory,
    /// Minimum raise size for the current betting round.
    pub min_raise: u64,
    /// The highest bet amount in the current round.
    pub last_bet: u64,
    /// Game configuration.
    pub config: GameConfig,
    /// Index in `history` where the current betting round started.
    round_start: usize,
    /// Both players are all-in — skip remaining streets to showdown.
    all_in_showdown: bool,
}

impl GameState {
    /// Creates a new game state with blinds posted.
    #[must_use]
    #[inline]
    pub const fn new(config: GameConfig) -> Self {
        // Cap blind commitments at actual stack sizes (tournament scenario
        // where a player has fewer chips than the blind).
        let sb_committed = if config.small_blind < config.initial_stacks[0] {
            config.small_blind
        } else {
            config.initial_stacks[0]
        };
        let bb_committed = if config.big_blind < config.initial_stacks[1] {
            config.big_blind
        } else {
            config.initial_stacks[1]
        };
        let committed = [sb_committed, bb_committed];
        let pot = committed[0] + committed[1];
        let sb_all_in = committed[0] >= config.initial_stacks[0];
        // Trigger all-in showdown when SB has committed their entire stack
        // from the blind.  SB is current_player with 0 remaining — they
        // cannot act, so the hand must resolve at showdown immediately.
        // This also covers the both-all-in case since sb_all_in implies
        // at least SB can't act.  BB-only all-in is NOT terminal here —
        // SB still gets to choose (fold/call/raise against the all-in BB).
        let all_in_showdown = sb_all_in;
        Self {
            street: Street::Preflop,
            current_player: Player::SB,
            pot,
            committed,
            history: ActionHistory::new(),
            min_raise: config.min_bet,
            last_bet: committed[1], // BB's actual commitment (may be < big_blind)
            config,
            round_start: 0,
            all_in_showdown,
        }
    }

    /// Returns `true` if the hand has ended (fold, showdown, or both all-in).
    #[must_use]
    #[inline]
    pub fn is_terminal(&self) -> bool {
        self.is_fold() || self.is_showdown() || self.all_in_showdown
    }

    /// Returns the number of board cards visible on the current street.
    #[must_use]
    #[inline]
    pub fn visible_board_count(&self, board_len: usize) -> usize {
        self.street.board_card_count().min(board_len)
    }

    /// Returns `true` if the last action was a fold.
    #[must_use]
    #[inline]
    pub fn is_fold(&self) -> bool {
        matches!(self.history.as_slice().last(), Some(Action::Fold))
    }

    /// Returns `true` if the hand reached showdown on the river.
    #[must_use]
    #[inline]
    pub fn is_showdown(&self) -> bool {
        self.street == Street::River && self.betting_round_closed()
    }

    /// Returns `true` if both players are all-in with no further decisions.
    #[must_use]
    #[inline]
    pub const fn is_all_in_showdown(&self) -> bool {
        self.all_in_showdown
    }

    /// Returns `true` if the current betting round is complete.
    ///
    /// A round closes when both players have acted:
    /// - A `Call` closes the round only after at least 2 actions (handles
    ///   the preflop BB option: SB calling does *not* end the round).
    /// - A `Check` closes the round when preceded by another `Check` or a
    ///   `Call` (the preflop SB-call → BB-check sequence).
    #[must_use]
    #[inline]
    pub fn betting_round_closed(&self) -> bool {
        let history = self.history.as_slice();
        if self.round_start >= history.len() || history.len() - self.round_start < 2 {
            return false;
        }
        let round_actions = &history[self.round_start..];
        let last = round_actions[round_actions.len() - 1];
        match last {
            Action::Call => true,
            Action::Check => {
                matches!(
                    round_actions[round_actions.len() - 2],
                    Action::Check | Action::Call
                )
            }
            _ => false,
        }
    }

    /// Returns the winner if the hand ended by fold.
    ///
    /// After a fold, `current_player` has been swapped to the folder's opponent
    /// (i.e. the player who did *not* fold), so this returns the winner.
    #[must_use]
    #[inline]
    pub fn winner(&self) -> Option<Player> {
        if matches!(self.history.as_slice().last(), Some(Action::Fold)) {
            Some(self.current_player)
        } else {
            None
        }
    }

    /// Returns the legal actions for the current player (stack-allocated).
    #[inline]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn legal_actions(&self) -> LegalActions {
        let mut actions = [Action::Fold; MAX_ACTIONS];

        let remaining = self.config.initial_stacks[self.current_player.index()]
            .saturating_sub(self.committed[self.current_player.index()]);

        // An all-in player (remaining == 0) cannot fold — they've already
        // committed their entire stack. Only offer Check in that case.
        let mut len = usize::from(remaining != 0);

        let to_call = self
            .last_bet
            .saturating_sub(self.committed[self.current_player.index()]);

        // When the opponent is all-in, only offer fold/call/short-all-in.
        // Bets and raises are pointless (no one to respond) and create
        // invalid game tree branches in the CFR solver.
        let opponent_all_in = self.committed[self.current_player.opponent().index()]
            >= self.config.initial_stacks[self.current_player.opponent().index()];

        if to_call == 0 {
            actions[len] = Action::Check;
            len += 1;

            if !opponent_all_in {
                for &frac in BET_FRACTIONS {
                    // Use u128 intermediate to prevent overflow when pot * frac
                    // exceeds u64::MAX (possible with extreme stack sizes).
                    #[allow(clippy::cast_lossless)]
                    let bet_size = ((u128::from(self.pot) * u128::from(frac) / u128::from(BET_DENOM))
                        .min(u128::from(remaining))) as u64;
                    if bet_size >= self.config.min_bet && len < MAX_ACTIONS - 1 {
                        let action = Action::Bet(bet_size);
                        if !actions[..len].contains(&action) {
                            actions[len] = action;
                            len += 1;
                        }
                    }
                }
            }
        } else if to_call <= remaining {
            actions[len] = Action::Call;
            len += 1;

            if !opponent_all_in {
                for &frac in RAISE_FRACTIONS {
                    // Use u128 intermediate to prevent overflow when pot * frac
                    // exceeds u64::MAX (possible with extreme stack sizes).
                    #[allow(clippy::cast_lossless)]
                    let pot_frac = (u128::from(self.pot) * u128::from(frac) / u128::from(RAISE_DENOM)) as u64;
                    let raise_over_call = pot_frac
                        .max(self.min_raise)
                        .min(remaining - to_call);
                    if raise_over_call >= self.min_raise
                        && len < MAX_ACTIONS - 1
                    {
                        let action = Action::Raise(raise_over_call);
                        if !actions[..len].contains(&action) {
                            actions[len] = action;
                            len += 1;
                        }
                    }
                }
            }
        }

        if remaining > 0 {
            // Skip AllIn when it would be identical to an existing action:
            // - opponent is all-in and Call suffices (to_call < remaining)
            // - to_call == remaining (AllIn ≡ Call, avoid duplicate branch)
            let skip_all_in = opponent_all_in && to_call < remaining
                || to_call == remaining;
            if !skip_all_in {
                let all_in_dup = actions[..len].contains(&Action::Bet(remaining))
                    || (to_call < remaining
                        && actions[..len].contains(&Action::Raise(remaining - to_call)));
                if !all_in_dup {
                    actions[len] = Action::AllIn;
                    len += 1;
                }
            }
        }

        LegalActions {
            actions,
            len: len as u8,
        }
    }

    /// Applies an action and returns the new game state.
    ///
    /// # Panics
    ///
    /// In debug builds, panics if the action is not legal.
    #[must_use]
    #[inline]
    pub fn apply_action(&self, action: Action) -> Self {
        debug_assert!(
            self.legal_actions().contains(&action) || self.is_terminal(),
            "Attempted to apply illegal action: {action:?}"
        );

        let mut new_state = self.clone();
        match action {
            Action::Fold | Action::Check => {}
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
                let to_call = self
                    .last_bet
                    .saturating_sub(self.committed[self.current_player.index()]);
                new_state.committed[self.current_player.index()] += remaining;
                new_state.pot += remaining;
                new_state.last_bet = new_state.committed[new_state.current_player.index()];
                let raise_portion = remaining.saturating_sub(to_call);
                if raise_portion > new_state.min_raise {
                    new_state.min_raise = raise_portion;
                }
            }
        }

        new_state.history.push(action);

        if new_state.is_fold() {
            new_state.current_player = self.current_player.opponent();
        } else {
            let both_all_in = new_state.committed[0] >= new_state.config.initial_stacks[0]
                && new_state.committed[1] >= new_state.config.initial_stacks[1];

            if both_all_in {
                new_state.all_in_showdown = true;
            } else if new_state.betting_round_closed() && new_state.street != Street::River {
                if let Some(next_street) = new_state.street.next() {
                    new_state.street = next_street;
                    new_state.last_bet = 0;
                    new_state.min_raise = new_state.config.min_bet;
                    new_state.current_player = Player::SB;
                    new_state.round_start = new_state.history.len();
                }

                // When exactly one player is all-in, no further betting is
                // possible (opponent_all_in prevents bets/raises). Skip
                // remaining streets and go straight to showdown, avoiding
                // 6 pointless check-check actions across Flop/Turn/River.
                let sb_all_in = new_state.committed[0]
                    >= new_state.config.initial_stacks[0];
                let bb_all_in = new_state.committed[1]
                    >= new_state.config.initial_stacks[1];
                let one_all_in = sb_all_in != bb_all_in;
                if one_all_in {
                    new_state.all_in_showdown = true;
                }
            } else {
                new_state.current_player = self.current_player.opponent();
            }
        }
        new_state
    }
}

/// Information set for a player (what they can see).
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfoSet {
    /// The player this info set belongs to.
    pub player: Player,
    /// Current betting street.
    pub street: Street,
    /// Player's hole cards.
    pub hole: [Card; 2],
    /// Community cards visible so far.
    pub board: CardSet,
    /// Betting history.
    pub history: ActionHistory,
}

impl InfoSet {
    /// Creates a new info set from cards and an existing action history.
    ///
    /// This avoids the `O(history_len)` cost of pushing actions one-by-one,
    /// which matters in the hot CFR traversal path where every node visit
    /// previously reconstructed the history from scratch.
    #[must_use]
    #[inline]
    #[allow(clippy::missing_const_for_fn)]
    pub fn from_cards_with_history(
        player: Player,
        street: Street,
        hole: &[Card; 2],
        board: CardSet,
        history: ActionHistory,
    ) -> Self {
        Self {
            player,
            street,
            hole: *hole,
            board,
            history,
        }
    }

    /// Creates a new info set from cards.
    #[must_use]
    #[inline]
    pub const fn from_cards(
        player: Player,
        street: Street,
        hole: &[Card; 2],
        board: CardSet,
    ) -> Self {
        Self {
            player,
            street,
            hole: *hole,
            board,
            history: ActionHistory::new(),
        }
    }

    /// Adds an action to the history.
    #[inline]
    pub fn add_action(&mut self, action: &Action) {
        self.history.push(*action);
    }
}

impl fmt::Display for InfoSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}/{}",
            self.player, self.street, self.hole[0], self.hole[1]
        )?;
        for card in self.board.as_slice() {
            write!(f, "/{card}")?;
        }
        if !self.history.is_empty() {
            write!(f, ":")?;
            for action in &self.history {
                write!(f, "{action}")?;
            }
        }
        Ok(())
    }
}
