//! Card representation, deck management, and card sets.

use std::fmt;

use serde::{Deserialize, Serialize};

const NUM_CARDS: usize = 52;

/// A playing card with rank (2-14, where 14=Ace) and suit (0-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    rank: u8,
    suit: u8,
}

impl Card {
    const MIN_RANK: u8 = 2;
    const MAX_RANK: u8 = 14;
    const NUM_SUITS: u8 = 4;

    /// Creates a new card with the given rank and suit.
    ///
    /// Returns `None` if rank is not in 2-14 or suit is not in 0-3.
    #[must_use]
    pub fn new(rank: u8, suit: u8) -> Option<Self> {
        if (Self::MIN_RANK..=Self::MAX_RANK).contains(&rank) && suit < Self::NUM_SUITS {
            Some(Card { rank, suit })
        } else {
            None
        }
    }

    /// Returns the rank of the card (2-14, where 14=Ace).
    #[must_use]
    #[inline]
    pub const fn rank(self) -> u8 {
        self.rank
    }

    /// Returns the suit of the card (0-3: clubs, diamonds, hearts, spades).
    #[must_use]
    #[inline]
    pub const fn suit(self) -> u8 {
        self.suit
    }

    /// Returns a static reference to all 52 cards in the deck.
    #[must_use]
    pub fn all() -> &'static [Card; NUM_CARDS] {
        &ALL_CARDS
    }

    #[inline]
    pub(crate) const fn placeholder() -> Self {
        Card { rank: 0, suit: 0 }
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rank_char = match self.rank {
            14 => 'A',
            13 => 'K',
            12 => 'Q',
            11 => 'J',
            10 => 'T',
            r => char::from(b'0' + r),
        };
        let suit_char = match self.suit {
            0 => 'c',
            1 => 'd',
            2 => 'h',
            3 => 's',
            _ => '?',
        };
        write!(f, "{}{}", rank_char, suit_char)
    }
}

static ALL_CARDS: [Card; NUM_CARDS] = {
    let mut cards = [Card { rank: 0, suit: 0 }; NUM_CARDS];
    let mut idx = 0;
    let mut rank = 2;
    while rank <= 14 {
        let mut suit = 0;
        while suit < 4 {
            cards[idx] = Card { rank, suit };
            idx += 1;
            suit += 1;
        }
        rank += 1;
    }
    cards
};

/// A fixed-size collection of up to 5 cards.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardSet {
    cards: [Card; 5],
    len: u8,
}

impl CardSet {
    /// Creates an empty card set.
    #[must_use]
    #[inline]
    pub const fn empty() -> Self {
        CardSet {
            cards: [Card { rank: 0, suit: 0 }; 5],
            len: 0,
        }
    }

    /// Creates a card set from a slice of cards (max 5 cards).
    #[must_use]
    #[inline]
    pub fn from_cards(cards: &[Card]) -> Self {
        let mut arr = [Card { rank: 0, suit: 0 }; 5];
        let len = cards.len().min(5);
        arr[..len].copy_from_slice(&cards[..len]);
        CardSet {
            cards: arr,
            len: len as u8,
        }
    }

    /// Returns the cards as a slice.
    #[must_use]
    #[inline]
    pub fn as_slice(&self) -> &[Card] {
        &self.cards[..self.len as usize]
    }

    /// Returns the number of cards in the set.
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns `true` if the set contains no cards.
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// A standard 52-card deck that can be shuffled and dealt.
#[derive(Debug, Clone)]
pub struct Deck {
    cards: Vec<Card>,
    pos: usize,
}

impl Deck {
    /// Creates a new deck with all 52 cards in order.
    #[must_use]
    pub fn new() -> Self {
        Deck {
            cards: Card::all().to_vec(),
            pos: 0,
        }
    }

    /// Shuffles the deck using the provided RNG and resets the deal position.
    #[inline]
    pub fn shuffle(&mut self, rng: &mut impl rand::Rng) {
        use rand::seq::SliceRandom;
        self.cards.shuffle(rng);
        self.pos = 0;
    }

    /// Deals one card from the deck, returning `None` if the deck is exhausted.
    pub fn deal_one(&mut self) -> Option<Card> {
        if self.pos < self.cards.len() {
            let card = self.cards[self.pos];
            self.pos += 1;
            Some(card)
        } else {
            None
        }
    }

    /// Deals `n` cards from the deck (or fewer if not enough cards remain).
    pub fn deal(&mut self, n: usize) -> Vec<Card> {
        let available = self.cards.len().saturating_sub(self.pos);
        let count = n.min(available);
        let result = self.cards[self.pos..self.pos + count].to_vec();
        self.pos += count;
        result
    }
}

impl Default for Deck {
    fn default() -> Self {
        Self::new()
    }
}
