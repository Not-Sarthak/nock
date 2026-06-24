use pinocchio::{
    account::AccountView,
    cpi::{Seed, Signer},
    error::ProgramError,
    Address, ProgramResult,
};
use pinocchio_associated_token_account::instructions::CreateIdempotent;
use pinocchio_system::instructions::CreateAccount;
use nock_interface::{
    constants::{
        ATA, MARKET_LEN, MAX_FEE_PPM, POOL_CREATED_EVENT_TAG, SEED_MARKET, SYSTEM,
    },
    error::AmmError,
    events::PoolCreatedEvent,
    instruction::CreatePoolData,
    state::{init, Market},
    ID,
};
use crate::helpers::assertions::{check_pubkey, check_signer, check_writable};
use crate::helpers::unsafe_eq::eq_addr;
use super::token_programs;

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [
        payer,
        market,
        base_mint,
        quote_mint,
        base_vault,
        quote_vault,
        system_program,
        base_token_program,
        quote_token_program,
        ata_program,
        _rent_sysvar,
        ..
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if data.len() < core::mem::size_of::<CreatePoolData>() {
        return Err(ProgramError::InvalidInstructionData);
    }
    let args: &CreatePoolData = unsafe { &*(data.as_ptr() as *const CreatePoolData) };

    check_signer(payer)?;
    check_writable(market)?;
    check_writable(base_vault)?;
    check_writable(quote_vault)?;
    check_pubkey(system_program, &SYSTEM)?;
    check_pubkey(ata_program, &ATA)?;
    if !token_programs::is_token_program(base_token_program.address())
        || !token_programs::is_token_program(quote_token_program.address())
    {
        return Err(AmmError::InvalidTokenProgram.into());
    }
    if !eq_addr(base_token_program.address().as_ref(), &args.base_token_program)
        || !eq_addr(quote_token_program.address().as_ref(), &args.quote_token_program)
    {
        return Err(AmmError::InvalidTokenProgram.into());
    }
    if !eq_addr(base_mint.address().as_ref(), &args.base_mint)
        || !eq_addr(quote_mint.address().as_ref(), &args.quote_mint)
    {
        return Err(AmmError::IncorrectTokenAccountMint.into());
    }

    let tick_size = args.tick_size_in_quote_atoms_per_base_unit;
    let maker_fee_ppm = args.maker_fee_ppm;
    let taker_fee_ppm = args.taker_fee_ppm;
    if tick_size == 0 || args.base_atoms_per_base_lot == 0 || args.quote_atoms_per_quote_lot == 0 {
        return Err(AmmError::InvalidMarketConfig.into());
    }
    if maker_fee_ppm.abs() > MAX_FEE_PPM || taker_fee_ppm.abs() > MAX_FEE_PPM {
        return Err(AmmError::InvalidMarketConfig.into());
    }
    if args.min_async_delay_slots > args.max_async_delay_slots {
        return Err(AmmError::InvalidMarketConfig.into());
    }
    if eq_addr(base_mint.address().as_ref(), &args.quote_mint) {
        return Err(AmmError::InvalidMarketConfig.into());
    }

    let market_bump = args.market_bump;
    let bump_bytes = [market_bump];
    let derived = Address::derive_address(
        &[
            SEED_MARKET,
            base_mint.address().as_ref(),
            quote_mint.address().as_ref(),
            &args.market_authority,
        ],
        Some(market_bump),
        &ID,
    );
    if market.address() != &derived {
        return Err(AmmError::InvalidPdaBump.into());
    }

    let lamports = super::rent_exempt(MARKET_LEN);
    let market_seeds = [
        Seed::from(SEED_MARKET),
        Seed::from(base_mint.address().as_ref()),
        Seed::from(quote_mint.address().as_ref()),
        Seed::from(&args.market_authority[..]),
        Seed::from(&bump_bytes[..]),
    ];
    let market_signer = [Signer::from(&market_seeds)];
    CreateAccount {
        from: payer,
        to: market,
        lamports,
        space: MARKET_LEN as u64,
        owner: &ID,
    }
    .invoke_signed(&market_signer)?;

    CreateIdempotent {
        funding_account: payer,
        account: base_vault,
        wallet: market,
        mint: base_mint,
        system_program,
        token_program: base_token_program,
    }
    .invoke()?;
    CreateIdempotent {
        funding_account: payer,
        account: quote_vault,
        wallet: market,
        mint: quote_mint,
        system_program,
        token_program: quote_token_program,
    }
    .invoke()?;

    let mut market_pubkey:     [u8; 32] = [0; 32];
    let mut base_mint_pubkey:  [u8; 32] = [0; 32];
    let mut quote_mint_pubkey: [u8; 32] = [0; 32];
    let mut admin_pubkey:      [u8; 32] = [0; 32];
    market_pubkey.copy_from_slice(market.address().as_ref());
    base_mint_pubkey.copy_from_slice(base_mint.address().as_ref());
    quote_mint_pubkey.copy_from_slice(quote_mint.address().as_ref());
    admin_pubkey.copy_from_slice(&args.market_authority);

    let market_id_pk:  [u8; 32] = market.address().as_ref().try_into().unwrap();
    let base_mint_pk:  [u8; 32] = base_mint.address().as_ref().try_into().unwrap();
    let quote_mint_pk: [u8; 32] = quote_mint.address().as_ref().try_into().unwrap();
    let base_vault_pk: [u8; 32] = base_vault.address().as_ref().try_into().unwrap();
    let quote_vault_pk:[u8; 32] = quote_vault.address().as_ref().try_into().unwrap();
    let mut market_data = market.try_borrow_mut()?;
    let market_state = init::<Market>(&mut market_data[..])?;
    market_state.market_id   = market_id_pk;
    market_state.base_mint   = base_mint_pk;
    market_state.quote_mint  = quote_mint_pk;
    market_state.base_vault  = base_vault_pk;
    market_state.quote_vault = quote_vault_pk;
    market_state.admin       = args.market_authority;
    market_state.base_atoms_per_base_lot                = args.base_atoms_per_base_lot;
    market_state.quote_atoms_per_quote_lot              = args.quote_atoms_per_quote_lot;
    market_state.tick_size_in_quote_atoms_per_base_unit = tick_size;
    market_state.raw_base_units_per_base_unit           = args.raw_base_units_per_base_unit;
    market_state.uncollected_fees_quote_lots            = 0;
    market_state.collected_fees_quote_lots              = 0;
    market_state.maker_fee_ppm                          = maker_fee_ppm;
    market_state.taker_fee_ppm                          = taker_fee_ppm;
    market_state.base_decimals                          = args.base_decimals;
    market_state.quote_decimals                         = args.quote_decimals;
    market_state.status                                 = 0;
    market_state.mode                                   = args.mode;
    market_state.market_bump                            = market_bump;
    market_state.sync_fee_multiplier                    = args.sync_fee_multiplier;
    market_state.min_async_delay_slots                  = args.min_async_delay_slots;
    market_state.max_async_delay_slots                  = args.max_async_delay_slots;
    market_state._reserved                              = 0;

    drop(market_data);
    let event = PoolCreatedEvent {
        tag: POOL_CREATED_EVENT_TAG,
        pool: market_pubkey,
        mint_a: base_mint_pubkey,
        mint_b: quote_mint_pubkey,
        admin: admin_pubkey,
        price_scale: tick_size,
        fee_bps: taker_fee_ppm as u64,
    };
    crate::helpers::event::emit(&event);
    Ok(())
}
