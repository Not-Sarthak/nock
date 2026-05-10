use pinocchio::error::ProgramError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum AmmError {
    InvalidAccountTag        = 0x100,
    AlreadyInitialized       = 0x101,
    UninitializedAccount     = 0x102,
    InvalidTokenProgram      = 0x103,
    InvalidSystemProgram     = 0x104,
    InvalidAtaProgram        = 0x105,
    InvalidArcherTreasury    = 0x106,
    InvalidTokenMintOwner    = 0x107,
    IncorrectTokenAccountMint = 0x108,
    OwnerNotProgram          = 0x109,
    InvalidPdaBump           = 0x10a,

    InvalidMakerSignature    = 0x1f0,
    UnauthorizedMaker        = 0x1f6,
    QuoteExpired             = 0x1f7,
    QuoteSizeTooSmall        = 0x1f8,
    QuoteIndexOutOfBounds    = 0x1f9,
    StaleSequence            = 0x1fd,

    SlippageExceeded         = 0x200,
    AmountInZero             = 0x201,
    InsufficientVaultBalance = 0x202,
    PriceMathOverflow        = 0x203,

    MarketPaused             = 0x300,
    MarketNotPaused          = 0x301,
    InvalidMarketConfig      = 0x302,
}

impl From<AmmError> for ProgramError {
    #[inline]
    fn from(e: AmmError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
