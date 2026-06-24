use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    error::AmmError,
    state::{load, MakerBook},
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::unsafe_eq::eq_addr;

pub fn process(accounts: &mut [AccountView], _data: &[u8]) -> ProgramResult {
    let [maker, _market, book, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    check_signer(maker)?;
    check_writable(book)?;
    check_writable(maker)?;
    check_owner(book, &ID)?;

    {
        let book_state = load::<MakerBook>(unsafe { book.borrow_unchecked() })?;
        if !eq_addr(maker.address().as_ref(), &book_state.maker) {
            return Err(AmmError::UnauthorizedMaker.into());
        }
        if book_state.base_locked != 0 || book_state.quote_locked != 0
            || book_state.base_free  != 0 || book_state.quote_free  != 0
        {
            return Err(AmmError::InvalidMarketConfig.into());
        }
    }

    let recovered = book.lamports();
    maker.set_lamports(maker.lamports().saturating_add(recovered));
    book.set_lamports(0);
    book.close()?;
    Ok(())
}
