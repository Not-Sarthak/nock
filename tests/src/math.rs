use nock_interface::error::AmmError;
use nock_interface::math::{level_price_ticks, price_fill, Fill, SIDE_BUY_BASE, SIDE_SELL_BASE};

const BASE_ATOMS_PER_LOT:  u64 = 1;
const QUOTE_ATOMS_PER_LOT: u64 = 1;
const TICK_SIZE:           u64 = 1;
const BASE_ATOMS_PER_UNIT: u64 = 1;

fn fill_at(
    side: u8,
    amount_in_lots: u64,
    price_ticks: i128,
    quote_atoms_per_quote_lot: u64,
    base_atoms_per_base_unit: u64,
    maker_fee_ppm: i32,
    taker_fee_ppm: i32,
) -> Result<Fill, AmmError> {
    price_fill(
        side,
        amount_in_lots,
        price_ticks,
        BASE_ATOMS_PER_LOT,
        quote_atoms_per_quote_lot,
        TICK_SIZE,
        base_atoms_per_base_unit,
        maker_fee_ppm,
        taker_fee_ppm,
    )
}

#[test]
fn bids_sit_below_the_mid_and_asks_above() {
    assert_eq!(level_price_ticks(SIDE_SELL_BASE, 1_000, 7, 3).unwrap(), 990);
    assert_eq!(level_price_ticks(SIDE_BUY_BASE, 1_000, 7, 3).unwrap(), 1_010);
}

#[test]
fn negative_offsets_cross_the_mid_the_other_way() {
    assert_eq!(level_price_ticks(SIDE_SELL_BASE, 1_000, -20, 0).unwrap(), 1_020);
    assert_eq!(level_price_ticks(SIDE_BUY_BASE, 1_000, -20, 0).unwrap(), 980);
}

#[test]
fn non_positive_price_is_rejected() {
    assert_eq!(
        level_price_ticks(SIDE_SELL_BASE, 10, 10, 0).unwrap_err(),
        AmmError::PriceMathOverflow
    );
    assert_eq!(
        level_price_ticks(SIDE_SELL_BASE, 10, 20, 0).unwrap_err(),
        AmmError::PriceMathOverflow
    );
}

#[test]
fn unit_scaling_rounds_against_the_taker() {
    let sell = fill_at(SIDE_SELL_BASE, 1, 3, QUOTE_ATOMS_PER_LOT, 2, 0, 0).unwrap();
    let buy = fill_at(SIDE_BUY_BASE, 1, 3, QUOTE_ATOMS_PER_LOT, 2, 0, 0).unwrap();

    assert_eq!(sell.gross_quote_lots, 1, "taker receives the floor");
    assert_eq!(buy.gross_quote_lots, 2, "taker pays the ceiling");
}

#[test]
fn lot_scaling_rounds_against_the_taker() {
    let sell = fill_at(SIDE_SELL_BASE, 10, 1, 4, BASE_ATOMS_PER_UNIT, 0, 0).unwrap();
    let buy = fill_at(SIDE_BUY_BASE, 10, 1, 4, BASE_ATOMS_PER_UNIT, 0, 0).unwrap();

    assert_eq!(sell.gross_quote_lots, 2);
    assert_eq!(buy.gross_quote_lots, 3);
}

#[test]
fn a_dust_fill_cannot_round_its_way_to_a_free_trade() {
    let f = fill_at(SIDE_BUY_BASE, 1, 1, QUOTE_ATOMS_PER_LOT, BASE_ATOMS_PER_UNIT, 0, 1).unwrap();
    assert_eq!(f.gross_quote_lots, 1);
    assert_eq!(f.taker_fee_lots, 1, "1 ppm of 1 lot still charges a lot");
}

#[test]
fn zero_and_negative_rates_charge_nothing() {
    let zero = fill_at(SIDE_BUY_BASE, 1_000, 1, QUOTE_ATOMS_PER_LOT, BASE_ATOMS_PER_UNIT, 0, 0).unwrap();
    assert_eq!(zero.taker_fee_lots, 0);
    assert_eq!(zero.maker_fee_lots, 0);

    let rebate =
        fill_at(SIDE_BUY_BASE, 1_000, 1, QUOTE_ATOMS_PER_LOT, BASE_ATOMS_PER_UNIT, -500, -500).unwrap();
    assert_eq!(rebate.taker_fee_lots, 0, "rebates are not funded from the pool");
    assert_eq!(rebate.maker_fee_lots, 0);
}

#[test]
fn fees_are_charged_to_both_sides_independently() {
    let f = fill_at(SIDE_BUY_BASE, 1_000_000, 1, QUOTE_ATOMS_PER_LOT, BASE_ATOMS_PER_UNIT, 200, 500)
        .unwrap();
    assert_eq!(f.gross_quote_lots, 1_000_000);
    assert_eq!(f.taker_fee_lots, 500);
    assert_eq!(f.maker_fee_lots, 200);
    assert_eq!(f.protocol_fee_lots(), 700);
}

