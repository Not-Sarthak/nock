//! Events travel as the data of a self-CPI (`disc::EMIT_EVENT`) signed by the
//! event-authority PDA. Inner-instruction data is kept in full by the runtime, unlike
//! program logs, which are cut off once a transaction's log budget runs out.
//!
//! Every event starts with an 8-byte tag. Structs are `repr(C)` with no padding, which
//! the `event!` macro proves at compile time, so the raw bytes are the wire format.

/// Marker for event structs whose bytes are fully initialized.
///
/// # Safety
/// Implementors must be `repr(C)` with no padding bytes.
pub unsafe trait Event: Sized {}

macro_rules! event {
    ($(#[$m:meta])* pub struct $name:ident { $(pub $f:ident : $t:ty,)* }) => {
        $(#[$m])*
        #[repr(C)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $name { $(pub $f: $t,)* }
        const _: () = assert!(
            core::mem::size_of::<$name>() == 0 $(+ core::mem::size_of::<$t>())*,
            "event struct has padding",
        );
        const _: () = assert!(core::mem::size_of::<$name>() <= crate::constants::MAX_EVENT_LEN);
        unsafe impl Event for $name {}
    };
}

pub mod tag {
    pub const POOL_CREATED: [u8; 8] = *b"NOCKPC02";
    pub const CONFIG_UPDATED: [u8; 8] = *b"NOCKCFG1";
    pub const MARKET_STATUS: [u8; 8] = *b"NOCKMST1";
    pub const ADMIN_PROPOSED: [u8; 8] = *b"NOCKADP1";
    pub const ADMIN_ACCEPTED: [u8; 8] = *b"NOCKADA1";
    pub const FEES_COLLECTED: [u8; 8] = *b"NOCKFEE1";
    pub const BOOK_INITIALIZED: [u8; 8] = *b"NOCKBKI1";
    pub const BOOK_UPDATED: [u8; 8] = *b"NOCKBKU2";
    pub const MID_UPDATED: [u8; 8] = *b"NOCKMID1";
    pub const BOOK_CLEARED: [u8; 8] = *b"NOCKCLR1";
    pub const HEARTBEAT: [u8; 8] = *b"NOCKHBT1";
    pub const SEQUENCE_SET: [u8; 8] = *b"NOCKSEQ1";
    pub const SEQUENCE_RESET: [u8; 8] = *b"NOCKSQR1";
    pub const HALT_SET: [u8; 8] = *b"NOCKHLT1";
    pub const SPREAD_UPDATED: [u8; 8] = *b"NOCKSPR1";
    pub const EXPIRY_UPDATED: [u8; 8] = *b"NOCKEXP1";
    pub const DELEGATE_SET: [u8; 8] = *b"NOCKDLG1";
    pub const DEPOSIT: [u8; 8] = *b"NOCKDEP1";
    pub const WITHDRAW: [u8; 8] = *b"NOCKWDR1";
    pub const BOOK_CLOSED: [u8; 8] = *b"NOCKBKC1";
    pub const SYNC_FILL: [u8; 8] = *b"NOCKSFL2";
    pub const ASYNC_SWAP_CREATED: [u8; 8] = *b"NOCKAOC2";
    pub const ASYNC_FILL: [u8; 8] = *b"NOCKAFL2";
    pub const ASYNC_CANCEL: [u8; 8] = *b"NOCKAOX2";

    /// Every event tag, for uniqueness checks.
    pub const ALL: [[u8; 8]; 24] = [
        POOL_CREATED,
        CONFIG_UPDATED,
        MARKET_STATUS,
        ADMIN_PROPOSED,
        ADMIN_ACCEPTED,
        FEES_COLLECTED,
        BOOK_INITIALIZED,
        BOOK_UPDATED,
        MID_UPDATED,
        BOOK_CLEARED,
        HEARTBEAT,
        SEQUENCE_SET,
        SEQUENCE_RESET,
        HALT_SET,
        SPREAD_UPDATED,
        EXPIRY_UPDATED,
        DELEGATE_SET,
        DEPOSIT,
        WITHDRAW,
        BOOK_CLOSED,
        SYNC_FILL,
        ASYNC_SWAP_CREATED,
        ASYNC_FILL,
        ASYNC_CANCEL,
    ];
}

event! {
    pub struct PoolCreatedEvent {
        pub tag: [u8; 8],
        pub market: [u8; 32],
        pub base_mint: [u8; 32],
        pub quote_mint: [u8; 32],
        pub admin: [u8; 32],
        pub tick_size_in_quote_atoms_per_base_unit: u64,
        pub base_atoms_per_base_lot: u64,
        pub quote_atoms_per_quote_lot: u64,
        pub maker_fee_ppm: u32,
        pub taker_fee_ppm: u32,
        pub min_async_delay_slots: u16,
        pub max_async_delay_slots: u16,
        pub _pad: [u8; 4],
    }
}

event! {
    pub struct ConfigUpdatedEvent {
        pub tag: [u8; 8],
        pub market: [u8; 32],
        pub admin: [u8; 32],
        pub maker_fee_ppm: u32,
        pub taker_fee_ppm: u32,
    }
}

event! {
    pub struct MarketStatusEvent {
        pub tag: [u8; 8],
        pub market: [u8; 32],
        pub admin: [u8; 32],
        pub slot: u64,
        pub paused: u8,
        pub _pad: [u8; 7],
    }
}

event! {
    /// Tagged `ADMIN_PROPOSED` or `ADMIN_ACCEPTED`.
    pub struct AdminEvent {
        pub tag: [u8; 8],
        pub market: [u8; 32],
        pub admin: [u8; 32],
        pub pending_admin: [u8; 32],
    }
}

event! {
    pub struct FeesCollectedEvent {
        pub tag: [u8; 8],
        pub market: [u8; 32],
        pub book: [u8; 32],
        pub maker: [u8; 32],
        pub destination: [u8; 32],
        pub fee_quote_lots: u64,
        pub fee_quote_atoms: u64,
    }
}

event! {
    /// Snapshot of a book's quoting state after any book-level change. The tag names the
    /// change: init, full update, mid, clear, heartbeat, sequence set or reset, halt,
    /// spread or expiry.
    pub struct BookEvent {
        pub tag: [u8; 8],
        pub book: [u8; 32],
        pub market: [u8; 32],
        pub maker: [u8; 32],
        pub signer: [u8; 32],
        pub sequence: u64,
        pub mid_price_ticks: u64,
        pub expiry_in_slots: u64,
        pub last_updated_slot: u64,
        pub slot: u64,
        pub sync_spread_ticks: u16,
        pub status: u8,
        pub num_bids: u8,
        pub num_asks: u8,
        pub _pad: [u8; 3],
    }
}

event! {
    pub struct DelegateSetEvent {
        pub tag: [u8; 8],
        pub book: [u8; 32],
        pub maker: [u8; 32],
        pub delegate: [u8; 32],
        pub min_price_ticks: u64,
        pub max_price_ticks: u64,
        pub max_level_lots: u64,
    }
}

event! {
    /// Tagged `DEPOSIT` or `WITHDRAW`; balances are after the move.
    pub struct InventoryEvent {
        pub tag: [u8; 8],
        pub book: [u8; 32],
        pub market: [u8; 32],
        pub maker: [u8; 32],
        pub base_lots: u64,
        pub quote_lots: u64,
        pub base_free: u64,
        pub quote_free: u64,
    }
}

event! {
    pub struct BookClosedEvent {
        pub tag: [u8; 8],
        pub book: [u8; 32],
        pub market: [u8; 32],
        pub maker: [u8; 32],
        pub refunded_lamports: u64,
    }
}

event! {
    pub struct SyncFillEvent {
        pub tag: [u8; 8],
        pub market: [u8; 32],
        pub book: [u8; 32],
        pub maker: [u8; 32],
        pub user: [u8; 32],
        pub sequence: u64,
        pub base_lots: u64,
        pub gross_quote_lots: u64,
        pub taker_quote_lots: u64,
        pub taker_fee_lots: u64,
        pub maker_fee_lots: u64,
        pub worst_price_ticks: u64,
        pub slot: u64,
        pub side: u8,
        pub _pad: [u8; 7],
    }
}

event! {
    pub struct AsyncSwapCreatedEvent {
        pub tag: [u8; 8],
        pub async_swap: [u8; 32],
        pub market: [u8; 32],
        pub user: [u8; 32],
        pub maker: [u8; 32],
        pub nonce: u64,
        pub base_lots: u64,
        pub limit_quote_lots: u64,
        pub price_limit_ticks: u64,
        pub expiry_slot: u64,
        pub escrow_atoms: u64,
        pub side: u8,
        pub _pad: [u8; 7],
    }
}

event! {
    pub struct AsyncFillEvent {
        pub tag: [u8; 8],
        pub async_swap: [u8; 32],
        pub market: [u8; 32],
        pub book: [u8; 32],
        pub maker: [u8; 32],
        pub signer: [u8; 32],
        pub user: [u8; 32],
        pub sequence: u64,
        pub base_lots: u64,
        pub gross_quote_lots: u64,
        pub taker_quote_lots: u64,
        pub taker_fee_lots: u64,
        pub maker_fee_lots: u64,
        pub worst_price_ticks: u64,
        pub refunded_atoms: u64,
        pub slot: u64,
        pub side: u8,
        pub _pad: [u8; 7],
    }
}

event! {
    pub struct AsyncCancelEvent {
        pub tag: [u8; 8],
        pub async_swap: [u8; 32],
        pub market: [u8; 32],
        pub user: [u8; 32],
        pub caller: [u8; 32],
        pub refunded_atoms: u64,
        pub slot: u64,
    }
}
