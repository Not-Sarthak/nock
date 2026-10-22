//! Async swaps on the built program: book-priced RFQ fills, escrow lifecycle, expiry
//! boundaries, named makers and cancellation.

use crate::harness::*;
use nock_interface::{constants::*, error::NockError, events::*, ID};
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;

const MID: u64 = 1_000_000;
const LADDER: [(u64, i64); 3] = [(2, 1_000), (3, 2_000), (5, 3_000)];
const LIFETIME: u64 = 10;

struct Setup {
    w: World,
    maker: Maker,
    user: Actor,
}

fn setup() -> Setup {
    let mut w = World::new();
    let maker = w.maker(1_000, 10_000_000, MID, &LADDER, &LADDER);
    let user = w.actor(RICH);
    Setup { w, maker, user }
}

fn sell_args(nonce: u64, expiry: u64) -> nock_interface::instruction::AsyncSwapCreateData {
    async_args(0, nonce, 6, 1, 0, expiry, any_maker())
}

fn buy_args(nonce: u64, expiry: u64) -> nock_interface::instruction::AsyncSwapCreateData {
    async_args(1, nonce, 6, 1_000, u64::MAX, expiry, any_maker())
}

#[test]
fn create_escrows_the_input_and_records_the_terms() {
    let mut s = setup();
    let expiry = s.w.env.slot() + LIFETIME;
    let (ix, swap, escrow) = s.w.async_create(&s.user, 7, sell_args(7, expiry));
    s.w.env.assert_signers_and_writables_enforced(&ix);
    let r = s.w.env.ok(&ix);

    assert_eq!(s.w.env.token_amount(&escrow), 6 * BASE_ATOMS_PER_LOT);
    assert_eq!(
        s.w.env.token_amount(&s.user.base),
        RICH - 6 * BASE_ATOMS_PER_LOT
    );
    let escrow_data = s.w.env.get(&escrow).data;
    assert_eq!(
        &escrow_data[32..64],
        swap.as_ref(),
        "escrow is owned by the swap PDA"
    );
    assert_eq!(s.w.env.get(&swap).owner, ID);
    assert_eq!(
        s.w.env.get(&swap).lamports,
        s.w.env.mollusk.sysvars.rent.minimum_balance(ASYNC_SWAP_LEN)
    );

    let rec = s.w.env.async_swap(&swap);
    assert_eq!(rec.tag, ASYNC_SWAP_TAG);
    assert_eq!(
        (
            rec.nonce,
            rec.base_lots,
            rec.limit_quote_lots,
            rec.expiry_slot,
            rec.side
        ),
        (7, 6, 1, expiry, 0)
    );
    assert_eq!(rec.escrow, escrow.to_bytes());
    assert_eq!(rec.maker, NO_ADDRESS);
    assert_eq!(
        rec.bump,
        async_pda(&s.w.market, &s.user.key, 7).1,
        "canonical bump stored"
    );

    let e: AsyncSwapCreatedEvent = event(&r, tag::ASYNC_SWAP_CREATED);
    assert_eq!(
        (e.nonce, e.escrow_atoms, e.expiry_slot),
        (7, 6 * BASE_ATOMS_PER_LOT, expiry)
    );
}

/// Audit P0: async `min_out = 0` handed the whole escrow to any filler for nothing.
#[test]
fn regression_async_zero_limits_rejected() {
    let s = setup();
    let expiry = s.w.env.slot() + LIFETIME;
    for args in [
        async_args(0, 1, 6, 0, 0, expiry, any_maker()),
        async_args(0, 1, 0, 1, 0, expiry, any_maker()),
        async_args(1, 1, 6, 0, u64::MAX, expiry, any_maker()),
        async_args(1, 1, 0, 1_000, u64::MAX, expiry, any_maker()),
    ] {
        let (ix, _, _) = s.w.async_create(&s.user, 1, args);
        s.w.env.expect(&ix, custom(NockError::AmountZero));
    }
}

