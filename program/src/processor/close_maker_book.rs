use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    error::AmmError,
    state::{load, MakerBook},
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::pda::close_pda;
use crate::helpers::unsafe_eq::eq_addr;

pub fn process(accounts: &mut [AccountView], _data: &[u8]) -> ProgramResult {
    let [maker, _market, book, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    check_signer(maker)?;
    check_writable(maker)?;
    check_writable(book)?;
    check_owner(book, &ID)?;

    {
        let book_data = book.try_borrow()?;
        let book_state = load::<MakerBook>(&book_data[..])?;
        if !eq_addr(maker.address().as_ref(), &book_state.maker) {
            return Err(AmmError::UnauthorizedMaker.into());
        }
        if book_state.base_free != 0 || book_state.quote_free != 0 {
            return Err(AmmError::InsufficientVaultBalance.into());
        }
    }

    close_pda(book, maker)
}
