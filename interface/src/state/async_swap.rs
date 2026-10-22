use super::Tagged;
use crate::constants::{ASYNC_SWAP_LEN, ASYNC_SWAP_TAG};

/// A taker's deferred swap. Same terms as a SyncSwap, filled later by a maker against
/// that maker's own book.
///
/// Sell base: `base_lots` sit in escrow; the user receives at least `limit_quote_lots`
/// net of fees. Buy base: `limit_quote_lots` (fee included) sit in escrow; the user
/// receives exactly `base_lots` and any unspent quote comes back.
#[repr(C)]
pub struct AsyncSwap {
    pub tag: [u8; 8],
    pub user: [u8; 32],
    pub market: [u8; 32],
    /// Only this maker's book may fill; `NO_ADDRESS` lets any book on the market fill.
    pub maker: [u8; 32],
    pub escrow: [u8; 32],
    pub nonce: u64,
    pub base_lots: u64,
    pub limit_quote_lots: u64,
    /// Worst level price the fill may touch: a floor on a sell, a ceiling on a buy.
    pub price_limit_ticks: u64,
    /// First slot at which the swap can no longer execute.
    pub expiry_slot: u64,
    pub side: u8,
    pub bump: u8,
    pub _pad: [u8; 6],
}
const _: () = assert!(core::mem::size_of::<AsyncSwap>() == ASYNC_SWAP_LEN);

impl Tagged for AsyncSwap {
    const TAG: [u8; 8] = ASYNC_SWAP_TAG;
    const LEN: usize = ASYNC_SWAP_LEN;
}
