use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    constants::MAX_LEVELS,
    error::AmmError,
    instruction::ClearBookData,
    state::{load_mut, MakerBook, MakerLevel},
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
    let new_seq = args.sequence_number;

    check_signer(signer)?;
    check_writable(book)?;
    check_owner(book, &ID)?;

    let mut book_data = book.try_borrow_mut()?;
    let book_state = load_mut::<MakerBook>(&mut book_data[..])?;
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
    Ok(())
}
