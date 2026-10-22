//! Maker books on the built program: init, quoting, delegate bounds, sequence, halt,
//! spread, expiry, inventory and close.

use crate::harness::*;
use nock_interface::{
    constants::*,
    error::NockError,
    events::*,
    instruction::disc,
    state::{maker_book::book_status, MakerLevel},
    ID,
};
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;

const MID: u64 = 1_000_000;

fn fresh_book(w: &mut World) -> (Actor, Pubkey) {
    let actor = w.actor(RICH);
    let (book, _) = book_pda(&w.market, &actor.key);
    (actor, book)
}

#[test]
fn init_book_creates_an_empty_book() {
    let mut w = World::new();
    let (actor, book) = fresh_book(&mut w);
    let ix = init_book_ix(actor.key, w.market, book);
    w.env.assert_signers_and_writables_enforced(&ix);
    let r = w.env.ok(&ix);

    let account = w.env.get(&book);
    assert_eq!(account.owner, ID);
    assert_eq!(
        account.lamports,
        w.env.mollusk.sysvars.rent.minimum_balance(MAKER_BOOK_LEN)
    );
    let b = w.env.book(&book);
    assert_eq!(b.tag, MAKER_BOOK_TAG);
    assert_eq!(b.maker, actor.key.to_bytes());
    assert_eq!(b.market, w.market.to_bytes());
    assert_eq!(b.delegate, NO_ADDRESS);
    assert_eq!(b.expiry_in_slots, DEFAULT_EXPIRY_IN_SLOTS);
    assert_eq!(
        b.maker_book_bump,
        book_pda(&w.market, &actor.key).1,
        "canonical bump stored"
    );
    let e: BookEvent = event(&r, tag::BOOK_INITIALIZED);
    assert_eq!(
        (e.book, e.maker, e.signer),
        (book.to_bytes(), actor.key.to_bytes(), actor.key.to_bytes())
    );
}

#[test]
fn init_book_rejects() {
    let mut w = World::new();
    let (actor, book) = fresh_book(&mut w);

    let (odd, _) = non_canonical(&[SEED_MAKER_BOOK, w.market.as_ref(), actor.key.as_ref()]);
    w.env.expect(
        &init_book_ix(actor.key, w.market, odd),
        custom(NockError::InvalidPda),
    );

    let other_maker = Pubkey::new_unique();
    let (theirs, _) = book_pda(&w.market, &other_maker);
    w.env.expect(
        &init_book_ix(actor.key, w.market, theirs),
        custom(NockError::InvalidPda),
    );

    let mut fake = w.env.get(&w.market);
    fake.owner = Pubkey::new_unique();
    let fake_market = Pubkey::new_unique();
    w.env.set(fake_market, fake);
    let (fb, _) = book_pda(&fake_market, &actor.key);
    w.env.expect(
        &init_book_ix(actor.key, fake_market, fb),
        failure(ProgramError::IllegalOwner),
    );

    let mut sys = init_book_ix(actor.key, w.market, book);
    sys.accounts[3].pubkey = Pubkey::new_unique();
    w.env.expect(&sys, failure(ProgramError::InvalidArgument));

    w.env.ok(&init_book_ix(actor.key, w.market, book));
    assert!(!w
        .env
        .try_ix(&init_book_ix(actor.key, w.market, book))
        .program_result
        .is_ok());

    // a book passed as the market
    let (b2, _) = book_pda(&book, &actor.key);
    w.env.expect(
        &init_book_ix(actor.key, book, b2),
        failure(ProgramError::InvalidAccountData),
    );
}

#[test]
fn init_book_survives_a_prefunded_address() {
    let mut w = World::new();
    let (actor, book) = fresh_book(&mut w);
    w.env.set(book, wallet(1));
    w.env.ok(&init_book_ix(actor.key, w.market, book));
    assert_eq!(
        w.env.get(&book).lamports,
        w.env.mollusk.sysvars.rent.minimum_balance(MAKER_BOOK_LEN)
    );
}

