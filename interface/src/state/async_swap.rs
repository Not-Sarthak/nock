use super::Tagged;
use crate::constants::{ASYNC_SWAP_LEN, ASYNC_SWAP_TAG};

#[repr(C)]
pub struct AsyncSwap {
    pub tag:             [u8; 8],
    pub user:            [u8; 32],
    pub market:          [u8; 32],
    pub _reserved:       [u8; 32],
    pub user_src_escrow: [u8; 32],
    pub amount_in_atoms: u64,
    pub min_out_atoms:   u64,
    pub expiry_slot:     u64,
    pub side:            u8,
    pub status:          u8,
    pub bump:            u8,
    pub _pad:            [u8; 5],
}
const _: () = assert!(core::mem::size_of::<AsyncSwap>() == ASYNC_SWAP_LEN);

impl Tagged for AsyncSwap {
    const TAG: [u8; 8] = ASYNC_SWAP_TAG;
    const LEN: usize = ASYNC_SWAP_LEN;
}

pub mod status {
    pub const PENDING: u8 = 0;
}
