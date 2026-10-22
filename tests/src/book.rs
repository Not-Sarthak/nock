use nock_interface::constants::{MAX_LEVELS, MAX_SEQUENCE_JUMP, NO_ADDRESS};
use nock_interface::error::NockError;
use nock_interface::state::{maker_book::book_status, MakerBook, MakerLevel, Role};

const MAKER: [u8; 32] = [1; 32];
const DELEGATE: [u8; 32] = [2; 32];
const STRANGER: [u8; 32] = [3; 32];

pub fn book() -> MakerBook {
    // SAFETY: MakerBook is plain integers and byte arrays; all-zero is a valid value.
    let mut b: MakerBook = unsafe { core::mem::zeroed() };
    b.maker = MAKER;
    b.expiry_in_slots = 2;
    b.mid_price_ticks = 1_000;
    b
}

fn level(size: u64, offset: i64) -> MakerLevel {
    MakerLevel {
        size_in_base_lots: size,
        price_offset_ticks: offset,
    }
}

#[test]
fn roles() {
    let mut b = book();
    assert_eq!(b.role_of(&MAKER), Ok(Role::Maker));
    assert_eq!(b.role_of(&DELEGATE), Err(NockError::UnauthorizedMaker));
    assert_eq!(b.role_of(&NO_ADDRESS), Err(NockError::UnauthorizedMaker));
    b.delegate = DELEGATE;
    assert_eq!(b.role_of(&DELEGATE), Ok(Role::Delegate));
    assert_eq!(b.role_of(&STRANGER), Err(NockError::UnauthorizedMaker));
    b.delegate = MAKER;
    assert_eq!(b.role_of(&MAKER), Ok(Role::Maker), "maker check wins");
}

#[test]
fn sequence_must_increase_within_the_jump_cap() {
    let mut b = book();
    assert_eq!(b.try_bump_sequence(0), Err(NockError::StaleSequence));
    assert_eq!(b.try_bump_sequence(5), Ok(()));
    assert_eq!(b.try_bump_sequence(5), Err(NockError::StaleSequence));
    assert_eq!(b.try_bump_sequence(4), Err(NockError::StaleSequence));
    assert_eq!(
        b.try_bump_sequence(5 + MAX_SEQUENCE_JUMP + 1),
        Err(NockError::SequenceJumpTooLarge)
    );
    assert_eq!(b.try_bump_sequence(5 + MAX_SEQUENCE_JUMP), Ok(()));
    assert_eq!(
        b.try_bump_sequence(u64::MAX),
        Err(NockError::SequenceJumpTooLarge),
        "nobody can pin the counter at u64::MAX in one step"
    );
}

#[test]
fn quotes_live_for_exactly_expiry_in_slots_slots() {
    let mut b = book();
    b.last_updated_slot = 100;
    b.expiry_in_slots = 2;
    assert!(b.quotes_live_at(100));
    assert!(b.quotes_live_at(101));
    assert!(!b.quotes_live_at(102));
    b.expiry_in_slots = 1;
    assert!(b.quotes_live_at(100));
    assert!(!b.quotes_live_at(101));
}

#[test]
fn fillable_checks_halt_expiry_and_pin() {
    let mut b = book();
    b.last_updated_slot = 10;
    b.sequence = 7;
    assert_eq!(b.check_fillable(10, None), Ok(()));
    assert_eq!(b.check_fillable(10, Some(7)), Ok(()));
    assert_eq!(
        b.check_fillable(10, Some(6)),
        Err(NockError::SequenceMismatch)
    );
    assert_eq!(b.check_fillable(12, None), Err(NockError::QuoteExpired));
    b.status = book_status::HALTED;
    assert_eq!(b.check_fillable(10, None), Err(NockError::BookHalted));
}

#[test]
fn quotes_must_be_best_first() {
    let mut b = book();
    b.bid_levels[0] = level(5, 1);
    b.bid_levels[1] = level(5, 3);
    b.ask_levels[0] = level(5, 1);
    b.ask_levels[1] = level(5, 2);
    assert_eq!(b.validate_quotes(Role::Maker), Ok(()));

    b.bid_levels[1] = level(5, 1);
    assert_eq!(
        b.validate_quotes(Role::Maker),
        Err(NockError::InvalidQuotes)
    );
    b.bid_levels[1] = level(5, 0);
    assert_eq!(
        b.validate_quotes(Role::Maker),
        Err(NockError::InvalidQuotes)
    );
}

#[test]
fn empty_levels_are_skipped_in_the_ordering() {
    let mut b = book();
    b.bid_levels[0] = level(0, 50);
    b.bid_levels[1] = level(5, 1);
    b.bid_levels[3] = level(5, 2);
    assert_eq!(b.validate_quotes(Role::Maker), Ok(()));
}

#[test]
fn a_crossed_book_is_rejected() {
    let mut b = book();
    b.bid_levels[0] = level(5, -3);
    b.ask_levels[0] = level(5, -3);
    assert_eq!(
        b.validate_quotes(Role::Maker),
        Err(NockError::InvalidQuotes)
    );
    b.sync_spread_ticks = 3;
    assert_eq!(
        b.validate_quotes(Role::Maker),
        Err(NockError::InvalidQuotes)
    );
    b.sync_spread_ticks = 4;
    assert_eq!(b.validate_quotes(Role::Maker), Ok(()));
}

#[test]
fn a_level_priced_at_or_below_zero_is_rejected() {
    let mut b = book();
    b.bid_levels[0] = level(5, 1_000);
    assert_eq!(
        b.validate_quotes(Role::Maker),
        Err(NockError::InvalidQuotes)
    );
}

#[test]
fn delegate_quotes_stay_inside_the_band_and_size_cap() {
    let mut b = book();
    b.delegate = DELEGATE;
    b.delegate_min_price_ticks = 990;
    b.delegate_max_price_ticks = 1_010;
    b.delegate_max_level_lots = 10;
    b.bid_levels[0] = level(10, 10);
    b.ask_levels[0] = level(10, 10);
    assert_eq!(b.validate_quotes(Role::Delegate), Ok(()));

    b.ask_levels[1] = level(1, 11);
    assert_eq!(
        b.validate_quotes(Role::Delegate),
        Err(NockError::OutsideDelegateBounds)
    );
    assert_eq!(
        b.validate_quotes(Role::Maker),
        Ok(()),
        "the maker is unbounded"
    );
    b.ask_levels[1] = MakerLevel::EMPTY;

    b.bid_levels[0] = level(11, 10);
    assert_eq!(
        b.validate_quotes(Role::Delegate),
        Err(NockError::OutsideDelegateBounds)
    );
    b.bid_levels[0] = level(10, 10);

    b.mid_price_ticks = 2_000;
    assert_eq!(
        b.validate_quotes(Role::Delegate),
        Err(NockError::OutsideDelegateBounds),
        "a delegate mid move reprices maker levels and must stay in band too"
    );
}

#[test]
fn a_fresh_delegate_can_post_nothing() {
    let mut b = book();
    b.delegate = DELEGATE;
    b.bid_levels[0] = level(1, 1);
    assert_eq!(
        b.validate_quotes(Role::Delegate),
        Err(NockError::OutsideDelegateBounds)
    );
}

#[test]
fn live_level_counts() {
    let mut b = book();
    b.bid_levels[0] = level(1, 1);
    b.bid_levels[MAX_LEVELS - 1] = level(1, 2);
    b.ask_levels[3] = level(1, 1);
    assert_eq!(b.live_levels(), (2, 1));
}
