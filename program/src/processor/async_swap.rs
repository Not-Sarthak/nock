//! Async swaps are deferred SyncSwaps. The user escrows the input under a nonce-keyed PDA;
//! a maker (or its delegate) later fills it from that maker's own book, under the same
//! pause, halt, expiry and fee rules as a SyncSwap.
//!
//! The user may name one maker or leave the swap open to every book on the market. Naming
//! fits an RFQ flow where the user already holds that maker's quote; leaving it open gets
//! the first maker willing to fill within the user's limits. Either way the user's limits
//! bind the fill, so who fills changes availability, not the worst-case terms.
//!
//! The escrow is the PDA's associated token account. It is emptied and closed on every
//! terminal path, and anything beyond the fill (unspent buy-side quote, or tokens someone
//! sent to the escrow) goes back to the user.

use super::read_market;
use crate::helpers::assertions::{
    check_owner, check_pubkey, check_signer, check_token_program, check_writable,
};
use crate::helpers::event::{check_event_accounts, emit};
use crate::helpers::pda::{
    close_pda, create_pda, find_canonical, rent_exempt, AsyncSwapSeeds, MarketSeeds,
};
use crate::helpers::token_account::{check_token_account, read_token_account};
use nock_interface::{
    constants::{ASYNC_SWAP_LEN, ATA, NO_ADDRESS, SEED_ASYNC_SWAP, SYSTEM},
    error::NockError,
    events::{tag, AsyncCancelEvent, AsyncFillEvent, AsyncSwapCreatedEvent},
    instruction::{parse, AsyncSwapCreateData},
    math::{fill_book, SIDE_BUY_BASE, SIDE_SELL_BASE},
    state::{init, load, load_mut, AsyncSwap, MakerBook},
    ID,
};
use pinocchio::{
    account::AccountView,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_associated_token_account::instructions::CreateIdempotent;
use pinocchio_token::instructions::{CloseAccount, Transfer};

pub fn process_create(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [user, market, async_swap, escrow, mint_in, user_src, system_program, token_program, ata_program, event_authority, program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let args = parse::<AsyncSwapCreateData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let side = args.side;
    let nonce = args.nonce;
    let base_lots = args.base_lots;
    let limit_quote_lots = args.limit_quote_lots;
    let price_limit_ticks = args.price_limit_ticks;
    let expiry_slot = args.expiry_slot;
    let maker = args.maker;

    if side != SIDE_SELL_BASE && side != SIDE_BUY_BASE {
        return Err(NockError::InvalidSide.into());
    }
    if base_lots == 0 || limit_quote_lots == 0 {
        return Err(NockError::AmountZero.into());
    }

    check_signer(user)?;
    check_writable(user)?;
    check_writable(async_swap)?;
    check_writable(escrow)?;
    check_writable(user_src)?;
    check_pubkey(system_program, &SYSTEM)?;
    check_pubkey(ata_program, &ATA)?;
    check_token_program(token_program)?;
    check_event_accounts(event_authority, program)?;

    let slot = Clock::get()?.slot;
    let escrow_atoms = read_market(market, |m| {
        if m.is_paused() {
            return Err(NockError::MarketPaused.into());
        }
        let expected_mint = if side == SIDE_SELL_BASE {
            &m.base_mint
        } else {
            &m.quote_mint
        };
        if mint_in.address().as_array() != expected_mint {
            return Err(NockError::InvalidMint.into());
        }
        let lifetime = expiry_slot
            .checked_sub(slot)
            .ok_or(NockError::AsyncWindowViolated)?;
        if lifetime < m.min_async_delay_slots as u64 || lifetime > m.max_async_delay_slots as u64 {
            return Err(NockError::AsyncWindowViolated.into());
        }
        let lots = m.lot_sizes();
        Ok(if side == SIDE_SELL_BASE {
            lots.base_atoms(base_lots)?
        } else {
            lots.quote_atoms(limit_quote_lots)?
        })
    })?;

    let nonce_bytes = nonce.to_le_bytes();
    let bump = find_canonical(
        &[
            SEED_ASYNC_SWAP,
            market.address().as_ref(),
            user.address().as_ref(),
            &nonce_bytes,
        ],
        async_swap,
    )?;
    let bump_bytes = [bump];
    let seeds = [
        Seed::from(SEED_ASYNC_SWAP),
        Seed::from(market.address().as_ref()),
        Seed::from(user.address().as_ref()),
        Seed::from(&nonce_bytes[..]),
        Seed::from(&bump_bytes[..]),
    ];
    create_pda(
        user,
        async_swap,
        rent_exempt(ASYNC_SWAP_LEN)?,
        ASYNC_SWAP_LEN as u64,
        &ID,
        &[Signer::from(&seeds)],
    )?;

    CreateIdempotent {
        funding_account: user,
        account: escrow,
        wallet: async_swap,
        mint: mint_in,
        system_program,
        token_program,
    }
    .invoke()?;
    Transfer::new(user_src, escrow, user, escrow_atoms).invoke()?;

    {
        let mut swap_data = async_swap.try_borrow_mut()?;
        let s = init::<AsyncSwap>(&mut swap_data[..])?;
        s.user = user.address().to_bytes();
        s.market = market.address().to_bytes();
        s.maker = maker;
        s.escrow = escrow.address().to_bytes();
        s.nonce = nonce;
        s.base_lots = base_lots;
        s.limit_quote_lots = limit_quote_lots;
        s.price_limit_ticks = price_limit_ticks;
        s.expiry_slot = expiry_slot;
        s.side = side;
        s.bump = bump;
        s._pad = [0; 6];
    }

    emit(
        &AsyncSwapCreatedEvent {
            tag: tag::ASYNC_SWAP_CREATED,
            async_swap: async_swap.address().to_bytes(),
            market: market.address().to_bytes(),
            user: user.address().to_bytes(),
            maker,
            nonce,
            base_lots,
            limit_quote_lots,
            price_limit_ticks,
            expiry_slot,
            escrow_atoms,
            side,
            _pad: [0; 7],
        },
        event_authority,
    )
}

pub fn process_execute(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [signer, market, book, async_swap, escrow, base_vault, quote_vault, user_dst, user_refund, user, token_program, event_authority, program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    check_signer(signer)?;
    check_writable(book)?;
    check_writable(async_swap)?;
    check_writable(escrow)?;
    check_writable(base_vault)?;
    check_writable(quote_vault)?;
    check_writable(user_dst)?;
    check_writable(user_refund)?;
    check_writable(user)?;
    check_owner(book, &ID)?;
    check_owner(async_swap, &ID)?;
    check_token_program(token_program)?;
    check_event_accounts(event_authority, program)?;

    let (market_seeds, lots, maker_fee_ppm, taker_fee_ppm, base_mint, quote_mint) =
        read_market(market, |m| {
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
                m.base_mint,
                m.quote_mint,
            ))
        })?;
    let slot = Clock::get()?.slot;

    let (swap_seeds, user_key, named_maker, side, base_lots, limit_quote_lots, price_limit_ticks) = {
        let swap_data = async_swap.try_borrow()?;
        let s = load::<AsyncSwap>(&swap_data[..])?;
        if market.address().as_array() != &s.market {
            return Err(NockError::MarketMismatch.into());
        }
        if escrow.address().as_array() != &s.escrow {
            return Err(NockError::IncorrectTokenAccount.into());
        }
        if user.address().as_array() != &s.user {
            return Err(ProgramError::InvalidArgument);
        }
        if slot >= s.expiry_slot {
            return Err(NockError::AsyncSwapExpired.into());
        }
        (
            AsyncSwapSeeds::new(s),
            s.user,
            s.maker,
            s.side,
            s.base_lots,
            s.limit_quote_lots,
            s.price_limit_ticks,
        )
    };

    let (input_mint, output_mint) = if side == SIDE_SELL_BASE {
        (base_mint, quote_mint)
    } else {
        (quote_mint, base_mint)
    };
    check_token_account(user_dst, &output_mint, &user_key)?;
    check_token_account(user_refund, &input_mint, &user_key)?;

    let (fill, maker, sequence) = {
        let mut book_data = book.try_borrow_mut()?;
        let b = load_mut::<MakerBook>(&mut book_data[..])?;
        if market.address().as_array() != &b.market {
            return Err(NockError::MarketMismatch.into());
        }
        b.role_of(signer.address().as_ref())?;
        if named_maker != NO_ADDRESS && named_maker != b.maker {
            return Err(NockError::MakerNotSelected.into());
        }
        b.check_fillable(slot, None)?;
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

    let swap_signer_seeds = swap_seeds.seeds();
    let swap_signer = [Signer::from(&swap_signer_seeds)];
    let market_signer_seeds = market_seeds.seeds();
    let market_signer = [Signer::from(&market_signer_seeds)];
    if side == SIDE_SELL_BASE {
        Transfer::new(escrow, base_vault, async_swap, fill.base_atoms)
            .invoke_signed(&swap_signer)?;
        Transfer::new(quote_vault, user_dst, market, quote_atoms).invoke_signed(&market_signer)?;
    } else {
        Transfer::new(escrow, quote_vault, async_swap, quote_atoms).invoke_signed(&swap_signer)?;
        Transfer::new(base_vault, user_dst, market, fill.base_atoms)
            .invoke_signed(&market_signer)?;
    }

    let refunded_atoms = settle_escrow(escrow, user_refund, user, async_swap, &swap_signer)?;
    let async_swap_key = async_swap.address().to_bytes();
    close_pda(async_swap, user)?;

    emit(
        &AsyncFillEvent {
            tag: tag::ASYNC_FILL,
            async_swap: async_swap_key,
            market: market.address().to_bytes(),
            book: book.address().to_bytes(),
            maker,
            signer: signer.address().to_bytes(),
            user: user_key,
            sequence,
            base_lots: fill.base_lots,
            gross_quote_lots: fill.gross_quote_lots,
            taker_quote_lots: fill.taker_quote_lots,
            taker_fee_lots: fill.taker_fee_lots,
            maker_fee_lots: fill.maker_fee_lots,
            worst_price_ticks: fill.worst_price_ticks,
            refunded_atoms,
            slot,
            side,
            _pad: [0; 7],
        },
        event_authority,
    )
}

/// The user may cancel at any time. Once expired, anyone may release it (a maker, or a
/// crank); the escrow and rent go only to the user either way.
pub fn process_cancel(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [caller, async_swap, escrow, user_refund, user, token_program, event_authority, program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    check_signer(caller)?;
    check_writable(async_swap)?;
    check_writable(escrow)?;
    check_writable(user_refund)?;
    check_writable(user)?;
    check_owner(async_swap, &ID)?;
    check_token_program(token_program)?;
    check_event_accounts(event_authority, program)?;

    let slot = Clock::get()?.slot;
    let (swap_seeds, user_key, market_key) = {
        let swap_data = async_swap.try_borrow()?;
        let s = load::<AsyncSwap>(&swap_data[..])?;
        if escrow.address().as_array() != &s.escrow {
            return Err(NockError::IncorrectTokenAccount.into());
        }
        if user.address().as_array() != &s.user {
            return Err(ProgramError::InvalidArgument);
        }
        if caller.address().as_array() != &s.user && slot < s.expiry_slot {
            return Err(NockError::AsyncSwapNotExpired.into());
        }
        (AsyncSwapSeeds::new(s), s.user, s.market)
    };

    let escrow_mint = read_token_account(escrow)?.mint;
    check_token_account(user_refund, &escrow_mint, &user_key)?;

    let swap_signer_seeds = swap_seeds.seeds();
    let swap_signer = [Signer::from(&swap_signer_seeds)];
    let refunded_atoms = settle_escrow(escrow, user_refund, user, async_swap, &swap_signer)?;
    let async_swap_key = async_swap.address().to_bytes();
    close_pda(async_swap, user)?;

    emit(
        &AsyncCancelEvent {
            tag: tag::ASYNC_CANCEL,
            async_swap: async_swap_key,
            market: market_key,
            user: user_key,
            caller: caller.address().to_bytes(),
            refunded_atoms,
            slot,
        },
        event_authority,
    )
}

/// Returns whatever the escrow still holds to `user_refund`, then closes the escrow and
/// sends its rent to `user`. Returns the atoms refunded.
fn settle_escrow(
    escrow: &AccountView,
    user_refund: &AccountView,
    user: &AccountView,
    async_swap: &AccountView,
    swap_signer: &[Signer],
) -> Result<u64, ProgramError> {
    let remaining = read_token_account(escrow)?.amount;
    if remaining > 0 {
        Transfer::new(escrow, user_refund, async_swap, remaining).invoke_signed(swap_signer)?;
    }
    CloseAccount::new(escrow, user, async_swap).invoke_signed(swap_signer)?;
    Ok(remaining)
}
