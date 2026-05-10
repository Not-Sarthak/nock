use pinocchio::{
    account::AccountView,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use nock_interface::{
    constants::{
        MAKER_BOOK_LEN, MAKER_BOOK_TAG, MARKET_LEN, MARKET_TAG, MAX_LEVELS,
        MAX_QUOTE_AGE_SLOTS, PPM_DENOM, SEED_MARKET, SYNC_FILL_EVENT_TAG,
    },
    error::AmmError,
    events::SyncFillEvent,
    instruction::SyncSwapData,
    state::{MakerBook, Market},
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
        tick_size, taker_fee_ppm, base_mint_key, quote_mint_key,
    ) = {
        let pool_data = unsafe { market.borrow_unchecked() };
        if pool_data.len() != MARKET_LEN || pool_data[0..8] != MARKET_TAG {
            return Err(AmmError::InvalidAccountTag.into());
        }
        let m = unsafe { &*(pool_data.as_ptr() as *const Market) };
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
        )
    };

    let (book_seq, book_maker, mid_ticks, level, sync_spread_ticks) = {
        let book_data: &mut [u8] = unsafe { book.borrow_unchecked_mut() };
        if book_data.len() != MAKER_BOOK_LEN || book_data[0..8] != MAKER_BOOK_TAG {
            return Err(AmmError::InvalidAccountTag.into());
        }
        let b = unsafe { &mut *(book_data.as_mut_ptr() as *mut MakerBook) };
        if !eq_addr(market.address().as_ref(), &b.market) {
            return Err(AmmError::InvalidAccountTag.into());
        }
        if clock.slot > b.last_updated_slot.saturating_add(MAX_QUOTE_AGE_SLOTS) {
            return Err(AmmError::QuoteExpired.into());
        }
        let lvl = if side == 0 {
            b.ask_levels[level_index as usize]
        } else {
            b.bid_levels[level_index as usize]
        };
        let seq = b.last_updated_sequence_number;
        let mk  = b.maker;
        let mid = b.mid_price_ticks;
        let spr = b.sync_spread_ticks;
        b.last_updated_sequence_number = seq.checked_add(1).ok_or(AmmError::PriceMathOverflow)?;
        b.last_updated_slot            = clock.slot;
        (seq, mk, mid, lvl, spr)
    };
    if level.size_in_base_lots == 0 {
        return Err(AmmError::QuoteIndexOutOfBounds.into());
    }
    if amount_in_lots > level.size_in_base_lots {
        return Err(AmmError::QuoteSizeTooSmall.into());
    }

    let price_offset = level.price_offset_ticks;
    let price_ticks: i128 = if side == 0 {
        (mid_ticks as i128) + (price_offset as i128) + (sync_spread_ticks as i128)
    } else {
        (mid_ticks as i128) - (price_offset as i128) - (sync_spread_ticks as i128)
    };
    if price_ticks <= 0 {
        return Err(AmmError::PriceMathOverflow.into());
    }
    let price_ticks_u: u64 = price_ticks as u64;

    let in_atoms  = amount_in_lots
        .checked_mul(base_atoms_per_base_lot)
        .ok_or(AmmError::PriceMathOverflow)?;
    let raw_base_units_per_base_unit_u128 = {
        let pool_data = unsafe { market.borrow_unchecked() };
        let m = unsafe { &*(pool_data.as_ptr() as *const Market) };
        m.raw_base_units_per_base_unit as u128
    }.max(1);
    let gross_out_atoms_u128 = (in_atoms as u128)
        .checked_mul(price_ticks_u as u128)
        .ok_or(AmmError::PriceMathOverflow)?
        .checked_mul(tick_size as u128)
        .ok_or(AmmError::PriceMathOverflow)?
        / raw_base_units_per_base_unit_u128;
    if gross_out_atoms_u128 == 0 || gross_out_atoms_u128 > u64::MAX as u128 {
        return Err(AmmError::PriceMathOverflow.into());
    }
    let gross_out_atoms = gross_out_atoms_u128 as u64;

    let fee_atoms = ((gross_out_atoms as i128) * (taker_fee_ppm as i128) / PPM_DENOM as i128).max(0) as u64;
    let net_out_atoms = gross_out_atoms.saturating_sub(fee_atoms);

    let out_atoms_per_lot = if side == 0 { quote_atoms_per_quote_lot } else { base_atoms_per_base_lot };
    let net_out_lots = net_out_atoms / out_atoms_per_lot;
    if net_out_lots < min_out_lots {
        return Err(AmmError::SlippageExceeded.into());
    }
    let net_out_atoms = net_out_lots
        .checked_mul(out_atoms_per_lot)
        .ok_or(AmmError::PriceMathOverflow)?;

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

    pinocchio_token::instructions::Transfer::new(vault_out, user_dst, market, net_out_atoms)
        .invoke_signed(&market_signer)?;

    if fee_atoms > 0 {
        let mut pool_data = market.try_borrow_mut()?;
        let m = unsafe { &mut *(pool_data.as_mut_ptr() as *mut Market) };
        m.uncollected_fees_quote_lots = m.uncollected_fees_quote_lots.saturating_add(fee_atoms / quote_atoms_per_quote_lot);
    }

    let mut user_pubkey:  [u8; 32] = [0; 32]; user_pubkey.copy_from_slice(user.address().as_ref());
    let mut market_pubkey:[u8; 32] = [0; 32]; market_pubkey.copy_from_slice(market.address().as_ref());

    let event = SyncFillEvent {
        tag:        SYNC_FILL_EVENT_TAG,
        pool:       market_pubkey,
        maker:      book_maker,
        user:       user_pubkey,
        sequence:   book_seq,
        amount_in:  amount_in_lots,
        amount_out: net_out_lots,
        fee:        fee_atoms,
        slot:       clock.slot,
        side:       side as u64,
    };
    crate::helpers::event::emit(&event);
    Ok(())
}
