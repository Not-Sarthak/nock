//! the generated client must speak the program's wire format: one discriminator byte then
//! packed little-endian arguments, accounts in the documented order with the fixed ones
//! filled in, and account layouts at the program's exact sizes and offsets.

use nock_client::accounts::{
    AsyncSwap, MakerBook, Market, ASYNC_SWAP_DISCRIMINATOR, MAKER_BOOK_DISCRIMINATOR,
    MARKET_DISCRIMINATOR,
};
use nock_client::errors::NockError;
use nock_client::instructions::{CreatePoolBuilder, SyncSwapBuilder, UpdateMakerBookBuilder};
use nock_client::types::MakerLevel;
use nock_client::NOCK_ID;
use solana_address::{address, Address};

const EVENT_AUTHORITY: Address = address!("3nRFM7xw5Xowb3Wzpzambay1Td57SVaVYQkjVzTSc7KC");
const SYSTEM: Address = address!("11111111111111111111111111111111");
const TOKEN: Address = address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA: Address = address!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

fn a(b: u8) -> Address {
    Address::new_from_array([b; 32])
}

#[test]
fn sync_swap_encodes_packed_args_and_fills_fixed_accounts() {
    let ix = SyncSwapBuilder::new()
        .user(a(1))
        .market(a(2))
        .book(a(3))
        .base_vault(a(4))
        .quote_vault(a(5))
        .user_base(a(6))
        .user_quote(a(7))
        .side(1)
        .pin_sequence(1)
        .base_lots(7)
        .limit_quote_lots(8)
        .price_limit_ticks(9)
        .expected_sequence(10)
        .instruction();
    assert_eq!(ix.program_id, NOCK_ID);
    let mut expected = vec![0x10, 1, 1];
    for v in [7u64, 8, 9, 10] {
        expected.extend_from_slice(&v.to_le_bytes());
    }
    assert_eq!(ix.data, expected);
    assert_eq!(ix.accounts.len(), 10);
    assert!(ix.accounts[0].is_signer && !ix.accounts[0].is_writable);
    assert!(
        !ix.accounts[1].is_writable,
        "the market is read-only on a fill"
    );
    assert!(ix.accounts[2].is_writable);
    assert_eq!(ix.accounts[7].pubkey, TOKEN);
    assert_eq!(ix.accounts[8].pubkey, EVENT_AUTHORITY);
    assert_eq!(ix.accounts[9].pubkey, NOCK_ID);
}

#[test]
fn update_maker_book_encodes_the_header_then_both_ladders() {
    let mut bids: [MakerLevel; 16] = core::array::from_fn(|_| MakerLevel {
        size_in_base_lots: 0,
        price_offset_ticks: 0,
    });
    bids[0] = MakerLevel {
        size_in_base_lots: 5,
        price_offset_ticks: -3,
    };
    let asks = bids.clone();
    let ix = UpdateMakerBookBuilder::new()
        .signer(a(1))
        .book(a(2))
        .sequence_number(11)
        .mid_price_ticks(1_000)
        .num_bids(1)
        .num_asks(1)
        .bid_levels(bids)
        .ask_levels(asks)
        .instruction();
    // disc + 24-byte header + 2 x 16 levels x 16 bytes
    assert_eq!(ix.data.len(), 1 + 24 + 2 * 16 * 16);
    assert_eq!(ix.data[0], 0x07);
    assert_eq!(&ix.data[1..9], &11u64.to_le_bytes());
    assert_eq!(&ix.data[9..17], &1_000u64.to_le_bytes());
    assert_eq!((ix.data[17], ix.data[18]), (1, 1));
    assert_eq!(&ix.data[19..25], &[0; 6]);
    assert_eq!(&ix.data[25..33], &5u64.to_le_bytes());
    assert_eq!(&ix.data[33..41], &(-3i64).to_le_bytes());
    assert_eq!(&ix.data[25 + 256..33 + 256], &5u64.to_le_bytes());
    assert_eq!(ix.accounts.len(), 4);
}

