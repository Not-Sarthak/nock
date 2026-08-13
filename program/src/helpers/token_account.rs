use crate::helpers::assertions::check_owner;
use nock_interface::constants::{MINT_DECIMALS_OFFSET, MINT_MIN_LEN, SPL_TOKEN};
use pinocchio::{account::AccountView, error::ProgramError};

pub const TOKEN_ACCOUNT_MIN_LEN: usize = 165;

#[inline]
pub fn mint_and_owner(acc: &AccountView) -> Result<([u8; 32], [u8; 32]), ProgramError> {
    check_owner(acc, &SPL_TOKEN)?;
    let data = acc.try_borrow()?;
    if data.len() < TOKEN_ACCOUNT_MIN_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    let mut mint = [0u8; 32];
    let mut owner = [0u8; 32];
    mint.copy_from_slice(&data[0..32]);
    owner.copy_from_slice(&data[32..64]);
    Ok((mint, owner))
}

#[inline]
pub fn mint_decimals(acc: &AccountView) -> Result<u8, ProgramError> {
    check_owner(acc, &SPL_TOKEN)?;
    let data = acc.try_borrow()?;
    if data.len() < MINT_MIN_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(data[MINT_DECIMALS_OFFSET])
}
