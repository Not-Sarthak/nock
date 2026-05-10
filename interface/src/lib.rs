#![no_std]

pub mod constants;
pub mod error;
pub mod events;
pub mod instruction;
pub mod state;

pub mod id {
    solana_address::declare_id!("DpMJkoYXVXLRxrPXdaDQgYS5zrGHQe6d4TP6hYM1fpKF");
}

pub use id::ID;
