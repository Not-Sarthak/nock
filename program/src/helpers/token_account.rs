use nock_interface::{constants::TOKEN_2022, error::NockError};
use pinocchio::{account::AccountView, error::ProgramError};
use pinocchio_token::state::{Account as TokenAccount, Mint};

/// The fields of an SPL token account this program reads.
pub struct TokenInfo {
    pub mint: [u8; 32],
    pub owner: [u8; 32],
    pub amount: u64,
}

/// Reads an initialized legacy SPL token account.
pub fn read_token_account(acc: &AccountView) -> Result<TokenInfo, ProgramError> {
    if acc.owned_by(&TOKEN_2022) {
        return Err(NockError::Token2022Unsupported.into());
    }
    let account =
        TokenAccount::from_account_view(acc).map_err(|_| NockError::IncorrectTokenAccount)?;
    if !account.is_initialized() {
        return Err(NockError::IncorrectTokenAccount.into());
    }
    Ok(TokenInfo {
        mint: account.mint().to_bytes(),
        owner: account.owner().to_bytes(),
        amount: account.amount(),
    })
}

/// Reads a token account and requires it to hold `mint` for `owner`.
pub fn check_token_account(
    acc: &AccountView,
    mint: &[u8; 32],
    owner: &[u8; 32],
) -> Result<TokenInfo, ProgramError> {
    let info = read_token_account(acc)?;
    if &info.mint != mint || &info.owner != owner {
        return Err(NockError::IncorrectTokenAccount.into());
    }
    Ok(info)
}

/// Decimals of an initialized legacy SPL mint.
pub fn read_mint_decimals(acc: &AccountView) -> Result<u8, ProgramError> {
    if acc.owned_by(&TOKEN_2022) {
        return Err(NockError::Token2022Unsupported.into());
    }
    let mint = Mint::from_account_view(acc).map_err(|_| NockError::InvalidMint)?;
    if !mint.is_initialized() {
        return Err(NockError::InvalidMint.into());
    }
    Ok(mint.decimals())
}
