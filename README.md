# nock

per-maker on-chain order book exchange on solana.

## Layout

```
interface/   state structs, errors, ix payload schemas, constants  (no_std rlib)
program/     entrypoint + per-ix processors                         (cdylib for SBF)
tests/       pure-Rust unit tests: layouts, offsets, tags
```

## Instructions

| Disc | Name                | Origin   | Notes                                      |
|------|---------------------|----------|--------------------------------------------|
| 0x00 | CreatePool          | own      | Inits market PDA + 2 vault ATAs            |
| 0x01 | ToggleMarketState   | own      | Admin: pause / unpause                     |
| 0x02 | UpdateConfig        | own      | Admin: set maker_fee_ppm / taker_fee_ppm   |
| 0x06 | InitMakerBook       | archer   | Permissionless: anyone allocates a book    |
| 0x07 | UpdateBook          | archer   | Replace mid + 16 bid + 16 ask levels       |
| 0x08 | UpdateMidPrice      | archer   | Mid-only refresh (fast path)               |
| 0x09 | ClearBook           | archer   | Wipe all 32 levels                         |
| 0x0a | SetBookDelegate     | archer   | Maker authorizes a delegate signer         |
| 0x0b | MakerDeposit        | archer   | Deposit base + quote in one ix             |
| 0x0c | MakerWithdraw       | archer   | Withdraw base + quote in one ix            |
| 0x0d | AsyncSwapCreate     | own      | User locks input tokens in escrow PDA      |
| 0x0e | AsyncSwapExecute    | own      | Maker fills the pending swap               |
| 0x0f | AsyncSwapCancel     | own      | Anyone refunds an expired async swap       |
| 0x10 | SyncSwap            | own      | User swaps directly against a maker book   |
| 0x11 | MakerHeartbeat      | own      | Bump sequence + last_updated_slot          |
| 0x12 | MakerSetSequence    | own      | Bump sequence only                         |
| 0x1a | UpdateSyncSpread    | archer   | Adjust per-book spread (in ticks)          |
| 0x1e | UpdateExpiryInSlots | archer   | Adjust quote-expiry window                 |
| 0x1f | CloseMakerBook      | archer   | Maker reclaims rent and closes book        |

Async swap honors `Market.min_async_delay_slots` / `Market.max_async_delay_slots` for expiry validation — same fields archer uses.

## Live CU benchmarks (surfpool localnet)

Measured by `scripts/bench_full.py`. CU consumed by the program, parsed from `consumed N of M` log lines.

| Disc | Name                  | CU      | Wire (B) |
|------|-----------------------|---------|----------|
| 0x00 | CreatePool            | 48,773  | 722      |
| 0x06 | InitMakerBook         | 1,687   | 311      |
| 0x07 | UpdateBook 16+16      | 857     | 822      |
| 0x08 | UpdateMidPrice        | **116** | 260      |
| 0x09 | ClearBook             | **136** | 252      |
| 0x0a | SetBookDelegate       | **120** | 277      |
| 0x0b | MakerDeposit          | 12,050  | 525      |
| 0x0c | MakerWithdraw         | 12,097  | 525      |
| 0x10 | SyncSwap              | 13,253  | 526      |
| 0x11 | MakerHeartbeat        | **115** | 260      |
| 0x12 | MakerSetSequence      | **114** | 252      |
| 0x1a | UpdateSyncSpread      | 164     | 279      |
| 0x1e | UpdateExpiryInSlots   | **93**  | 252      |

All six bolded paths come in under archer's stated <150 CU fast-path target.

## Program size

| What                  | Bytes      |
|-----------------------|-----------:|
| `nock.so`             |  58,488    |
| Archer mainnet `.so`  | 346,536    |
| Ratio                 |  5.9× smaller |

## Account types

| Tag       | Size | What                                  |
|-----------|------|---------------------------------------|
| ACHRMKT1  | 272  | Market (per market: mints, vaults, fees, async-delay window) |
| ACHRMKR1  | 776  | MakerBook (per maker × market: 16 bids, 16 asks, mid, inventory) |
| NOCKASW1  | 168  | AsyncSwap (pending swap request)      |

Field offsets inside `ACHRMKT1` and `ACHRMKR1` are verified against archer in `tests/src/state.rs`.

## Build & bench

```
cargo build-sbf --manifest-path program/Cargo.toml
solana program deploy --url http://127.0.0.1:8899 \
  --program-id target/deploy/nock-keypair.json \
  target/deploy/nock.so
python3 scripts/bench_full.py
```

## Program ID

```
DpMJkoYXVXLRxrPXdaDQgYS5zrGHQe6d4TP6hYM1fpKF
```
