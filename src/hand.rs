//! Poker hand evaluation (high card through straight flush).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::card::Card;

/// Bitmask for wheel straight (A-2-3-4-5): bits set at ranks 14, 5, 4, 3, 2.
const WHEEL_STRAIGHT_MASK: u32 = 0x403C;

/// Result of flush detection containing flush cards and count.
/// Uses fixed-size array to avoid heap allocation in hot path.
type FlushResult = Option<([Card; 7], usize)>;

/// A evaluated poker hand with a comparable rank value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Hand {
    rank: u32,
}

impl Hand {
    /// Returns the internal rank value for this hand.
    #[must_use]
    #[inline]
    pub const fn rank(self) -> u32 {
        self.rank
    }

    /// Returns the type of hand (pair, flush, etc.).
    #[must_use]
    #[inline]
    pub const fn hand_type(&self) -> HandType {
        match self.rank >> 24 {
            0 => HandType::HighCard,
            1 => HandType::Pair,
            2 => HandType::TwoPair,
            3 => HandType::ThreeOfAKind,
            4 => HandType::Straight,
            5 => HandType::Flush,
            6 => HandType::FullHouse,
            7 => HandType::FourOfAKind,
            8 => HandType::StraightFlush,
            9 => HandType::RoyalFlush,
            _ => HandType::HighCard,
        }
    }

    /// Evaluates a poker hand from hole cards and board.
    #[must_use]
    #[inline]
    pub fn evaluate(hole: &[Card; 2], board: &[Card]) -> Self {
        let mut all_cards: [Card; 7] = [
            hole[0],
            hole[1],
            Card::placeholder(),
            Card::placeholder(),
            Card::placeholder(),
            Card::placeholder(),
            Card::placeholder(),
        ];
        let len = 2 + board.len().min(5);
        all_cards[2..len].copy_from_slice(&board[..len - 2]);
        all_cards[..len].sort_unstable_by_key(|b| std::cmp::Reverse(b.rank()));

        let rank = Self::evaluate_hand_rank(&all_cards[..len]);
        Self { rank }
    }

    #[inline]
    fn evaluate_hand_rank(cards: &[Card]) -> u32 {
        if cards.len() < 5 {
            let mut kickers = [0u8; 5];
            for (i, c) in cards.iter().enumerate() {
                kickers[i] = c.rank();
            }
            return Self::hand_rank(0, &kickers[..cards.len()]);
        }

        let flush = Self::find_flush(cards);
        let straight = Self::find_straight(cards);

        if let (Some((flush_cards, flush_len)), Some(straight_high)) = (&flush, straight) {
            if Self::is_straight_flush(&flush_cards[..*flush_len], straight_high) {
                if straight_high == 14 {
                    return Self::hand_rank(9, &[14]);
                }
                return Self::hand_rank(8, &[straight_high]);
            }
        }

        let ranks: [u8; 7] = {
            let mut arr = [0u8; 7];
            for (i, c) in cards.iter().enumerate() {
                arr[i] = c.rank();
            }
            arr
        };
        let counts = Self::count_ranks(&ranks[..cards.len()]);

        if let Some(rank) = Self::find_four_of_a_kind(&counts) {
            let kicker = Self::best_kicker(&counts, &[rank]);
            return Self::hand_rank(7, &[rank, kicker]);
        }

        if let Some((trips, pair)) = Self::find_full_house(&counts) {
            return Self::hand_rank(6, &[trips, pair]);
        }

        if let Some((flush_cards, flush_len)) = flush {
            let mut kickers = [0u8; 5];
            for (i, c) in flush_cards.iter().take(flush_len.min(5)).enumerate() {
                kickers[i] = c.rank();
            }
            return Self::hand_rank(5, &kickers);
        }

        if let Some(high) = straight {
            return Self::hand_rank(4, &[high]);
        }

        if let Some(rank) = Self::find_three_of_a_kind(&counts) {
            let kickers = Self::best_kickers_fixed::<2>(&counts, &[rank]);
            return Self::hand_rank(3, &[rank, kickers[0], kickers[1]]);
        }

        if let Some((high, low)) = Self::find_two_pair(&counts) {
            let kicker = Self::best_kicker(&counts, &[high, low]);
            return Self::hand_rank(2, &[high, low, kicker]);
        }

        if let Some(rank) = Self::find_pair(&counts) {
            let kickers = Self::best_kickers_fixed::<3>(&counts, &[rank]);
            return Self::hand_rank(1, &[rank, kickers[0], kickers[1], kickers[2]]);
        }

        let mut kickers = [0u8; 5];
        for (i, c) in cards.iter().take(5).enumerate() {
            kickers[i] = c.rank();
        }
        Self::hand_rank(0, &kickers)
    }

