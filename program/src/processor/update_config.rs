use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    constants::MAX_FEE_PPM,
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
    if data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let maker_fee_ppm = i32::from_le_bytes(data[0..4].try_into().unwrap());
    let taker_fee_ppm = i32::from_le_bytes(data[4..8].try_into().unwrap());
    if maker_fee_ppm.abs() > MAX_FEE_PPM || taker_fee_ppm.abs() > MAX_FEE_PPM {
        return Err(AmmError::InvalidMarketConfig.into());
    }

    check_signer(admin)?;
    check_writable(market)?;
    check_owner(market, &ID)?;

    let mut market_data = market.try_borrow_mut()?;
    let market_state = load_mut::<Market>(&mut market_data[..])?;
    if !eq_addr(admin.address().as_ref(), &market_state.admin) {
        return Err(AmmError::UnauthorizedMaker.into());
    }
    market_state.maker_fee_ppm = maker_fee_ppm;
    market_state.taker_fee_ppm = taker_fee_ppm;
    Ok(())
}
