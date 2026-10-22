use nock_interface::{constants::*, events::tag, ID};
use pinocchio::Address;

fn pairwise_distinct<T: PartialEq + core::fmt::Debug>(items: &[T]) {
    for i in 0..items.len() {
        for j in (i + 1)..items.len() {
            assert_ne!(items[i], items[j], "items {i} and {j} collide");
        }
    }
}

#[test]
fn account_tags_pairwise_distinct() {
    pairwise_distinct(&[MARKET_TAG, MAKER_BOOK_TAG, ASYNC_SWAP_TAG]);
}

#[test]
fn event_tags_pairwise_distinct_and_disjoint_from_account_tags() {
    pairwise_distinct(&tag::ALL);
    for t in tag::ALL {
        assert!(![MARKET_TAG, MAKER_BOOK_TAG, ASYNC_SWAP_TAG].contains(&t));
    }
}

#[test]
fn seeds_pairwise_distinct() {
    pairwise_distinct(&[
        SEED_MARKET,
        SEED_MAKER_BOOK,
        SEED_ASYNC_SWAP,
        SEED_EVENT_AUTHORITY,
    ]);
}

#[test]
fn account_sizes() {
    assert_eq!(MARKET_LEN, 280);
    assert_eq!(MAKER_BOOK_LEN, 704);
    assert_eq!(ASYNC_SWAP_LEN, 184);
    assert_eq!(MAX_LEVELS, 16);
}

#[test]
fn program_id_is_the_owner_key() {
    assert_eq!(
        ID,
        Address::from_str_const("5rV6P9piXvRdVfhEVu9oJRHai7RS5iAJ1wuTNmeMCYhB")
    );
}

#[test]
fn event_authority_bump_is_canonical_for_the_program_id() {
    let (address, bump) = Address::find_program_address(&[SEED_EVENT_AUTHORITY], &ID);
    assert_eq!(
        bump, EVENT_AUTHORITY_BUMP,
        "set EVENT_AUTHORITY_BUMP to {bump} (or run `make set-program-id`)"
    );
    assert_eq!(address, EVENT_AUTHORITY);
}

#[test]
fn fee_cap_is_below_the_denominator() {
    assert!((MAX_FEE_PPM as u64) < PPM_DENOM);
}

#[test]
fn max_mint_decimals_is_the_largest_that_fits() {
    assert!(DECIMAL_RADIX
        .checked_pow(MAX_MINT_DECIMALS as u32)
        .is_some());
    assert!(DECIMAL_RADIX
        .checked_pow(MAX_MINT_DECIMALS as u32 + 1)
        .is_none());
}

#[test]
fn token_program_ids_differ() {
    assert_ne!(SPL_TOKEN, TOKEN_2022);
}
