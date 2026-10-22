use core::mem::offset_of;
use nock_interface::instruction::*;
use nock_interface::state::{AsyncSwap, MakerBook, MakerLevel, Market};

#[test]
fn market_field_offsets() {
    assert_eq!(offset_of!(Market, tag), 0);
    assert_eq!(offset_of!(Market, base_mint), 8);
    assert_eq!(offset_of!(Market, quote_mint), 40);
    assert_eq!(offset_of!(Market, base_vault), 72);
    assert_eq!(offset_of!(Market, quote_vault), 104);
    assert_eq!(offset_of!(Market, seed_authority), 136);
    assert_eq!(offset_of!(Market, admin), 168);
    assert_eq!(offset_of!(Market, pending_admin), 200);
    assert_eq!(offset_of!(Market, base_atoms_per_base_lot), 232);
    assert_eq!(offset_of!(Market, quote_atoms_per_quote_lot), 240);
    assert_eq!(
        offset_of!(Market, tick_size_in_quote_atoms_per_base_unit),
        248
    );
    assert_eq!(offset_of!(Market, base_atoms_per_base_unit), 256);
    assert_eq!(offset_of!(Market, maker_fee_ppm), 264);
    assert_eq!(offset_of!(Market, taker_fee_ppm), 268);
    assert_eq!(offset_of!(Market, min_async_delay_slots), 272);
    assert_eq!(offset_of!(Market, max_async_delay_slots), 274);
    assert_eq!(offset_of!(Market, status), 276);
    assert_eq!(offset_of!(Market, market_bump), 277);
    assert_eq!(core::mem::size_of::<Market>(), 280);
}

#[test]
fn maker_book_field_offsets() {
    assert_eq!(offset_of!(MakerBook, tag), 0);
    assert_eq!(offset_of!(MakerBook, maker), 8);
    assert_eq!(offset_of!(MakerBook, market), 40);
    assert_eq!(offset_of!(MakerBook, delegate), 72);
    assert_eq!(offset_of!(MakerBook, mid_price_ticks), 104);
    assert_eq!(offset_of!(MakerBook, base_free), 112);
    assert_eq!(offset_of!(MakerBook, quote_free), 120);
    assert_eq!(offset_of!(MakerBook, fees_quote_lots), 128);
    assert_eq!(offset_of!(MakerBook, sequence), 136);
    assert_eq!(offset_of!(MakerBook, last_updated_slot), 144);
    assert_eq!(offset_of!(MakerBook, expiry_in_slots), 152);
    assert_eq!(offset_of!(MakerBook, delegate_min_price_ticks), 160);
    assert_eq!(offset_of!(MakerBook, delegate_max_price_ticks), 168);
    assert_eq!(offset_of!(MakerBook, delegate_max_level_lots), 176);
    assert_eq!(offset_of!(MakerBook, status), 184);
    assert_eq!(offset_of!(MakerBook, maker_book_bump), 185);
    assert_eq!(offset_of!(MakerBook, sync_spread_ticks), 186);
    assert_eq!(offset_of!(MakerBook, bid_levels), 192);
    assert_eq!(offset_of!(MakerBook, ask_levels), 448);
    assert_eq!(core::mem::size_of::<MakerBook>(), 704);
}

#[test]
fn async_swap_field_offsets() {
    assert_eq!(offset_of!(AsyncSwap, tag), 0);
    assert_eq!(offset_of!(AsyncSwap, user), 8);
    assert_eq!(offset_of!(AsyncSwap, market), 40);
    assert_eq!(offset_of!(AsyncSwap, maker), 72);
    assert_eq!(offset_of!(AsyncSwap, escrow), 104);
    assert_eq!(offset_of!(AsyncSwap, nonce), 136);
    assert_eq!(offset_of!(AsyncSwap, base_lots), 144);
    assert_eq!(offset_of!(AsyncSwap, limit_quote_lots), 152);
    assert_eq!(offset_of!(AsyncSwap, price_limit_ticks), 160);
    assert_eq!(offset_of!(AsyncSwap, expiry_slot), 168);
    assert_eq!(offset_of!(AsyncSwap, side), 176);
    assert_eq!(offset_of!(AsyncSwap, bump), 177);
    assert_eq!(core::mem::size_of::<AsyncSwap>(), 184);
}

#[test]
fn maker_level_size_is_16() {
    assert_eq!(core::mem::size_of::<MakerLevel>(), 16);
}

#[test]
fn payload_lengths() {
    assert_eq!(CREATE_POOL_DATA_LEN, 36);
    assert_eq!(UPDATE_MAKER_BOOK_DATA_LEN, 24 + 2 * 16 * 16);
    assert_eq!(SYNC_SWAP_DATA_LEN, 34);
    assert_eq!(ASYNC_SWAP_CREATE_DATA_LEN, 73);
    assert_eq!(SET_BOOK_HALT_DATA_LEN, 9);
    assert_eq!(UPDATE_SYNC_SPREAD_DATA_LEN, 10);
}

#[test]
fn parse_rejects_wrong_lengths() {
    let data = [0u8; SYNC_SWAP_DATA_LEN + 1];
    assert!(parse::<SyncSwapData>(&data[..SYNC_SWAP_DATA_LEN]).is_some());
    assert!(parse::<SyncSwapData>(&data).is_none());
    assert!(parse::<SyncSwapData>(&data[..SYNC_SWAP_DATA_LEN - 1]).is_none());
}

#[test]
fn sync_swap_payload_field_order() {
    let mut data = [0u8; SYNC_SWAP_DATA_LEN];
    data[0] = 1;
    data[1] = 1;
    data[2..10].copy_from_slice(&7u64.to_le_bytes());
    data[10..18].copy_from_slice(&8u64.to_le_bytes());
    data[18..26].copy_from_slice(&9u64.to_le_bytes());
    data[26..34].copy_from_slice(&10u64.to_le_bytes());
    let args = parse::<SyncSwapData>(&data).unwrap();
    let (side, pin, base, limit, price, seq) = (
        args.side,
        args.pin_sequence,
        args.base_lots,
        args.limit_quote_lots,
        args.price_limit_ticks,
        args.expected_sequence,
    );
    assert_eq!((side, pin, base, limit, price, seq), (1, 1, 7, 8, 9, 10));
}

#[test]
fn instruction_discriminators_are_unique() {
    use nock_interface::instruction::disc::*;
    let all = [
        CREATE_POOL,
        TOGGLE_MARKET_STATE,
        UPDATE_CONFIG,
        COLLECT_FEES,
        PROPOSE_ADMIN,
        ACCEPT_ADMIN,
        INIT_MAKER_BOOK,
        UPDATE_MAKER_BOOK,
        UPDATE_MID_PRICE,
        CLEAR_BOOK,
        SET_BOOK_DELEGATE,
        MAKER_DEPOSIT,
        MAKER_WITHDRAW,
        ASYNC_SWAP_CREATE,
        ASYNC_SWAP_EXECUTE,
        ASYNC_SWAP_CANCEL,
        SYNC_SWAP,
        MAKER_HEARTBEAT,
        MAKER_SET_SEQUENCE,
        SET_BOOK_HALT,
        MAKER_RESET_SEQUENCE,
        UPDATE_SYNC_SPREAD,
        UPDATE_EXPIRY_IN_SLOTS,
        CLOSE_MAKER_BOOK,
        EMIT_EVENT,
    ];
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            assert_ne!(all[i], all[j]);
        }
    }
}
