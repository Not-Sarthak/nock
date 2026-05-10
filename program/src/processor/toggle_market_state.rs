use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    constants::{MARKET_LEN, MARKET_TAG},
    error::AmmError,
    state::Market,
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [market, admin, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let paused = data[0] != 0;

    check_signer(admin)?;
    check_writable(market)?;
    check_owner(market, &ID)?;

    let mut market_data = market.try_borrow_mut()?;
    if market_data.len() != MARKET_LEN || market_data[0..8] != MARKET_TAG {
        return Err(AmmError::InvalidAccountTag.into());
    }
    let market_state = unsafe { &mut *(market_data.as_mut_ptr() as *mut Market) };
    if !crate::helpers::unsafe_eq::eq_addr(admin.address().as_ref(), &market_state.admin) {
        return Err(AmmError::UnauthorizedMaker.into());
    }
    market_state.set_paused(paused);
    Ok(())
}
