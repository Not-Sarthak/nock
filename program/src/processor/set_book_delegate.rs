use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    error::AmmError,
    state::{load_mut, MakerBook},
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::unsafe_eq::{eq_addr, pubkey_copy_32};

pub fn process(accounts: &mut [AccountView], _data: &[u8]) -> ProgramResult {
    let [maker, book, delegate, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    check_signer(maker)?;
    check_writable(book)?;
    check_owner(book, &ID)?;

    let book_state = load_mut::<MakerBook>(unsafe { book.borrow_unchecked_mut() })?;
    if !eq_addr(maker.address().as_ref(), &book_state.maker) {
        return Err(AmmError::UnauthorizedMaker.into());
    }
    unsafe {
        pubkey_copy_32(delegate.address().as_ref().as_ptr(), book_state.delegate.as_mut_ptr());
    }
    Ok(())
}
