use super::read_market;
use crate::helpers::assertions::{check_pubkey, check_signer, check_writable};
use crate::helpers::event::{book_event, check_event_accounts, emit};
use crate::helpers::pda::{create_pda, find_canonical, rent_exempt};
use nock_interface::{
    constants::{
        DEFAULT_EXPIRY_IN_SLOTS, MAKER_BOOK_LEN, MAX_LEVELS, NO_ADDRESS, SEED_MAKER_BOOK, SYSTEM,
    },
    events::tag,
    state::{init, maker_book::book_status, MakerBook, MakerLevel},
    ID,
};
use pinocchio::{
    account::AccountView,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

/// Allocates the signing maker's book on a market: one per (market, maker).
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [maker, market, book, system_program, event_authority, program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    check_signer(maker)?;
    check_writable(maker)?;
    check_writable(book)?;
    check_pubkey(system_program, &SYSTEM)?;
    check_event_accounts(event_authority, program)?;
    read_market(market, |_| Ok(()))?;

    let bump = find_canonical(
        &[
            SEED_MAKER_BOOK,
            market.address().as_ref(),
            maker.address().as_ref(),
        ],
        book,
    )?;
    let bump_bytes = [bump];
    let seeds = [
        Seed::from(SEED_MAKER_BOOK),
        Seed::from(market.address().as_ref()),
        Seed::from(maker.address().as_ref()),
        Seed::from(&bump_bytes[..]),
    ];
    create_pda(
        maker,
        book,
        rent_exempt(MAKER_BOOK_LEN)?,
        MAKER_BOOK_LEN as u64,
        &ID,
        &[Signer::from(&seeds)],
    )?;

    let slot = Clock::get()?.slot;
    let book_key = book.address().to_bytes();
    let event = {
        let mut book_data = book.try_borrow_mut()?;
        let b = init::<MakerBook>(&mut book_data[..])?;
        b.maker = maker.address().to_bytes();
        b.market = market.address().to_bytes();
        b.delegate = NO_ADDRESS;
        b.mid_price_ticks = 0;
        b.base_free = 0;
        b.quote_free = 0;
        b.fees_quote_lots = 0;
        b.sequence = 0;
        b.last_updated_slot = 0;
        b.expiry_in_slots = DEFAULT_EXPIRY_IN_SLOTS;
        b.delegate_min_price_ticks = 0;
        b.delegate_max_price_ticks = 0;
        b.delegate_max_level_lots = 0;
        b.status = book_status::ACTIVE;
        b.maker_book_bump = bump;
        b.sync_spread_ticks = 0;
        b._pad = [0; 4];
        b.bid_levels = [MakerLevel::EMPTY; MAX_LEVELS];
        b.ask_levels = [MakerLevel::EMPTY; MAX_LEVELS];
        book_event(tag::BOOK_INITIALIZED, book_key, b, b.maker, slot)
    };
    emit(&event, event_authority)
}