#[test]
fn create_rejects() {
    let mut s = setup();
    let now = s.w.env.slot();
    let create = |w: &World, args| w.async_create(&s.user, 1, args).0;

    s.w.env.expect(
        &create(&s.w, async_args(2, 1, 6, 1, 0, now + LIFETIME, any_maker())),
        custom(NockError::InvalidSide),
    );
    for expiry in [
        now - 1,
        now,
        now + MIN_ASYNC as u64 - 1,
        now + MAX_ASYNC as u64 + 1,
    ] {
        s.w.env.expect(
            &create(&s.w, sell_args(1, expiry)),
            custom(NockError::AsyncWindowViolated),
        );
    }
    for expiry in [now + MIN_ASYNC as u64, now + MAX_ASYNC as u64] {
        assert!(s
            .w
            .env
            .try_ix(&create(&s.w, sell_args(1, expiry)))
            .program_result
            .is_ok());
    }

    // the wrong input mint for the side
    let mut ix = create(&s.w, sell_args(1, now + LIFETIME));
    ix.accounts[4].pubkey = s.w.quote_mint;
    s.w.env.expect(&ix, custom(NockError::InvalidMint));

    // a non-canonical record address
    let (odd, _) = non_canonical(&[
        SEED_ASYNC_SWAP,
        s.w.market.as_ref(),
        s.user.key.as_ref(),
        &1u64.to_le_bytes(),
    ]);
    let mut ix = create(&s.w, sell_args(1, now + LIFETIME));
    ix.accounts[2].pubkey = odd;
    ix.accounts[3].pubkey = ata(&odd, &s.w.base_mint);
    s.w.env.expect(&ix, custom(NockError::InvalidPda));

    // an escrow that is not the record's associated token account
    let mut ix = create(&s.w, sell_args(1, now + LIFETIME));
    ix.accounts[3].pubkey = ata(&s.user.key, &s.w.base_mint);
    assert!(!s.w.env.try_ix(&ix).program_result.is_ok());

    let mut ix = create(&s.w, sell_args(1, now + LIFETIME));
    ix.accounts[7].pubkey = TOKEN_2022;
    s.w.env.expect(&ix, custom(NockError::Token2022Unsupported));

    // more than the user holds
    let poor = s.w.actor(5 * BASE_ATOMS_PER_LOT);
    let (ix, _, _) = s.w.async_create(&poor, 1, sell_args(1, now + LIFETIME));
    assert!(!s.w.env.try_ix(&ix).program_result.is_ok());

    s.w.env.ok(&s.w.toggle_paused(true));
    s.w.env.expect(
        &create(&s.w, sell_args(1, now + LIFETIME)),
        custom(NockError::MarketPaused),
    );
}

#[test]
fn create_survives_prefunded_record_and_escrow_and_allows_many_nonces() {
    let mut s = setup();
    let expiry = s.w.env.slot() + LIFETIME;
    let (swap, _) = async_pda(&s.w.market, &s.user.key, 3);
    s.w.env.set(swap, wallet(1));
    s.w.env.set(ata(&swap, &s.w.base_mint), wallet(1));
    let (ix, _, _) = s.w.async_create(&s.user, 3, sell_args(3, expiry));
    s.w.env.ok(&ix);
    let (ix, _, _) = s.w.async_create(&s.user, 4, buy_args(4, expiry));
    s.w.env.ok(&ix);
    let (ix, _, _) = s.w.async_create(&s.user, 3, sell_args(3, expiry));
    assert!(
        !s.w.env.try_ix(&ix).program_result.is_ok(),
        "a live nonce cannot be reused"
    );
}

fn rents(s: &Setup, swap: &Pubkey, escrow: &Pubkey) -> u64 {
    s.w.env.get(swap).lamports + s.w.env.get(escrow).lamports
}

