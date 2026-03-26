//! Game state machine, actions, player positions, and streets.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::card::{Card, CardSet};
use crate::config::GameConfig;

/// Number of players in heads-up poker.
pub const NUM_PLAYERS: usize = 2;

const POT_BET_FRACTION_NUM: u64 = 1;
const POT_BET_FRACTION_DENOM: u64 = 2;

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

/// Legal actions result with stack allocation (max 6 actions in heads-up NLHE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegalActions {
    actions: [Action; 6],
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

impl IntoIterator for LegalActions {
    type Item = Action;
    type IntoIter = std::array::IntoIter<Action, 6>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let mut arr = [Action::Fold; 6];
        arr[..self.len as usize].copy_from_slice(&self.actions[..self.len as usize]);
        arr.into_iter()
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
    pub history: Vec<Action>,
    /// Minimum raise size for the current betting round.
    pub min_raise: u64,
    /// The highest bet amount in the current round.
    pub last_bet: u64,
    /// Game configuration.
    pub config: GameConfig,
    round_start: usize,
}

impl GameState {
    /// Creates a new game state with blinds posted.
    #[must_use]
    #[inline]
    pub const fn new(config: GameConfig) -> Self {
        Self {
            street: Street::Preflop,
            current_player: Player::SB,
            pot: config.small_blind + config.big_blind,
            committed: [config.small_blind, config.big_blind],
            history: Vec::new(),
            min_raise: config.min_bet,
            last_bet: config.big_blind,
            config,
            round_start: 0,
        }
    }

    /// Returns `true` if the hand has ended (fold or showdown).
    #[must_use]
    #[inline]
    pub fn is_terminal(&self) -> bool {
        self.is_fold() || self.is_showdown()
    }

    /// Returns `true` if the last action was a fold.
    #[must_use]
    #[inline]
    pub fn is_fold(&self) -> bool {
        self.history
            .last()
            .is_some_and(|a| matches!(a, Action::Fold))
    }

    /// Returns `true` if the hand reached showdown on the river.
    #[must_use]
    #[inline]
    pub fn is_showdown(&self) -> bool {
        self.street == Street::River && self.betting_round_closed()
    }

    /// Returns `true` if the current betting round is complete.
    #[must_use]
    #[inline]
    pub fn betting_round_closed(&self) -> bool {
        let round_actions = &self.history[self.round_start..];
        let Some(last) = round_actions.last() else {
            return false;
        };
        match last {
            Action::Call => true,
            Action::Check => {
                round_actions.len() >= 2 && round_actions[round_actions.len() - 2] == Action::Check
            }
            _ => false,
        }
    }

    /// Returns the winner if the hand ended by fold.
    #[must_use]
    #[inline]
    pub fn winner(&self) -> Option<Player> {
        if matches!(self.history.last(), Some(Action::Fold)) {
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
        let mut actions = [Action::Fold; 6];
        let mut len = 1;

        let remaining = self.config.initial_stacks[self.current_player.index()]
            .saturating_sub(self.committed[self.current_player.index()]);

        let to_call = self
            .last_bet
            .saturating_sub(self.committed[self.current_player.index()]);

        if to_call == 0 {
            actions[len] = Action::Check;
            len += 1;

            let bet_size =
                (self.pot * POT_BET_FRACTION_NUM / POT_BET_FRACTION_DENOM).min(remaining);
            if bet_size > 0 {
                actions[len] = Action::Bet(bet_size);
                len += 1;
            }
        } else if to_call <= remaining {
            actions[len] = Action::Call;
            len += 1;

            let raise_size = self.min_raise.max(to_call);
            if raise_size <= remaining && remaining > to_call {
                actions[len] = Action::Raise(raise_size.min(remaining - to_call));
                len += 1;
            }
        }

        if remaining > 0 {
            actions[len] = Action::AllIn;
            len += 1;
        }

        LegalActions {
            actions,
            len: len as u8,
        }
    }

    /// Applies an action and returns the new game state.
    #[must_use]
    #[inline]
    pub fn apply_action(&self, action: Action) -> Self {
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
                new_state.committed[new_state.current_player.index()] += remaining;
                new_state.pot += remaining;
                new_state.last_bet = new_state.committed[new_state.current_player.index()];
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
                Street::River => {
                    debug_assert!(false, "Street::River should be handled by condition above");
                    Street::River
                }
            };
            new_state.last_bet = 0;
            new_state.min_raise = new_state.config.min_bet;
            new_state.current_player = Player::SB;
            new_state.round_start = new_state.history.len();
        } else {
            new_state.current_player = self.current_player.opponent();
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
    pub history: Vec<Action>,
}

impl InfoSet {
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
            history: Vec::new(),
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
