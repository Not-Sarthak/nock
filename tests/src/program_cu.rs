//! Compute units per instruction on the built program, each asserted under a budget.
//! `cargo test -p nock-tests compute_units -- --nocapture` prints the table.

use crate::harness::*;
use nock_interface::instruction::disc;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

/// Budget per instruction: the cost measured with solana 3.1.6 platform tools plus about
/// 25% headroom. A change that blows one is a regression to look at, not a number to bump.
const BUDGETS: &[(&str, u64)] = &[
    ("create_pool", 52_000),
    ("init_maker_book", 8_500),
    ("maker_deposit", 5_000),
    ("maker_withdraw", 5_100),
    ("update_maker_book (16 + 16 levels)", 5_000),
    ("update_mid_price", 4_500),
    ("maker_heartbeat", 2_200),
    ("set_book_halt", 2_200),
    ("sync_swap (1 level)", 7_500),
    ("sync_swap (16 levels)", 14_000),
    ("async_swap_create", 29_000),
    ("async_swap_execute", 10_500),
    ("async_swap_cancel", 5_600),
    ("collect_fees", 3_800),
    ("close_maker_book", 1_800),
];

fn budget(name: &str) -> u64 {
    BUDGETS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, b)| *b)
        .expect("budget listed")
}

#[test]
fn compute_units() {
    let mut measured: Vec<(&str, u64)> = Vec::new();
    let mut record = |w: &mut World, name: &'static str, ix: &Instruction| {
        let r = w.env.ok(ix);
        measured.push((name, r.compute_units_consumed));
    };

    // CreatePool, measured on its own
    let mut w = World::new();
    let authority = Pubkey::new_unique();
    w.env.set(authority, wallet(SOL));
    let (market, _) = market_pda(&w.base_mint, &w.quote_mint, &authority);
    let ix = create_pool_ix(
        w.payer,
        authority,
        market,
        w.base_mint,
        w.quote_mint,
        ata(&market, &w.base_mint),
        ata(&market, &w.quote_mint),
        &PoolArgs::default(),
    );
    record(&mut w, "create_pool", &ix);

    let maker = w.actor(RICH);
    let (book, _) = book_pda(&w.market, &maker.key);
    let ix = init_book_ix(maker.key, w.market, book);
    record(&mut w, "init_maker_book", &ix);
    let ix = w.deposit_ix(&maker, book, 10_000, 100_000_000);
    record(&mut w, "maker_deposit", &ix);
    let ladder: Vec<(u64, i64)> = (1..=16).map(|k| (100, 100 * k)).collect();
    record(
        &mut w,
        "update_maker_book (16 + 16 levels)",
        &update_book_ix(maker.key, book, 1, 1_000_000, &ladder, &ladder),
    );
    record(
        &mut w,
        "update_mid_price",
        &mid_ix(maker.key, book, 2, 1_000_100),
    );
    record(
        &mut w,
        "maker_heartbeat",
        &seq_ix(disc::MAKER_HEARTBEAT, maker.key, book, 3),
    );
    record(&mut w, "set_book_halt", &halt_ix(maker.key, book, 4, 0));

    let user = w.actor(RICH);
    let ix = w.swap_ix(&user, book, buy(10, u64::MAX));
    record(&mut w, "sync_swap (1 level)", &ix);
    let ix = w.swap_ix(&user, book, sell(1_590, 0));
    record(&mut w, "sync_swap (16 levels)", &ix);

    let expiry = w.env.slot() + MAX_ASYNC as u64;
    let (ix, swap, escrow) =
        w.async_create(&user, 1, async_args(0, 1, 5, 1, 0, expiry, any_maker()));
    record(&mut w, "async_swap_create", &ix);
    let ix = w.async_execute(maker.key, book, &user, swap, escrow, 0);
    record(&mut w, "async_swap_execute", &ix);
    let (ix, swap, escrow) =
        w.async_create(&user, 2, async_args(0, 2, 5, 1, 0, expiry, any_maker()));
    w.env.ok(&ix);
    record(
        &mut w,
        "async_swap_cancel",
        &async_cancel_ix(user.key, swap, escrow, user.base, user.key),
    );

    let dest = w.admin_quote_account();
    let ix = collect_fees_ix(w.market, book, w.quote_vault, dest);
    record(&mut w, "collect_fees", &ix);
    let b = w.env.book(&book);
    let ix = w.withdraw_ix(&maker, book, b.base_free, b.quote_free);
    record(&mut w, "maker_withdraw", &ix);
    record(&mut w, "close_maker_book", &close_book_ix(maker.key, book));

    println!("\n| instruction | CU | budget |\n|---|---:|---:|");
    for (name, cu) in &measured {
        println!("| {name} | {cu} | {} |", budget(name));
    }
    for (name, cu) in &measured {
        assert!(
            *cu <= budget(name),
            "{name}: {cu} CU over budget {}",
            budget(name)
        );
    }
    assert_eq!(
        measured.len(),
        BUDGETS.len(),
        "every budgeted instruction measured"
    );
}
