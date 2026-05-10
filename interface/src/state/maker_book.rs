use super::Tagged;
use crate::constants::{MAKER_BOOK_LEN, MAKER_BOOK_TAG, MAX_LEVELS};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MakerLevel {
    pub size_in_base_lots:  u64,
    pub price_offset_ticks: i64,
}
const _: () = assert!(core::mem::size_of::<MakerLevel>() == 16);

#[repr(C)]
pub struct MakerBook {
    pub tag:                          [u8; 8],
    pub maker:                        [u8; 32],
    pub market:                       [u8; 32],
    pub delegate:                     [u8; 32],
    pub mid_price_ticks:              u64,
    pub quote_delta_per_tick:         u64,
    pub min_mid_price_ticks:          u64,
    pub quote_locked:                 u64,
    pub quote_free:                   u64,
    pub base_locked:                  u64,
    pub base_free:                    u64,
    pub status:                       u8,
    pub maker_book_bump:              u8,
    pub sync_spread_ticks:            u16,
    pub _status_padding:              [u8; 4],
    pub last_updated_sequence_number: u64,
    pub total_bid_base_lots:          u64,
    pub tick_conversion_num:          u64,
    pub tick_conversion_den:          u64,
    pub bid_levels:                   [MakerLevel; MAX_LEVELS],
    pub ask_levels:                   [MakerLevel; MAX_LEVELS],
    pub last_updated_slot:            u64,
    pub expiry_in_slots:              u64,
    pub _reserved:                    [u64; 6],
}
const _: () = assert!(core::mem::size_of::<MakerBook>() == MAKER_BOOK_LEN);

impl Tagged for MakerBook {
    const TAG: [u8; 8] = MAKER_BOOK_TAG;
    const LEN: usize   = MAKER_BOOK_LEN;
}

pub mod book_status {
    pub const ACTIVE: u8 = 0;
    pub const CLOSED: u8 = 1;
}

impl MakerBook {
    #[inline(always)]
    pub fn try_bump_sequence(&mut self, new_seq: u64) -> bool {
        if new_seq <= self.last_updated_sequence_number {
            return false;
        }
        self.last_updated_sequence_number = new_seq;
        true
    }
}
