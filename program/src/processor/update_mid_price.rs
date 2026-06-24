use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    error::AmmError,
    instruction::UpdateMidPriceData,
    state::{load_mut, MakerBook},
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::unsafe_eq::eq_addr;

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [signer, book, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if data.len() < core::mem::size_of::<UpdateMidPriceData>() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let args: &UpdateMidPriceData = unsafe { &*(data.as_ptr() as *const UpdateMidPriceData) };
    let new_seq = args.sequence_number;
    let new_mid = args.new_mid_ticks;

    check_signer(signer)?;
    check_writable(book)?;
    check_owner(book, &ID)?;

    let book_state = load_mut::<MakerBook>(unsafe { book.borrow_unchecked_mut() })?;
    let signer_bytes = signer.address().as_ref();
    if !eq_addr(signer_bytes, &book_state.maker) && !eq_addr(signer_bytes, &book_state.delegate) {
        return Err(AmmError::UnauthorizedMaker.into());
    }
    if !book_state.try_bump_sequence(new_seq) {
        return Err(AmmError::StaleSequence.into());
    }
    book_state.mid_price_ticks = new_mid;
    Ok(())
}