#[test]
fn create_pool_defaults_the_program_accounts() {
    let ix = CreatePoolBuilder::new()
        .payer(a(1))
        .authority(a(2))
        .market(a(3))
        .base_mint(a(4))
        .quote_mint(a(5))
        .base_vault(a(6))
        .quote_vault(a(7))
        .tick_size_in_quote_atoms_per_base_unit(1)
        .base_atoms_per_base_lot(2)
        .quote_atoms_per_quote_lot(3)
        .maker_fee_ppm(4)
        .taker_fee_ppm(5)
        .min_async_delay_slots(6)
        .max_async_delay_slots(7)
        .instruction();
    assert_eq!(ix.data.len(), 1 + 36);
    assert_eq!(ix.data[0], 0x00);
    assert!(ix.accounts[1].is_signer, "the market authority signs");
    let fixed: Vec<Address> = ix.accounts[7..].iter().map(|m| m.pubkey).collect();
    assert_eq!(fixed, vec![SYSTEM, TOKEN, ATA, EVENT_AUTHORITY, NOCK_ID]);
}

#[test]
fn accounts_decode_at_the_program_sizes_and_offsets() {
    assert_eq!(
        (Market::LEN, MakerBook::LEN, AsyncSwap::LEN),
        (280, 704, 184)
    );
    assert_eq!(&MARKET_DISCRIMINATOR, b"NOCKMKT2");
    assert_eq!(&MAKER_BOOK_DISCRIMINATOR, b"NOCKMKR2");
    assert_eq!(&ASYNC_SWAP_DISCRIMINATOR, b"NOCKASW2");

    let mut data = vec![0u8; MakerBook::LEN];
    data[..8].copy_from_slice(&MAKER_BOOK_DISCRIMINATOR);
    data[8..40].copy_from_slice(&[9; 32]);
    data[136..144].copy_from_slice(&42u64.to_le_bytes());
    data[186..188].copy_from_slice(&7u16.to_le_bytes());
    data[192..200].copy_from_slice(&3u64.to_le_bytes());
    data[448 + 8..448 + 16].copy_from_slice(&(-1i64).to_le_bytes());
    let book = MakerBook::from_bytes(&data).unwrap();
    assert_eq!(book.maker, a(9));
    assert_eq!(book.sequence, 42);
    assert_eq!(book.sync_spread_ticks, 7);
    assert_eq!(book.bid_levels[0].size_in_base_lots, 3);
    assert_eq!(book.ask_levels[0].price_offset_ticks, -1);

    let mut data = vec![0u8; Market::LEN];
    data[..8].copy_from_slice(&MARKET_DISCRIMINATOR);
    data[264..268].copy_from_slice(&500u32.to_le_bytes());
    data[277] = 254;
    let market = Market::from_bytes(&data).unwrap();
    assert_eq!((market.maker_fee_ppm, market.market_bump), (500, 254));

    let mut data = vec![0u8; AsyncSwap::LEN];
    data[..8].copy_from_slice(&ASYNC_SWAP_DISCRIMINATOR);
    data[168..176].copy_from_slice(&77u64.to_le_bytes());
    data[176] = 1;
    let swap = AsyncSwap::from_bytes(&data).unwrap();
    assert_eq!((swap.expiry_slot, swap.side), (77, 1));
}

#[test]
fn pdas_and_errors_match_the_program() {
    let (market, _) = Market::find_pda(&a(1), &a(2), &a(3));
    let expected = Address::find_program_address(
        &[b"market", a(1).as_ref(), a(2).as_ref(), a(3).as_ref()],
        &NOCK_ID,
    );
    assert_eq!(market, expected.0);
    let (swap, _) = AsyncSwap::find_pda(&a(1), &a(2), 5);
    let expected = Address::find_program_address(
        &[b"async", a(1).as_ref(), a(2).as_ref(), &5u64.to_le_bytes()],
        &NOCK_ID,
    );
    assert_eq!(swap, expected.0);
    let (authority, _) = Address::find_program_address(&[b"event_authority"], &NOCK_ID);
    assert_eq!(authority, EVENT_AUTHORITY);
    assert_eq!(NockError::InvalidTokenProgram as u32, 0);
    assert_eq!(NockError::NoPendingAdmin as u32, 33);
}
