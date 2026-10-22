use crate::constants::{MAX_LEVELS, PPM_DENOM};
use crate::error::NockError;
use crate::state::{MakerBook, MakerLevel};

/// Taker sells base into the maker's bids.
pub const SIDE_SELL_BASE: u8 = 0;
/// Taker buys base from the maker's asks.
pub const SIDE_BUY_BASE: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LotSizes {
    pub base_atoms_per_base_lot: u64,
    pub quote_atoms_per_quote_lot: u64,
    pub tick_size_in_quote_atoms_per_base_unit: u64,
    pub base_atoms_per_base_unit: u64,
}

impl LotSizes {
    #[inline]
    pub fn base_atoms(&self, lots: u64) -> Result<u64, NockError> {
        lots.checked_mul(self.base_atoms_per_base_lot)
            .ok_or(NockError::MathOverflow)
    }

    #[inline]
    pub fn quote_atoms(&self, lots: u64) -> Result<u64, NockError> {
        lots.checked_mul(self.quote_atoms_per_quote_lot)
            .ok_or(NockError::MathOverflow)
    }
}

/// Price of a level in ticks. Bids sit `offset + spread` below the mid, asks the same
/// distance above it. A price that is not a positive u64 is an invalid quote.
#[inline]
pub fn level_price_ticks(
    side: u8,
    mid_price_ticks: u64,
    price_offset_ticks: i64,
    sync_spread_ticks: u16,
) -> Result<u64, NockError> {
    let mid = mid_price_ticks as i128;
    let distance = price_offset_ticks as i128 + sync_spread_ticks as i128;
    let price = if side == SIDE_SELL_BASE {
        mid - distance
    } else {
        mid + distance
    };
    if price <= 0 || price > u64::MAX as i128 {
        return Err(NockError::InvalidQuotes);
    }
    Ok(price as u64)
}

/// Taker fee rounds up: dust never trades for free.
#[inline]
pub fn taker_fee_lots(gross_quote_lots: u64, fee_ppm: u32) -> u64 {
    ((gross_quote_lots as u128) * (fee_ppm as u128)).div_ceil(PPM_DENOM as u128) as u64
}

