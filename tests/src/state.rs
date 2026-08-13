use core::mem::offset_of;
use nock_interface::state::{AsyncSwap, MakerBook, MakerLevel, Market};

#[test]
fn market_field_offsets() {
    assert_eq!(offset_of!(Market, tag),         0);
    assert_eq!(offset_of!(Market, market_id),   8);
    assert_eq!(offset_of!(Market, base_mint),   40);
    assert_eq!(offset_of!(Market, quote_mint),  72);
    assert_eq!(offset_of!(Market, base_vault),  104);
    assert_eq!(offset_of!(Market, quote_vault), 136);
    assert_eq!(offset_of!(Market, admin),       168);
    assert_eq!(offset_of!(Market, base_atoms_per_base_lot),                200);
    assert_eq!(offset_of!(Market, quote_atoms_per_quote_lot),              208);
    assert_eq!(offset_of!(Market, tick_size_in_quote_atoms_per_base_unit), 216);
    assert_eq!(offset_of!(Market, base_atoms_per_base_unit),               224);
    assert_eq!(offset_of!(Market, uncollected_fees_quote_lots),            232);
    assert_eq!(offset_of!(Market, collected_fees_quote_lots),              240);
    assert_eq!(offset_of!(Market, maker_fee_ppm),         248);
    assert_eq!(offset_of!(Market, taker_fee_ppm),         252);
    assert_eq!(offset_of!(Market, base_decimals),         256);
    assert_eq!(offset_of!(Market, quote_decimals),        257);
    assert_eq!(offset_of!(Market, status),                258);
    assert_eq!(offset_of!(Market, market_bump),           259);
    assert_eq!(offset_of!(Market, min_async_delay_slots), 260);
    assert_eq!(offset_of!(Market, max_async_delay_slots), 262);
    assert_eq!(offset_of!(Market, _reserved),             264);
}

#[test]
fn maker_book_field_offsets() {
    assert_eq!(offset_of!(MakerBook, tag),      0);
    assert_eq!(offset_of!(MakerBook, maker),    8);
    assert_eq!(offset_of!(MakerBook, market),   40);
    assert_eq!(offset_of!(MakerBook, delegate), 72);
    assert_eq!(offset_of!(MakerBook, mid_price_ticks), 104);
    assert_eq!(offset_of!(MakerBook, quote_free),      112);
    assert_eq!(offset_of!(MakerBook, base_free),       120);
    assert_eq!(offset_of!(MakerBook, last_updated_sequence_number), 128);
    assert_eq!(offset_of!(MakerBook, last_updated_slot),            136);
    assert_eq!(offset_of!(MakerBook, expiry_in_slots),              144);
    assert_eq!(offset_of!(MakerBook, status),            152);
    assert_eq!(offset_of!(MakerBook, maker_book_bump),   153);
    assert_eq!(offset_of!(MakerBook, sync_spread_ticks), 154);
    assert_eq!(offset_of!(MakerBook, _pad),              156);
    assert_eq!(offset_of!(MakerBook, bid_levels), 160);
    assert_eq!(offset_of!(MakerBook, ask_levels), 416);
    assert_eq!(offset_of!(MakerBook, _reserved),  672);
}

#[test]
fn async_swap_field_offsets() {
    assert_eq!(offset_of!(AsyncSwap, tag),             0);
    assert_eq!(offset_of!(AsyncSwap, user),            8);
    assert_eq!(offset_of!(AsyncSwap, market),          40);
    assert_eq!(offset_of!(AsyncSwap, user_src_escrow), 104);
    assert_eq!(offset_of!(AsyncSwap, amount_in_atoms), 136);
    assert_eq!(offset_of!(AsyncSwap, min_out_atoms),   144);
    assert_eq!(offset_of!(AsyncSwap, expiry_slot),     152);
    assert_eq!(offset_of!(AsyncSwap, side),            160);
    assert_eq!(offset_of!(AsyncSwap, status),          161);
    assert_eq!(offset_of!(AsyncSwap, bump),            162);
}

#[test]
fn maker_level_size_is_16() {
    assert_eq!(core::mem::size_of::<MakerLevel>(), 16);
}

#[test]
fn async_swap_size() {
    assert_eq!(core::mem::size_of::<AsyncSwap>(), 168);
}