#[test]
fn update_book_writes_levels_and_stamps_the_slot() {
    let mut w = World::new();
    let maker = w.maker(10, 10, MID, &[], &[]);
    let ix = update_book_ix(
        maker.actor.key,
        maker.book,
        2,
        MID,
        &[(3, 1), (4, 5)],
        &[(6, 2)],
    );
    w.env.assert_signers_and_writables_enforced(&ix);
    w.env.warp(START_SLOT + 7);
    let r = w.env.ok(&ix);
    let b = w.env.book(&maker.book);
    assert_eq!(
        (b.sequence, b.mid_price_ticks, b.last_updated_slot),
        (2, MID, START_SLOT + 7)
    );
    assert_eq!(b.bid_levels[1].size_in_base_lots, 4);
    assert_eq!(b.bid_levels[2].size_in_base_lots, 0);
    assert_eq!(b.ask_levels[0].price_offset_ticks, 2);
    let e: BookEvent = event(&r, tag::BOOK_UPDATED);
    assert_eq!((e.num_bids, e.num_asks, e.sequence), (2, 1, 2));
}

#[test]
fn update_book_rejects_bad_quotes_and_signers() {
    let mut w = World::new();
    let maker = w.maker(10, 10, MID, &[], &[]);
    let k = maker.actor.key;
    let b = maker.book;
    w.env.expect(
        &update_book_ix(k, b, 1, MID, &[], &[]),
        custom(NockError::StaleSequence),
    );
    w.env.expect(
        &update_book_ix(k, b, 2, MID, &[(1, 2), (1, 1)], &[]),
        custom(NockError::InvalidQuotes),
    );
    w.env.expect(
        &update_book_ix(k, b, 2, MID, &[(1, -5)], &[(1, -5)]),
        custom(NockError::InvalidQuotes),
    );
    w.env.expect(
        &update_book_ix(k, b, 2, 5, &[(1, 5)], &[]),
        custom(NockError::InvalidQuotes),
    );
    let stranger = w.actor(0);
    w.env.expect(
        &update_book_ix(stranger.key, b, 2, MID, &[], &[]),
        custom(NockError::UnauthorizedMaker),
    );

    let mut too_many = update_book_ix(k, b, 2, MID, &[], &[]);
    too_many.data[1 + 16] = (MAX_LEVELS + 1) as u8;
    w.env
        .expect(&too_many, failure(ProgramError::InvalidInstructionData));
    let mut short = update_book_ix(k, b, 2, MID, &[], &[]);
    short.data.pop();
    w.env
        .expect(&short, failure(ProgramError::InvalidInstructionData));

    let mut foreign = w.env.get(&b);
    foreign.owner = Pubkey::new_unique();
    let fake = Pubkey::new_unique();
    w.env.set(fake, foreign);
    w.env.expect(
        &update_book_ix(k, fake, 2, MID, &[], &[]),
        failure(ProgramError::IllegalOwner),
    );
    w.env.expect(
        &update_book_ix(k, w.market, 2, MID, &[], &[]),
        failure(ProgramError::InvalidAccountData),
    );
}

