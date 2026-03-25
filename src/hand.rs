use std::fmt;

use crate::card::Card;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hand {
    rank: u32,
}

impl Hand {
    #[must_use]
    #[inline]
    pub const fn rank(self) -> u32 {
        self.rank
    }

    #[must_use]
    #[inline]
    pub fn hand_type(&self) -> HandType {
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

    #[must_use]
    #[inline]
    pub fn evaluate(hole: &[Card; 2], board: &[Card]) -> Self {
        let mut all_cards: Vec<Card> = hole.iter().copied().chain(board.iter().copied()).collect();
        all_cards.sort_by_key(|b| std::cmp::Reverse(b.rank()));

        let rank = Self::evaluate_hand_rank(&all_cards);
        Hand { rank }
    }

    #[inline]
    fn evaluate_hand_rank(cards: &[Card]) -> u32 {
        if cards.len() < 5 {
            let kickers: Vec<u8> = cards.iter().map(|c| c.rank()).collect();
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

        let ranks: Vec<u8> = cards.iter().map(|c| c.rank()).collect();
        let counts = Self::count_ranks(&ranks);

        if let Some(rank) = Self::find_four_of_a_kind(&counts) {
            let kicker = Self::best_kicker(&counts, &[rank]);
            return Self::hand_rank(7, &[rank, kicker]);
        }

        if let Some((trips, pair)) = Self::find_full_house(&counts) {
            return Self::hand_rank(6, &[trips, pair]);
        }

        if let Some(flush_cards) = flush {
            let kickers: Vec<u8> = flush_cards.iter().take(5).map(|c| c.rank()).collect();
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

        let kickers: Vec<u8> = cards.iter().take(5).map(|c| c.rank()).collect();
        Self::hand_rank(0, &kickers)
    }

    #[inline]
    fn hand_rank(hand_type: u32, values: &[u8]) -> u32 {
        let mut rank = hand_type << 24;
        for (i, &v) in values.iter().enumerate() {
            rank += (v as u32) << (20 - i * 4);
        }
        rank
    }

    #[inline]
    fn best_kicker(counts: &[u8; 15], excluded: &[u8]) -> u8 {
        for (rank, &count) in counts.iter().enumerate().rev() {
            if count > 0 && !excluded.contains(&(rank as u8)) {
                return rank as u8;
            }
        }
        0
    }

    #[inline]
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

    #[inline]
    fn find_flush(cards: &[Card]) -> Option<Vec<Card>> {
        let mut suit_counts = [0usize; 4];
        for card in cards {
            suit_counts[card.suit() as usize] += 1;
        }
        for (suit, &count) in suit_counts.iter().enumerate() {
            if count >= 5 {
                let flush_cards: Vec<Card> = cards
                    .iter()
                    .filter(|c| c.suit() as usize == suit)
                    .copied()
                    .collect();
                return Some(flush_cards);
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

        if rank_mask & 0x403C == 0x403C {
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
    fn find_four_of_a_kind(counts: &[u8; 15]) -> Option<u8> {
        for (rank, &count) in counts.iter().enumerate() {
            if count == 4 {
                return Some(rank as u8);
            }
        }
        None
    }

    #[inline]
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
    fn find_three_of_a_kind(counts: &[u8; 15]) -> Option<u8> {
        for (rank, &count) in counts.iter().enumerate().rev() {
            if count == 3 {
                return Some(rank as u8);
            }
        }
        None
    }

    #[inline]
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
    fn find_pair(counts: &[u8; 15]) -> Option<u8> {
        for (rank, &count) in counts.iter().enumerate().rev() {
            if count == 2 {
                return Some(rank as u8);
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandType {
    HighCard,
    Pair,
    TwoPair,
    ThreeOfAKind,
    Straight,
    Flush,
    FullHouse,
    FourOfAKind,
    StraightFlush,
    RoyalFlush,
}

impl fmt::Display for HandType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandType::HighCard => write!(f, "High Card"),
            HandType::Pair => write!(f, "Pair"),
            HandType::TwoPair => write!(f, "Two Pair"),
            HandType::ThreeOfAKind => write!(f, "Three of a Kind"),
            HandType::Straight => write!(f, "Straight"),
            HandType::Flush => write!(f, "Flush"),
            HandType::FullHouse => write!(f, "Full House"),
            HandType::FourOfAKind => write!(f, "Four of a Kind"),
            HandType::StraightFlush => write!(f, "Straight Flush"),
            HandType::RoyalFlush => write!(f, "Royal Flush"),
        }
    }
}

impl fmt::Display for Hand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.hand_type())
    }
}
