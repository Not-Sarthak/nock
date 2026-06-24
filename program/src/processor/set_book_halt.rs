use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    error::AmmError,
    state::{load_mut, maker_book::book_status, MakerBook},
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::unsafe_eq::eq_addr;

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [signer, book, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let halted = data[0] != 0;

    check_signer(signer)?;
    check_writable(book)?;
    check_owner(book, &ID)?;

    let b = load_mut::<MakerBook>(unsafe { book.borrow_unchecked_mut() })?;
    let signer_bytes = signer.address().as_ref();
    if !eq_addr(signer_bytes, &b.maker) && !eq_addr(signer_bytes, &b.delegate) {
        return Err(AmmError::UnauthorizedMaker.into());
    }
    b.status = if halted { book_status::HALTED } else { book_status::ACTIVE };
    Ok(())
}
