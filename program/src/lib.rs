#![no_std]

#[cfg(not(feature = "no-entrypoint"))]
mod entrypoint;

pub mod helpers;
pub mod processor;
pub use nock_interface as interface;
