use crate::book::book;
use nock_interface::constants::{MAX_FEE_PPM, MAX_LEVELS};
use nock_interface::error::NockError;
use nock_interface::math::*;
use nock_interface::state::{MakerBook, MakerLevel};

const UNIT: LotSizes = LotSizes {
    base_atoms_per_base_lot: 1,
    quote_atoms_per_quote_lot: 1,
    tick_size_in_quote_atoms_per_base_unit: 1,
    base_atoms_per_base_unit: 1,
};

fn ladder(levels: &[(u64, i64)]) -> [MakerLevel; MAX_LEVELS] {
    let mut out = [MakerLevel::EMPTY; MAX_LEVELS];
    for (i, (size, offset)) in levels.iter().enumerate() {
        out[i] = MakerLevel {
            size_in_base_lots: *size,
            price_offset_ticks: *offset,
        };
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn quote(
    side: u8,
    levels: &[(u64, i64)],
    base_lots: u64,
    limit: u64,
    price_limit: u64,
    lots: &LotSizes,
    maker_ppm: u32,
    taker_ppm: u32,
) -> Result<Fill, NockError> {
    quote_fill(
        side,
        &ladder(levels),
        1_000,
        0,
        base_lots,
        limit,
        price_limit,
        lots,
        maker_ppm,
        taker_ppm,
    )
}

#[test]
fn bids_sit_below_the_mid_and_asks_above() {
    assert_eq!(level_price_ticks(SIDE_SELL_BASE, 1_000, 7, 3), Ok(990));
    assert_eq!(level_price_ticks(SIDE_BUY_BASE, 1_000, 7, 3), Ok(1_010));
}

#[test]
fn non_positive_price_is_an_invalid_quote() {
    assert_eq!(
        level_price_ticks(SIDE_SELL_BASE, 10, 10, 0),
        Err(NockError::InvalidQuotes)
    );
    assert_eq!(
        level_price_ticks(SIDE_SELL_BASE, 10, 20, 0),
        Err(NockError::InvalidQuotes)
    );
}

#[test]
fn fees_round_against_the_taker_and_for_the_maker() {
    assert_eq!(
        taker_fee_lots(1, 1),
        1,
        "1 ppm of 1 lot still costs the taker a lot"
    );
    assert_eq!(
        maker_fee_lots(1, MAX_FEE_PPM),
        0,
        "a dust fill never costs the maker a lot"
    );
    assert_eq!(taker_fee_lots(1_000_000, 500), 500);
    assert_eq!(maker_fee_lots(1_000_000, 200), 200);
    assert_eq!(maker_fee_lots(9_999, 100), 0);
    assert_eq!(taker_fee_lots(9_999, 100), 1);
}

#[test]
fn a_single_level_fill() {
    let sell = quote(SIDE_SELL_BASE, &[(10, 0)], 10, 0, 0, &UNIT, 0, 0).unwrap();
    assert_eq!(sell.gross_quote_lots, 10_000);
    assert_eq!(sell.taker_quote_lots, 10_000);
    assert_eq!(sell.worst_price_ticks, 1_000);
    assert_eq!(sell.taken[0], 10);
}

#[test]
fn walks_levels_best_first_and_sums_the_notional() {
    let levels = [(2, 1), (3, 2), (5, 3)];
    let buy = quote(SIDE_BUY_BASE, &levels, 6, u64::MAX, u64::MAX, &UNIT, 0, 0).unwrap();
    assert_eq!(&buy.taken[..3], &[2, 3, 1]);
    assert_eq!(buy.gross_quote_lots, 2 * 1_001 + 3 * 1_002 + 1_003);
    assert_eq!(buy.worst_price_ticks, 1_003);

    let sell = quote(SIDE_SELL_BASE, &levels, 6, 0, 0, &UNIT, 0, 0).unwrap();
    assert_eq!(sell.gross_quote_lots, 2 * 999 + 3 * 998 + 997);
    assert_eq!(sell.worst_price_ticks, 997);
}

#[test]
fn empty_levels_are_skipped() {
    let buy = quote(
        SIDE_BUY_BASE,
        &[(0, 1), (4, 2)],
        4,
        u64::MAX,
        u64::MAX,
        &UNIT,
        0,
        0,
    )
    .unwrap();
    assert_eq!(&buy.taken[..2], &[0, 4]);
}

#[test]
fn all_or_nothing() {
    assert_eq!(
        quote(
            SIDE_BUY_BASE,
            &[(2, 1), (3, 2)],
            6,
            u64::MAX,
            u64::MAX,
            &UNIT,
            0,
            0
        ),
        Err(NockError::InsufficientLiquidity)
    );
}

#[test]
fn the_price_limit_bounds_the_worst_level() {
    let levels = [(2, 1), (3, 2)];
    assert_eq!(
        quote(SIDE_BUY_BASE, &levels, 3, u64::MAX, 1_001, &UNIT, 0, 0),
        Err(NockError::SlippageExceeded)
    );
    assert!(quote(SIDE_BUY_BASE, &levels, 2, u64::MAX, 1_001, &UNIT, 0, 0).is_ok());
    assert_eq!(
        quote(SIDE_SELL_BASE, &levels, 3, 0, 999, &UNIT, 0, 0),
        Err(NockError::SlippageExceeded)
    );
    assert!(quote(SIDE_SELL_BASE, &levels, 3, 0, 998, &UNIT, 0, 0).is_ok());
}

#[test]
fn the_quote_limit_bounds_the_average_after_fees() {
    let levels = [(10, 0)];
    let buy = quote(
        SIDE_BUY_BASE,
        &levels,
        10,
        u64::MAX,
        u64::MAX,
        &UNIT,
        0,
        500,
    )
    .unwrap();
    assert_eq!(buy.taker_quote_lots, 10_000 + 5);
    assert!(quote(SIDE_BUY_BASE, &levels, 10, 10_005, u64::MAX, &UNIT, 0, 500).is_ok());
    assert_eq!(
        quote(SIDE_BUY_BASE, &levels, 10, 10_004, u64::MAX, &UNIT, 0, 500),
        Err(NockError::SlippageExceeded)
    );

    let sell = quote(SIDE_SELL_BASE, &levels, 10, 0, 0, &UNIT, 0, 500).unwrap();
    assert_eq!(sell.taker_quote_lots, 10_000 - 5);
    assert!(quote(SIDE_SELL_BASE, &levels, 10, 9_995, 0, &UNIT, 0, 500).is_ok());
    assert_eq!(
        quote(SIDE_SELL_BASE, &levels, 10, 9_996, 0, &UNIT, 0, 500),
        Err(NockError::SlippageExceeded)
    );
}

#[test]
fn notional_rounds_once_against_the_taker() {
    // Asks at 1, 2 and 3 ticks, one lot each, two base atoms per unit: rounding each level
    // up would charge 1 + 1 + 2 = 4 lots; summing first charges ceil(6 / 2) = 3.
    let lots = LotSizes {
        base_atoms_per_base_unit: 2,
        ..UNIT
    };
    let mid_one = |side, levels: &[(u64, i64)]| {
        quote_fill(
            side,
            &ladder(levels),
            1,
            0,
            3,
            u64::MAX,
            if side == SIDE_BUY_BASE { u64::MAX } else { 0 },
            &lots,
            0,
            0,
        )
    };
    let buy = mid_one(SIDE_BUY_BASE, &[(1, 0), (1, 1), (1, 2)]).unwrap();
    assert_eq!(buy.gross_quote_lots, (1 + 2 + 3_u64).div_ceil(2));
    let sell = quote_fill(
        SIDE_SELL_BASE,
        &ladder(&[(3, 0)]),
        3,
        0,
        3,
        0,
        0,
        &lots,
        0,
        0,
    )
    .unwrap();
    assert_eq!(sell.gross_quote_lots, 9 / 2, "seller gets the floor");
}

#[test]
fn a_fill_that_rounds_to_nothing_is_rejected() {
    let lots = LotSizes {
        quote_atoms_per_quote_lot: 1_000_000,
        ..UNIT
    };
    assert_eq!(
        quote(SIDE_SELL_BASE, &[(1, 0)], 1, 0, 0, &lots, 0, 0),
        Err(NockError::QuoteSizeTooSmall)
    );
}

#[test]
fn a_seller_never_nets_zero() {
    // gross 1 lot, taker fee rounds up to 1 lot: the taker would get nothing.
    let f = quote_fill(
        SIDE_SELL_BASE,
        &ladder(&[(1, 0)]),
        1,
        0,
        1,
        0,
        0,
        &UNIT,
        0,
        1,
    );
    assert_eq!(f, Err(NockError::QuoteSizeTooSmall));
}

#[test]
fn bad_inputs() {
    assert_eq!(
        quote(2, &[(1, 0)], 1, 0, 0, &UNIT, 0, 0),
        Err(NockError::InvalidSide)
    );
    assert_eq!(
        quote(SIDE_BUY_BASE, &[(1, 0)], 0, u64::MAX, u64::MAX, &UNIT, 0, 0),
        Err(NockError::AmountZero)
    );
    let broken = LotSizes {
        quote_atoms_per_quote_lot: 0,
        ..UNIT
    };
    assert_eq!(
        quote(
            SIDE_BUY_BASE,
            &[(1, 0)],
            1,
            u64::MAX,
            u64::MAX,
            &broken,
            0,
            0
        ),
        Err(NockError::InvalidMarketConfig)
    );
}

#[test]
fn overflow_is_caught_not_wrapped() {
    let huge = LotSizes {
        base_atoms_per_base_lot: u64::MAX,
        ..UNIT
    };
    assert_eq!(
        quote(SIDE_BUY_BASE, &[(2, 0)], 2, u64::MAX, u64::MAX, &huge, 0, 0),
        Err(NockError::MathOverflow)
    );
    let wide = LotSizes {
        tick_size_in_quote_atoms_per_base_unit: u64::MAX,
        ..UNIT
    };
    assert_eq!(
        quote(
            SIDE_BUY_BASE,
            &[(u64::MAX, 0)],
            u64::MAX,
            u64::MAX,
            u64::MAX,
            &wide,
            0,
            0
        ),
        Err(NockError::MathOverflow)
    );
}

fn funded_book() -> MakerBook {
    let mut b = book();
    b.base_free = 1_000;
    b.quote_free = 10_000_000;
    b.bid_levels = ladder(&[(5, 1), (5, 2)]);
    b.ask_levels = ladder(&[(5, 1), (5, 2)]);
    b
}

/// Quote lots the vault holds on behalf of this book.
fn quote_claims(b: &MakerBook) -> i128 {
    b.quote_free as i128 + b.fees_quote_lots as i128
}

#[test]
fn a_sell_conserves_every_lot() {
    let mut b = funded_book();
    let before_quote = quote_claims(&b);
    let before_base = b.base_free;
    let fill = fill_book(&mut b, SIDE_SELL_BASE, 7, 0, 0, &UNIT, 200, 500).unwrap();
    assert_eq!(
        quote_claims(&b),
        before_quote - fill.taker_quote_lots as i128
    );
    assert_eq!(b.base_free, before_base + 7);
    assert_eq!(b.fees_quote_lots, fill.protocol_fee_lots());
    assert_eq!(b.bid_levels[0].size_in_base_lots, 0);
    assert_eq!(b.bid_levels[1].size_in_base_lots, 3);
}

#[test]
fn a_buy_conserves_every_lot() {
    let mut b = funded_book();
    let before_quote = quote_claims(&b);
    let before_base = b.base_free;
    let fill = fill_book(
        &mut b,
        SIDE_BUY_BASE,
        7,
        u64::MAX,
        u64::MAX,
        &UNIT,
        200,
        500,
    )
    .unwrap();
    assert_eq!(
        quote_claims(&b),
        before_quote + fill.taker_quote_lots as i128
    );
    assert_eq!(b.base_free, before_base - 7);
    assert_eq!(b.ask_levels[1].size_in_base_lots, 3);
}

#[test]
fn conservation_holds_across_ragged_inputs() {
    for amount in [1u64, 3, 7, 10] {
        for qapl in [1u64, 4, 97] {
            for unit in [1u64, 2, 1_000] {
                for (maker_ppm, taker_ppm) in [
                    (0, 0),
                    (0, 500),
                    (250, 0),
                    (137, 613),
                    (MAX_FEE_PPM, MAX_FEE_PPM),
                ] {
                    for side in [SIDE_SELL_BASE, SIDE_BUY_BASE] {
                        let lots = LotSizes {
                            quote_atoms_per_quote_lot: qapl,
                            base_atoms_per_base_unit: unit,
                            ..UNIT
                        };
                        let mut b = funded_book();
                        let (q0, base0) = (quote_claims(&b), b.base_free);
                        let limit = if side == SIDE_SELL_BASE { 0 } else { u64::MAX };
                        let Ok(f) = fill_book(
                            &mut b, side, amount, limit, limit, &lots, maker_ppm, taker_ppm,
                        ) else {
                            continue;
                        };
                        let (dq, dbase) = if side == SIDE_SELL_BASE {
                            (-(f.taker_quote_lots as i128), amount as i128)
                        } else {
                            (f.taker_quote_lots as i128, -(amount as i128))
                        };
                        assert_eq!(quote_claims(&b), q0 + dq, "{side} {amount} {qapl} {unit}");
                        assert_eq!(b.base_free as i128, base0 as i128 + dbase);
                        assert!(f.maker_fee_lots <= f.gross_quote_lots);
                    }
                }
            }
        }
    }
}

#[test]
fn a_failed_fill_leaves_the_book_untouched() {
    let mut b = funded_book();
    b.quote_free = 1;
    let snapshot = (
        b.base_free,
        b.quote_free,
        b.fees_quote_lots,
        b.bid_levels[0].size_in_base_lots,
    );
    assert_eq!(
        fill_book(&mut b, SIDE_SELL_BASE, 7, 0, 0, &UNIT, 0, 0),
        Err(NockError::InsufficientInventory)
    );
    assert_eq!(
        snapshot,
        (
            b.base_free,
            b.quote_free,
            b.fees_quote_lots,
            b.bid_levels[0].size_in_base_lots
        )
    );

    let mut b = funded_book();
    b.base_free = 6;
    assert_eq!(
        fill_book(&mut b, SIDE_BUY_BASE, 7, u64::MAX, u64::MAX, &UNIT, 0, 0),
        Err(NockError::InsufficientInventory)
    );
}
