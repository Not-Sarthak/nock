use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    constants::MAX_QUOTE_AGE_SLOTS,
    error::AmmError,
    instruction::UpdateExpiryInSlotsData,
    state::{load_mut, MakerBook},
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::unsafe_eq::eq_addr;

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [signer, book, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if data.len() < core::mem::size_of::<UpdateExpiryInSlotsData>() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let args: &UpdateExpiryInSlotsData = unsafe { &*(data.as_ptr() as *const UpdateExpiryInSlotsData) };
    let expiry = args.expiry_in_slots;
    if expiry > MAX_QUOTE_AGE_SLOTS {
        return Err(AmmError::InvalidMarketConfig.into());
    }

    check_signer(signer)?;
    check_writable(book)?;
    check_owner(book, &ID)?;

    let mut book_data = book.try_borrow_mut()?;
    let book_state = load_mut::<MakerBook>(&mut book_data[..])?;
    let signer_bytes = signer.address().as_ref();
    if !eq_addr(signer_bytes, &book_state.maker) && !eq_addr(signer_bytes, &book_state.delegate) {
        return Err(AmmError::UnauthorizedMaker.into());
    }
    book_state.expiry_in_slots = expiry;
    Ok(())
}
