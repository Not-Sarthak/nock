mod data;

pub use data::*;

/// Instruction discriminators: the first byte of instruction data.
pub mod disc {
    pub const CREATE_POOL: u8 = 0x00;
    pub const TOGGLE_MARKET_STATE: u8 = 0x01;
    pub const UPDATE_CONFIG: u8 = 0x02;
    pub const COLLECT_FEES: u8 = 0x03;
    pub const PROPOSE_ADMIN: u8 = 0x04;
    pub const ACCEPT_ADMIN: u8 = 0x05;
    pub const INIT_MAKER_BOOK: u8 = 0x06;
    pub const UPDATE_MAKER_BOOK: u8 = 0x07;
    pub const UPDATE_MID_PRICE: u8 = 0x08;
    pub const CLEAR_BOOK: u8 = 0x09;
    pub const SET_BOOK_DELEGATE: u8 = 0x0a;
    pub const MAKER_DEPOSIT: u8 = 0x0b;
    pub const MAKER_WITHDRAW: u8 = 0x0c;
    pub const ASYNC_SWAP_CREATE: u8 = 0x0d;
    pub const ASYNC_SWAP_EXECUTE: u8 = 0x0e;
    pub const ASYNC_SWAP_CANCEL: u8 = 0x0f;
    pub const SYNC_SWAP: u8 = 0x10;
    pub const MAKER_HEARTBEAT: u8 = 0x11;
    pub const MAKER_SET_SEQUENCE: u8 = 0x12;
    pub const SET_BOOK_HALT: u8 = 0x13;
    pub const MAKER_RESET_SEQUENCE: u8 = 0x14;
    pub const UPDATE_SYNC_SPREAD: u8 = 0x1a;
    pub const UPDATE_EXPIRY_IN_SLOTS: u8 = 0x1e;
    pub const CLOSE_MAKER_BOOK: u8 = 0x1f;
    /// Self-CPI that carries an event; only the event-authority PDA can sign it.
    pub const EMIT_EVENT: u8 = 0xff;
}
