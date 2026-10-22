use crate::constants::MAX_LEVELS;
use crate::state::MakerLevel;

/// Reinterprets instruction data as a packed payload. Payloads are `repr(C, packed)`, so
/// any alignment is valid; the length must match exactly.
#[inline]
pub fn parse<T: Payload>(data: &[u8]) -> Option<&T> {
    if data.len() != T::LEN {
        return None;
    }
    Some(unsafe { &*(data.as_ptr() as *const T) })
}

/// Marker for packed instruction payloads.
///
/// # Safety
/// Implementors must be `repr(C, packed)`, valid for every bit pattern, and exactly `LEN`
/// bytes long.
pub unsafe trait Payload {
    const LEN: usize;
}

macro_rules! payload {
    ($ty:ty, $len:expr) => {
        const _: () = assert!(core::mem::size_of::<$ty>() == $len);
        unsafe impl Payload for $ty {
            const LEN: usize = $len;
        }
    };
}

/// tick (8) + two lot sizes (16) + two fee rates (8) + two async delays (4).
pub const CREATE_POOL_DATA_LEN: usize = 36;
#[repr(C, packed)]
pub struct CreatePoolData {
    pub tick_size_in_quote_atoms_per_base_unit: u64,
    pub base_atoms_per_base_lot: u64,
    pub quote_atoms_per_quote_lot: u64,
    pub maker_fee_ppm: u32,
    pub taker_fee_ppm: u32,
    pub min_async_delay_slots: u16,
    pub max_async_delay_slots: u16,
}
payload!(CreatePoolData, CREATE_POOL_DATA_LEN);

/// One flag byte: non-zero pauses.
pub const TOGGLE_MARKET_STATE_DATA_LEN: usize = 1;
#[repr(C, packed)]
pub struct ToggleMarketStateData {
    pub paused: u8,
}
payload!(ToggleMarketStateData, TOGGLE_MARKET_STATE_DATA_LEN);

/// Two fee rates.
pub const UPDATE_CONFIG_DATA_LEN: usize = 8;
#[repr(C, packed)]
pub struct UpdateConfigData {
    pub maker_fee_ppm: u32,
    pub taker_fee_ppm: u32,
}
payload!(UpdateConfigData, UPDATE_CONFIG_DATA_LEN);

/// One address.
pub const PROPOSE_ADMIN_DATA_LEN: usize = 32;
#[repr(C, packed)]
pub struct ProposeAdminData {
    pub new_admin: [u8; 32],
}
payload!(ProposeAdminData, PROPOSE_ADMIN_DATA_LEN);

/// sequence (8) + mid (8) + two level counts (2) + padding to 8 (6).
pub const UPDATE_MAKER_BOOK_HEADER_LEN: usize = 24;
#[repr(C, packed)]
pub struct UpdateMakerBookHeader {
    pub sequence_number: u64,
    pub mid_price_ticks: u64,
    pub num_bids: u8,
    pub num_asks: u8,
    pub _pad: [u8; 6],
}
const _: () =
    assert!(core::mem::size_of::<UpdateMakerBookHeader>() == UPDATE_MAKER_BOOK_HEADER_LEN);

/// Header plus both full level arrays; levels past `num_*` are ignored and cleared.
pub const UPDATE_MAKER_BOOK_DATA_LEN: usize =
    UPDATE_MAKER_BOOK_HEADER_LEN + 2 * MAX_LEVELS * core::mem::size_of::<MakerLevel>();
#[repr(C, packed)]
pub struct UpdateMakerBookData {
    pub header: UpdateMakerBookHeader,
    pub bid_levels: [MakerLevel; MAX_LEVELS],
    pub ask_levels: [MakerLevel; MAX_LEVELS],
}
payload!(UpdateMakerBookData, UPDATE_MAKER_BOOK_DATA_LEN);

/// sequence (8) + mid (8).
pub const UPDATE_MID_PRICE_DATA_LEN: usize = 16;
#[repr(C, packed)]
pub struct UpdateMidPriceData {
    pub sequence_number: u64,
    pub new_mid_ticks: u64,
}
payload!(UpdateMidPriceData, UPDATE_MID_PRICE_DATA_LEN);

