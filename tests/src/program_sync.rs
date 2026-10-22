//! SyncSwap on the built program: multi-level walks, limits, pinning, expiry, halts,
//! substitution attacks, per-book fees and a conservation property test.

use crate::harness::*;
use nock_interface::{
    constants::*,
    error::NockError,
    events::*,
    instruction::{SyncSwapData, PIN_SEQUENCE},
};
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;

const MID: u64 = 1_000_000;
/// Asks at 1_001_000, 1_002_000, 1_003_000 ticks; bids mirror them below the mid.
const LADDER: [(u64, i64); 3] = [(2, 1_000), (3, 2_000), (5, 3_000)];

fn ladder_world() -> (World, Maker, Actor) {
    let mut w = World::new();
    let maker = w.maker(1_000, 10_000_000, MID, &LADDER, &LADDER);
    let user = w.actor(RICH);
    (w, maker, user)
}

#[test]
fn a_buy_walks_asks_best_first_and_settles_exactly() {
    let (mut w, maker, user) = ladder_world();
    let ix = w.swap_ix(&user, maker.book, buy(6, u64::MAX));
    w.env.assert_signers_and_writables_enforced(&ix);
    let (base_vault, quote_vault) = (
        w.env.token_amount(&w.base_vault),
        w.env.token_amount(&w.quote_vault),
    );
    let before = w.env.book(&maker.book);
    let r = w.env.ok(&ix);

    // notional 2 * 1_001 + 3 * 1_002 + 1 * 1_003 = 6_011 quote atoms -> 602 lots (ceil);
    // taker fee ceil(602 * 500 / 1e6) = 1; maker fee floor(602 * 200 / 1e6) = 0.
    let e: SyncFillEvent = event(&r, tag::SYNC_FILL);
    assert_eq!(
        (
            e.gross_quote_lots,
            e.taker_fee_lots,
            e.maker_fee_lots,
            e.taker_quote_lots
        ),
        (602, 1, 0, 603)
    );
    assert_eq!(
        (e.base_lots, e.worst_price_ticks, e.side, e.sequence),
        (6, 1_003_000, 1, 1)
    );
    assert_eq!(e.user, user.key.to_bytes());

    assert_eq!(
        w.env.token_amount(&user.base),
        RICH + 6 * BASE_ATOMS_PER_LOT
    );
    assert_eq!(
        w.env.token_amount(&user.quote),
        RICH - 603 * QUOTE_ATOMS_PER_LOT
    );
    assert_eq!(
        w.env.token_amount(&w.base_vault),
        base_vault - 6 * BASE_ATOMS_PER_LOT
    );
    assert_eq!(
        w.env.token_amount(&w.quote_vault),
        quote_vault + 603 * QUOTE_ATOMS_PER_LOT
    );

    let after = w.env.book(&maker.book);
    let sizes: Vec<u64> = after.ask_levels[..3]
        .iter()
        .map(|l| l.size_in_base_lots)
        .collect();
    assert_eq!(sizes, vec![0, 0, 4]);
    assert_eq!(after.base_free, before.base_free - 6);
    assert_eq!(after.quote_free, before.quote_free + 602);
    assert_eq!(after.fees_quote_lots, 1);
}

#[test]
fn a_sell_walks_bids_best_first() {
    let (mut w, maker, user) = ladder_world();
    let r = w.env.ok(&w.swap_ix(&user, maker.book, sell(6, 0)));
    // 2 * 999 + 3 * 998 + 997 = 5_989 atoms -> 598 lots (floor); taker fee 1.
    let e: SyncFillEvent = event(&r, tag::SYNC_FILL);
    assert_eq!(
        (e.gross_quote_lots, e.taker_quote_lots, e.worst_price_ticks),
        (598, 597, 997_000)
    );
    assert_eq!(
        w.env.token_amount(&user.quote),
        RICH + 597 * QUOTE_ATOMS_PER_LOT
    );
    assert_eq!(
        w.env.token_amount(&user.base),
        RICH - 6 * BASE_ATOMS_PER_LOT
    );
}

#[test]
fn fills_are_all_or_nothing() {
    let (w, maker, user) = ladder_world();
    w.env.expect(
        &w.swap_ix(&user, maker.book, buy(11, u64::MAX)),
        custom(NockError::InsufficientLiquidity),
    );
    w.env.expect(
        &w.swap_ix(&user, maker.book, sell(11, 0)),
        custom(NockError::InsufficientLiquidity),
    );
    w.env.expect(
        &w.swap_ix(&user, maker.book, buy(0, u64::MAX)),
        custom(NockError::AmountZero),
    );
}

