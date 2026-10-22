use super::Tagged;
use crate::constants::{MAKER_BOOK_LEN, MAKER_BOOK_TAG, MAX_LEVELS, MAX_SEQUENCE_JUMP, NO_ADDRESS};
use crate::error::NockError;
use crate::math::{level_price_ticks, SIDE_BUY_BASE, SIDE_SELL_BASE};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MakerLevel {
    pub size_in_base_lots: u64,
    pub price_offset_ticks: i64,
}
const _: () = assert!(core::mem::size_of::<MakerLevel>() == 2 * core::mem::size_of::<u64>());

impl MakerLevel {
    pub const EMPTY: MakerLevel = MakerLevel {
        size_in_base_lots: 0,
        price_offset_ticks: 0,
    };
}

#[repr(C)]
pub struct MakerBook {
    pub tag: [u8; 8],
    pub maker: [u8; 32],
    pub market: [u8; 32],
    /// Quoting key with bounded powers; `NO_ADDRESS` when none is set.
    pub delegate: [u8; 32],

    pub mid_price_ticks: u64,
    pub base_free: u64,
    pub quote_free: u64,
    /// Protocol fees accrued by fills on this book, owed to the market admin.
    pub fees_quote_lots: u64,

    pub sequence: u64,
    /// Slot of the last quote refresh; quotes live for `expiry_in_slots` slots from here.
    pub last_updated_slot: u64,
    pub expiry_in_slots: u64,

    /// Every level price a delegate produces must sit inside this band.
    pub delegate_min_price_ticks: u64,
    pub delegate_max_price_ticks: u64,
    /// Largest level size a delegate may post.
    pub delegate_max_level_lots: u64,

    pub status: u8,
    pub maker_book_bump: u8,
    pub sync_spread_ticks: u16,
    pub _pad: [u8; 4],

    /// Best first: bid and ask offsets strictly increase among live levels.
    pub bid_levels: [MakerLevel; MAX_LEVELS],
    pub ask_levels: [MakerLevel; MAX_LEVELS],
}
const _: () = assert!(core::mem::size_of::<MakerBook>() == MAKER_BOOK_LEN);

impl Tagged for MakerBook {
    const TAG: [u8; 8] = MAKER_BOOK_TAG;
    const LEN: usize = MAKER_BOOK_LEN;
}

pub mod book_status {
    pub const ACTIVE: u8 = 0;
    pub const HALTED: u8 = 1;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Maker,
    Delegate,
}

impl MakerBook {
    /// Who `signer` is on this book. The maker check runs first, so a delegate equal to
    /// the maker keeps full maker powers.
    #[inline]
    pub fn role_of(&self, signer: &[u8]) -> Result<Role, NockError> {
        if signer == self.maker {
            return Ok(Role::Maker);
        }
        if self.delegate != NO_ADDRESS && signer == self.delegate {
            return Ok(Role::Delegate);
        }
        Err(NockError::UnauthorizedMaker)
    }

    /// Strictly increasing, and never more than `MAX_SEQUENCE_JUMP` ahead.
    #[inline]
    pub fn try_bump_sequence(&mut self, new_seq: u64) -> Result<(), NockError> {
        if new_seq <= self.sequence {
            return Err(NockError::StaleSequence);
        }
        if new_seq - self.sequence > MAX_SEQUENCE_JUMP {
            return Err(NockError::SequenceJumpTooLarge);
        }
        self.sequence = new_seq;
        Ok(())
    }

    /// Quotes refreshed at slot `s` are live in slots `s .. s + expiry_in_slots`.
    #[inline]
    pub fn quotes_live_at(&self, slot: u64) -> bool {
        slot < self.last_updated_slot.saturating_add(self.expiry_in_slots)
    }

    /// Everything a taker needs before touching this book, besides the market pause.
    #[inline]
    pub fn check_fillable(
        &self,
        slot: u64,
        expected_sequence: Option<u64>,
    ) -> Result<(), NockError> {
        if self.status != book_status::ACTIVE {
            return Err(NockError::BookHalted);
        }
        if !self.quotes_live_at(slot) {
            return Err(NockError::QuoteExpired);
        }
        if let Some(seq) = expected_sequence {
            if seq != self.sequence {
                return Err(NockError::SequenceMismatch);
            }
        }
        Ok(())
    }

    /// Book invariants after any quote change: live levels priced above zero, best first
    /// on each side, the book not crossed, and a delegate inside the maker's bounds.
    pub fn validate_quotes(&self, role: Role) -> Result<(), NockError> {
        let best_bid = self.validate_side(SIDE_SELL_BASE, &self.bid_levels, role)?;
        let best_ask = self.validate_side(SIDE_BUY_BASE, &self.ask_levels, role)?;
        if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
            if bid >= ask {
                return Err(NockError::InvalidQuotes);
            }
        }
        Ok(())
    }

    fn validate_side(
        &self,
        side: u8,
        levels: &[MakerLevel; MAX_LEVELS],
        role: Role,
    ) -> Result<Option<u64>, NockError> {
        let mut best: Option<u64> = None;
        let mut prev_offset: Option<i64> = None;
        for level in levels.iter() {
            if level.size_in_base_lots == 0 {
                continue;
            }
            if let Some(prev) = prev_offset {
                if level.price_offset_ticks <= prev {
                    return Err(NockError::InvalidQuotes);
                }
            }
            prev_offset = Some(level.price_offset_ticks);

            let price = level_price_ticks(
                side,
                self.mid_price_ticks,
                level.price_offset_ticks,
                self.sync_spread_ticks,
            )?;
            if best.is_none() {
                best = Some(price);
            }
            if role == Role::Delegate
                && (level.size_in_base_lots > self.delegate_max_level_lots
                    || price < self.delegate_min_price_ticks
                    || price > self.delegate_max_price_ticks)
            {
                return Err(NockError::OutsideDelegateBounds);
            }
        }
        Ok(best)
    }

    /// Number of live levels per side, for events.
    #[inline]
    pub fn live_levels(&self) -> (u8, u8) {
        let count = |levels: &[MakerLevel; MAX_LEVELS]| {
            levels.iter().filter(|l| l.size_in_base_lots != 0).count() as u8
        };
        (count(&self.bid_levels), count(&self.ask_levels))
    }
}
