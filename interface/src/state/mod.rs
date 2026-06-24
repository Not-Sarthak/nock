use pinocchio::error::ProgramError;

use crate::error::AmmError;

pub mod market;
pub mod maker_book;
pub mod async_swap;

pub use market::Market;
pub use maker_book::{MakerBook, MakerLevel};
pub use async_swap::AsyncSwap;

pub trait Tagged {
    const TAG: [u8; 8];
    const LEN: usize;
}

#[inline]
pub fn load<T: Tagged>(bytes: &[u8]) -> Result<&T, ProgramError> {
    if bytes.len() != T::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    if bytes[0..8] != T::TAG {
        return Err(AmmError::InvalidAccountTag.into());
    }
    Ok(unsafe { &*(bytes.as_ptr() as *const T) })
}

#[inline]
pub fn load_mut<T: Tagged>(bytes: &mut [u8]) -> Result<&mut T, ProgramError> {
    if bytes.len() != T::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    if bytes[0..8] != T::TAG {
        return Err(AmmError::InvalidAccountTag.into());
    }
    Ok(unsafe { &mut *(bytes.as_mut_ptr() as *mut T) })
}

#[inline]
pub fn init<T: Tagged>(bytes: &mut [u8]) -> Result<&mut T, ProgramError> {
    if bytes.len() != T::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    if bytes[0..8] != [0u8; 8] {
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    bytes[0..8].copy_from_slice(&T::TAG);
    Ok(unsafe { &mut *(bytes.as_mut_ptr() as *mut T) })
}