/// Audit P1: a delegate could push the sequence to u64::MAX and freeze every update.
#[test]
fn regression_sequence_brick() {
    let mut w = World::new();
    let maker = w.maker(10, 10, MID, &[], &[]);
    let delegate = w.actor(0);
    w.env.ok(&delegate_ix(
        maker.actor.key,
        maker.book,
        delegate.key,
        0,
        u64::MAX,
        u64::MAX,
    ));

    for tag in [
        disc::MAKER_SET_SEQUENCE,
        disc::MAKER_HEARTBEAT,
        disc::CLEAR_BOOK,
    ] {
        w.env.expect(
            &seq_ix(tag, delegate.key, maker.book, u64::MAX),
            custom(NockError::SequenceJumpTooLarge),
        );
    }
    let r = w.env.ok(&seq_ix(
        disc::MAKER_SET_SEQUENCE,
        delegate.key,
        maker.book,
        1 + MAX_SEQUENCE_JUMP,
    ));
    let e: BookEvent = event(&r, tag::SEQUENCE_SET);
    assert_eq!(e.signer, delegate.key.to_bytes());
    assert_eq!(w.env.book(&maker.book).sequence, 1 + MAX_SEQUENCE_JUMP);

    // only the maker can reset, to any value
    w.env.expect(
        &seq_ix(disc::MAKER_RESET_SEQUENCE, delegate.key, maker.book, 0),
        custom(NockError::UnauthorizedMaker),
    );
    let reset = seq_ix(disc::MAKER_RESET_SEQUENCE, maker.actor.key, maker.book, 3);
    w.env.assert_signers_and_writables_enforced(&reset);
    let r = w.env.ok(&reset);
    let e: BookEvent = event(&r, tag::SEQUENCE_RESET);
    assert_eq!(e.sequence, 3);
    w.env.ok(&update_book_ix(
        maker.actor.key,
        maker.book,
        4,
        MID,
        &[],
        &[],
    ));
}

/// Audit P1: halt and resume were unsequenced, so a stale resume could land after a halt.
#[test]
fn regression_halt_ordering() {
    let mut w = World::new();
    let maker = w.maker(10, 1_000, MID, &[(5, 1)], &[(5, 1)]);
    let halt = halt_ix(maker.actor.key, maker.book, 5, 1);
    w.env.assert_signers_and_writables_enforced(&halt);
    let r = w.env.ok(&halt);
    let e: BookEvent = event(&r, tag::HALT_SET);
    assert_eq!(e.status, book_status::HALTED);

    // the resume signed earlier with sequence 4 lands late: rejected
    w.env.expect(
        &halt_ix(maker.actor.key, maker.book, 4, 0),
        custom(NockError::StaleSequence),
    );
    assert_eq!(w.env.book(&maker.book).status, book_status::HALTED);
    let user = w.actor(RICH);
    w.env.expect(
        &w.swap_ix(&user, maker.book, buy(1, u64::MAX)),
        custom(NockError::BookHalted),
    );

    w.env.expect(
        &halt_ix(maker.actor.key, maker.book, 6, 2),
        failure(ProgramError::InvalidInstructionData),
    );
    w.env.ok(&halt_ix(maker.actor.key, maker.book, 6, 0));
    w.env.ok(&w.swap_ix(&user, maker.book, buy(1, u64::MAX)));
}

#[test]
fn a_delegate_may_halt_but_only_the_maker_resumes() {
    let mut w = World::new();
    let maker = w.maker(10, 10, MID, &[], &[]);
    let delegate = w.actor(0);
    w.env.ok(&delegate_ix(
        maker.actor.key,
        maker.book,
        delegate.key,
        0,
        u64::MAX,
        u64::MAX,
    ));
    w.env.ok(&halt_ix(delegate.key, maker.book, 2, 1));
    w.env.expect(
        &halt_ix(delegate.key, maker.book, 3, 0),
        custom(NockError::UnauthorizedMaker),
    );
    w.env.ok(&halt_ix(maker.actor.key, maker.book, 3, 0));
}

