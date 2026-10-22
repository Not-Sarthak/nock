# nock

per-maker on-chain order book exchange on solana.

## architecture

```
 admin ── create_pool ──▶ Market ◀── toggle / update_config / propose + accept admin
                            │  base vault (ATA)   quote vault (ATA)
                            │        ▲  │               ▲  │
 maker ── init_maker_book ──┼──▶ MakerBook (per market × maker)
   │                        │    mid, spread, 16 bids, 16 asks,
   ├── deposit / withdraw ──┼──▶ base_free, quote_free, fees
   ├── update_book / mid / clear / heartbeat / halt / spread / expiry   (sequenced)
   └── set_book_delegate ──▶ delegate: quotes only, inside a price band and size cap
                            │
 taker ── sync_swap ────────┴──▶ walks one book best first, all or nothing
   │                             vaults ⇄ taker, fees accrue on the book
   │
   └── async_swap_create ──▶ AsyncSwap (nonce) + escrow (ATA of the record)
                                  │
            maker / delegate ── async_swap_execute ──▶ priced from that maker's book
            user any time, anyone after expiry ── async_swap_cancel ──▶ full refund
                                  │
                                  ▼
                     escrow emptied and closed, rent to the user

 anyone ── collect_fees ──▶ book fees ──▶ admin's quote account
 every state change ── self-CPI (0xff, event authority) ──▶ event
```

## layout

```
interface/   state layouts, instruction payloads, events, errors, pricing and fill maths
program/     entrypoint, per-instruction processors, helpers (PDA, rent, tokens, events)
tests/       unit tests, plus mollusk tests on the built .so with SPL Token and ATA
idl/         codama IDL (nock.json)
client/      generated rust client (nock-client), its own workspace
```

## instructions

| disc | name | actor | notes |
|------|------|-------|-------|
| 0x00 | CreatePool | payer + authority | market PDA and both vault ATAs; the authority is a seed and signs |
| 0x01 | ToggleMarketState | admin | pause or resume; withdraw and cancel still work while paused |
| 0x02 | UpdateConfig | admin | maker and taker fee, each at most `MAX_FEE_PPM` |
| 0x03 | CollectFees | anyone | sweeps one book's fees to a quote account owned by the admin |
| 0x04 | ProposeAdmin | admin | starts a two-step rotation; zero cancels |
| 0x05 | AcceptAdmin | pending admin | completes the rotation |
| 0x06 | InitMakerBook | maker | one book per market and maker |
| 0x07 | UpdateMakerBook | maker/delegate | mid and both ladders, best first, not crossed |
| 0x08 | UpdateMidPrice | maker/delegate | mid only; refreshes the quote slot |
| 0x09 | ClearBook | maker/delegate | empties both ladders |
| 0x0a | SetBookDelegate | maker | delegate, price band, max level size |
| 0x0b | MakerDeposit | maker | base and/or quote lots into the vaults |
| 0x0c | MakerWithdraw | maker | free lots out of the vaults |
| 0x0d | AsyncSwapCreate | user | escrows the input under a nonce; optional named maker |
| 0x0e | AsyncSwapExecute | maker/delegate | fills from the maker's book; same rules and fees as a sync swap |
| 0x0f | AsyncSwapCancel | user / anyone after expiry | refunds the escrow, closes it and the record |
| 0x10 | SyncSwap | user | walks one book best first, all or nothing; optional sequence pin |
| 0x11 | MakerHeartbeat | maker/delegate | keeps quotes live |
| 0x12 | MakerSetSequence | maker/delegate | bumps the sequence |
| 0x13 | SetBookHalt | maker/delegate | halt; only the maker resumes |
| 0x14 | MakerResetSequence | maker | sets the sequence to any value |
| 0x1a | UpdateSyncSpread | maker/delegate | spread in ticks on both sides |
| 0x1e | UpdateExpiryInSlots | maker/delegate | quote lifetime, 1 to 10 slots |
| 0x1f | CloseMakerBook | maker | empty book only; rent back to the maker |
| 0xff | EmitEvent | program | self-CPI carrying an event; event-authority PDA signs |

## account types

| tag | size | what | pda seeds |
|-----|-----:|------|-----------|
| `NOCKMKT2` | 280 | Market: mints, vaults, admin, pending admin, lot sizes, fees, async window | `"market", base_mint, quote_mint, authority` |
| `NOCKMKR2` | 704 | MakerBook: mid, spread, 16 bids, 16 asks, inventory, fees, sequence, delegate bounds | `"maker", market, maker` |
| `NOCKASW2` | 184 | AsyncSwap: user, maker, escrow, size, limits, expiry | `"async", market, user, nonce` |

## metrics

| program | bytes |
|---------|------:|
| `nock.so` | 127,376 |

| instruction | compute units |
|-------------|--------------:|
| create_pool | 41,549 |
| init_maker_book | 6,576 |
| maker_deposit | 3,960 |
| update_maker_book (16 + 16 levels) | 3,954 |
| update_mid_price | 3,466 |
| maker_heartbeat | 1,699 |
| sync_swap (1 level) | 5,939 |
| sync_swap (16 levels) | 10,908 |
| async_swap_create | 22,978 |
| async_swap_execute | 8,105 |
| async_swap_cancel | 4,423 |
| collect_fees | 3,044 |

## build & deploy

```
make build
make test
make lint
make client-test
make verify-build
make verify-hash
make verify-deployed RPC_URL=<rpc>
make check-program-id
make deploy URL=<rpc>
make set-program-id PROGRAM_KEYPAIR=<new-program-keypair.json>
```

## program id

```
5rV6P9piXvRdVfhEVu9oJRHai7RS5iAJ1wuTNmeMCYhB
```
