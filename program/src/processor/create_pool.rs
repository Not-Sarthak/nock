use crate::helpers::assertions::{check_pubkey, check_signer, check_token_program, check_writable};
use crate::helpers::event::{check_event_accounts, emit};
use crate::helpers::pda::{create_pda, find_canonical, rent_exempt};
use crate::helpers::token_account::read_mint_decimals;
use nock_interface::{
    constants::{
        ATA, DECIMAL_RADIX, MARKET_LEN, MAX_FEE_PPM, MAX_MINT_DECIMALS, MIN_ASYNC_LIFETIME_SLOTS,
        NO_ADDRESS, SEED_MARKET, SYSTEM,
    },
    error::NockError,
    events::{tag, PoolCreatedEvent},
    instruction::{parse, CreatePoolData},
    state::{init, market::market_status, Market},
    ID,
};
use pinocchio::{
    account::AccountView,
    cpi::{Seed, Signer},
    error::ProgramError,
    ProgramResult,
};
use pinocchio_associated_token_account::instructions::CreateIdempotent;

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [payer, authority, market, base_mint, quote_mint, base_vault, quote_vault, system_program, token_program, ata_program, event_authority, program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let args = parse::<CreatePoolData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let tick_size = args.tick_size_in_quote_atoms_per_base_unit;
    let base_atoms_per_base_lot = args.base_atoms_per_base_lot;
    let quote_atoms_per_quote_lot = args.quote_atoms_per_quote_lot;
    let maker_fee_ppm = args.maker_fee_ppm;
    let taker_fee_ppm = args.taker_fee_ppm;
    let min_async_delay_slots = args.min_async_delay_slots;
    let max_async_delay_slots = args.max_async_delay_slots;

    check_signer(payer)?;
    check_writable(payer)?;
    check_signer(authority)?;
    check_writable(market)?;
    check_writable(base_vault)?;
    check_writable(quote_vault)?;
    check_pubkey(system_program, &SYSTEM)?;
    check_pubkey(ata_program, &ATA)?;
    check_token_program(token_program)?;
    check_event_accounts(event_authority, program)?;

    if base_mint.address() == quote_mint.address() {
        return Err(NockError::InvalidMarketConfig.into());
    }
    if tick_size == 0 || base_atoms_per_base_lot == 0 || quote_atoms_per_quote_lot == 0 {
        return Err(NockError::InvalidMarketConfig.into());
    }
    if maker_fee_ppm > MAX_FEE_PPM || taker_fee_ppm > MAX_FEE_PPM {
        return Err(NockError::InvalidFee.into());
    }
    if min_async_delay_slots < MIN_ASYNC_LIFETIME_SLOTS
        || min_async_delay_slots > max_async_delay_slots
    {
        return Err(NockError::InvalidMarketConfig.into());
    }

    let base_decimals = read_mint_decimals(base_mint)?;
    read_mint_decimals(quote_mint)?;
    if base_decimals > MAX_MINT_DECIMALS {
        return Err(NockError::InvalidMint.into());
    }
    let base_atoms_per_base_unit = DECIMAL_RADIX.pow(base_decimals as u32);

    let bump = find_canonical(
        &[
            SEED_MARKET,
            base_mint.address().as_ref(),
            quote_mint.address().as_ref(),
            authority.address().as_ref(),
        ],
        market,
    )?;
    let bump_bytes = [bump];
    let market_seeds = [
        Seed::from(SEED_MARKET),
        Seed::from(base_mint.address().as_ref()),
        Seed::from(quote_mint.address().as_ref()),
        Seed::from(authority.address().as_ref()),
        Seed::from(&bump_bytes[..]),
    ];
    create_pda(
        payer,
        market,
        rent_exempt(MARKET_LEN)?,
        MARKET_LEN as u64,
        &ID,
        &[Signer::from(&market_seeds)],
    )?;

    CreateIdempotent {
        funding_account: payer,
        account: base_vault,
        wallet: market,
        mint: base_mint,
        system_program,
        token_program,
    }
    .invoke()?;
    CreateIdempotent {
        funding_account: payer,
        account: quote_vault,
        wallet: market,
        mint: quote_mint,
        system_program,
        token_program,
    }
    .invoke()?;

    {
        let mut market_data = market.try_borrow_mut()?;
        let m = init::<Market>(&mut market_data[..])?;
        m.base_mint = base_mint.address().to_bytes();
        m.quote_mint = quote_mint.address().to_bytes();
        m.base_vault = base_vault.address().to_bytes();
        m.quote_vault = quote_vault.address().to_bytes();
        m.seed_authority = authority.address().to_bytes();
        m.admin = authority.address().to_bytes();
        m.pending_admin = NO_ADDRESS;
        m.base_atoms_per_base_lot = base_atoms_per_base_lot;
        m.quote_atoms_per_quote_lot = quote_atoms_per_quote_lot;
        m.tick_size_in_quote_atoms_per_base_unit = tick_size;
        m.base_atoms_per_base_unit = base_atoms_per_base_unit;
        m.maker_fee_ppm = maker_fee_ppm;
        m.taker_fee_ppm = taker_fee_ppm;
        m.min_async_delay_slots = min_async_delay_slots;
        m.max_async_delay_slots = max_async_delay_slots;
        m.status = market_status::ACTIVE;
        m.market_bump = bump;
        m._pad = [0; 2];
    }

    emit(
        &PoolCreatedEvent {
            tag: tag::POOL_CREATED,
            market: market.address().to_bytes(),
            base_mint: base_mint.address().to_bytes(),
            quote_mint: quote_mint.address().to_bytes(),
            admin: authority.address().to_bytes(),
            tick_size_in_quote_atoms_per_base_unit: tick_size,
            base_atoms_per_base_lot,
            quote_atoms_per_quote_lot,
            maker_fee_ppm,
            taker_fee_ppm,
            min_async_delay_slots,
            max_async_delay_slots,
            _pad: [0; 4],
        },
        event_authority,
    )
}
