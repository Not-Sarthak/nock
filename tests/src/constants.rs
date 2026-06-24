use nock_interface::constants::*;

#[test]
fn account_tags_pairwise_distinct() {
    let tags = [MARKET_TAG, MAKER_BOOK_TAG, ASYNC_SWAP_TAG];
    for i in 0..tags.len() {
        for j in (i + 1)..tags.len() {
            assert_ne!(tags[i], tags[j]);
        }
    }
}

#[test]
fn event_tags_pairwise_distinct() {
    let tags = [
        POOL_CREATED_EVENT_TAG, BOOK_UPDATE_EVENT_TAG, SYNC_FILL_EVENT_TAG,
        ASYNC_SWAP_CREATED_TAG, ASYNC_FILL_EVENT_TAG, ASYNC_CANCEL_EVENT_TAG,
    ];
    for i in 0..tags.len() {
        for j in (i + 1)..tags.len() {
            assert_ne!(tags[i], tags[j]);
        }
    }
}

#[test]
fn seeds_pairwise_distinct() {
    let seeds: [&[u8]; 5] = [
        SEED_MARKET, SEED_MAKER_BOOK,
        SEED_TREASURY, SEED_ASYNC_SWAP, SEED_ASYNC_ESCROW,
    ];
    for i in 0..seeds.len() {
        for j in (i + 1)..seeds.len() {
            assert_ne!(seeds[i], seeds[j]);
        }
    }
}

#[test]
fn account_sizes() {
    assert_eq!(MARKET_LEN, 272, "Market header fixed at 272 bytes");
    assert_eq!(MAKER_BOOK_LEN, 776, "MakerBook fixed at 776 bytes");
    assert_eq!(MAX_LEVELS, 16, "16 bid + 16 ask levels per book");
}
