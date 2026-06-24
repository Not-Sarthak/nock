use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    error::AmmError,
    state::{load_mut, Market},
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::unsafe_eq::eq_addr;

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
    let market_state = load_mut::<Market>(&mut market_data[..])?;
    if !eq_addr(admin.address().as_ref(), &market_state.admin) {
        return Err(AmmError::UnauthorizedMaker.into());
    }
    market_state.set_paused(paused);
    Ok(())
}
