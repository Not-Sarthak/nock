use super::Tagged;
use crate::constants::{MARKET_LEN, MARKET_TAG};

#[repr(C)]
pub struct Market {
    pub tag: [u8; 8],
    pub base_mint: [u8; 32],
    pub quote_mint: [u8; 32],
    pub base_vault: [u8; 32],
    pub quote_vault: [u8; 32],
    /// Authority that signed CreatePool. It is a PDA seed, so it never changes.
    pub seed_authority: [u8; 32],
    pub admin: [u8; 32],
    /// Proposed next admin; `NO_ADDRESS` when no rotation is in flight.
    pub pending_admin: [u8; 32],

    pub base_atoms_per_base_lot: u64,
    pub quote_atoms_per_quote_lot: u64,
    pub tick_size_in_quote_atoms_per_base_unit: u64,
    pub base_atoms_per_base_unit: u64,

    pub maker_fee_ppm: u32,
    pub taker_fee_ppm: u32,

    pub min_async_delay_slots: u16,
    pub max_async_delay_slots: u16,
    pub status: u8,
    pub market_bump: u8,
    pub _pad: [u8; 2],
}
const _: () = assert!(core::mem::size_of::<Market>() == MARKET_LEN);

impl Tagged for Market {
    const TAG: [u8; 8] = MARKET_TAG;
    const LEN: usize = MARKET_LEN;
}

pub mod market_status {
    pub const ACTIVE: u8 = 0;
    pub const PAUSED: u8 = 1;
}

impl Market {
    #[inline(always)]
    pub fn is_paused(&self) -> bool {
        self.status == market_status::PAUSED
    }

    #[inline(always)]
    pub fn set_paused(&mut self, paused: bool) {
        self.status = if paused {
            market_status::PAUSED
        } else {
            market_status::ACTIVE
        };
    }

    #[inline(always)]
    pub fn lot_sizes(&self) -> crate::math::LotSizes {
        crate::math::LotSizes {
            base_atoms_per_base_lot: self.base_atoms_per_base_lot,
            quote_atoms_per_quote_lot: self.quote_atoms_per_quote_lot,
            tick_size_in_quote_atoms_per_base_unit: self.tick_size_in_quote_atoms_per_base_unit,
            base_atoms_per_base_unit: self.base_atoms_per_base_unit,
        }
    }
}
