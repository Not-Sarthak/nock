use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    constants::{MAKER_BOOK_LEN, MAKER_BOOK_TAG},
    error::AmmError,
    instruction::UpdateExpiryInSlotsData,
    state::MakerBook,
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
    let expiry = { let e = args.expiry_in_slots; e };

    check_signer(maker)?;
    check_writable(book)?;
    check_owner(book, &ID)?;

    let book_data: &mut [u8] = unsafe { book.borrow_unchecked_mut() };
    if book_data.len() != MAKER_BOOK_LEN || book_data[0..8] != MAKER_BOOK_TAG {
        return Err(AmmError::InvalidAccountTag.into());
    }
    let book_state = unsafe { &mut *(book_data.as_mut_ptr() as *mut MakerBook) };
    if !eq_addr(maker.address().as_ref(), &book_state.maker) {
        return Err(AmmError::UnauthorizedMaker.into());
    }
    book_state.expiry_in_slots = expiry;
    Ok(())
}
