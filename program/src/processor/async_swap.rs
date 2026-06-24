use pinocchio::{
    account::AccountView,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    Address, ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;
use nock_interface::{
    constants::{
        ASYNC_CANCEL_EVENT_TAG, ASYNC_FILL_EVENT_TAG, ASYNC_SWAP_CREATED_TAG,
        ASYNC_SWAP_LEN, SEED_ASYNC_SWAP, SYSTEM,
    },
    error::AmmError,
    events::{AsyncCancelEvent, AsyncFillEvent, AsyncSwapCreatedEvent},
    state::{async_swap::status, init, load, load_mut, AsyncSwap, MakerBook, Market},
    ID,
};
use crate::helpers::assertions::{check_owner, check_pubkey, check_signer, check_writable};
use crate::helpers::token_account::mint_and_owner;
use crate::helpers::unsafe_eq::{eq_addr, pubkey_copy_32};
use super::token_programs;

pub fn process_create(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [user, async_swap, escrow, user_src, mint_in, market, system_program, token_program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if data.len() < 1 + 1 + 8 + 8 + 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let bump = data[0];
    let side = data[1];
    let amount_in = u64::from_le_bytes(data[2..10].try_into().unwrap());
    let min_out = u64::from_le_bytes(data[10..18].try_into().unwrap());
    let expiry_slot = u64::from_le_bytes(data[18..26].try_into().unwrap());

    if amount_in == 0 {
        return Err(AmmError::AmountInZero.into());
    }
    if side > 1 {
        return Err(ProgramError::InvalidInstructionData);
    }

    check_signer(user)?;
    check_writable(async_swap)?;
    check_writable(escrow)?;
    check_writable(user_src)?;
    check_owner(market, &ID)?;
    check_pubkey(system_program, &SYSTEM)?;
    if !token_programs::is_token_program(token_program.address()) {
        return Err(AmmError::InvalidTokenProgram.into());
    }

    let now = Clock::get()?.slot;
    {
        let m = load::<Market>(unsafe { market.borrow_unchecked() })?;
        let expected = if side == 0 { &m.base_mint } else { &m.quote_mint };
        if !eq_addr(mint_in.address().as_ref(), expected) {
            return Err(AmmError::IncorrectTokenAccountMint.into());
        }
        let delay = expiry_slot.saturating_sub(now);
        if expiry_slot <= now
            || delay < m.min_async_delay_slots as u64
            || delay > m.max_async_delay_slots as u64
        {
            return Err(AmmError::QuoteExpired.into());
        }
    }

    let bump_bytes = [bump];
    let derived = Address::derive_address(
        &[SEED_ASYNC_SWAP, user.address().as_ref(), market.address().as_ref()],
        Some(bump),
        &ID,
    );
    if async_swap.address() != &derived {
        return Err(AmmError::InvalidPdaBump.into());
    }

    let (escrow_mint, escrow_auth) = mint_and_owner(escrow)?;
    if !eq_addr(mint_in.address().as_ref(), &escrow_mint) {
        return Err(AmmError::IncorrectTokenAccountMint.into());
    }
    if !eq_addr(async_swap.address().as_ref(), &escrow_auth) {
        return Err(AmmError::InvalidDestination.into());
    }

    let lamports = super::rent_exempt(ASYNC_SWAP_LEN);
    let seeds = [
        Seed::from(SEED_ASYNC_SWAP),
        Seed::from(user.address().as_ref()),
        Seed::from(market.address().as_ref()),
        Seed::from(&bump_bytes[..]),
    ];
    let signer = [Signer::from(&seeds)];
    CreateAccount {
        from: user,
        to: async_swap,
        lamports,
        space: ASYNC_SWAP_LEN as u64,
        owner: &ID,
    }
    .invoke_signed(&signer)?;

    pinocchio_token::instructions::Transfer::new(user_src, escrow, user, amount_in).invoke()?;

    let mut user_pubkey:  [u8; 32] = [0; 32]; user_pubkey.copy_from_slice(user.address().as_ref());
    let mut market_pubkey: [u8; 32] = [0; 32]; market_pubkey.copy_from_slice(market.address().as_ref());
    let mut async_swap_pubkey: [u8; 32] = [0; 32]; async_swap_pubkey.copy_from_slice(async_swap.address().as_ref());

    {
        let mut async_swap_data = async_swap.try_borrow_mut()?;
        let s = init::<AsyncSwap>(&mut async_swap_data[..])?;
        unsafe {
            pubkey_copy_32(user.address().as_ref().as_ptr(),   s.user.as_mut_ptr());
            pubkey_copy_32(market.address().as_ref().as_ptr(), s.market.as_mut_ptr());
            pubkey_copy_32(escrow.address().as_ref().as_ptr(), s.user_src_escrow.as_mut_ptr());
        }
        s.user_dst_ata = [0; 32];
        s.amount_in    = amount_in;
        s.min_out      = min_out;
        s.expiry_slot  = expiry_slot;
        s.side         = side;
        s.status       = status::PENDING;
        s.bump         = bump;
        s._pad         = [0; 5];
    }

    let event = AsyncSwapCreatedEvent {
        tag: ASYNC_SWAP_CREATED_TAG,
        async_swap: async_swap_pubkey,
        user: user_pubkey,
        pool: market_pubkey,
        amount_in,
        min_out,
        expiry_slot,
        side: side as u64,
    };
    crate::helpers::event::emit(&event);
    Ok(())
}

pub fn process_execute(accounts: &mut [AccountView], _data: &[u8]) -> ProgramResult {
    let [maker, async_swap, escrow, user_dst, maker_src, maker_dst, market, book, token_program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    check_signer(maker)?;
    check_writable(async_swap)?;
    check_writable(escrow)?;
    check_writable(user_dst)?;
    check_writable(maker_src)?;
    check_writable(maker_dst)?;
    check_owner(market, &ID)?;
    check_owner(book, &ID)?;
    check_owner(async_swap, &ID)?;
    if !token_programs::is_token_program(token_program.address()) {
        return Err(AmmError::InvalidTokenProgram.into());
    }

    {
        let b = load::<MakerBook>(unsafe { book.borrow_unchecked() })?;
        if !eq_addr(maker.address().as_ref(), &b.maker) || !eq_addr(market.address().as_ref(), &b.market) {
            return Err(AmmError::UnauthorizedMaker.into());
        }
    }

    let (base_mint_key, quote_mint_key) = {
        let m = load::<Market>(unsafe { market.borrow_unchecked() })?;
        (m.base_mint, m.quote_mint)
    };

    let mut async_swap_pubkey: [u8; 32] = [0; 32]; async_swap_pubkey.copy_from_slice(async_swap.address().as_ref());
    let mut maker_pubkey:      [u8; 32] = [0; 32]; maker_pubkey.copy_from_slice(maker.address().as_ref());

    let (side, user_key, amount_in, min_out, bump) = {
        let async_swap_data = async_swap.try_borrow()?;
        let s = load::<AsyncSwap>(&async_swap_data[..])?;
        if s.status != status::PENDING {
            return Err(AmmError::AlreadyInitialized.into());
        }
        if !eq_addr(market.address().as_ref(), &s.market) {
            return Err(AmmError::InvalidAccountTag.into());
        }
        if !eq_addr(escrow.address().as_ref(), &s.user_src_escrow) {
            return Err(AmmError::InvalidDestination.into());
        }
        (s.side, s.user, s.amount_in, s.min_out, s.bump)
    };

    let now = Clock::get()?.slot;
    {
        let async_swap_data = async_swap.try_borrow()?;
        let s = load::<AsyncSwap>(&async_swap_data[..])?;
        if now > s.expiry_slot {
            return Err(AmmError::QuoteExpired.into());
        }
    }

    let input_mint  = if side == 0 { base_mint_key } else { quote_mint_key };
    let output_mint = if side == 0 { quote_mint_key } else { base_mint_key };

    let (escrow_mint, escrow_auth)        = mint_and_owner(escrow)?;
    let (user_dst_mint, user_dst_owner)   = mint_and_owner(user_dst)?;
    let (maker_src_mint, _)               = mint_and_owner(maker_src)?;
    let (maker_dst_mint, _)               = mint_and_owner(maker_dst)?;
    if !eq_addr(&input_mint, &escrow_mint) || !eq_addr(&output_mint, &maker_src_mint)
        || !eq_addr(&input_mint, &maker_dst_mint) || !eq_addr(&output_mint, &user_dst_mint)
    {
        return Err(AmmError::IncorrectTokenAccountMint.into());
    }
    if !eq_addr(async_swap.address().as_ref(), &escrow_auth) {
        return Err(AmmError::InvalidDestination.into());
    }
    if !eq_addr(&user_key, &user_dst_owner) {
        return Err(AmmError::InvalidDestination.into());
    }

    {
        let mut async_swap_data = async_swap.try_borrow_mut()?;
        let s = load_mut::<AsyncSwap>(&mut async_swap_data[..])?;
        s.status = status::FILLED;
        unsafe {
            pubkey_copy_32(user_dst.address().as_ref().as_ptr(), s.user_dst_ata.as_mut_ptr());
        }
    }

    pinocchio_token::instructions::Transfer::new(maker_src, user_dst, maker, min_out).invoke()?;

    let bump_bytes = [bump];
    let seeds = [
        Seed::from(SEED_ASYNC_SWAP),
        Seed::from(&user_key[..]),
        Seed::from(market.address().as_ref()),
        Seed::from(&bump_bytes[..]),
    ];
    let signer = [Signer::from(&seeds)];
    pinocchio_token::instructions::Transfer::new(escrow, maker_dst, async_swap, amount_in)
        .invoke_signed(&signer)?;

    let event = AsyncFillEvent {
        tag: ASYNC_FILL_EVENT_TAG,
        async_swap: async_swap_pubkey,
        maker: maker_pubkey,
        user: user_key,
        amount_out: min_out,
        slot: now,
    };
    crate::helpers::event::emit(&event);
    Ok(())
}

pub fn process_cancel(accounts: &mut [AccountView], _data: &[u8]) -> ProgramResult {
    let [caller, async_swap, escrow, user_src, token_program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    check_signer(caller)?;
    check_writable(async_swap)?;
    check_writable(escrow)?;
    check_writable(user_src)?;
    check_owner(async_swap, &ID)?;
    if !token_programs::is_token_program(token_program.address()) {
        return Err(AmmError::InvalidTokenProgram.into());
    }

    let mut async_swap_data = async_swap.try_borrow_mut()?;
    let s = load_mut::<AsyncSwap>(&mut async_swap_data[..])?;
    if s.status != status::PENDING {
        return Err(AmmError::AlreadyInitialized.into());
    }
    let now = Clock::get()?.slot;
    if now <= s.expiry_slot {
        return Err(AmmError::QuoteExpired.into());
    }
    if !eq_addr(escrow.address().as_ref(), &s.user_src_escrow) {
        return Err(AmmError::InvalidDestination.into());
    }
    if user_src.address() == escrow.address() {
        return Err(AmmError::InvalidDestination.into());
    }

    let user_pubkey:   [u8; 32] = s.user;
    let market_pubkey: [u8; 32] = s.market;
    let amount = s.amount_in;
    let bump = s.bump;
    s.status = status::CANCELLED;
    drop(async_swap_data);

    let (_user_src_mint, user_src_owner) = mint_and_owner(user_src)?;
    if !eq_addr(&user_pubkey, &user_src_owner) {
        return Err(AmmError::InvalidDestination.into());
    }

    let bump_bytes = [bump];
    let seeds = [
        Seed::from(SEED_ASYNC_SWAP),
        Seed::from(&user_pubkey[..]),
        Seed::from(&market_pubkey[..]),
        Seed::from(&bump_bytes[..]),
    ];
    let signer = [Signer::from(&seeds)];
    pinocchio_token::instructions::Transfer::new(escrow, user_src, async_swap, amount)
        .invoke_signed(&signer)?;

    let mut async_swap_pubkey = [0u8; 32]; async_swap_pubkey.copy_from_slice(async_swap.address().as_ref());
    let event = AsyncCancelEvent {
        tag: ASYNC_CANCEL_EVENT_TAG,
        async_swap: async_swap_pubkey,
        user: user_pubkey,
        refunded: amount,
        slot: now,
    };
    crate::helpers::event::emit(&event);
    Ok(())
}