/// One sequence number; shared by ClearBook, MakerHeartbeat, MakerSetSequence and
/// MakerResetSequence.
pub const SEQUENCE_DATA_LEN: usize = 8;
#[repr(C, packed)]
pub struct SequenceData {
    pub sequence_number: u64,
}
payload!(SequenceData, SEQUENCE_DATA_LEN);

/// band floor (8) + band ceiling (8) + max level size (8).
pub const SET_BOOK_DELEGATE_DATA_LEN: usize = 24;
#[repr(C, packed)]
pub struct SetBookDelegateData {
    pub min_price_ticks: u64,
    pub max_price_ticks: u64,
    pub max_level_lots: u64,
}
payload!(SetBookDelegateData, SET_BOOK_DELEGATE_DATA_LEN);

/// base lots (8) + quote lots (8). Shared by MakerDeposit and MakerWithdraw.
pub const MAKER_FUNDS_DATA_LEN: usize = 16;
#[repr(C, packed)]
pub struct MakerFundsData {
    pub base_lots: u64,
    pub quote_lots: u64,
}
payload!(MakerFundsData, MAKER_FUNDS_DATA_LEN);

/// sequence (8) + flag (1).
pub const SET_BOOK_HALT_DATA_LEN: usize = 9;
#[repr(C, packed)]
pub struct SetBookHaltData {
    pub sequence_number: u64,
    pub halted: u8,
}
payload!(SetBookHaltData, SET_BOOK_HALT_DATA_LEN);

/// sequence (8) + spread (2).
pub const UPDATE_SYNC_SPREAD_DATA_LEN: usize = 10;
#[repr(C, packed)]
pub struct UpdateSyncSpreadData {
    pub sequence_number: u64,
    pub sync_spread_ticks: u16,
}
payload!(UpdateSyncSpreadData, UPDATE_SYNC_SPREAD_DATA_LEN);

/// sequence (8) + expiry (8).
pub const UPDATE_EXPIRY_IN_SLOTS_DATA_LEN: usize = 16;
#[repr(C, packed)]
pub struct UpdateExpiryInSlotsData {
    pub sequence_number: u64,
    pub expiry_in_slots: u64,
}
payload!(UpdateExpiryInSlotsData, UPDATE_EXPIRY_IN_SLOTS_DATA_LEN);

/// Taker pins nothing.
pub const PIN_NONE: u8 = 0;
/// Taker requires the book's sequence to equal `expected_sequence`.
pub const PIN_SEQUENCE: u8 = 1;

/// side (1) + pin flag (1) + base lots, quote limit, price limit, sequence (32).
pub const SYNC_SWAP_DATA_LEN: usize = 34;
#[repr(C, packed)]
pub struct SyncSwapData {
    pub side: u8,
    pub pin_sequence: u8,
    pub base_lots: u64,
    /// Sell: least net quote lots received. Buy: most quote lots paid, fee included.
    pub limit_quote_lots: u64,
    /// Sell: lowest level price touched. Buy: highest level price touched.
    pub price_limit_ticks: u64,
    pub expected_sequence: u64,
}
payload!(SyncSwapData, SYNC_SWAP_DATA_LEN);

/// side (1) + nonce, base lots, quote limit, price limit, expiry (40) + maker (32).
pub const ASYNC_SWAP_CREATE_DATA_LEN: usize = 73;
#[repr(C, packed)]
pub struct AsyncSwapCreateData {
    pub side: u8,
    pub nonce: u64,
    pub base_lots: u64,
    pub limit_quote_lots: u64,
    pub price_limit_ticks: u64,
    pub expiry_slot: u64,
    /// Only this maker may fill; `NO_ADDRESS` lets any book on the market fill.
    pub maker: [u8; 32],
}
payload!(AsyncSwapCreateData, ASYNC_SWAP_CREATE_DATA_LEN);