    #[inline]
    #[allow(clippy::cast_lossless)]
    fn hand_rank(hand_type: u32, values: &[u8]) -> u32 {
        debug_assert!(
            values.len() <= 5,
            "values array too large for hand rank encoding"
        );
        let mut rank = hand_type << 24;
        for (i, &v) in values.iter().enumerate() {
            rank += u32::from(v) << (20 - i * 4);
        }
        rank
    }

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn best_kicker(counts: &[u8; 15], excluded: &[u8]) -> u8 {
        for (rank, &count) in counts.iter().enumerate().rev() {
            if count > 0 && !excluded.contains(&(rank as u8)) {
                return rank as u8;
            }
        }
        0
    }

    #[must_use]
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn best_kickers_fixed<const N: usize>(counts: &[u8; 15], excluded: &[u8]) -> [u8; N] {
        let mut kickers = [0u8; N];
        let mut found = 0;
        for (rank, &count) in counts.iter().enumerate().rev() {
            if count > 0 && !excluded.contains(&(rank as u8)) {
                kickers[found] = rank as u8;
                found += 1;
                if found == N {
                    return kickers;
                }
            }
        }
        kickers
    }

    #[inline]
    fn find_flush(cards: &[Card]) -> FlushResult {
        let mut suit_counts = [0usize; 4];
        for card in cards {
            suit_counts[card.suit() as usize] += 1;
        }
        for (suit, &count) in suit_counts.iter().enumerate() {
            if count >= 5 {
                let mut flush_cards = [Card::placeholder(); 7];
                let mut len = 0;
                for &card in cards {
                    if card.suit() as usize == suit {
                        flush_cards[len] = card;
                        len += 1;
                    }
                }
                return Some((flush_cards, len));
            }
        }
        None
    }

    #[inline]
    fn find_straight(cards: &[Card]) -> Option<u8> {
        let mut rank_mask: u32 = 0;
        for card in cards {
            rank_mask |= 1 << card.rank();
        }

        for high in (5..=14).rev() {
            let straight_mask = ((1u32 << 5) - 1) << (high - 4);
            if rank_mask & straight_mask == straight_mask {
                return Some(high);
            }
        }

        if rank_mask & WHEEL_STRAIGHT_MASK == WHEEL_STRAIGHT_MASK {
            return Some(5);
        }
        None
    }

    #[inline]
    fn is_straight_flush(flush_cards: &[Card], straight_high: u8) -> bool {
        Self::find_straight(flush_cards) == Some(straight_high)
    }

    #[inline]
    fn count_ranks(ranks: &[u8]) -> [u8; 15] {
        let mut counts = [0u8; 15];
        for &rank in ranks {
            counts[rank as usize] += 1;
        }
        counts
    }

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn find_four_of_a_kind(counts: &[u8; 15]) -> Option<u8> {
        for (rank, &count) in counts.iter().enumerate() {
            if count == 4 {
                return Some(rank as u8);
            }
        }
        None
    }

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
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

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn find_three_of_a_kind(counts: &[u8; 15]) -> Option<u8> {
        for (rank, &count) in counts.iter().enumerate().rev() {
            if count == 3 {
                return Some(rank as u8);
            }
        }
        None
    }

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn find_two_pair(counts: &[u8; 15]) -> Option<(u8, u8)> {
        let mut first: Option<u8> = None;
        let mut second: Option<u8> = None;
        for (rank, &count) in counts.iter().enumerate().rev() {
            if count == 2 {
                if first.is_none() {
                    first = Some(rank as u8);
                } else {
                    second = Some(rank as u8);
                    break;
                }
            }
        }
        first.zip(second)
    }

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn find_pair(counts: &[u8; 15]) -> Option<u8> {
        for (rank, &count) in counts.iter().enumerate().rev() {
            if count == 2 {
                return Some(rank as u8);
            }
        }
        None
    }
}

/// The type/category of a poker hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum HandType {
    /// High card only.
    HighCard,
    /// One pair.
    Pair,
    /// Two pairs.
    TwoPair,
    /// Three of a kind.
    ThreeOfAKind,
    /// Straight (5 consecutive cards).
    Straight,
    /// Flush (5 cards of same suit).
    Flush,
    /// Full house (three of a kind + pair).
    FullHouse,
    /// Four of a kind.
    FourOfAKind,
    /// Straight flush (straight + flush).
    StraightFlush,
    /// Royal flush (A-K-Q-J-T of same suit).
    RoyalFlush,
}

impl fmt::Display for HandType {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HighCard => write!(f, "High Card"),
            Self::Pair => write!(f, "Pair"),
            Self::TwoPair => write!(f, "Two Pair"),
            Self::ThreeOfAKind => write!(f, "Three of a Kind"),
            Self::Straight => write!(f, "Straight"),
            Self::Flush => write!(f, "Flush"),
            Self::FullHouse => write!(f, "Full House"),
            Self::FourOfAKind => write!(f, "Four of a Kind"),
            Self::StraightFlush => write!(f, "Straight Flush"),
            Self::RoyalFlush => write!(f, "Royal Flush"),
        }
    }
}

impl fmt::Display for Hand {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.hand_type())
    }
}
