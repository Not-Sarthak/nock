use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    constants::{MAKER_BOOK_LEN, MAKER_BOOK_TAG},
    error::AmmError,
    state::MakerBook,
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::unsafe_eq::eq_addr;

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [signer, book, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let new_seq = u64::from_le_bytes(data[0..8].try_into().unwrap());

    check_signer(signer)?;
    check_writable(book)?;
    check_owner(book, &ID)?;

    let book_data: &mut [u8] = unsafe { book.borrow_unchecked_mut() };
    if book_data.len() != MAKER_BOOK_LEN || book_data[0..8] != MAKER_BOOK_TAG {
        return Err(AmmError::InvalidAccountTag.into());
    }
    let b = unsafe { &mut *(book_data.as_mut_ptr() as *mut MakerBook) };
    let signer_bytes = signer.address().as_ref();
    if !eq_addr(signer_bytes, &b.maker) && !eq_addr(signer_bytes, &b.delegate) {
        return Err(AmmError::UnauthorizedMaker.into());
    }
    if !b.try_bump_sequence(new_seq) {
        return Err(AmmError::StaleSequence.into());
    }
    Ok(())
}
