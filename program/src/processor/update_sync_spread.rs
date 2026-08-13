use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    error::AmmError,
    instruction::UpdateSyncSpreadData,
    state::{load_mut, MakerBook},
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::unsafe_eq::eq_addr;

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [signer, book, market, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if data.len() < core::mem::size_of::<UpdateSyncSpreadData>() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let args: &UpdateSyncSpreadData = unsafe { &*(data.as_ptr() as *const UpdateSyncSpreadData) };
    let new_spread = args.sync_spread_ticks;

    check_signer(signer)?;
    check_writable(book)?;
    check_owner(book, &ID)?;
    check_owner(market, &ID)?;

    let mut book_data = book.try_borrow_mut()?;
    let book_state = load_mut::<MakerBook>(&mut book_data[..])?;
    let signer_bytes = signer.address().as_ref();
    if !eq_addr(signer_bytes, &book_state.maker) && !eq_addr(signer_bytes, &book_state.delegate) {
        return Err(AmmError::UnauthorizedMaker.into());
    }
    if !eq_addr(market.address().as_ref(), &book_state.market) {
        return Err(AmmError::InvalidAccountTag.into());
    }
    book_state.sync_spread_ticks = new_spread;
    Ok(())
}
