use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    error::AmmError,
    instruction::UpdateExpiryInSlotsData,
    state::{load_mut, MakerBook},
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::unsafe_eq::eq_addr;

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [maker, book, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if data.len() < core::mem::size_of::<UpdateExpiryInSlotsData>() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let args: &UpdateExpiryInSlotsData = unsafe { &*(data.as_ptr() as *const UpdateExpiryInSlotsData) };
    let expiry = args.expiry_in_slots;

    check_signer(maker)?;
    check_writable(book)?;
    check_owner(book, &ID)?;

    let book_state = load_mut::<MakerBook>(unsafe { book.borrow_unchecked_mut() })?;
    if !eq_addr(maker.address().as_ref(), &book_state.maker) {
        return Err(AmmError::UnauthorizedMaker.into());
    }
    book_state.expiry_in_slots = expiry;
    Ok(())
}