#[test]
fn limits_bound_the_worst_level_and_the_average() {
    let (w, maker, user) = ladder_world();
    let worst = SyncSwapData {
        price_limit_ticks: 1_002_000,
        ..buy(6, u64::MAX)
    };
    w.env.expect(
        &w.swap_ix(&user, maker.book, worst),
        custom(NockError::SlippageExceeded),
    );
    let ok = SyncSwapData {
        price_limit_ticks: 1_002_000,
        ..buy(5, u64::MAX)
    };
    assert!(w
        .env
        .try_ix(&w.swap_ix(&user, maker.book, ok))
        .program_result
        .is_ok());

    w.env.expect(
        &w.swap_ix(&user, maker.book, buy(6, 602)),
        custom(NockError::SlippageExceeded),
    );
    assert!(w
        .env
        .try_ix(&w.swap_ix(&user, maker.book, buy(6, 603)))
        .program_result
        .is_ok());
    w.env.expect(
        &w.swap_ix(&user, maker.book, sell(6, 598)),
        custom(NockError::SlippageExceeded),
    );
    assert!(w
        .env
        .try_ix(&w.swap_ix(&user, maker.book, sell(6, 597)))
        .program_result
        .is_ok());
    let floor = SyncSwapData {
        price_limit_ticks: 998_000,
        ..sell(6, 0)
    };
    w.env.expect(
        &w.swap_ix(&user, maker.book, floor),
        custom(NockError::SlippageExceeded),
    );
}

#[test]
fn the_expected_sequence_pin() {
    let (mut w, mut maker, user) = ladder_world();
    let pinned = |seq| SyncSwapData {
        pin_sequence: PIN_SEQUENCE,
        expected_sequence: seq,
        ..buy(1, u64::MAX)
    };
    assert!(w
        .env
        .try_ix(&w.swap_ix(&user, maker.book, pinned(1)))
        .program_result
        .is_ok());
    w.env.ok(&update_book_ix(
        maker.actor.key,
        maker.book,
        maker.next_seq(),
        MID + 50_000,
        &LADDER,
        &LADDER,
    ));
    w.env.expect(
        &w.swap_ix(&user, maker.book, pinned(1)),
        custom(NockError::SequenceMismatch),
    );
    w.env.ok(&w.swap_ix(&user, maker.book, pinned(2)));
    let bad_flag = SyncSwapData {
        pin_sequence: 2,
        ..buy(1, u64::MAX)
    };
    w.env.expect(
        &w.swap_ix(&user, maker.book, bad_flag),
        failure(ProgramError::InvalidInstructionData),
    );
}

#[test]
fn quotes_expire_exactly_after_expiry_in_slots() {
    let (mut w, mut maker, user) = ladder_world();
    let ix = w.swap_ix(&user, maker.book, buy(1, u64::MAX));
    // refreshed at START_SLOT, default 10 slots: live through START_SLOT + 9
    w.env.warp(START_SLOT + DEFAULT_EXPIRY_IN_SLOTS - 1);
    assert!(w.env.try_ix(&ix).program_result.is_ok());
    w.env.warp(START_SLOT + DEFAULT_EXPIRY_IN_SLOTS);
    w.env.expect(&ix, custom(NockError::QuoteExpired));

    // a heartbeat revives them; expiry 1 means the refresh slot only
    w.env
        .ok(&expiry_ix(maker.actor.key, maker.book, maker.next_seq(), 1));
    w.env.ok(&seq_ix(
        nock_interface::instruction::disc::MAKER_HEARTBEAT,
        maker.actor.key,
        maker.book,
        maker.next_seq(),
    ));
    assert!(w.env.try_ix(&ix).program_result.is_ok());
    w.env.warp(START_SLOT + DEFAULT_EXPIRY_IN_SLOTS + 1);
    w.env.expect(&ix, custom(NockError::QuoteExpired));
}

#[test]
fn halted_books_and_paused_markets_do_not_fill() {
    let (mut w, maker, user) = ladder_world();
    let ix = w.swap_ix(&user, maker.book, buy(1, u64::MAX));
    w.env.ok(&w.toggle_paused(true));
    w.env.expect(&ix, custom(NockError::MarketPaused));
    w.env.ok(&w.toggle_paused(false));
    w.env.ok(&halt_ix(maker.actor.key, maker.book, 2, 1));
    w.env.expect(&ix, custom(NockError::BookHalted));
}

