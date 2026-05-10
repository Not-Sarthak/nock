use pinocchio::{
    account::AccountView,
    cpi::{Seed, Signer},
    error::ProgramError,
    Address, ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;
use nock_interface::{
    constants::{
        MAKER_BOOK_LEN, MAKER_BOOK_TAG, MARKET_LEN, MARKET_TAG, MAX_LEVELS,
        SEED_MAKER_BOOK, SYSTEM,
    },
    error::AmmError,
    state::{MakerBook, MakerLevel, Market},
    ID,
};
use crate::helpers::assertions::{check_owner, check_pubkey, check_signer, check_writable};

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [maker, market, book, system_program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let bump = data[0];

    check_signer(maker)?;
    check_writable(book)?;
    check_owner(market, &ID)?;
    check_pubkey(system_program, &SYSTEM)?;

    {
        let market_data = unsafe { market.borrow_unchecked() };
        if market_data.len() != MARKET_LEN || market_data[0..8] != MARKET_TAG {
            return Err(AmmError::InvalidAccountTag.into());
        }
    }

    let bump_bytes = [bump];
    let derived = Address::derive_address(
        &[SEED_MAKER_BOOK, market.address().as_ref(), maker.address().as_ref()],
        Some(bump),
        &ID,
    );
    if book.address() != &derived {
        return Err(AmmError::InvalidPdaBump.into());
    }
    let lamports = super::rent_exempt(MAKER_BOOK_LEN);
    let seeds = [
        Seed::from(SEED_MAKER_BOOK),
        Seed::from(market.address().as_ref()),
        Seed::from(maker.address().as_ref()),
        Seed::from(&bump_bytes[..]),
    ];
    let signer = [Signer::from(&seeds)];
    CreateAccount {
        from: maker,
        to: book,
        lamports,
        space: MAKER_BOOK_LEN as u64,
        owner: &ID,
    }
    .invoke_signed(&signer)?;

    let mut market_id: [u8; 32] = [0; 32];
    {
        let market_data = unsafe { market.borrow_unchecked() };
        let market_state = unsafe { &*(market_data.as_ptr() as *const Market) };
        market_id.copy_from_slice(&market_state.market_id);
    }

    let mut book_data = book.try_borrow_mut()?;
    let book_state = unsafe { &mut *(book_data.as_mut_ptr() as *mut MakerBook) };
    book_state.tag = MAKER_BOOK_TAG;
    unsafe {
        crate::helpers::unsafe_eq::pubkey_copy_32(maker.address().as_ref().as_ptr(), book_state.maker.as_mut_ptr());
        crate::helpers::unsafe_eq::pubkey_copy_32(market_id.as_ptr(), book_state.market.as_mut_ptr());
        crate::helpers::unsafe_eq::pubkey_copy_32(maker.address().as_ref().as_ptr(), book_state.delegate.as_mut_ptr());
    }
    book_state.mid_price_ticks              = 0;
    book_state.quote_delta_per_tick         = 0;
    book_state.min_mid_price_ticks          = 0;
    book_state.quote_locked                 = 0;
    book_state.quote_free                   = 0;
    book_state.base_locked                  = 0;
    book_state.base_free                    = 0;
    book_state.status                       = 0;
    book_state.maker_book_bump              = bump;
    book_state.sync_spread_ticks            = 0;
    book_state._status_padding              = [0; 4];
    book_state.last_updated_sequence_number = 0;
    book_state.total_bid_base_lots          = 0;
    book_state.tick_conversion_num          = 1;
    book_state.tick_conversion_den          = 1;
    book_state.bid_levels                   = [MakerLevel { size_in_base_lots: 0, price_offset_ticks: 0 }; MAX_LEVELS];
    book_state.ask_levels                   = [MakerLevel { size_in_base_lots: 0, price_offset_ticks: 0 }; MAX_LEVELS];
    book_state.last_updated_slot            = 0;
    book_state.expiry_in_slots              = 0;
    book_state._reserved                    = [0u64; 6];
    Ok(())
}