/// Maker fee rounds down: a dust fill can never cost the maker a whole extra lot.
#[inline]
pub fn maker_fee_lots(gross_quote_lots: u64, fee_ppm: u32) -> u64 {
    ((gross_quote_lots as u128) * (fee_ppm as u128) / (PPM_DENOM as u128)) as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fill {
    pub base_lots: u64,
    pub base_atoms: u64,
    pub gross_quote_lots: u64,
    pub taker_fee_lots: u64,
    pub maker_fee_lots: u64,
    /// Quote lots the taker receives on a sell or pays on a buy, taker fee applied.
    pub taker_quote_lots: u64,
    pub worst_price_ticks: u64,
    /// Base lots taken from each level, by level index.
    pub taken: [u64; MAX_LEVELS],
}

impl Fill {
    #[inline(always)]
    pub fn protocol_fee_lots(&self) -> u64 {
        self.taker_fee_lots + self.maker_fee_lots
    }
}

/// Prices an all-or-nothing fill of `base_lots` against `levels`, walking them best first.
///
/// The notional is summed exactly across levels and rounded once, against the taker.
/// `price_limit_ticks` bounds the worst level touched (a floor on a sell, a ceiling on a
/// buy); `limit_quote_lots` bounds the whole fill after the taker fee (a floor on what a
/// seller receives, a ceiling on what a buyer pays).
#[allow(clippy::too_many_arguments)]
pub fn quote_fill(
    side: u8,
    levels: &[MakerLevel; MAX_LEVELS],
    mid_price_ticks: u64,
    sync_spread_ticks: u16,
    base_lots: u64,
    limit_quote_lots: u64,
    price_limit_ticks: u64,
    lots: &LotSizes,
    maker_fee_ppm: u32,
    taker_fee_ppm: u32,
) -> Result<Fill, NockError> {
    if side != SIDE_SELL_BASE && side != SIDE_BUY_BASE {
        return Err(NockError::InvalidSide);
    }
    if base_lots == 0 {
        return Err(NockError::AmountZero);
    }
    if lots.quote_atoms_per_quote_lot == 0 || lots.base_atoms_per_base_unit == 0 {
        return Err(NockError::InvalidMarketConfig);
    }
    let selling = side == SIDE_SELL_BASE;

    let mut remaining = base_lots;
    let mut numerator: u128 = 0;
    let mut taken = [0u64; MAX_LEVELS];
    let mut worst_price_ticks = 0u64;
    for (i, level) in levels.iter().enumerate() {
        if remaining == 0 {
            break;
        }
        if level.size_in_base_lots == 0 {
            continue;
        }
        let price = level_price_ticks(
            side,
            mid_price_ticks,
            level.price_offset_ticks,
            sync_spread_ticks,
        )?;
        let beyond_limit = if selling {
            price < price_limit_ticks
        } else {
            price > price_limit_ticks
        };
        if beyond_limit {
            return Err(NockError::SlippageExceeded);
        }

        let take = level.size_in_base_lots.min(remaining);
        let level_numerator = (lots.base_atoms(take)? as u128)
            .checked_mul(price as u128)
            .ok_or(NockError::MathOverflow)?
            .checked_mul(lots.tick_size_in_quote_atoms_per_base_unit as u128)
            .ok_or(NockError::MathOverflow)?;
        numerator = numerator
            .checked_add(level_numerator)
            .ok_or(NockError::MathOverflow)?;
        taken[i] = take;
        remaining -= take;
        worst_price_ticks = price;
    }
    if remaining != 0 {
        return Err(NockError::InsufficientLiquidity);
    }

    let unit = lots.base_atoms_per_base_unit as u128;
    let lot = lots.quote_atoms_per_quote_lot as u128;
    let gross = if selling {
        numerator / unit / lot
    } else {
        numerator.div_ceil(unit).div_ceil(lot)
    };
    if gross == 0 {
        return Err(NockError::QuoteSizeTooSmall);
    }
    if gross > u64::MAX as u128 {
        return Err(NockError::MathOverflow);
    }
    let gross_quote_lots = gross as u64;

    let taker_fee = taker_fee_lots(gross_quote_lots, taker_fee_ppm);
    let maker_fee = maker_fee_lots(gross_quote_lots, maker_fee_ppm);
    let taker_quote_lots = if selling {
        let net = gross_quote_lots
            .checked_sub(taker_fee)
            .ok_or(NockError::MathOverflow)?;
        if net == 0 {
            return Err(NockError::QuoteSizeTooSmall);
        }
        if net < limit_quote_lots {
            return Err(NockError::SlippageExceeded);
        }
        net
    } else {
        let total = gross_quote_lots
            .checked_add(taker_fee)
            .ok_or(NockError::MathOverflow)?;
        if total > limit_quote_lots {
            return Err(NockError::SlippageExceeded);
        }
        total
    };

    Ok(Fill {
        base_lots,
        base_atoms: lots.base_atoms(base_lots)?,
        gross_quote_lots,
        taker_fee_lots: taker_fee,
        maker_fee_lots: maker_fee,
        taker_quote_lots,
        worst_price_ticks,
        taken,
    })
}

/// Settles `fill` against the book: consumes level sizes, moves inventory between the
/// maker's free balances and accrues protocol fees on the book. Checks everything before
/// writing anything.
///
/// Quote conservation: a sell debits `gross + maker_fee` and accrues `taker_fee + maker_fee`,
/// a net `-(gross - taker_fee)` that leaves the vault; a buy credits `gross - maker_fee` and
/// accrues both fees, a net `gross + taker_fee` that enters the vault.
pub fn apply_fill(book: &mut MakerBook, side: u8, fill: &Fill) -> Result<(), NockError> {
    let protocol_fee = fill.protocol_fee_lots();
    let (base_free, quote_free) = if side == SIDE_SELL_BASE {
        let maker_debit = fill
            .gross_quote_lots
            .checked_add(fill.maker_fee_lots)
            .ok_or(NockError::MathOverflow)?;
        let quote_free = book
            .quote_free
            .checked_sub(maker_debit)
            .ok_or(NockError::InsufficientInventory)?;
        let base_free = book
            .base_free
            .checked_add(fill.base_lots)
            .ok_or(NockError::MathOverflow)?;
        (base_free, quote_free)
    } else {
        let base_free = book
            .base_free
            .checked_sub(fill.base_lots)
            .ok_or(NockError::InsufficientInventory)?;
        let maker_credit = fill
            .gross_quote_lots
            .checked_sub(fill.maker_fee_lots)
            .ok_or(NockError::MathOverflow)?;
        let quote_free = book
            .quote_free
            .checked_add(maker_credit)
            .ok_or(NockError::MathOverflow)?;
        (base_free, quote_free)
    };
    let fees = book
        .fees_quote_lots
        .checked_add(protocol_fee)
        .ok_or(NockError::MathOverflow)?;

    let levels = if side == SIDE_SELL_BASE {
        &mut book.bid_levels
    } else {
        &mut book.ask_levels
    };
    if levels
        .iter()
        .zip(fill.taken.iter())
        .any(|(level, take)| *take > level.size_in_base_lots)
    {
        return Err(NockError::InsufficientLiquidity);
    }
    for (level, take) in levels.iter_mut().zip(fill.taken.iter()) {
        level.size_in_base_lots -= *take;
    }
    book.base_free = base_free;
    book.quote_free = quote_free;
    book.fees_quote_lots = fees;
    Ok(())
}

/// Quotes and settles a taker fill against `book` in one step.
#[allow(clippy::too_many_arguments)]
pub fn fill_book(
    book: &mut MakerBook,
    side: u8,
    base_lots: u64,
    limit_quote_lots: u64,
    price_limit_ticks: u64,
    lots: &LotSizes,
    maker_fee_ppm: u32,
    taker_fee_ppm: u32,
) -> Result<Fill, NockError> {
    let levels = if side == SIDE_SELL_BASE {
        &book.bid_levels
    } else {
        &book.ask_levels
    };
    let fill = quote_fill(
        side,
        levels,
        book.mid_price_ticks,
        book.sync_spread_ticks,
        base_lots,
        limit_quote_lots,
        price_limit_ticks,
        lots,
        maker_fee_ppm,
        taker_fee_ppm,
    )?;
    apply_fill(book, side, &fill)?;
    Ok(fill)
}