#[test]
fn substitution_attacks_are_rejected() {
    let (mut w, maker, user) = ladder_world();
    let base = w.swap_ix(&user, maker.book, buy(1, u64::MAX));

    // the attacker's own token account in place of a vault
    let fake_vault = Pubkey::new_unique();
    w.env
        .set(fake_vault, token_account(&w.base_mint, &user.key, RICH));
    let mut ix = base.clone();
    ix.accounts[3].pubkey = fake_vault;
    w.env.expect(&ix, custom(NockError::IncorrectVault));
    let mut ix = base.clone();
    ix.accounts[3].pubkey = w.quote_vault;
    ix.accounts[4].pubkey = w.base_vault;
    w.env.expect(&ix, custom(NockError::IncorrectVault));

    // a book from another market
    let other = w.other_market();
    let other_maker = w.actor(RICH);
    let (other_book, _) = book_pda(&other.market, &other_maker.key);
    w.env
        .ok(&init_book_ix(other_maker.key, other.market, other_book));
    let mut ix = base.clone();
    ix.accounts[2].pubkey = other_book;
    w.env.expect(&ix, custom(NockError::MarketMismatch));

    // that market with this market's vaults
    let mut ix = base.clone();
    ix.accounts[1].pubkey = other.market;
    w.env.expect(&ix, custom(NockError::IncorrectVault));

    // account type confusion
    let mut ix = base.clone();
    ix.accounts[2].pubkey = w.market;
    w.env.expect(&ix, failure(ProgramError::InvalidAccountData));
    let mut ix = base.clone();
    ix.accounts[1].pubkey = maker.book;
    w.env.expect(&ix, failure(ProgramError::InvalidAccountData));
    let mut foreign = w.env.get(&maker.book);
    foreign.owner = Pubkey::new_unique();
    let fake_book = Pubkey::new_unique();
    w.env.set(fake_book, foreign);
    let mut ix = base.clone();
    ix.accounts[2].pubkey = fake_book;
    w.env.expect(&ix, failure(ProgramError::IllegalOwner));

    // token programs
    let mut ix = base.clone();
    ix.accounts[7].pubkey = TOKEN_2022;
    w.env.expect(&ix, custom(NockError::Token2022Unsupported));
    let mut ix = base.clone();
    ix.accounts[7].pubkey = Pubkey::new_unique();
    w.env.expect(&ix, custom(NockError::InvalidTokenProgram));

    // event accounts
    let mut ix = base.clone();
    ix.accounts[8].pubkey = Pubkey::new_unique();
    w.env.expect(&ix, custom(NockError::InvalidEventAuthority));

    // spending someone else's tokens
    let victim = w.actor(RICH);
    let mut ix = base.clone();
    ix.accounts[6].pubkey = victim.quote;
    assert!(!w.env.try_ix(&ix).program_result.is_ok());

    let mut ix = base;
    ix.data[1] = 2;
    w.env.expect(&ix, custom(NockError::InvalidSide));
}

#[test]
fn inventory_bounds_fills() {
    let (mut w, maker, user) = ladder_world();
    let b = w.env.book(&maker.book);
    w.env.ok(&w.withdraw_ix(
        &maker.actor,
        maker.book,
        b.base_free - 3,
        b.quote_free - 100,
    ));
    w.env.expect(
        &w.swap_ix(&user, maker.book, buy(4, u64::MAX)),
        custom(NockError::InsufficientInventory),
    );
    w.env.expect(
        &w.swap_ix(&user, maker.book, sell(2, 0)),
        custom(NockError::InsufficientInventory),
    );
    w.env.ok(&w.swap_ix(&user, maker.book, buy(3, u64::MAX)));
}

/// Audit P1: the maker fee rounded up per fill, so dust fills cost the maker a whole lot each.
#[test]
fn regression_dust_maker_fee_griefing() {
    let (mut w, maker, user) = ladder_world();
    let before = w.env.book(&maker.book);
    let r = w.env.ok(&w.swap_ix(&user, maker.book, buy(1, u64::MAX)));
    let e: SyncFillEvent = event(&r, tag::SYNC_FILL);
    assert_eq!(e.gross_quote_lots, 101);
    assert_eq!(e.maker_fee_lots, 0, "200 ppm of 101 lots floors to 0");
    let after = w.env.book(&maker.book);
    assert_eq!(
        after.quote_free,
        before.quote_free + 101,
        "maker receives the full notional"
    );

    // dust fills cost the maker nothing extra
    let mut book_quote = after.quote_free;
    for _ in 0..5 {
        let r = w.env.ok(&w.swap_ix(&user, maker.book, sell(1, 0)));
        let e: SyncFillEvent = event(&r, tag::SYNC_FILL);
        assert_eq!(e.maker_fee_lots, 0);
        let now = w.env.book(&maker.book).quote_free;
        assert_eq!(now, book_quote - e.gross_quote_lots);
        book_quote = now;
    }
}

