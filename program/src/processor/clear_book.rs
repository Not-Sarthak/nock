use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    constants::{MAKER_BOOK_LEN, MAKER_BOOK_TAG, MAX_LEVELS},
    error::AmmError,
    instruction::ClearBookData,
    state::{MakerBook, MakerLevel},
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::unsafe_eq::eq_addr;

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [signer, book, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if data.len() < core::mem::size_of::<ClearBookData>() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let args: &ClearBookData = unsafe { &*(data.as_ptr() as *const ClearBookData) };
    let new_seq = { let s = args.sequence_number; s };

    check_signer(signer)?;
    check_writable(book)?;
    check_owner(book, &ID)?;

    let book_data: &mut [u8] = unsafe { book.borrow_unchecked_mut() };
    if book_data.len() != MAKER_BOOK_LEN || book_data[0..8] != MAKER_BOOK_TAG {
        return Err(AmmError::InvalidAccountTag.into());
    }
    let book_state = unsafe { &mut *(book_data.as_mut_ptr() as *mut MakerBook) };
    let signer_bytes = signer.address().as_ref();
    if !eq_addr(signer_bytes, &book_state.maker) && !eq_addr(signer_bytes, &book_state.delegate) {
        return Err(AmmError::UnauthorizedMaker.into());
    }
    if !book_state.try_bump_sequence(new_seq) {
        return Err(AmmError::StaleSequence.into());
    }
    let zero = MakerLevel { size_in_base_lots: 0, price_offset_ticks: 0 };
    for i in 0..MAX_LEVELS {
        book_state.bid_levels[i] = zero;
        book_state.ask_levels[i] = zero;
    }
    book_state.total_bid_base_lots = 0;
    Ok(())
}
