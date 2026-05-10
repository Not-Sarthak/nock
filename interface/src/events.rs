use crate::constants::SYNC_FILL_EVENT_LEN;

#[repr(C)]
pub struct SyncFillEvent {
    pub tag:        [u8; 8],
    pub pool:       [u8; 32],
    pub maker:      [u8; 32],
    pub user:       [u8; 32],
    pub sequence:   u64,
    pub amount_in:  u64,
    pub amount_out: u64,
    pub fee:        u64,
    pub slot:       u64,
    pub side:       u64,
}
const _: () = assert!(core::mem::size_of::<SyncFillEvent>() == SYNC_FILL_EVENT_LEN);

#[repr(C)]
pub struct AsyncSwapCreatedEvent {
    pub tag:         [u8; 8],
    pub async_swap:  [u8; 32],
    pub user:        [u8; 32],
    pub pool:        [u8; 32],
    pub amount_in:   u64,
    pub min_out:     u64,
    pub expiry_slot: u64,
    pub side:        u64,
}
const _: () = assert!(core::mem::size_of::<AsyncSwapCreatedEvent>() == 8 + 32*3 + 8*4);

#[repr(C)]
pub struct AsyncFillEvent {
    pub tag:        [u8; 8],
    pub async_swap: [u8; 32],
    pub maker:      [u8; 32],
    pub user:       [u8; 32],
    pub amount_out: u64,
    pub slot:       u64,
}
const _: () = assert!(core::mem::size_of::<AsyncFillEvent>() == 8 + 32*3 + 8*2);

#[repr(C)]
pub struct AsyncCancelEvent {
    pub tag:        [u8; 8],
    pub async_swap: [u8; 32],
    pub user:       [u8; 32],
    pub refunded:   u64,
    pub slot:       u64,
}
const _: () = assert!(core::mem::size_of::<AsyncCancelEvent>() == 8 + 32*2 + 8*2);

#[repr(C)]
pub struct BookUpdateEvent {
    pub tag:           [u8; 8],
    pub book:          [u8; 32],
    pub maker:         [u8; 32],
    pub pool:          [u8; 32],
    pub new_sequence:  u64,
    pub num_quotes:    u64,
}
const _: () = assert!(core::mem::size_of::<BookUpdateEvent>() == 8 + 32*3 + 8*2);

#[repr(C)]
pub struct PoolCreatedEvent {
    pub tag:         [u8; 8],
    pub pool:        [u8; 32],
    pub mint_a:      [u8; 32],
    pub mint_b:      [u8; 32],
    pub admin:       [u8; 32],
    pub price_scale: u64,
    pub fee_bps:     u64,
}
const _: () = assert!(core::mem::size_of::<PoolCreatedEvent>() == 8 + 32*4 + 8*2);
