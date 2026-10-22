use pinocchio::error::ProgramError;

pub mod async_swap;
pub mod maker_book;
pub mod market;

pub use async_swap::AsyncSwap;
pub use maker_book::{MakerBook, MakerLevel, Role};
pub use market::Market;

pub trait Tagged {
    const TAG: [u8; 8];
    const LEN: usize;
}

/// Tag width at the start of every account.
const TAG_LEN: usize = 8;

#[inline]
pub fn load<T: Tagged>(bytes: &[u8]) -> Result<&T, ProgramError> {
    if bytes.len() != T::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    if bytes[..TAG_LEN] != T::TAG {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(unsafe { &*(bytes.as_ptr() as *const T) })
}

#[inline]
pub fn load_mut<T: Tagged>(bytes: &mut [u8]) -> Result<&mut T, ProgramError> {
    if bytes.len() != T::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    if bytes[..TAG_LEN] != T::TAG {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(unsafe { &mut *(bytes.as_mut_ptr() as *mut T) })
}

#[inline]
pub fn init<T: Tagged>(bytes: &mut [u8]) -> Result<&mut T, ProgramError> {
    if bytes.len() != T::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    if bytes[..TAG_LEN] != [0u8; TAG_LEN] {
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    bytes[..TAG_LEN].copy_from_slice(&T::TAG);
    Ok(unsafe { &mut *(bytes.as_mut_ptr() as *mut T) })
}
