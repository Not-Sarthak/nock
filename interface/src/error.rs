use pinocchio::error::ProgramError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum NockError {
    InvalidTokenProgram = 0,
    Token2022Unsupported = 1,
    InvalidMint = 2,
    IncorrectVault = 3,
    IncorrectTokenAccount = 4,
    InvalidPda = 5,
    MarketMismatch = 6,
    UnauthorizedAdmin = 7,
    UnauthorizedMaker = 8,
    MarketPaused = 9,
    InvalidMarketConfig = 10,
    InvalidFee = 11,
    BookHalted = 12,
    QuoteExpired = 13,
    StaleSequence = 14,
    SequenceJumpTooLarge = 15,
    SequenceMismatch = 16,
    InvalidQuotes = 17,
    OutsideDelegateBounds = 18,
    InvalidExpiry = 19,
    AmountZero = 20,
    InsufficientLiquidity = 21,
    SlippageExceeded = 22,
    InsufficientInventory = 23,
    MathOverflow = 24,
    QuoteSizeTooSmall = 25,
    InvalidSide = 26,
    AsyncWindowViolated = 27,
    AsyncSwapExpired = 28,
    AsyncSwapNotExpired = 29,
    MakerNotSelected = 30,
    BookNotEmpty = 31,
    InvalidEventAuthority = 32,
    NoPendingAdmin = 33,
}

impl From<NockError> for ProgramError {
    #[inline]
    fn from(e: NockError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