#[test]
fn sell_leg_conserves_value() {
    let f = fill_at(SIDE_SELL_BASE, 1_000_000, 1, QUOTE_ATOMS_PER_LOT, BASE_ATOMS_PER_UNIT, 200, 500)
        .unwrap();

    let taker_receives = f.gross_quote_lots - f.taker_fee_lots;
    let maker_debit = f.gross_quote_lots + f.maker_fee_lots;
    let vault_delta = -(taker_receives as i128);
    let accounting_delta = -(maker_debit as i128) + f.protocol_fee_lots() as i128;

    assert_eq!(vault_delta, accounting_delta);
}

#[test]
fn buy_leg_conserves_value() {
    let f = fill_at(SIDE_BUY_BASE, 1_000_000, 1, QUOTE_ATOMS_PER_LOT, BASE_ATOMS_PER_UNIT, 200, 500)
        .unwrap();

    let taker_pays = f.gross_quote_lots + f.taker_fee_lots;
    let maker_credit = f.gross_quote_lots - f.maker_fee_lots;
    let vault_delta = taker_pays as i128;
    let accounting_delta = maker_credit as i128 + f.protocol_fee_lots() as i128;

    assert_eq!(vault_delta, accounting_delta);
}

#[test]
fn conservation_holds_across_ragged_inputs() {
    for amount in [1u64, 7, 13, 999, 100_003] {
        for price in [1i128, 3, 17, 1_009] {
            for qapl in [1u64, 4, 97] {
                for unit in [1u64, 2, 1_000] {
                    for (maker_ppm, taker_ppm) in [(0, 0), (0, 500), (250, 0), (137, 613)] {
                        let sell = fill_at(SIDE_SELL_BASE, amount, price, qapl, unit, maker_ppm, taker_ppm);
                        if let Ok(f) = sell {
                            let vault = -((f.gross_quote_lots - f.taker_fee_lots) as i128);
                            let acct = -((f.gross_quote_lots + f.maker_fee_lots) as i128)
                                + f.protocol_fee_lots() as i128;
                            assert_eq!(vault, acct, "sell {amount} {price} {qapl} {unit}");
                        }

                        let buy = fill_at(SIDE_BUY_BASE, amount, price, qapl, unit, maker_ppm, taker_ppm);
                        if let Ok(f) = buy {
                            let vault = (f.gross_quote_lots + f.taker_fee_lots) as i128;
                            let acct = (f.gross_quote_lots - f.maker_fee_lots) as i128
                                + f.protocol_fee_lots() as i128;
                            assert_eq!(vault, acct, "buy {amount} {price} {qapl} {unit}");
                            assert!(f.maker_fee_lots <= f.gross_quote_lots);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a_buy_costs_strictly_more_as_the_mid_moves_up() {
    let cost = |mid: u64| {
        let price = level_price_ticks(SIDE_BUY_BASE, mid, 0, 0).unwrap();
        let f = fill_at(SIDE_BUY_BASE, 100, price, QUOTE_ATOMS_PER_LOT, BASE_ATOMS_PER_UNIT, 0, 500)
            .unwrap();
        f.gross_quote_lots + f.taker_fee_lots
    };

    assert!(cost(1_000) < cost(1_000_000));
    assert!(cost(1_000_000) < cost(1_000_000_000));
}

#[test]
fn base_leg_overflow_is_caught_not_wrapped() {
    let err = price_fill(
        SIDE_BUY_BASE,
        2,
        1,
        u64::MAX,
        QUOTE_ATOMS_PER_LOT,
        TICK_SIZE,
        BASE_ATOMS_PER_UNIT,
        0,
        0,
    )
    .unwrap_err();
    assert_eq!(err, AmmError::PriceMathOverflow);
}

#[test]
fn notional_overflow_is_caught_not_wrapped() {
    let err = price_fill(
        SIDE_BUY_BASE,
        u64::MAX,
        u64::MAX as i128,
        1,
        QUOTE_ATOMS_PER_LOT,
        u64::MAX,
        BASE_ATOMS_PER_UNIT,
        0,
        0,
    )
    .unwrap_err();
    assert_eq!(err, AmmError::PriceMathOverflow);
}

#[test]
fn a_sell_that_rounds_to_nothing_is_rejected() {
    let err = fill_at(SIDE_SELL_BASE, 1, 1, 1_000, BASE_ATOMS_PER_UNIT, 0, 0).unwrap_err();
    assert_eq!(err, AmmError::QuoteSizeTooSmall);
}

#[test]
fn degenerate_market_config_is_rejected() {
    assert_eq!(
        fill_at(SIDE_BUY_BASE, 1, 1, 0, BASE_ATOMS_PER_UNIT, 0, 0).unwrap_err(),
        AmmError::PriceMathOverflow
    );
    assert_eq!(
        fill_at(SIDE_BUY_BASE, 1, 1, QUOTE_ATOMS_PER_LOT, 0, 0, 0).unwrap_err(),
        AmmError::PriceMathOverflow
    );
    assert_eq!(
        fill_at(SIDE_BUY_BASE, 1, 0, QUOTE_ATOMS_PER_LOT, BASE_ATOMS_PER_UNIT, 0, 0).unwrap_err(),
        AmmError::PriceMathOverflow
    );
}
