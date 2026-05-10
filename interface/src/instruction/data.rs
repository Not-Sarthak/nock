use crate::constants::{CREATE_POOL_DATA_LEN, MAX_LEVELS, UPDATE_MAKER_BOOK_DATA_LEN};

#[repr(C, packed)]
pub struct CreatePoolData {
    pub market_authority:                       [u8; 32],
    pub base_mint:                              [u8; 32],
    pub base_token_program:                     [u8; 32],
    pub quote_mint:                             [u8; 32],
    pub quote_token_program:                    [u8; 32],
    pub tick_size_in_quote_atoms_per_base_unit: u64,
    pub base_atoms_per_base_lot:                u64,
    pub quote_atoms_per_quote_lot:              u64,
    pub raw_base_units_per_base_unit:           u64,
    pub maker_fee_ppm:                          i32,
    pub taker_fee_ppm:                          i32,
    pub base_decimals:                          u8,
    pub quote_decimals:                         u8,
    pub mode:                                   u8,
    pub sync_fee_multiplier:                    u8,
    pub min_async_delay_slots:                  u16,
    pub max_async_delay_slots:                  u16,
    pub market_bump:                            u8,
    pub _pad:                                   [u8; 3],
}
const _: () = assert!(core::mem::size_of::<CreatePoolData>() == CREATE_POOL_DATA_LEN);

#[repr(C, packed)]
pub struct UpdateMakerBookHeader {
    pub sequence_number:   u64,
    pub mid_price_ticks:   u64,
    pub last_updated_slot: u64,
    pub num_bids:          u8,
    pub num_asks:          u8,
    pub _pad:              [u8; 6],
}
const _: () = assert!(core::mem::size_of::<UpdateMakerBookHeader>() == 32);

#[repr(C, packed)]
pub struct UpdateMakerBookData {
    pub header:     UpdateMakerBookHeader,
    pub bid_levels: [crate::state::MakerLevel; MAX_LEVELS],
    pub ask_levels: [crate::state::MakerLevel; MAX_LEVELS],
}
const _: () = assert!(core::mem::size_of::<UpdateMakerBookData>() == UPDATE_MAKER_BOOK_DATA_LEN);

#[repr(C, packed)]
pub struct SyncSwapData {
    pub amount_in:      u64,
    pub min_amount_out: u64,
    pub side:           u8,
    pub quote_index:    u8,
}
const _: () = assert!(core::mem::size_of::<SyncSwapData>() == 18);

#[repr(C, packed)]
pub struct UpdateMidPriceData {
    pub sequence_number: u64,
    pub new_mid_ticks:   u64,
}
const _: () = assert!(core::mem::size_of::<UpdateMidPriceData>() == 16);

#[repr(C, packed)]
pub struct ClearBookData {
    pub sequence_number: u64,
}
const _: () = assert!(core::mem::size_of::<ClearBookData>() == 8);

#[repr(C, packed)]
pub struct MakerDepositData {
    pub base_lots:  u64,
    pub quote_lots: u64,
}
const _: () = assert!(core::mem::size_of::<MakerDepositData>() == 16);

pub type MakerWithdrawData = MakerDepositData;

#[repr(C, packed)]
pub struct UpdateSyncSpreadData {
    pub sync_spread_ticks: u16,
}
const _: () = assert!(core::mem::size_of::<UpdateSyncSpreadData>() == 2);

#[repr(C, packed)]
pub struct UpdateExpiryInSlotsData {
    pub expiry_in_slots: u64,
}
const _: () = assert!(core::mem::size_of::<UpdateExpiryInSlotsData>() == 8);
