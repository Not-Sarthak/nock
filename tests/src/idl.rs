//! `idl/nock.json` (Codama) must describe exactly what the program does: ids, sizes,
//! discriminators, instruction payloads, events and error codes.

use core::mem::size_of;
use nock_interface::{
    constants::*,
    error::NockError,
    events::*,
    instruction::{disc, *},
    ID,
};
use serde_json::Value;

fn idl() -> Value {
    let path = format!("{}/../idl/nock.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn size(t: &Value, idl: &Value) -> usize {
    match t["kind"].as_str().unwrap() {
        "fixedSizeTypeNode" => t["size"].as_u64().unwrap() as usize,
        "publicKeyTypeNode" => 32,
        "numberTypeNode" => match t["format"].as_str().unwrap() {
            "u8" => 1,
            "u16" => 2,
            "u32" => 4,
            "u64" | "i64" => 8,
            other => panic!("format {other}"),
        },
        "arrayTypeNode" => t["count"]["value"].as_u64().unwrap() as usize * size(&t["item"], idl),
        "definedTypeLinkNode" => {
            let name = t["name"].as_str().unwrap();
            let defined = idl["program"]["definedTypes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|d| d["name"] == name)
                .unwrap();
            struct_size(&defined["type"], idl)
        }
        other => panic!("type {other}"),
    }
}

fn struct_size(s: &Value, idl: &Value) -> usize {
    s["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| size(&f["type"], idl))
        .sum()
}

fn discriminator(fields: &Value) -> Vec<u8> {
    let hex = fields["fields"][0]["defaultValue"]["data"]
        .as_str()
        .unwrap();
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn program_id_and_accounts() {
    let idl = idl();
    let program = &idl["program"];
    assert_eq!(program["publicKey"], ID.to_string());
    let expected = [
        ("market", MARKET_TAG, MARKET_LEN),
        ("makerBook", MAKER_BOOK_TAG, MAKER_BOOK_LEN),
        ("asyncSwap", ASYNC_SWAP_TAG, ASYNC_SWAP_LEN),
    ];
    let accounts = program["accounts"].as_array().unwrap();
    assert_eq!(accounts.len(), expected.len());
    for (name, tag, len) in expected {
        let a = accounts.iter().find(|a| a["name"] == name).unwrap();
        assert_eq!(a["size"].as_u64().unwrap() as usize, len, "{name}");
        assert_eq!(struct_size(&a["data"], &idl), len, "{name} fields");
        assert_eq!(discriminator(&a["data"]), tag, "{name}");
    }
}

#[test]
fn instructions() {
    let idl = idl();
    // (name, discriminator, payload bytes, accounts including the two event accounts)
    let expected: &[(&str, u8, usize, usize)] = &[
        ("createPool", disc::CREATE_POOL, CREATE_POOL_DATA_LEN, 12),
        (
            "toggleMarketState",
            disc::TOGGLE_MARKET_STATE,
            TOGGLE_MARKET_STATE_DATA_LEN,
            4,
        ),
        (
            "updateConfig",
            disc::UPDATE_CONFIG,
            UPDATE_CONFIG_DATA_LEN,
            4,
        ),
        ("collectFees", disc::COLLECT_FEES, 0, 7),
        (
            "proposeAdmin",
            disc::PROPOSE_ADMIN,
            PROPOSE_ADMIN_DATA_LEN,
            4,
        ),
        ("acceptAdmin", disc::ACCEPT_ADMIN, 0, 4),
        ("initMakerBook", disc::INIT_MAKER_BOOK, 0, 6),
        (
            "updateMakerBook",
            disc::UPDATE_MAKER_BOOK,
            UPDATE_MAKER_BOOK_DATA_LEN,
            4,
        ),
        (
            "updateMidPrice",
            disc::UPDATE_MID_PRICE,
            UPDATE_MID_PRICE_DATA_LEN,
            4,
        ),
        ("clearBook", disc::CLEAR_BOOK, SEQUENCE_DATA_LEN, 4),
        (
            "setBookDelegate",
            disc::SET_BOOK_DELEGATE,
            SET_BOOK_DELEGATE_DATA_LEN,
            5,
        ),
        (
            "makerDeposit",
            disc::MAKER_DEPOSIT,
            MAKER_FUNDS_DATA_LEN,
            10,
        ),
        (
            "makerWithdraw",
            disc::MAKER_WITHDRAW,
            MAKER_FUNDS_DATA_LEN,
            10,
        ),
        (
            "asyncSwapCreate",
            disc::ASYNC_SWAP_CREATE,
            ASYNC_SWAP_CREATE_DATA_LEN,
            11,
        ),
        ("asyncSwapExecute", disc::ASYNC_SWAP_EXECUTE, 0, 13),
        ("asyncSwapCancel", disc::ASYNC_SWAP_CANCEL, 0, 8),
        ("syncSwap", disc::SYNC_SWAP, SYNC_SWAP_DATA_LEN, 10),
        (
            "makerHeartbeat",
            disc::MAKER_HEARTBEAT,
            SEQUENCE_DATA_LEN,
            4,
        ),
        (
            "makerSetSequence",
            disc::MAKER_SET_SEQUENCE,
            SEQUENCE_DATA_LEN,
            4,
        ),
        (
            "setBookHalt",
            disc::SET_BOOK_HALT,
            SET_BOOK_HALT_DATA_LEN,
            4,
        ),
        (
            "makerResetSequence",
            disc::MAKER_RESET_SEQUENCE,
            SEQUENCE_DATA_LEN,
            4,
        ),
        (
            "updateSyncSpread",
            disc::UPDATE_SYNC_SPREAD,
            UPDATE_SYNC_SPREAD_DATA_LEN,
            4,
        ),
        (
            "updateExpiryInSlots",
            disc::UPDATE_EXPIRY_IN_SLOTS,
            UPDATE_EXPIRY_IN_SLOTS_DATA_LEN,
            4,
        ),
        ("closeMakerBook", disc::CLOSE_MAKER_BOOK, 0, 4),
    ];
    let ixs = idl["program"]["instructions"].as_array().unwrap();
    assert_eq!(ixs.len(), expected.len());
    for (name, d, len, accounts) in expected {
        let ix = ixs
            .iter()
            .find(|i| i["name"] == *name)
            .unwrap_or_else(|| panic!("{name}"));
        let args = ix["arguments"].as_array().unwrap();
        assert_eq!(
            args[0]["defaultValue"]["number"].as_u64().unwrap() as u8,
            *d,
            "{name}"
        );
        let payload: usize = args[1..].iter().map(|a| size(&a["type"], &idl)).sum();
        assert_eq!(payload, *len, "{name} payload");
        let metas = ix["accounts"].as_array().unwrap();
        assert_eq!(metas.len(), *accounts, "{name} accounts");
        let tail: Vec<&str> = metas[metas.len() - 2..]
            .iter()
            .map(|m| m["name"].as_str().unwrap())
            .collect();
        assert_eq!(tail, vec!["eventAuthority", "program"], "{name}");
        assert_eq!(
            metas[metas.len() - 2]["defaultValue"]["publicKey"],
            EVENT_AUTHORITY.to_string()
        );
    }
}

#[test]
fn events() {
    let idl = idl();
    let sizes = [
        (tag::POOL_CREATED, size_of::<PoolCreatedEvent>()),
        (tag::CONFIG_UPDATED, size_of::<ConfigUpdatedEvent>()),
        (tag::MARKET_STATUS, size_of::<MarketStatusEvent>()),
        (tag::ADMIN_PROPOSED, size_of::<AdminEvent>()),
        (tag::ADMIN_ACCEPTED, size_of::<AdminEvent>()),
        (tag::FEES_COLLECTED, size_of::<FeesCollectedEvent>()),
        (tag::DELEGATE_SET, size_of::<DelegateSetEvent>()),
        (tag::DEPOSIT, size_of::<InventoryEvent>()),
        (tag::WITHDRAW, size_of::<InventoryEvent>()),
        (tag::BOOK_CLOSED, size_of::<BookClosedEvent>()),
        (tag::SYNC_FILL, size_of::<SyncFillEvent>()),
        (tag::ASYNC_SWAP_CREATED, size_of::<AsyncSwapCreatedEvent>()),
        (tag::ASYNC_FILL, size_of::<AsyncFillEvent>()),
        (tag::ASYNC_CANCEL, size_of::<AsyncCancelEvent>()),
    ];
    let events = idl["program"]["events"].as_array().unwrap();
    assert_eq!(events.len(), tag::ALL.len());
    for e in events {
        let d = discriminator(&e["data"]);
        assert!(tag::ALL.iter().any(|t| t[..] == d[..]), "unknown tag {d:?}");
        let expected = sizes
            .iter()
            .find(|(t, _)| t[..] == d[..])
            .map(|(_, s)| *s)
            .unwrap_or(size_of::<BookEvent>());
        assert_eq!(struct_size(&e["data"], &idl), expected, "{}", e["name"]);
    }
}

#[test]
fn errors() {
    use NockError::*;
    let all = [
        InvalidTokenProgram,
        Token2022Unsupported,
        InvalidMint,
        IncorrectVault,
        IncorrectTokenAccount,
        InvalidPda,
        MarketMismatch,
        UnauthorizedAdmin,
        UnauthorizedMaker,
        MarketPaused,
        InvalidMarketConfig,
        InvalidFee,
        BookHalted,
        QuoteExpired,
        StaleSequence,
        SequenceJumpTooLarge,
        SequenceMismatch,
        InvalidQuotes,
        OutsideDelegateBounds,
        InvalidExpiry,
        AmountZero,
        InsufficientLiquidity,
        SlippageExceeded,
        InsufficientInventory,
        MathOverflow,
        QuoteSizeTooSmall,
        InvalidSide,
        AsyncWindowViolated,
        AsyncSwapExpired,
        AsyncSwapNotExpired,
        MakerNotSelected,
        BookNotEmpty,
        InvalidEventAuthority,
        NoPendingAdmin,
    ];
    let idl = idl();
    let errors = idl["program"]["errors"].as_array().unwrap();
    assert_eq!(errors.len(), all.len());
    for (i, e) in all.iter().enumerate() {
        assert_eq!(*e as u32, i as u32, "codes are dense");
        let debug = format!("{e:?}");
        let camel = debug[..1].to_lowercase() + &debug[1..];
        assert_eq!(errors[i]["name"], camel);
        assert_eq!(errors[i]["code"].as_u64().unwrap(), i as u64);
    }
}
