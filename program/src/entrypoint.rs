use pinocchio::{
    account::AccountView, error::ProgramError, no_allocator, nostd_panic_handler,
    program_entrypoint, Address, ProgramResult,
};

use crate::processor;

program_entrypoint!(process_instruction);
no_allocator!();
nostd_panic_handler!();

#[inline(always)]
pub fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let (disc, data) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;

    match *disc {
        0x00 => processor::create_pool::process(accounts, data),
        0x01 => processor::toggle_market_state::process(accounts, data),
        0x02 => processor::update_config::process(accounts, data),
        0x03 => processor::collect_fees::process(accounts, data),
        0x06 => processor::init_maker_book::process(accounts, data),
        0x07 => processor::update_maker_book::process(accounts, data),
        0x08 => processor::update_mid_price::process(accounts, data),
        0x09 => processor::clear_book::process(accounts, data),
        0x0a => processor::set_book_delegate::process(accounts, data),
        0x0b => processor::maker_deposit_funds::process(accounts, data),
        0x0c => processor::maker_withdraw_funds::process(accounts, data),
        0x0d => processor::async_swap::process_create(accounts, data),
        0x0e => processor::async_swap::process_execute(accounts, data),
        0x0f => processor::async_swap::process_cancel(accounts, data),
        0x10 => processor::sync_swap::process(accounts, data),
        0x11 => processor::maker_heartbeat::process(accounts, data),
        0x12 => processor::maker_set_sequence::process(accounts, data),
        0x13 => processor::set_book_halt::process(accounts, data),
        0x1a => processor::update_sync_spread::process(accounts, data),
        0x1e => processor::update_expiry_in_slots::process(accounts, data),
        0x1f => processor::close_maker_book::process(accounts, data),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
