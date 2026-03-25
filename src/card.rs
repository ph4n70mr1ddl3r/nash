use std::fmt;

use serde::{Deserialize, Serialize};

const NUM_CARDS: usize = 52;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    rank: u8,
    suit: u8,
}

impl Card {
    const MIN_RANK: u8 = 2;
    const MAX_RANK: u8 = 14;
    const NUM_SUITS: u8 = 4;

    #[must_use]
    pub fn new(rank: u8, suit: u8) -> Option<Self> {
        if (Self::MIN_RANK..=Self::MAX_RANK).contains(&rank) && suit < Self::NUM_SUITS {
            Some(Card { rank, suit })
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub const fn rank(self) -> u8 {
        self.rank
    }

    #[must_use]
    #[inline]
    pub const fn suit(self) -> u8 {
        self.suit
    }

    #[must_use]
    pub fn all() -> &'static [Card; NUM_CARDS] {
        &ALL_CARDS
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

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardSet {
    cards: [Card; 5],
    len: u8,
}

impl CardSet {
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

    #[must_use]
    #[inline]
    pub fn as_slice(&self) -> &[Card] {
        &self.cards[..self.len as usize]
    }

    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Debug, Clone)]
pub struct Deck {
    cards: Vec<Card>,
    pos: usize,
}

impl Deck {
    #[must_use]
    pub fn new() -> Self {
        Deck {
            cards: Card::all().to_vec(),
            pos: 0,
        }
    }

    pub fn shuffle(&mut self, rng: &mut impl rand::Rng) {
        use rand::seq::SliceRandom;
        self.cards.shuffle(rng);
        self.pos = 0;
    }

    pub fn deal_one(&mut self) -> Option<Card> {
        if self.pos < self.cards.len() {
            let card = self.cards[self.pos];
            self.pos += 1;
            Some(card)
        } else {
            None
        }
    }

    pub fn deal(&mut self, n: usize) -> Vec<Card> {
        let mut result = Vec::with_capacity(n);
        for _ in 0..n {
            if let Some(card) = self.deal_one() {
                result.push(card);
            }
        }
        result
    }
}

impl Default for Deck {
    fn default() -> Self {
        Self::new()
    }
}
