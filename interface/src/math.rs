use crate::constants::PPM_DENOM;
use crate::error::AmmError;

pub const SIDE_SELL_BASE: u8 = 0;
pub const SIDE_BUY_BASE:  u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fill {
    pub base_atoms: u64,
    pub gross_quote_lots: u64,
    pub taker_fee_lots: u64,
    pub maker_fee_lots: u64,
}

impl Fill {
    #[inline(always)]
    pub fn protocol_fee_lots(&self) -> u64 {
        self.taker_fee_lots + self.maker_fee_lots
    }
}

#[inline]
pub fn level_price_ticks(
    side: u8,
    mid_price_ticks: u64,
    price_offset_ticks: i64,
    sync_spread_ticks: u16,
) -> Result<i128, AmmError> {
    let mid    = mid_price_ticks as i128;
    let offset = price_offset_ticks as i128;
    let spread = sync_spread_ticks as i128;

    let price = if side == SIDE_SELL_BASE {
        mid - offset - spread
    } else {
        mid + offset + spread
    };

    if price <= 0 {
        return Err(AmmError::PriceMathOverflow);
    }
    Ok(price)
}

#[inline]
fn fee_lots_ceil(gross_quote_lots: u64, fee_ppm: i32) -> u64 {
    if fee_ppm <= 0 {
        return 0;
    }
    (((gross_quote_lots as u128) * (fee_ppm as u128)).div_ceil(PPM_DENOM as u128)) as u64
}

#[allow(clippy::too_many_arguments)]
pub fn price_fill(
    side: u8,
    amount_in_lots: u64,
    price_ticks: i128,
    base_atoms_per_base_lot: u64,
    quote_atoms_per_quote_lot: u64,
    tick_size_in_quote_atoms_per_base_unit: u64,
    base_atoms_per_base_unit: u64,
    maker_fee_ppm: i32,
    taker_fee_ppm: i32,
) -> Result<Fill, AmmError> {
    if price_ticks <= 0 {
        return Err(AmmError::PriceMathOverflow);
    }
    if quote_atoms_per_quote_lot == 0 || base_atoms_per_base_unit == 0 {
        return Err(AmmError::PriceMathOverflow);
    }

    let taker_pays_quote = side == SIDE_BUY_BASE;

    let base_atoms = amount_in_lots
        .checked_mul(base_atoms_per_base_lot)
        .ok_or(AmmError::PriceMathOverflow)?;

    let numerator = (base_atoms as u128)
        .checked_mul(price_ticks as u128)
        .ok_or(AmmError::PriceMathOverflow)?
        .checked_mul(tick_size_in_quote_atoms_per_base_unit as u128)
        .ok_or(AmmError::PriceMathOverflow)?;

    let denominator = base_atoms_per_base_unit as u128;
    let quote_atoms = if taker_pays_quote {
        numerator.div_ceil(denominator)
    } else {
        numerator / denominator
    };

    if quote_atoms == 0 || quote_atoms > u64::MAX as u128 {
        return Err(AmmError::PriceMathOverflow);
    }

    let lot = quote_atoms_per_quote_lot as u128;
    let gross_quote_lots = if taker_pays_quote {
        quote_atoms.div_ceil(lot)
    } else {
        quote_atoms / lot
    } as u64;

    if gross_quote_lots == 0 {
        return Err(AmmError::QuoteSizeTooSmall);
    }

    Ok(Fill {
        base_atoms,
        gross_quote_lots,
        taker_fee_lots: fee_lots_ceil(gross_quote_lots, taker_fee_ppm),
        maker_fee_lots: fee_lots_ceil(gross_quote_lots, maker_fee_ppm),
    })
}
