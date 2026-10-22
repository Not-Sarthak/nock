use super::{parse_flag, read_market};
use crate::helpers::assertions::{check_owner, check_signer, check_token_program, check_writable};
use crate::helpers::event::{check_event_accounts, emit};
use crate::helpers::pda::MarketSeeds;
use nock_interface::{
    error::NockError,
    events::{tag, SyncFillEvent},
    instruction::{parse, SyncSwapData},
    math::{fill_book, SIDE_SELL_BASE},
    state::{load_mut, MakerBook},
    ID,
};
use pinocchio::{
    account::AccountView,
    cpi::Signer,
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_token::instructions::Transfer;

/// Fills `base_lots` against one maker's book, walking its levels best first, all or
/// nothing. The Market is only read, so fills on different books never contend.
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [user, market, book, base_vault, quote_vault, user_base, user_quote, token_program, event_authority, program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let args = parse::<SyncSwapData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let side = args.side;
    let base_lots = args.base_lots;
    let limit_quote_lots = args.limit_quote_lots;
    let price_limit_ticks = args.price_limit_ticks;
    let expected_sequence = if parse_flag(args.pin_sequence)? {
        Some(args.expected_sequence)
    } else {
        None
    };

    check_signer(user)?;
    check_writable(book)?;
    check_writable(base_vault)?;
    check_writable(quote_vault)?;
    check_writable(user_base)?;
    check_writable(user_quote)?;
    check_owner(book, &ID)?;
    check_token_program(token_program)?;
    check_event_accounts(event_authority, program)?;

    let (seeds, lots, maker_fee_ppm, taker_fee_ppm) = read_market(market, |m| {
        if m.is_paused() {
            return Err(NockError::MarketPaused.into());
        }
        if base_vault.address().as_array() != &m.base_vault
            || quote_vault.address().as_array() != &m.quote_vault
        {
            return Err(NockError::IncorrectVault.into());
        }
        Ok((
            MarketSeeds::new(m),
            m.lot_sizes(),
            m.maker_fee_ppm,
            m.taker_fee_ppm,
        ))
    })?;
    let slot = Clock::get()?.slot;

    let (fill, maker, sequence) = {
        let mut book_data = book.try_borrow_mut()?;
        let b = load_mut::<MakerBook>(&mut book_data[..])?;
        if market.address().as_array() != &b.market {
            return Err(NockError::MarketMismatch.into());
        }
        b.check_fillable(slot, expected_sequence)?;
        let fill = fill_book(
            b,
            side,
            base_lots,
            limit_quote_lots,
            price_limit_ticks,
            &lots,
            maker_fee_ppm,
            taker_fee_ppm,
        )?;
        (fill, b.maker, b.sequence)
    };
    let quote_atoms = lots.quote_atoms(fill.taker_quote_lots)?;

    let market_seeds = seeds.seeds();
    let signer = [Signer::from(&market_seeds)];
    if side == SIDE_SELL_BASE {
        Transfer::new(user_base, base_vault, user, fill.base_atoms).invoke()?;
        Transfer::new(quote_vault, user_quote, market, quote_atoms).invoke_signed(&signer)?;
    } else {
        Transfer::new(user_quote, quote_vault, user, quote_atoms).invoke()?;
        Transfer::new(base_vault, user_base, market, fill.base_atoms).invoke_signed(&signer)?;
    }

    emit(
        &SyncFillEvent {
            tag: tag::SYNC_FILL,
            market: market.address().to_bytes(),
            book: book.address().to_bytes(),
            maker,
            user: user.address().to_bytes(),
            sequence,
            base_lots: fill.base_lots,
            gross_quote_lots: fill.gross_quote_lots,
            taker_quote_lots: fill.taker_quote_lots,
            taker_fee_lots: fill.taker_fee_lots,
            maker_fee_lots: fill.maker_fee_lots,
            worst_price_ticks: fill.worst_price_ticks,
            slot,
            side,
            _pad: [0; 7],
        },
        event_authority,
    )
}