#[test]
fn fees_accrue_per_book_and_never_touch_the_market() {
    let mut w = World::new();
    let a = w.maker(1_000, 10_000_000, MID, &LADDER, &LADDER);
    let b = w.maker(1_000, 10_000_000, MID, &LADDER, &LADDER);
    let user = w.actor(RICH);
    let market_before = w.env.get(&w.market);

    let ra = w.env.ok(&w.swap_ix(&user, a.book, buy(10, u64::MAX)));
    let rb = w.env.ok(&w.swap_ix(&user, b.book, sell(4, 0)));
    let ea: SyncFillEvent = event(&ra, tag::SYNC_FILL);
    let eb: SyncFillEvent = event(&rb, tag::SYNC_FILL);
    assert_eq!(
        w.env.book(&a.book).fees_quote_lots,
        ea.taker_fee_lots + ea.maker_fee_lots
    );
    assert_eq!(
        w.env.book(&b.book).fees_quote_lots,
        eb.taker_fee_lots + eb.maker_fee_lots
    );
    assert_eq!(
        w.env.get(&w.market),
        market_before,
        "the market account is never written by a fill"
    );
}

/// xorshift64*: a fixed-seed generator so the property test is reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Over a long random sequence of fills, requotes, deposits, withdrawals, fee sweeps and
/// slot advances across three books, every vault atom stays accounted for: base vault =
/// sum of base_free; quote vault = sum of quote_free + fees; and no token is created or
/// destroyed across all holders.
#[test]
fn conservation_holds_over_random_fill_sequences() {
    const SEED: u64 = 0x6e6f_636b_2026;
    const STEPS: usize = 400;
    let mut rng = Rng(SEED);
    let mut w = World::new();
    let makers: Vec<Maker> = (0..3)
        .map(|_| w.maker(500, 1_000_000, MID, &LADDER, &LADDER))
        .collect();
    let users: Vec<Actor> = (0..2).map(|_| w.actor(RICH)).collect();
    let dest = w.admin_quote_account();
    let books: Vec<Pubkey> = makers.iter().map(|m| m.book).collect();
    let mut seqs: Vec<u64> = makers.iter().map(|m| m.sequence).collect();
    let wallets: Vec<(Pubkey, Pubkey)> = makers
        .iter()
        .map(|m| (m.actor.base, m.actor.quote))
        .chain(users.iter().map(|u| (u.base, u.quote)))
        .collect();

    let holders = |w: &World| -> (u64, u64) {
        let base = w.env.token_amount(&w.base_vault);
        let quote = w.env.token_amount(&w.quote_vault) + w.env.token_amount(&dest);
        wallets.iter().fold((base, quote), |(b, q), (wb, wq)| {
            (b + w.env.token_amount(wb), q + w.env.token_amount(wq))
        })
    };
    let totals = holders(&w);
    let mut fills = 0;

    for step in 0..STEPS {
        let i = rng.below(makers.len() as u64) as usize;
        let op = rng.below(7);
        let ix = match op {
            0 | 1 => {
                let user = &users[rng.below(users.len() as u64) as usize];
                let lots = 1 + rng.below(12);
                let args = if op == 0 {
                    buy(lots, u64::MAX)
                } else {
                    sell(lots, 0)
                };
                w.swap_ix(user, books[i], args)
            }
            2 => {
                let mid = MID - 20_000 + rng.below(40_000);
                seqs[i] += 1;
                let seq = seqs[i];
                let bids: Vec<(u64, i64)> = (1..5).map(|k| (1 + rng.below(20), 500 * k)).collect();
                let asks: Vec<(u64, i64)> = (1..5).map(|k| (1 + rng.below(20), 500 * k)).collect();
                update_book_ix(makers[i].actor.key, books[i], seq, mid, &bids, &asks)
            }
            3 => w.deposit_ix(
                &makers[i].actor,
                books[i],
                rng.below(50),
                1 + rng.below(50_000),
            ),
            4 => {
                let b = w.env.book(&books[i]);
                w.withdraw_ix(
                    &makers[i].actor,
                    books[i],
                    rng.below(b.base_free + 1),
                    rng.below(b.quote_free + 1),
                )
            }
            5 => collect_fees_ix(w.market, books[i], w.quote_vault, dest),
            _ => {
                let slot = w.env.slot() + 1;
                w.env.warp(slot);
                seqs[i] += 1;
                let seq = seqs[i];
                seq_ix(
                    nock_interface::instruction::disc::MAKER_HEARTBEAT,
                    makers[i].actor.key,
                    books[i],
                    seq,
                )
            }
        };
        let result = w.env.run(&ix);
        if result.program_result.is_ok() && op <= 1 {
            fills += 1;
        }

        let (base_owed, quote_owed) = owed(&w.env, &books);
        assert_eq!(
            w.env.token_amount(&w.base_vault),
            base_owed * BASE_ATOMS_PER_LOT,
            "base vault, step {step}"
        );
        assert_eq!(
            w.env.token_amount(&w.quote_vault),
            quote_owed * QUOTE_ATOMS_PER_LOT,
            "quote vault, step {step}"
        );
        assert_eq!(holders(&w), totals, "token supply, step {step}");
    }
    assert!(
        fills > STEPS / 10,
        "the sequence must actually trade: {fills} fills"
    );
}
