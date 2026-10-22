use super::read_market;
use crate::helpers::assertions::{check_owner, check_signer, check_token_program, check_writable};
use crate::helpers::event::{check_event_accounts, emit};
use nock_interface::{
    error::NockError,
    events::{tag, InventoryEvent},
    instruction::{parse, MakerFundsData},
    state::{load_mut, MakerBook, Role},
    ID,
};
use pinocchio::{account::AccountView, error::ProgramError, ProgramResult};
use pinocchio_token::instructions::Transfer;

/// Moves base and/or quote lots from the maker's token accounts into the market vaults and
/// credits them to the maker's book.
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [maker, market, book, base_vault, quote_vault, maker_base, maker_quote, token_program, event_authority, program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let args = parse::<MakerFundsData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let base_lots = args.base_lots;
    let quote_lots = args.quote_lots;
    if base_lots == 0 && quote_lots == 0 {
        return Err(NockError::AmountZero.into());
    }

    check_signer(maker)?;
    check_writable(book)?;
    check_owner(book, &ID)?;
    check_token_program(token_program)?;
    check_event_accounts(event_authority, program)?;

    let lots = read_market(market, |m| {
        if m.is_paused() {
            return Err(NockError::MarketPaused.into());
        }
        if base_vault.address().as_array() != &m.base_vault
            || quote_vault.address().as_array() != &m.quote_vault
        {
            return Err(NockError::IncorrectVault.into());
        }
        Ok(m.lot_sizes())
    })?;
    let base_atoms = lots.base_atoms(base_lots)?;
    let quote_atoms = lots.quote_atoms(quote_lots)?;

    let (base_free, quote_free) = {
        let mut book_data = book.try_borrow_mut()?;
        let b = load_mut::<MakerBook>(&mut book_data[..])?;
        if b.role_of(maker.address().as_ref())? != Role::Maker {
            return Err(NockError::UnauthorizedMaker.into());
        }
        if market.address().as_array() != &b.market {
            return Err(NockError::MarketMismatch.into());
        }
        b.base_free = b
            .base_free
            .checked_add(base_lots)
            .ok_or(NockError::MathOverflow)?;
        b.quote_free = b
            .quote_free
            .checked_add(quote_lots)
            .ok_or(NockError::MathOverflow)?;
        (b.base_free, b.quote_free)
    };

    if base_lots > 0 {
        check_writable(base_vault)?;
        check_writable(maker_base)?;
        Transfer::new(maker_base, base_vault, maker, base_atoms).invoke()?;
    }
    if quote_lots > 0 {
        check_writable(quote_vault)?;
        check_writable(maker_quote)?;
        Transfer::new(maker_quote, quote_vault, maker, quote_atoms).invoke()?;
    }

    emit(
        &InventoryEvent {
            tag: tag::DEPOSIT,
            book: book.address().to_bytes(),
            market: market.address().to_bytes(),
            maker: maker.address().to_bytes(),
            base_lots,
            quote_lots,
            base_free,
            quote_free,
        },
        event_authority,
    )
}
