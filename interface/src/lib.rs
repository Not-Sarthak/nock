#![no_std]

pub mod constants;
pub mod error;
pub mod events;
pub mod instruction;
pub mod math;
pub mod state;

pub mod id {
    solana_address::declare_id!("5rV6P9piXvRdVfhEVu9oJRHai7RS5iAJ1wuTNmeMCYhB");
}

pub use id::ID;
