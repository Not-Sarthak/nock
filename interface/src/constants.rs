use pinocchio::Address;

/// Price levels per side of a maker book.
pub const MAX_LEVELS: usize = 16;

// Account discriminators. The trailing digit is the layout version.
pub const MARKET_TAG: [u8; 8] = *b"NOCKMKT2";
pub const MAKER_BOOK_TAG: [u8; 8] = *b"NOCKMKR2";
pub const ASYNC_SWAP_TAG: [u8; 8] = *b"NOCKASW2";

pub const MARKET_LEN: usize = 280;
pub const MAKER_BOOK_LEN: usize = 704;
pub const ASYNC_SWAP_LEN: usize = 184;

pub const SEED_MARKET: &[u8] = b"market";
pub const SEED_MAKER_BOOK: &[u8] = b"maker";
pub const SEED_ASYNC_SWAP: &[u8] = b"async";
pub const SEED_EVENT_AUTHORITY: &[u8] = b"event_authority";

/// Canonical bump of the event-authority PDA under `ID`. `make set-program-id` rewrites it
/// together with the program id; `tests/src/constants.rs` fails if the two drift apart.
pub const EVENT_AUTHORITY_BUMP: u8 = 255;

/// PDA that signs the self-CPI carrying every event, so only this program can emit one.
pub const EVENT_AUTHORITY: Address = Address::derive_address_const(
    &[SEED_EVENT_AUTHORITY],
    Some(EVENT_AUTHORITY_BUMP),
    &crate::ID,
);

/// Hard ceiling on either fee rate: 10% of notional. Compile-time, so no admin key can exceed it.
pub const MAX_FEE_PPM: u32 = 100_000;

/// Fee rates are parts per million of the gross quote notional.
pub const PPM_DENOM: u64 = 1_000_000;

/// Longest quote lifetime a maker may configure: 10 slots, about 4 seconds.
pub const MAX_QUOTE_AGE_SLOTS: u64 = 10;

/// Quote lifetime a fresh book starts with.
pub const DEFAULT_EXPIRY_IN_SLOTS: u64 = MAX_QUOTE_AGE_SLOTS;

/// Largest forward step one sequence bump may take (2^32). Wide enough for a millisecond-clock
/// sequence across ~49 days of downtime; pinning the counter near u64::MAX would take 2^32
/// transactions, and the maker can always reset it.
pub const MAX_SEQUENCE_JUMP: u64 = 1 << 32;

/// Expiry is exclusive, so one slot of lifetime is the least that leaves a slot to execute in.
pub const MIN_ASYNC_LIFETIME_SLOTS: u16 = 1;

/// Largest mint decimals for which 10^decimals still fits in a u64.
pub const MAX_MINT_DECIMALS: u8 = 19;

/// Mint decimals are base-10 exponents.
pub const DECIMAL_RADIX: u64 = 10;

/// The all-zero address (the system program id) can never sign, so it marks
/// "no delegate" on a book and "any maker" on an async swap.
pub const NO_ADDRESS: [u8; 32] = [0; 32];

/// Upper bound on a serialized event, comfortably above the largest one; every event asserts it fits.
pub const MAX_EVENT_LEN: usize = 512;

pub const SYSTEM: Address = Address::from_str_const("11111111111111111111111111111111");
pub const ATA: Address = Address::from_str_const("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const SPL_TOKEN: Address =
    Address::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const TOKEN_2022: Address =
    Address::from_str_const("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