/// Audit P1: a delegate had unbounded quoting power over the maker's inventory.
#[test]
fn regression_delegate_outside_band() {
    let mut w = World::new();
    let maker = w.maker(100, 100_000, MID, &[(5, 1)], &[(5, 1)]);
    let delegate = w.actor(0);
    let d = delegate.key;
    let b = maker.book;

    // a delegate with no bounds set can post nothing
    w.env.ok(&delegate_ix(maker.actor.key, b, d, 0, 0, 0));
    w.env.expect(
        &update_book_ix(d, b, 2, MID, &[(1, 1)], &[]),
        custom(NockError::OutsideDelegateBounds),
    );

    let set = delegate_ix(maker.actor.key, b, d, MID - 100, MID + 100, 10);
    w.env.assert_signers_and_writables_enforced(&set);
    let r = w.env.ok(&set);
    let e: DelegateSetEvent = event(&r, tag::DELEGATE_SET);
    assert_eq!(
        (
            e.delegate,
            e.min_price_ticks,
            e.max_price_ticks,
            e.max_level_lots
        ),
        (d.to_bytes(), MID - 100, MID + 100, 10)
    );

    let r = w
        .env
        .ok(&update_book_ix(d, b, 2, MID, &[(10, 50)], &[(10, 100)]));
    let e: BookEvent = event(&r, tag::BOOK_UPDATED);
    assert_eq!(
        (e.maker, e.signer),
        (maker.actor.key.to_bytes(), d.to_bytes())
    );

    // a 1-tick ask for a colluding taker is out of band
    w.env.expect(
        &update_book_ix(d, b, 3, 1_000, &[], &[(10, -999)]),
        custom(NockError::OutsideDelegateBounds),
    );
    w.env.expect(
        &update_book_ix(d, b, 3, MID, &[], &[(10, 101)]),
        custom(NockError::OutsideDelegateBounds),
    );
    w.env.expect(
        &update_book_ix(d, b, 3, MID, &[], &[(11, 1)]),
        custom(NockError::OutsideDelegateBounds),
    );
    w.env.expect(
        &mid_ix(d, b, 3, MID + 1),
        custom(NockError::OutsideDelegateBounds),
    );
    w.env.expect(
        &spread_ix(d, b, 3, 1),
        custom(NockError::OutsideDelegateBounds),
    );
    // the maker is not bounded
    w.env.ok(&update_book_ix(
        maker.actor.key,
        b,
        3,
        MID,
        &[],
        &[(1_000, 5_000)],
    ));

    // no fund powers
    w.env.expect(
        &w.withdraw_ix(&delegate, b, 1, 0),
        custom(NockError::UnauthorizedMaker),
    );
    w.env
        .expect(&close_book_ix(d, b), custom(NockError::UnauthorizedMaker));
    w.env.expect(
        &delegate_ix(d, b, d, 0, u64::MAX, u64::MAX),
        custom(NockError::UnauthorizedMaker),
    );
    w.env.expect(
        &w.deposit_ix(&delegate, b, 1, 0),
        custom(NockError::UnauthorizedMaker),
    );

    // revoked
    w.env
        .ok(&delegate_ix(maker.actor.key, b, any_maker(), 0, 0, 0));
    w.env.expect(
        &seq_ix(disc::MAKER_HEARTBEAT, d, b, 10),
        custom(NockError::UnauthorizedMaker),
    );

    w.env.expect(
        &delegate_ix(maker.actor.key, b, d, 2, 1, 0),
        failure(ProgramError::InvalidInstructionData),
    );
}

