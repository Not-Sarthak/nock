use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    constants::{MAKER_BOOK_LEN, MAKER_BOOK_TAG, MARKET_LEN, MARKET_TAG},
    error::AmmError,
    instruction::UpdateSyncSpreadData,
    state::MakerBook,
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
    let new_spread = { let s = args.sync_spread_ticks; s };

    check_signer(signer)?;
    check_writable(book)?;
    check_owner(book, &ID)?;
    check_owner(market, &ID)?;
    {
        let m = unsafe { market.borrow_unchecked() };
        if m.len() != MARKET_LEN || m[0..8] != MARKET_TAG {
            return Err(AmmError::InvalidAccountTag.into());
        }
    }

    let book_data: &mut [u8] = unsafe { book.borrow_unchecked_mut() };
    if book_data.len() != MAKER_BOOK_LEN || book_data[0..8] != MAKER_BOOK_TAG {
        return Err(AmmError::InvalidAccountTag.into());
    }
    let book_state = unsafe { &mut *(book_data.as_mut_ptr() as *mut MakerBook) };
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