#[test]
fn execute_sell_fills_from_the_book_and_closes_everything() {
    let mut s = setup();
    let expiry = s.w.env.slot() + LIFETIME;
    let (ix, swap, escrow) = s.w.async_create(&s.user, 1, sell_args(1, expiry));
    s.w.env.ok(&ix);
    let rent = rents(&s, &swap, &escrow);
    let lamports_before = s.w.env.get(&s.user.key).lamports;
    let quote_before = s.w.env.token_amount(&s.user.quote);
    let base_vault = s.w.env.token_amount(&s.w.base_vault);

    let ix =
        s.w.async_execute(s.maker.actor.key, s.maker.book, &s.user, swap, escrow, 0);
    s.w.env.assert_signers_and_writables_enforced(&ix);
    let r = s.w.env.ok(&ix);

    // same price as the sync sell of 6 lots: 598 gross, 597 net
    let e: AsyncFillEvent = event(&r, tag::ASYNC_FILL);
    assert_eq!(
        (
            e.gross_quote_lots,
            e.taker_quote_lots,
            e.refunded_atoms,
            e.side
        ),
        (598, 597, 0, 0)
    );
    assert_eq!(e.signer, s.maker.actor.key.to_bytes());
    assert_eq!(
        s.w.env.token_amount(&s.user.quote),
        quote_before + 597 * QUOTE_ATOMS_PER_LOT
    );
    assert_eq!(
        s.w.env.token_amount(&s.w.base_vault),
        base_vault + 6 * BASE_ATOMS_PER_LOT
    );
    assert_eq!(
        s.w.env.book(&s.maker.book).fees_quote_lots,
        e.taker_fee_lots + e.maker_fee_lots
    );

    assert_eq!(s.w.env.get(&swap).lamports, 0);
    assert_eq!(s.w.env.get(&escrow).lamports, 0);
    assert_eq!(s.w.env.get(&s.user.key).lamports, lamports_before + rent);

    // no second fill
    assert!(!s.w.env.try_ix(&ix).program_result.is_ok());
    assert!(!s
        .w
        .env
        .try_ix(&async_cancel_ix(
            s.user.key,
            swap,
            escrow,
            s.user.base,
            s.user.key
        ))
        .program_result
        .is_ok());
}

#[test]
fn execute_buy_refunds_unspent_quote() {
    let mut s = setup();
    let expiry = s.w.env.slot() + LIFETIME;
    let (ix, swap, escrow) = s.w.async_create(&s.user, 1, buy_args(1, expiry));
    s.w.env.ok(&ix);
    let r =
        s.w.env.ok(&s
            .w
            .async_execute(s.maker.actor.key, s.maker.book, &s.user, swap, escrow, 1));
    // costs 603 lots of the 1_000 escrowed
    let e: AsyncFillEvent = event(&r, tag::ASYNC_FILL);
    assert_eq!(
        (e.taker_quote_lots, e.refunded_atoms),
        (603, 397 * QUOTE_ATOMS_PER_LOT)
    );
    assert_eq!(
        s.w.env.token_amount(&s.user.quote),
        RICH - 603 * QUOTE_ATOMS_PER_LOT
    );
    assert_eq!(
        s.w.env.token_amount(&s.user.base),
        RICH + 6 * BASE_ATOMS_PER_LOT
    );
}

/// Audit P1: the escrow token account was never closed, leaking its rent and anything sent
/// to it.
#[test]
fn regression_escrow_rent_leak() {
    for cancel in [false, true] {
        let mut s = setup();
        let expiry = s.w.env.slot() + LIFETIME;
        let (ix, swap, escrow) = s.w.async_create(&s.user, 1, sell_args(1, expiry));
        s.w.env.ok(&ix);
        // someone sends extra tokens to the escrow
        let mut account = s.w.env.get(&escrow);
        let donated = 12_345;
        let amount = s.w.env.token_amount(&escrow) + donated;
        account.data[64..72].copy_from_slice(&amount.to_le_bytes());
        s.w.env.set(escrow, account);

        let rent = rents(&s, &swap, &escrow);
        let lamports_before = s.w.env.get(&s.user.key).lamports;
        let base_before = s.w.env.token_amount(&s.user.base);
        let refunded = if cancel {
            let r = s.w.env.ok(&async_cancel_ix(
                s.user.key,
                swap,
                escrow,
                s.user.base,
                s.user.key,
            ));
            event::<AsyncCancelEvent>(&r, tag::ASYNC_CANCEL).refunded_atoms
        } else {
            let r = s.w.env.ok(&s.w.async_execute(
                s.maker.actor.key,
                s.maker.book,
                &s.user,
                swap,
                escrow,
                0,
            ));
            event::<AsyncFillEvent>(&r, tag::ASYNC_FILL).refunded_atoms
        };
        let expected = if cancel {
            6 * BASE_ATOMS_PER_LOT + donated
        } else {
            donated
        };
        assert_eq!(refunded, expected);
        assert_eq!(s.w.env.token_amount(&s.user.base), base_before + expected);
        let closed = s.w.env.get(&escrow);
        assert_eq!(
            (closed.lamports, closed.data.len()),
            (0, 0),
            "escrow closed"
        );
        assert_eq!(
            s.w.env.get(&s.user.key).lamports,
            lamports_before + rent,
            "both rents back to the user"
        );
    }
}