#[test]
fn mid_clear_heartbeat_spread_and_expiry() {
    let mut w = World::new();
    let mut maker = w.maker(10, 1_000, MID, &[(5, 1)], &[(5, 1)]);
    let k = maker.actor.key;
    let b = maker.book;

    w.env.warp(START_SLOT + 3);
    let ix = mid_ix(k, b, maker.next_seq(), MID + 10);
    w.env.assert_signers_and_writables_enforced(&ix);
    let r = w.env.ok(&ix);
    let book = w.env.book(&b);
    assert_eq!(
        (book.mid_price_ticks, book.last_updated_slot),
        (MID + 10, START_SLOT + 3)
    );
    assert_eq!(
        event::<BookEvent>(&r, tag::MID_UPDATED).mid_price_ticks,
        MID + 10
    );
    w.env.expect(
        &mid_ix(k, b, maker.next_seq(), 1),
        custom(NockError::InvalidQuotes),
    );

    w.env.warp(START_SLOT + 5);
    let ix = seq_ix(disc::MAKER_HEARTBEAT, k, b, maker.next_seq());
    w.env.assert_signers_and_writables_enforced(&ix);
    let r = w.env.ok(&ix);
    assert_eq!(w.env.book(&b).last_updated_slot, START_SLOT + 5);
    assert_eq!(
        event::<BookEvent>(&r, tag::HEARTBEAT).last_updated_slot,
        START_SLOT + 5
    );

    let ix = spread_ix(k, b, maker.next_seq(), 7);
    w.env.assert_signers_and_writables_enforced(&ix);
    let r = w.env.ok(&ix);
    assert_eq!(
        event::<BookEvent>(&r, tag::SPREAD_UPDATED).sync_spread_ticks,
        7
    );
    w.env.ok(&update_book_ix(
        k,
        b,
        maker.next_seq(),
        50_000,
        &[(5, 1)],
        &[],
    ));
    w.env.expect(
        &spread_ix(k, b, maker.next_seq(), u16::MAX),
        custom(NockError::InvalidQuotes),
    );

    for bad in [0, MAX_QUOTE_AGE_SLOTS + 1] {
        w.env.expect(
            &expiry_ix(k, b, maker.next_seq(), bad),
            custom(NockError::InvalidExpiry),
        );
    }
    let ix = expiry_ix(k, b, maker.next_seq(), 1);
    w.env.assert_signers_and_writables_enforced(&ix);
    let r = w.env.ok(&ix);
    assert_eq!(
        event::<BookEvent>(&r, tag::EXPIRY_UPDATED).expiry_in_slots,
        1
    );

    let ix = seq_ix(disc::CLEAR_BOOK, k, b, maker.next_seq());
    w.env.assert_signers_and_writables_enforced(&ix);
    let r = w.env.ok(&ix);
    let book = w.env.book(&b);
    assert!(book
        .bid_levels
        .iter()
        .chain(book.ask_levels.iter())
        .all(|l: &MakerLevel| l.size_in_base_lots == 0));
    let e: BookEvent = event(&r, tag::BOOK_CLEARED);
    assert_eq!((e.num_bids, e.num_asks), (0, 0));

    let stale = maker.sequence;
    w.env.expect(
        &seq_ix(disc::MAKER_HEARTBEAT, k, b, stale),
        custom(NockError::StaleSequence),
    );
}

#[test]
fn deposit_and_withdraw_move_lots_between_wallet_and_vaults() {
    let mut w = World::new();
    let (actor, book) = fresh_book(&mut w);
    w.env.ok(&init_book_ix(actor.key, w.market, book));

    let ix = w.deposit_ix(&actor, book, 7, 9);
    w.env.assert_signers_and_writables_enforced(&ix);
    let r = w.env.ok(&ix);
    assert_eq!(w.env.token_amount(&w.base_vault), 7 * BASE_ATOMS_PER_LOT);
    assert_eq!(w.env.token_amount(&w.quote_vault), 9 * QUOTE_ATOMS_PER_LOT);
    assert_eq!(
        w.env.token_amount(&actor.base),
        RICH - 7 * BASE_ATOMS_PER_LOT
    );
    let e: InventoryEvent = event(&r, tag::DEPOSIT);
    assert_eq!(
        (e.base_lots, e.quote_lots, e.base_free, e.quote_free),
        (7, 9, 7, 9)
    );

    // one side only: the other token account is not touched
    w.env.ok(&w.deposit_ix(&actor, book, 0, 1));

    let ix = w.withdraw_ix(&actor, book, 7, 10);
    w.env.assert_signers_and_writables_enforced(&ix);
    w.env.ok(&w.toggle_paused(true));
    let r = w.env.ok(&ix);
    let e: InventoryEvent = event(&r, tag::WITHDRAW);
    assert_eq!((e.base_free, e.quote_free), (0, 0));
    assert_eq!(w.env.token_amount(&actor.base), RICH);
    assert_eq!(w.env.token_amount(&w.base_vault), 0);
}

