#![no_std]

pub mod constants;
pub mod error;
pub mod events;
pub mod instruction;
pub mod state;

pub mod id {
    solana_address::declare_id!("7VAMrYiGYS8orio29rF5U4WPaNRz8ymBEtoTbwNjJg2z");
}

pub use id::ID;