#[test]
fn execute_and_cancel_at_the_expiry_boundary() {
    let mut s = setup();
    let expiry = s.w.env.slot() + LIFETIME;
    let (ix, swap, escrow) = s.w.async_create(&s.user, 1, sell_args(1, expiry));
    s.w.env.ok(&ix);
    // keep the maker's quotes live across the warp
    let exec =
        s.w.async_execute(s.maker.actor.key, s.maker.book, &s.user, swap, escrow, 0);
    let stranger = s.w.actor(0);
    let release = async_cancel_ix(stranger.key, swap, escrow, s.user.base, s.user.key);

    s.w.env.warp(expiry - 1);
    s.w.env.ok(&seq_ix(
        nock_interface::instruction::disc::MAKER_HEARTBEAT,
        s.maker.actor.key,
        s.maker.book,
        2,
    ));
    assert!(s.w.env.try_ix(&exec).program_result.is_ok());
    s.w.env
        .expect(&release, custom(NockError::AsyncSwapNotExpired));

    s.w.env.warp(expiry);
    s.w.env.ok(&seq_ix(
        nock_interface::instruction::disc::MAKER_HEARTBEAT,
        s.maker.actor.key,
        s.maker.book,
        3,
    ));
    s.w.env.expect(&exec, custom(NockError::AsyncSwapExpired));
    let r = s.w.env.ok(&release);
    let e: AsyncCancelEvent = event(&r, tag::ASYNC_CANCEL);
    assert_eq!(
        (e.caller, e.user),
        (stranger.key.to_bytes(), s.user.key.to_bytes())
    );
    assert_eq!(s.w.env.token_amount(&s.user.base), RICH);
}

/// Audit P0: the user could not get their escrow back before expiry.
#[test]
fn regression_early_async_cancel() {
    let mut s = setup();
    let expiry = s.w.env.slot() + MAX_ASYNC as u64;
    let (ix, swap, escrow) = s.w.async_create(&s.user, 1, buy_args(1, expiry));
    s.w.env.ok(&ix);
    s.w.env.ok(&s.w.toggle_paused(true));
    let cancel = async_cancel_ix(s.user.key, swap, escrow, s.user.quote, s.user.key);
    s.w.env.assert_signers_and_writables_enforced(&cancel);
    s.w.env.ok(&cancel);
    assert_eq!(
        s.w.env.token_amount(&s.user.quote),
        RICH,
        "full refund, even while paused"
    );
    assert_eq!(s.w.env.get(&swap).lamports, 0);
}

#[test]
fn cancel_rejects() {
    let mut s = setup();
    let expiry = s.w.env.slot() + LIFETIME;
    let (ix, swap, escrow) = s.w.async_create(&s.user, 1, sell_args(1, expiry));
    s.w.env.ok(&ix);
    let thief = s.w.actor(0);
    s.w.env.expect(
        &async_cancel_ix(s.user.key, swap, escrow, thief.base, s.user.key),
        custom(NockError::IncorrectTokenAccount),
    );
    s.w.env.expect(
        &async_cancel_ix(s.user.key, swap, escrow, s.user.quote, s.user.key),
        custom(NockError::IncorrectTokenAccount),
    );
    s.w.env.expect(
        &async_cancel_ix(s.user.key, swap, escrow, s.user.base, thief.key),
        failure(ProgramError::InvalidArgument),
    );
    s.w.env.expect(
        &async_cancel_ix(s.user.key, swap, s.user.base, s.user.base, s.user.key),
        custom(NockError::IncorrectTokenAccount),
    );
    s.w.env.expect(
        &async_cancel_ix(s.user.key, s.maker.book, escrow, s.user.base, s.user.key),
        failure(ProgramError::InvalidAccountData),
    );
}

#[test]
fn a_named_maker_or_its_delegate_fills_and_no_one_else() {
    let mut s = setup();
    let other = s.w.maker(1_000, 10_000_000, MID, &LADDER, &LADDER);
    let delegate = s.w.actor(0);
    s.w.env.ok(&delegate_ix(
        s.maker.actor.key,
        s.maker.book,
        delegate.key,
        0,
        u64::MAX,
        u64::MAX,
    ));
    let expiry = s.w.env.slot() + LIFETIME;
    let args = async_args(0, 1, 6, 1, 0, expiry, s.maker.actor.key);
    let (ix, swap, escrow) = s.w.async_create(&s.user, 1, args);
    s.w.env.ok(&ix);

    let by_other =
        s.w.async_execute(other.actor.key, other.book, &s.user, swap, escrow, 0);
    s.w.env
        .expect(&by_other, custom(NockError::MakerNotSelected));
    let by_delegate =
        s.w.async_execute(delegate.key, s.maker.book, &s.user, swap, escrow, 0);
    let r = s.w.env.ok(&by_delegate);
    let e: AsyncFillEvent = event(&r, tag::ASYNC_FILL);
    assert_eq!(
        (e.maker, e.signer),
        (s.maker.actor.key.to_bytes(), delegate.key.to_bytes())
    );
}

