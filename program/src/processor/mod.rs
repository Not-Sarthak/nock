pub mod create_pool;
pub mod toggle_market_state;
pub mod update_config;
pub mod async_swap;
pub mod init_maker_book;
pub mod update_maker_book;
pub mod update_mid_price;
pub mod clear_book;
pub mod set_book_delegate;
pub mod maker_deposit_funds;
pub mod maker_withdraw_funds;
pub mod sync_swap;
pub mod maker_heartbeat;
pub mod maker_set_sequence;
pub mod update_sync_spread;
pub mod update_expiry_in_slots;
pub mod close_maker_book;

pub mod token_programs {
    use nock_interface::constants::SPL_TOKEN;
    use pinocchio::Address;

    #[inline(always)]
    pub fn is_token_program(p: &Address) -> bool {
        p == &SPL_TOKEN
    }
}

#[inline(always)]
pub const fn rent_exempt(n: usize) -> u64 {
    (128 + n as u64) * 6960
}
