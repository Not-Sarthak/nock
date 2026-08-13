use pinocchio::{
    account::AccountView,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use nock_interface::{
    constants::{MAX_LEVELS, MAX_QUOTE_AGE_SLOTS, SEED_MARKET, SYNC_FILL_EVENT_TAG},
    error::AmmError,
    events::SyncFillEvent,
    instruction::SyncSwapData,
    math::{level_price_ticks, price_fill, SIDE_SELL_BASE},
    state::{load, load_mut, maker_book::book_status, MakerBook, Market},
    ID,
};
use crate::helpers::unsafe_eq::eq_addr;
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use super::token_programs;

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [
        user,
        market,
        user_dst,
        base_mint,
        quote_mint,
        base_vault,
        quote_vault,
        user_src,
        token_program,
        book,
        ..
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if data.len() != core::mem::size_of::<SyncSwapData>() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let args: &SyncSwapData = unsafe { &*(data.as_ptr() as *const SyncSwapData) };
    let amount_in_lots = args.amount_in_lots;
    let limit_lots     = args.limit_lots;
    let side           = args.side;
    let level_index    = args.quote_index as usize;

    if amount_in_lots == 0 {
        return Err(AmmError::AmountInZero.into());
    }
    if level_index >= MAX_LEVELS {
        return Err(AmmError::QuoteIndexOutOfBounds.into());
    }
    if side > 1 {
        return Err(ProgramError::InvalidInstructionData);
    }

    check_signer(user)?;
    check_writable(market)?;
    check_writable(user_dst)?;
    check_writable(base_vault)?;
    check_writable(quote_vault)?;
    check_writable(user_src)?;
    check_writable(book)?;
    check_owner(market, &ID)?;
    check_owner(book, &ID)?;
    if !token_programs::is_token_program(token_program.address()) {
        return Err(AmmError::InvalidTokenProgram.into());
    }

    let clock = Clock::get()?;

    let (
        admin, market_bump, base_atoms_per_base_lot, quote_atoms_per_quote_lot,
        tick_size, base_atoms_per_base_unit, maker_fee_ppm, taker_fee_ppm,
        base_mint_key, quote_mint_key,
    ) = {
        let market_data = market.try_borrow()?;
        let m = load::<Market>(&market_data[..])?;
        if m.is_paused() {
            return Err(AmmError::MarketPaused.into());
        }
        if !eq_addr(base_vault.address().as_ref(),  &m.base_vault)
            || !eq_addr(quote_vault.address().as_ref(), &m.quote_vault)
            || !eq_addr(base_mint.address().as_ref(),   &m.base_mint)
            || !eq_addr(quote_mint.address().as_ref(),  &m.quote_mint)
        {
            return Err(AmmError::IncorrectTokenAccountMint.into());
        }
        if m.tick_size_in_quote_atoms_per_base_unit == 0
            || m.base_atoms_per_base_lot == 0
            || m.quote_atoms_per_quote_lot == 0
            || m.base_atoms_per_base_unit == 0
        {
            return Err(AmmError::PriceMathOverflow.into());
        }
        (
            m.admin, m.market_bump,
            m.base_atoms_per_base_lot, m.quote_atoms_per_quote_lot,
            m.tick_size_in_quote_atoms_per_base_unit, m.base_atoms_per_base_unit,
            m.maker_fee_ppm, m.taker_fee_ppm,
            m.base_mint, m.quote_mint,
        )
    };

    let (book_seq, book_maker, in_atoms, out_atoms, protocol_fee_lots, in_report_lots, out_report_lots) = {
        let mut book_data = book.try_borrow_mut()?;
        let b = load_mut::<MakerBook>(&mut book_data[..])?;

        if !eq_addr(market.address().as_ref(), &b.market) {
            return Err(AmmError::InvalidAccountTag.into());
        }
        if b.status != book_status::ACTIVE {
            return Err(AmmError::BookHalted.into());
        }

        let max_age = if b.expiry_in_slots == 0 { MAX_QUOTE_AGE_SLOTS } else { b.expiry_in_slots };
        if clock.slot > b.last_updated_slot.saturating_add(max_age) {
            return Err(AmmError::QuoteExpired.into());
        }

        let level = if side == SIDE_SELL_BASE {
            b.bid_levels[level_index]
        } else {
            b.ask_levels[level_index]
        };
        if level.size_in_base_lots == 0 {
            return Err(AmmError::QuoteIndexOutOfBounds.into());
        }
        if amount_in_lots > level.size_in_base_lots {
            return Err(AmmError::QuoteSizeTooSmall.into());
        }

        let price_ticks = level_price_ticks(
            side,
            b.mid_price_ticks,
            level.price_offset_ticks,
            b.sync_spread_ticks,
        )?;

        let fill = price_fill(
            side,
            amount_in_lots,
            price_ticks,
            base_atoms_per_base_lot,
            quote_atoms_per_quote_lot,
            tick_size,
            base_atoms_per_base_unit,
            maker_fee_ppm,
            taker_fee_ppm,
        )?;

        let gross        = fill.gross_quote_lots;
        let taker_fee    = fill.taker_fee_lots;
        let maker_fee    = fill.maker_fee_lots;
        let protocol_fee = fill.protocol_fee_lots();

        let remaining = level.size_in_base_lots - amount_in_lots;
        if side == SIDE_SELL_BASE {
            b.bid_levels[level_index].size_in_base_lots = remaining;
        } else {
            b.ask_levels[level_index].size_in_base_lots = remaining;
        }

        let (in_atoms, out_atoms, in_report, out_report) = if side == SIDE_SELL_BASE {
            let net_quote_lots = gross.checked_sub(taker_fee).ok_or(AmmError::PriceMathOverflow)?;
            if net_quote_lots < limit_lots {
                return Err(AmmError::SlippageExceeded.into());
            }
            let maker_debit = gross.checked_add(maker_fee).ok_or(AmmError::PriceMathOverflow)?;
            if b.quote_free < maker_debit {
                return Err(AmmError::InsufficientVaultBalance.into());
            }
            b.quote_free -= maker_debit;
            b.base_free = b.base_free
                .checked_add(amount_in_lots)
                .ok_or(AmmError::PriceMathOverflow)?;

            let out_atoms = net_quote_lots
                .checked_mul(quote_atoms_per_quote_lot)
                .ok_or(AmmError::PriceMathOverflow)?;
            (fill.base_atoms, out_atoms, amount_in_lots, net_quote_lots)
        } else {
            let total_quote_in_lots = gross
                .checked_add(taker_fee)
                .ok_or(AmmError::PriceMathOverflow)?;
            if total_quote_in_lots > limit_lots {
                return Err(AmmError::SlippageExceeded.into());
            }
            if b.base_free < amount_in_lots {
                return Err(AmmError::InsufficientVaultBalance.into());
            }
            let maker_credit = gross.checked_sub(maker_fee).ok_or(AmmError::PriceMathOverflow)?;
            b.base_free -= amount_in_lots;
            b.quote_free = b.quote_free
                .checked_add(maker_credit)
                .ok_or(AmmError::PriceMathOverflow)?;

            let in_atoms = total_quote_in_lots
                .checked_mul(quote_atoms_per_quote_lot)
                .ok_or(AmmError::PriceMathOverflow)?;
            (in_atoms, fill.base_atoms, total_quote_in_lots, amount_in_lots)
        };

        (
            b.last_updated_sequence_number, b.maker,
            in_atoms, out_atoms, protocol_fee, in_report, out_report,
        )
    };

    if protocol_fee_lots > 0 {
        let mut market_data = market.try_borrow_mut()?;
        let m = load_mut::<Market>(&mut market_data[..])?;
        m.uncollected_fees_quote_lots =
            m.uncollected_fees_quote_lots.saturating_add(protocol_fee_lots);
    }

    let (vault_in, vault_out) = if side == SIDE_SELL_BASE {
        (base_vault, quote_vault)
    } else {
        (quote_vault, base_vault)
    };

    pinocchio_token::instructions::Transfer::new(user_src, vault_in, user, in_atoms).invoke()?;

    let bump_bytes = [market_bump];
    let market_seeds = [
        Seed::from(SEED_MARKET),
        Seed::from(&base_mint_key[..]),
        Seed::from(&quote_mint_key[..]),
        Seed::from(&admin[..]),
        Seed::from(&bump_bytes[..]),
    ];
    let market_signer = [Signer::from(&market_seeds)];
    pinocchio_token::instructions::Transfer::new(vault_out, user_dst, market, out_atoms)
        .invoke_signed(&market_signer)?;

    let mut user_pubkey:   [u8; 32] = [0; 32];
    let mut market_pubkey: [u8; 32] = [0; 32];
    user_pubkey.copy_from_slice(user.address().as_ref());
    market_pubkey.copy_from_slice(market.address().as_ref());

    let event = SyncFillEvent {
        tag:        SYNC_FILL_EVENT_TAG,
        pool:       market_pubkey,
        maker:      book_maker,
        user:       user_pubkey,
        sequence:   book_seq,
        amount_in:  in_report_lots,
        amount_out: out_report_lots,
        fee:        protocol_fee_lots,
        slot:       clock.slot,
        side:       side as u64,
    };
    crate::helpers::event::emit(&event);
    Ok(())
}
