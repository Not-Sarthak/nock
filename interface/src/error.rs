use pinocchio::error::ProgramError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum AmmError {
    InvalidAccountTag         = 0x100,
    InvalidTokenProgram       = 0x103,
    IncorrectTokenAccountMint = 0x108,
    InvalidPdaBump            = 0x10a,
    InvalidDestination        = 0x10b,

    UnauthorizedMaker     = 0x1f6,
    QuoteExpired          = 0x1f7,
    QuoteSizeTooSmall     = 0x1f8,
    QuoteIndexOutOfBounds = 0x1f9,
    StaleSequence         = 0x1fd,

    SlippageExceeded         = 0x200,
    AmountInZero             = 0x201,
    InsufficientVaultBalance = 0x202,
    PriceMathOverflow        = 0x203,

    MarketPaused        = 0x300,
    InvalidMarketConfig = 0x302,
    BookHalted          = 0x303,

    SwapNotPending = 0x400,
}

impl From<AmmError> for ProgramError {
    #[inline]
    fn from(e: AmmError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
