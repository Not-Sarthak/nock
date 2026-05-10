use pinocchio::{
    account::AccountView, error::ProgramError, ProgramResult,
};
use nock_interface::{
    constants::{MAKER_BOOK_LEN, MAKER_BOOK_TAG, MARKET_LEN, MARKET_TAG},
    error::AmmError,
    instruction::MakerDepositData,
    state::{MakerBook, Market},
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::unsafe_eq::eq_addr;
use super::token_programs;

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [
        market,
        book,
        maker,
        base_mint,
        quote_mint,
        base_vault,
        quote_vault,
        maker_base_ata,
        maker_quote_ata,
        base_token_program,
        quote_token_program,
        ..
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if data.len() < core::mem::size_of::<MakerDepositData>() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let args: &MakerDepositData = unsafe { &*(data.as_ptr() as *const MakerDepositData) };
    let base_lots  = { let b = args.base_lots; b };
    let quote_lots = { let q = args.quote_lots; q };
    if base_lots == 0 && quote_lots == 0 {
        return Err(AmmError::AmountInZero.into());
    }

    check_signer(maker)?;
    check_writable(book)?;
    check_writable(base_vault)?;
    check_writable(quote_vault)?;
    check_writable(maker_base_ata)?;
    check_writable(maker_quote_ata)?;
    check_owner(market, &ID)?;
    check_owner(book, &ID)?;
    if !token_programs::is_token_program(base_token_program.address())
        || !token_programs::is_token_program(quote_token_program.address())
    {
        return Err(AmmError::InvalidTokenProgram.into());
    }

    let (base_atoms_per_lot, quote_atoms_per_lot) = {
        let pool_data = unsafe { market.borrow_unchecked() };
        if pool_data.len() != MARKET_LEN || pool_data[0..8] != MARKET_TAG {
            return Err(AmmError::InvalidAccountTag.into());
        }
        let m = unsafe { &*(pool_data.as_ptr() as *const Market) };
        if !eq_addr(base_mint.address().as_ref(),   &m.base_mint)
            || !eq_addr(quote_mint.address().as_ref(),  &m.quote_mint)
            || !eq_addr(base_vault.address().as_ref(),  &m.base_vault)
            || !eq_addr(quote_vault.address().as_ref(), &m.quote_vault)
        {
            return Err(AmmError::IncorrectTokenAccountMint.into());
        }
        (m.base_atoms_per_base_lot, m.quote_atoms_per_quote_lot)
    };

    {
        let book_data: &mut [u8] = unsafe { book.borrow_unchecked_mut() };
        if book_data.len() != MAKER_BOOK_LEN || book_data[0..8] != MAKER_BOOK_TAG {
            return Err(AmmError::InvalidAccountTag.into());
        }
        let b = unsafe { &mut *(book_data.as_mut_ptr() as *mut MakerBook) };
        if !eq_addr(maker.address().as_ref(), &b.maker) {
            return Err(AmmError::UnauthorizedMaker.into());
        }
        if !eq_addr(market.address().as_ref(), &b.market) {
            return Err(AmmError::InvalidAccountTag.into());
        }
        if base_lots > 0 {
            b.base_free = b.base_free.checked_add(base_lots).ok_or(AmmError::PriceMathOverflow)?;
        }
        if quote_lots > 0 {
            b.quote_free = b.quote_free.checked_add(quote_lots).ok_or(AmmError::PriceMathOverflow)?;
        }
    }

    if base_lots > 0 {
        let amt = base_lots.checked_mul(base_atoms_per_lot).ok_or(AmmError::PriceMathOverflow)?;
        pinocchio_token::instructions::Transfer::new(maker_base_ata, base_vault, maker, amt).invoke()?;
    }
    if quote_lots > 0 {
        let amt = quote_lots.checked_mul(quote_atoms_per_lot).ok_or(AmmError::PriceMathOverflow)?;
        pinocchio_token::instructions::Transfer::new(maker_quote_ata, quote_vault, maker, amt).invoke()?;
    }
    Ok(())
}
