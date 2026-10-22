use nock_interface::{
    constants::{SPL_TOKEN, TOKEN_2022},
    error::NockError,
};
use pinocchio::{account::AccountView, error::ProgramError, Address, ProgramResult};

#[inline(always)]
pub fn check_signer(account: &AccountView) -> ProgramResult {
    if !account.is_signer() {
        Err(ProgramError::MissingRequiredSignature)
    } else {
        Ok(())
    }
}

#[inline(always)]
pub fn check_writable(account: &AccountView) -> ProgramResult {
    if !account.is_writable() {
        Err(ProgramError::InvalidArgument)
    } else {
        Ok(())
    }
}

#[inline(always)]
pub fn check_owner(account: &AccountView, expected: &Address) -> ProgramResult {
    if account.owner() != expected {
        Err(ProgramError::IllegalOwner)
    } else {
        Ok(())
    }
}

#[inline(always)]
pub fn check_pubkey(account: &AccountView, expected: &Address) -> ProgramResult {
    if account.address() != expected {
        Err(ProgramError::InvalidArgument)
    } else {
        Ok(())
    }
}

/// Legacy SPL Token only; Token-2022 gets its own error so clients see why.
#[inline(always)]
pub fn check_token_program(account: &AccountView) -> ProgramResult {
    if account.address() == &SPL_TOKEN {
        Ok(())
    } else if account.address() == &TOKEN_2022 {
        Err(NockError::Token2022Unsupported.into())
    } else {
        Err(NockError::InvalidTokenProgram.into())
    }
}
