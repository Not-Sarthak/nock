use super::read_market;
use crate::helpers::assertions::{check_owner, check_token_program, check_writable};
use crate::helpers::event::{check_event_accounts, emit};
use crate::helpers::pda::MarketSeeds;
use crate::helpers::token_account::check_token_account;
use nock_interface::{
    error::NockError,
    events::{tag, FeesCollectedEvent},
    state::{load_mut, MakerBook},
    ID,
};
use pinocchio::{account::AccountView, cpi::Signer, error::ProgramError, ProgramResult};

/// Sweeps one book's accrued fees to the admin's quote token account. Permissionless:
/// the destination is pinned to the admin, so anyone (the maker before closing a book,
/// or a crank) may trigger it.
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [market, book, quote_vault, destination, token_program, event_authority, program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    check_writable(book)?;
    check_writable(quote_vault)?;
    check_writable(destination)?;
    check_owner(book, &ID)?;
    check_token_program(token_program)?;
    check_event_accounts(event_authority, program)?;

    let (seeds, admin, quote_mint, lots_to_atoms) = read_market(market, |m| {
        if quote_vault.address().as_array() != &m.quote_vault {
            return Err(NockError::IncorrectVault.into());
        }
        Ok((MarketSeeds::new(m), m.admin, m.quote_mint, m.lot_sizes()))
    })?;
    check_token_account(destination, &quote_mint, &admin)?;

    let (fee_lots, maker) = {
        let mut book_data = book.try_borrow_mut()?;
        let b = load_mut::<MakerBook>(&mut book_data[..])?;
        if market.address().as_array() != &b.market {
            return Err(NockError::MarketMismatch.into());
        }
        let lots = b.fees_quote_lots;
        b.fees_quote_lots = 0;
        (lots, b.maker)
    };
    if fee_lots == 0 {
        return Ok(());
    }
    let fee_atoms = lots_to_atoms.quote_atoms(fee_lots)?;

    let market_seeds = seeds.seeds();
    pinocchio_token::instructions::Transfer::new(quote_vault, destination, market, fee_atoms)
        .invoke_signed(&[Signer::from(&market_seeds)])?;

    emit(
        &FeesCollectedEvent {
            tag: tag::FEES_COLLECTED,
            market: market.address().to_bytes(),
            book: book.address().to_bytes(),
            maker,
            destination: destination.address().to_bytes(),
            fee_quote_lots: fee_lots,
            fee_quote_atoms: fee_atoms,
        },
        event_authority,
    )
}
