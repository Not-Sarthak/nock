use nock_interface::{
    constants::{SEED_ASYNC_SWAP, SEED_MARKET},
    error::NockError,
    state::{AsyncSwap, Market},
    ID,
};
use pinocchio::{
    account::AccountView,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{
        get_sysvar,
        rent::{ACCOUNT_STORAGE_OVERHEAD, RENT_ID},
    },
    Address, ProgramResult,
};
use pinocchio_system::instructions::{Allocate, Assign, CreateAccount, Transfer};

/// Bytes of the rent sysvar read: lamports per byte-year (u64) then exemption threshold
/// in years (f64). The trailing burn percent is not needed.
const RENT_PREFIX_LEN: usize = 16;
/// Split point between the two fields above.
const RENT_RATE_LEN: usize = 8;

/// Rent-exempt minimum for `len` bytes, computed exactly as the runtime does
/// (`(overhead + len) * lamports_per_byte_year * exemption_threshold`), so it is right
/// both before and after the threshold is folded into the rate (SIMD-0194).
#[inline]
pub fn rent_exempt(len: usize) -> Result<u64, ProgramError> {
    let mut buf = [0u8; RENT_PREFIX_LEN];
    get_sysvar(&mut buf, &RENT_ID, 0)?;
    let (rate, threshold) = buf.split_at(RENT_RATE_LEN);
    let lamports_per_byte_year = u64::from_le_bytes(
        rate.try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?,
    );
    let exemption_threshold = f64::from_le_bytes(
        threshold
            .try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?,
    );
    let bytes = ACCOUNT_STORAGE_OVERHEAD
        .checked_add(len as u64)
        .ok_or(NockError::MathOverflow)?;
    let per_year = bytes
        .checked_mul(lamports_per_byte_year)
        .ok_or(NockError::MathOverflow)?;
    Ok((per_year as f64 * exemption_threshold) as u64)
}

/// Requires `target` to be the canonical PDA for `seeds` and returns its bump.
#[inline]
pub fn find_canonical(seeds: &[&[u8]], target: &AccountView) -> Result<u8, ProgramError> {
    let (address, bump) =
        Address::try_find_program_address(seeds, &ID).ok_or(NockError::InvalidPda)?;
    if target.address() != &address {
        return Err(NockError::InvalidPda.into());
    }
    Ok(bump)
}

/// Creates a program-owned PDA. Tolerates lamports sent to the address beforehand, which
/// would otherwise make `CreateAccount` fail forever.
pub fn create_pda(
    payer: &AccountView,
    target: &AccountView,
    lamports: u64,
    space: u64,
    owner: &Address,
    signer: &[Signer],
) -> ProgramResult {
    let existing = target.lamports();

    if existing == 0 {
        return CreateAccount {
            from: payer,
            to: target,
            lamports,
            space,
            owner,
        }
        .invoke_signed(signer);
    }

    if let Some(shortfall) = lamports.checked_sub(existing) {
        if shortfall > 0 {
            Transfer {
                from: payer,
                to: target,
                lamports: shortfall,
            }
            .invoke()?;
        }
    }

    Allocate {
        account: target,
        space,
    }
    .invoke_signed(signer)?;
    Assign {
        account: target,
        owner,
    }
    .invoke_signed(signer)
}

/// Moves every lamport to `refund_to` and closes `target`. Returns the lamports moved.
pub fn close_pda(
    target: &mut AccountView,
    refund_to: &mut AccountView,
) -> Result<u64, ProgramError> {
    let refund = target.lamports();
    let credited = refund_to
        .lamports()
        .checked_add(refund)
        .ok_or(NockError::MathOverflow)?;
    refund_to.set_lamports(credited);
    target.set_lamports(0);
    target.close()?;
    Ok(refund)
}

/// Signer seeds of a market PDA, owned so the `Seed`s can borrow them.
pub struct MarketSeeds {
    base_mint: [u8; 32],
    quote_mint: [u8; 32],
    seed_authority: [u8; 32],
    bump: [u8; 1],
}

impl MarketSeeds {
    #[inline]
    pub fn new(m: &Market) -> Self {
        Self {
            base_mint: m.base_mint,
            quote_mint: m.quote_mint,
            seed_authority: m.seed_authority,
            bump: [m.market_bump],
        }
    }

    #[inline]
    pub fn seeds(&self) -> [Seed<'_>; 5] {
        [
            Seed::from(SEED_MARKET),
            Seed::from(&self.base_mint[..]),
            Seed::from(&self.quote_mint[..]),
            Seed::from(&self.seed_authority[..]),
            Seed::from(&self.bump[..]),
        ]
    }
}

/// Signer seeds of an async-swap PDA.
pub struct AsyncSwapSeeds {
    market: [u8; 32],
    user: [u8; 32],
    nonce: [u8; 8],
    bump: [u8; 1],
}

impl AsyncSwapSeeds {
    #[inline]
    pub fn new(s: &AsyncSwap) -> Self {
        Self {
            market: s.market,
            user: s.user,
            nonce: s.nonce.to_le_bytes(),
            bump: [s.bump],
        }
    }

    #[inline]
    pub fn seeds(&self) -> [Seed<'_>; 5] {
        [
            Seed::from(SEED_ASYNC_SWAP),
            Seed::from(&self.market[..]),
            Seed::from(&self.user[..]),
            Seed::from(&self.nonce[..]),
            Seed::from(&self.bump[..]),
        ]
    }
}
