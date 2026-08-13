use pinocchio::{
    account::AccountView, error::ProgramError,
    sysvars::{clock::Clock, Sysvar}, ProgramResult,
};
use nock_interface::{
    error::AmmError,
    state::{load_mut, MakerBook},
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::unsafe_eq::eq_addr;

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [signer, book, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let new_seq = u64::from_le_bytes(data[0..8].try_into().unwrap());

    check_signer(signer)?;
    check_writable(book)?;
    check_owner(book, &ID)?;

    let slot = Clock::get()?.slot;

    let mut book_data = book.try_borrow_mut()?;
    let b = load_mut::<MakerBook>(&mut book_data[..])?;
    let signer_bytes = signer.address().as_ref();
    if !eq_addr(signer_bytes, &b.maker) && !eq_addr(signer_bytes, &b.delegate) {
        return Err(AmmError::UnauthorizedMaker.into());
    }
    if !b.try_bump_sequence(new_seq) {
        return Err(AmmError::StaleSequence.into());
    }
    b.last_updated_slot = slot;
    Ok(())
}
