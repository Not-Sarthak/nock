use pinocchio::Address;

pub const MAX_LEVELS: usize = 16;

pub const MARKET_TAG:     [u8; 8] = *b"NOCKMKT1";
pub const MAKER_BOOK_TAG: [u8; 8] = *b"NOCKMKR1";
pub const ASYNC_SWAP_TAG: [u8; 8] = *b"NOCKASW1";

pub const POOL_CREATED_EVENT_TAG: [u8; 8] = *b"NOCKPC01";
pub const BOOK_UPDATE_EVENT_TAG:  [u8; 8] = *b"NOCKBKU1";
pub const SYNC_FILL_EVENT_TAG:    [u8; 8] = *b"NOCKSFL1";
pub const ASYNC_SWAP_CREATED_TAG: [u8; 8] = *b"NOCKAOC1";
pub const ASYNC_FILL_EVENT_TAG:   [u8; 8] = *b"NOCKAFL1";
pub const ASYNC_CANCEL_EVENT_TAG: [u8; 8] = *b"NOCKAOX1";

pub const MARKET_LEN:     usize = 272;
pub const MAKER_BOOK_LEN: usize = 776;
pub const ASYNC_SWAP_LEN: usize = 168;

pub const CREATE_POOL_DATA_LEN:       usize = 200;
pub const UPDATE_MAKER_BOOK_DATA_LEN: usize = 32 + (MAX_LEVELS * 2 * 16);

pub const SYNC_FILL_EVENT_LEN: usize = 32 + 32 + 32 + 8 * 5 + 8 + 8;

pub const SEED_MARKET:     &[u8] = b"market";
pub const SEED_MAKER_BOOK: &[u8] = b"maker";
pub const SEED_ASYNC_SWAP: &[u8] = b"async";

pub const MAX_FEE_PPM: i32 = 100_000;
pub const PPM_DENOM:   u64 = 1_000_000;

pub const MAX_QUOTE_AGE_SLOTS: u64 = 10;

pub const MINT_DECIMALS_OFFSET: usize = 44;
pub const MINT_MIN_LEN:         usize = 82;

pub const SYSTEM:    Address = Address::from_str_const("11111111111111111111111111111111");
pub const ATA:       Address = Address::from_str_const("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const SPL_TOKEN: Address = Address::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