#[test]
fn execute_rejects() {
    let mut s = setup();
    let expiry = s.w.env.slot() + MAX_ASYNC as u64;
    let (ix, swap, escrow) = s.w.async_create(&s.user, 1, sell_args(1, expiry));
    s.w.env.ok(&ix);
    let k = s.maker.actor.key;
    let b = s.maker.book;
    let exec = s.w.async_execute(k, b, &s.user, swap, escrow, 0);

    let stranger = s.w.actor(0);
    let mut ix = exec.clone();
    ix.accounts[0].pubkey = stranger.key;
    s.w.env.expect(&ix, custom(NockError::UnauthorizedMaker));

    // outputs must belong to the user
    let thief = s.w.actor(0);
    let mut ix = exec.clone();
    ix.accounts[7].pubkey = thief.quote;
    s.w.env
        .expect(&ix, custom(NockError::IncorrectTokenAccount));
    let mut ix = exec.clone();
    ix.accounts[8].pubkey = s.user.quote;
    s.w.env
        .expect(&ix, custom(NockError::IncorrectTokenAccount));
    let mut ix = exec.clone();
    ix.accounts[9].pubkey = thief.key;
    s.w.env.expect(&ix, failure(ProgramError::InvalidArgument));
    let mut ix = exec.clone();
    ix.accounts[4].pubkey = s.user.base;
    s.w.env
        .expect(&ix, custom(NockError::IncorrectTokenAccount));
    let mut ix = exec.clone();
    ix.accounts[5].pubkey = thief.base;
    s.w.env.expect(&ix, custom(NockError::IncorrectVault));

    // a book from another market, and this swap against another market
    let other = s.w.other_market();
    let other_maker = s.w.actor(RICH);
    let (other_book, _) = book_pda(&other.market, &other_maker.key);
    s.w.env
        .ok(&init_book_ix(other_maker.key, other.market, other_book));
    let mut ix = exec.clone();
    ix.accounts[0].pubkey = other_maker.key;
    ix.accounts[2].pubkey = other_book;
    s.w.env.expect(&ix, custom(NockError::MarketMismatch));
    let mut ix = exec.clone();
    ix.accounts[1].pubkey = other.market;
    ix.accounts[5].pubkey = other.base_vault;
    ix.accounts[6].pubkey = other.quote_vault;
    s.w.env.expect(&ix, custom(NockError::MarketMismatch));

    // the user's limits bind
    let (ix2, swap2, escrow2) =
        s.w.async_create(&s.user, 2, async_args(0, 2, 6, 598, 0, expiry, any_maker()));
    s.w.env.ok(&ix2);
    s.w.env.expect(
        &s.w.async_execute(k, b, &s.user, swap2, escrow2, 0),
        custom(NockError::SlippageExceeded),
    );
    let (ix3, swap3, escrow3) = s.w.async_create(
        &s.user,
        3,
        async_args(1, 3, 6, 1_000, 1_002_000, expiry, any_maker()),
    );
    s.w.env.ok(&ix3);
    s.w.env.expect(
        &s.w.async_execute(k, b, &s.user, swap3, escrow3, 1),
        custom(NockError::SlippageExceeded),
    );

    // book and market state
    s.w.env.ok(&halt_ix(k, b, 2, 1));
    s.w.env.expect(&exec, custom(NockError::BookHalted));
    s.w.env.ok(&halt_ix(k, b, 3, 0));
    s.w.env.ok(&s.w.toggle_paused(true));
    s.w.env.expect(&exec, custom(NockError::MarketPaused));
    s.w.env.ok(&s.w.toggle_paused(false));
    s.w.env.warp(s.w.env.slot() + DEFAULT_EXPIRY_IN_SLOTS);
    s.w.env.expect(&exec, custom(NockError::QuoteExpired));
}