#[test]
fn deposit_and_withdraw_reject() {
    let mut w = World::new();
    let maker = w.maker(5, 5, MID, &[], &[]);
    let a = &maker.actor;
    let b = maker.book;
    w.env
        .expect(&w.deposit_ix(a, b, 0, 0), custom(NockError::AmountZero));
    w.env
        .expect(&w.withdraw_ix(a, b, 0, 0), custom(NockError::AmountZero));
    w.env.expect(
        &w.withdraw_ix(a, b, 6, 0),
        custom(NockError::InsufficientInventory),
    );
    w.env.expect(
        &w.withdraw_ix(a, b, 0, 6),
        custom(NockError::InsufficientInventory),
    );

    // vault substitution: the maker's own accounts in place of the vaults
    let mut ix = w.deposit_ix(a, b, 1, 0);
    ix.accounts[3].pubkey = a.quote;
    w.env.expect(&ix, custom(NockError::IncorrectVault));
    let mut ix = w.withdraw_ix(a, b, 1, 0);
    ix.accounts[3].pubkey = a.base;
    w.env.expect(&ix, custom(NockError::IncorrectVault));

    // a book from another market
    let other = w.other_market();
    let mut ix = w.deposit_ix(a, b, 1, 0);
    ix.accounts[1].pubkey = other.market;
    ix.accounts[3].pubkey = other.base_vault;
    ix.accounts[4].pubkey = other.quote_vault;
    w.env.expect(&ix, custom(NockError::MarketMismatch));

    let mut ix = w.deposit_ix(a, b, 1, 0);
    ix.accounts[7].pubkey = TOKEN_2022;
    w.env.expect(&ix, custom(NockError::Token2022Unsupported));

    w.env.ok(&w.toggle_paused(true));
    w.env
        .expect(&w.deposit_ix(a, b, 1, 0), custom(NockError::MarketPaused));
}

#[test]
fn close_book_refunds_rent_then_emits() {
    let mut w = World::new();
    let maker = w.maker(5, 5, MID, &[], &[]);
    let a = &maker.actor;
    w.env.expect(
        &close_book_ix(a.key, maker.book),
        custom(NockError::BookNotEmpty),
    );
    w.env.ok(&w.withdraw_ix(a, maker.book, 5, 0));
    w.env.expect(
        &close_book_ix(a.key, maker.book),
        custom(NockError::BookNotEmpty),
    );
    w.env.ok(&w.withdraw_ix(a, maker.book, 0, 5));

    let ix = close_book_ix(a.key, maker.book);
    w.env.assert_signers_and_writables_enforced(&ix);
    let rent = w.env.get(&maker.book).lamports;
    let before = w.env.get(&a.key).lamports;
    let r = w.env.ok(&ix);
    assert_eq!(w.env.get(&a.key).lamports, before + rent);
    let closed = w.env.get(&maker.book);
    assert_eq!((closed.lamports, closed.data.len()), (0, 0));
    let e: BookClosedEvent = event(&r, tag::BOOK_CLOSED);
    assert_eq!((e.book, e.refunded_lamports), (maker.book.to_bytes(), rent));

    // the maker can open the book again, with a fresh sequence
    w.env.ok(&init_book_ix(a.key, w.market, maker.book));
    assert_eq!(w.env.book(&maker.book).sequence, 0);
}

#[test]
fn close_book_waits_for_fees_to_be_collected() {
    let mut w = World::new();
    let maker = w.maker(100, 100_000, MID, &[(50, 1)], &[(50, 1)]);
    let user = w.actor(RICH);
    w.env.ok(&w.swap_ix(&user, maker.book, buy(20, u64::MAX)));
    let b = w.env.book(&maker.book);
    w.env
        .ok(&w.withdraw_ix(&maker.actor, maker.book, b.base_free, b.quote_free));
    w.env.expect(
        &close_book_ix(maker.actor.key, maker.book),
        custom(NockError::BookNotEmpty),
    );
    let dest = w.admin_quote_account();
    w.env
        .ok(&collect_fees_ix(w.market, maker.book, w.quote_vault, dest));
    w.env.ok(&close_book_ix(maker.actor.key, maker.book));
    assert_eq!(
        w.env.token_amount(&w.quote_vault),
        0,
        "every quote atom left the vault"
    );
    assert_eq!(w.env.token_amount(&w.base_vault), 0);
}
