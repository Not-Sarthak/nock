use pinocchio::{
    account::AccountView,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use nock_interface::{
    constants::{
        MAX_LEVELS, MAX_QUOTE_AGE_SLOTS, PPM_DENOM, SEED_MARKET, SYNC_FILL_EVENT_TAG,
    },
    error::AmmError,
    events::SyncFillEvent,
    instruction::SyncSwapData,
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
    let amount_in_lots = args.amount_in;
    let min_out_lots   = args.min_amount_out;
    let side           = args.side;
    let level_index    = args.quote_index;

    if amount_in_lots == 0 {
        return Err(AmmError::AmountInZero.into());
    }
    if (level_index as usize) >= MAX_LEVELS {
        return Err(AmmError::QuoteIndexOutOfBounds.into());
    }
    if side > 1 {
        return Err(ProgramError::InvalidInstructionData);
    }

    check_signer(user)?;
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
        tick_size, taker_fee_ppm, base_mint_key, quote_mint_key, scale,
    ) = {
        let m = load::<Market>(unsafe { market.borrow_unchecked() })?;
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
        {
            return Err(AmmError::PriceMathOverflow.into());
        }
        (
            m.admin, m.market_bump,
            m.base_atoms_per_base_lot, m.quote_atoms_per_quote_lot,
            m.tick_size_in_quote_atoms_per_base_unit, m.taker_fee_ppm,
            m.base_mint, m.quote_mint,
            (m.raw_base_units_per_base_unit as u128).max(1),
        )
    };

    let (book_seq, book_maker, in_atoms, out_atoms, fee_quote_lots, in_report_lots, out_report_lots) = {
        let b = load_mut::<MakerBook>(unsafe { book.borrow_unchecked_mut() })?;
        if !eq_addr(market.address().as_ref(), &b.market) {
            return Err(AmmError::InvalidAccountTag.into());
        }
        if b.status != book_status::ACTIVE {
            return Err(AmmError::BookHalted.into());
        }
        if clock.slot > b.last_updated_slot.saturating_add(MAX_QUOTE_AGE_SLOTS) {
            return Err(AmmError::QuoteExpired.into());
        }

        let level = if side == 0 {
            b.bid_levels[level_index as usize]
        } else {
            b.ask_levels[level_index as usize]
        };
        if level.size_in_base_lots == 0 {
            return Err(AmmError::QuoteIndexOutOfBounds.into());
        }
        if amount_in_lots > level.size_in_base_lots {
            return Err(AmmError::QuoteSizeTooSmall.into());
        }

        let mid    = b.mid_price_ticks as i128;
        let offset = level.price_offset_ticks as i128;
        let spread = b.sync_spread_ticks as i128;
        let price_ticks: i128 = if side == 0 {
            mid - offset - spread
        } else {
            mid + offset + spread
        };
        if price_ticks <= 0 {
            return Err(AmmError::PriceMathOverflow.into());
        }

        let base_atoms = amount_in_lots
            .checked_mul(base_atoms_per_base_lot)
            .ok_or(AmmError::PriceMathOverflow)?;
        let gross_quote_atoms = (base_atoms as u128)
            .checked_mul(price_ticks as u128)
            .ok_or(AmmError::PriceMathOverflow)?
            .checked_mul(tick_size as u128)
            .ok_or(AmmError::PriceMathOverflow)?
            / scale;
        if gross_quote_atoms == 0 || gross_quote_atoms > u64::MAX as u128 {
            return Err(AmmError::PriceMathOverflow.into());
        }
        let gross_quote_lots = (gross_quote_atoms as u64) / quote_atoms_per_quote_lot;
        if gross_quote_lots == 0 {
            return Err(AmmError::QuoteSizeTooSmall.into());
        }
        let fee_quote_lots =
            (((gross_quote_lots as i128) * (taker_fee_ppm as i128)) / PPM_DENOM as i128).max(0) as u64;

        let seq = b.last_updated_sequence_number;

        let (in_atoms, out_atoms, in_report, out_report) = if side == 0 {
            let net_quote_lots = gross_quote_lots.saturating_sub(fee_quote_lots);
            if net_quote_lots < min_out_lots {
                return Err(AmmError::SlippageExceeded.into());
            }
            if b.quote_free < gross_quote_lots {
                return Err(AmmError::InsufficientVaultBalance.into());
            }
            b.base_free  = b.base_free.checked_add(amount_in_lots).ok_or(AmmError::PriceMathOverflow)?;
            b.quote_free -= gross_quote_lots;

            let in_atoms  = base_atoms;
            let out_atoms = net_quote_lots
                .checked_mul(quote_atoms_per_quote_lot)
                .ok_or(AmmError::PriceMathOverflow)?;
            (in_atoms, out_atoms, amount_in_lots, net_quote_lots)
        } else {
            if amount_in_lots < min_out_lots {
                return Err(AmmError::SlippageExceeded.into());
            }
            if b.base_free < amount_in_lots {
                return Err(AmmError::InsufficientVaultBalance.into());
            }
            let total_quote_in_lots = gross_quote_lots
                .checked_add(fee_quote_lots)
                .ok_or(AmmError::PriceMathOverflow)?;
            b.base_free  -= amount_in_lots;
            b.quote_free  = b.quote_free.checked_add(gross_quote_lots).ok_or(AmmError::PriceMathOverflow)?;

            let in_atoms  = total_quote_in_lots
                .checked_mul(quote_atoms_per_quote_lot)
                .ok_or(AmmError::PriceMathOverflow)?;
            let out_atoms = base_atoms;
            (in_atoms, out_atoms, total_quote_in_lots, amount_in_lots)
        };

        b.last_updated_sequence_number = seq.checked_add(1).ok_or(AmmError::PriceMathOverflow)?;
        b.last_updated_slot            = clock.slot;
        (seq, b.maker, in_atoms, out_atoms, fee_quote_lots, in_report, out_report)
    };

    if fee_quote_lots > 0 {
        let mut pool_data = market.try_borrow_mut()?;
        let m = load_mut::<Market>(&mut pool_data[..])?;
        m.uncollected_fees_quote_lots = m.uncollected_fees_quote_lots.saturating_add(fee_quote_lots);
    }

    let (vault_in, vault_out) = if side == 0 {
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

    let mut user_pubkey:  [u8; 32] = [0; 32]; user_pubkey.copy_from_slice(user.address().as_ref());
    let mut market_pubkey:[u8; 32] = [0; 32]; market_pubkey.copy_from_slice(market.address().as_ref());

    let event = SyncFillEvent {
        tag:        SYNC_FILL_EVENT_TAG,
        pool:       market_pubkey,
        maker:      book_maker,
        user:       user_pubkey,
        sequence:   book_seq,
        amount_in:  in_report_lots,
        amount_out: out_report_lots,
        fee:        fee_quote_lots,
        slot:       clock.slot,
        side:       side as u64,
    };
    crate::helpers::event::emit(&event);
    Ok(())
}
