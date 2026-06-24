use pinocchio::{
    account::AccountView, error::ProgramError,
    sysvars::{clock::Clock, Sysvar}, ProgramResult,
};
use nock_interface::{
    constants::{BOOK_UPDATE_EVENT_TAG, MAX_LEVELS, MAX_WIRE_LEVELS},
    error::AmmError,
    events::BookUpdateEvent,
    instruction::UpdateMakerBookData,
    state::{load_mut, MakerBook, MakerLevel},
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::unsafe_eq::eq_addr;

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [signer, book, market, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if data.len() < core::mem::size_of::<UpdateMakerBookData>() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let args: &UpdateMakerBookData = unsafe { &*(data.as_ptr() as *const UpdateMakerBookData) };
    let new_seq    = args.header.sequence_number;
    let mid_ticks  = args.header.mid_price_ticks;
    let num_bids   = args.header.num_bids as usize;
    let num_asks   = args.header.num_asks as usize;
    if num_bids > MAX_WIRE_LEVELS || num_asks > MAX_WIRE_LEVELS {
        return Err(ProgramError::InvalidInstructionData);
    }

    check_signer(signer)?;
    check_writable(book)?;
    check_owner(book, &ID)?;
    check_owner(market, &ID)?;

    let slot = Clock::get()?.slot;

    let mut book_data = book.try_borrow_mut()?;
    let book_state = load_mut::<MakerBook>(&mut book_data[..])?;

    let signer_bytes = signer.address().as_ref();
    if !eq_addr(signer_bytes, &book_state.maker) && !eq_addr(signer_bytes, &book_state.delegate) {
        return Err(AmmError::UnauthorizedMaker.into());
    }
    if !eq_addr(market.address().as_ref(), &book_state.market) {
        return Err(AmmError::InvalidAccountTag.into());
    }
    if !book_state.try_bump_sequence(new_seq) {
        return Err(AmmError::StaleSequence.into());
    }

    book_state.mid_price_ticks   = mid_ticks;
    book_state.last_updated_slot = slot;

    let bid_src = unsafe {
        (&raw const (*(data.as_ptr() as *const UpdateMakerBookData)).bid_levels) as *const MakerLevel
    };
    let ask_src = unsafe {
        (&raw const (*(data.as_ptr() as *const UpdateMakerBookData)).ask_levels) as *const MakerLevel
    };
    for i in 0..num_bids {
        book_state.bid_levels[i] = unsafe { core::ptr::read_unaligned(bid_src.add(i)) };
    }
    for i in num_bids..MAX_LEVELS {
        book_state.bid_levels[i] = MakerLevel { size_in_base_lots: 0, price_offset_ticks: 0 };
    }
    for i in 0..num_asks {
        book_state.ask_levels[i] = unsafe { core::ptr::read_unaligned(ask_src.add(i)) };
    }
    for i in num_asks..MAX_LEVELS {
        book_state.ask_levels[i] = MakerLevel { size_in_base_lots: 0, price_offset_ticks: 0 };
    }
    drop(book_data);

    let mut book_pubkey:   [u8; 32] = [0; 32]; book_pubkey.copy_from_slice(book.address().as_ref());
    let mut signer_pubkey: [u8; 32] = [0; 32]; signer_pubkey.copy_from_slice(signer.address().as_ref());
    let mut market_pubkey: [u8; 32] = [0; 32]; market_pubkey.copy_from_slice(market.address().as_ref());
    let event = BookUpdateEvent {
        tag: BOOK_UPDATE_EVENT_TAG,
        book: book_pubkey,
        maker: signer_pubkey,
        pool: market_pubkey,
        new_sequence: new_seq,
        num_quotes: (num_bids + num_asks) as u64,
    };
    crate::helpers::event::emit(&event);
    Ok(())
}
