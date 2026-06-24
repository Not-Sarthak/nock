use pinocchio::{
    account::AccountView,
    cpi::{Seed, Signer},
    error::ProgramError,
    ProgramResult,
};
use nock_interface::{
    constants::SEED_MARKET,
    error::AmmError,
    state::{load_mut, Market},
    ID,
};
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::unsafe_eq::eq_addr;
use super::token_programs;

pub fn process(accounts: &mut [AccountView], _data: &[u8]) -> ProgramResult {
    let [market, admin, quote_vault, dst, token_program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    check_signer(admin)?;
    check_writable(market)?;
    check_writable(quote_vault)?;
    check_writable(dst)?;
    check_owner(market, &ID)?;
    if !token_programs::is_token_program(token_program.address()) {
        return Err(AmmError::InvalidTokenProgram.into());
    }

    let (amount_atoms, market_bump, base_mint_key, quote_mint_key, admin_key) = {
        let mut pool_data = market.try_borrow_mut()?;
        let m = load_mut::<Market>(&mut pool_data[..])?;
        if !eq_addr(admin.address().as_ref(), &m.admin) {
            return Err(AmmError::UnauthorizedMaker.into());
        }
        if !eq_addr(quote_vault.address().as_ref(), &m.quote_vault) {
            return Err(AmmError::IncorrectTokenAccountMint.into());
        }
        let lots = m.uncollected_fees_quote_lots;
        if lots == 0 {
            return Ok(());
        }
        let atoms = lots.checked_mul(m.quote_atoms_per_quote_lot).ok_or(AmmError::PriceMathOverflow)?;
        m.uncollected_fees_quote_lots = 0;
        m.collected_fees_quote_lots = m.collected_fees_quote_lots.saturating_add(lots);
        (atoms, m.market_bump, m.base_mint, m.quote_mint, m.admin)
    };

    let bump_bytes = [market_bump];
    let market_seeds = [
        Seed::from(SEED_MARKET),
        Seed::from(&base_mint_key[..]),
        Seed::from(&quote_mint_key[..]),
        Seed::from(&admin_key[..]),
        Seed::from(&bump_bytes[..]),
    ];
    let market_signer = [Signer::from(&market_seeds)];
    pinocchio_token::instructions::Transfer::new(quote_vault, dst, market, amount_atoms)
        .invoke_signed(&market_signer)?;
    Ok(())
}
