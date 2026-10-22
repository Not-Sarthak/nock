use nock_interface::instruction::disc;
use pinocchio::{
    account::AccountView, error::ProgramError, no_allocator, nostd_panic_handler,
    program_entrypoint, Address, ProgramResult,
};

use crate::{helpers::event, processor};

program_entrypoint!(process_instruction);
no_allocator!();
nostd_panic_handler!();

#[inline(always)]
pub fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let (tag, data) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;

    match *tag {
        disc::CREATE_POOL => processor::create_pool::process(accounts, data),
        disc::TOGGLE_MARKET_STATE => processor::toggle_market_state::process(accounts, data),
        disc::UPDATE_CONFIG => processor::update_config::process(accounts, data),
        disc::COLLECT_FEES => processor::collect_fees::process(accounts, data),
        disc::PROPOSE_ADMIN => processor::rotate_admin::process_propose(accounts, data),
        disc::ACCEPT_ADMIN => processor::rotate_admin::process_accept(accounts, data),
        disc::INIT_MAKER_BOOK => processor::init_maker_book::process(accounts, data),
        disc::UPDATE_MAKER_BOOK => processor::update_maker_book::process(accounts, data),
        disc::UPDATE_MID_PRICE => processor::update_mid_price::process(accounts, data),
        disc::CLEAR_BOOK => processor::clear_book::process(accounts, data),
        disc::SET_BOOK_DELEGATE => processor::set_book_delegate::process(accounts, data),
        disc::MAKER_DEPOSIT => processor::maker_deposit_funds::process(accounts, data),
        disc::MAKER_WITHDRAW => processor::maker_withdraw_funds::process(accounts, data),
        disc::ASYNC_SWAP_CREATE => processor::async_swap::process_create(accounts, data),
        disc::ASYNC_SWAP_EXECUTE => processor::async_swap::process_execute(accounts, data),
        disc::ASYNC_SWAP_CANCEL => processor::async_swap::process_cancel(accounts, data),
        disc::SYNC_SWAP => processor::sync_swap::process(accounts, data),
        disc::MAKER_HEARTBEAT => processor::maker_heartbeat::process(accounts, data),
        disc::MAKER_SET_SEQUENCE => processor::maker_set_sequence::process_set(accounts, data),
        disc::SET_BOOK_HALT => processor::set_book_halt::process(accounts, data),
        disc::MAKER_RESET_SEQUENCE => processor::maker_set_sequence::process_reset(accounts, data),
        disc::UPDATE_SYNC_SPREAD => processor::update_sync_spread::process(accounts, data),
        disc::UPDATE_EXPIRY_IN_SLOTS => processor::update_expiry_in_slots::process(accounts, data),
        disc::CLOSE_MAKER_BOOK => processor::close_maker_book::process(accounts, data),
        disc::EMIT_EVENT => event::process_emit(accounts),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
