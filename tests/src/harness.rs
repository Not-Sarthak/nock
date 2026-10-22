//! Mollusk harness over the built program (`target/deploy/nock.so`) plus the real SPL
//! Token and Associated Token programs.

use mollusk_svm::{
    program::{create_program_account_loader_v3, keyed_account_for_system_program, loader_keys},
    result::{InstructionResult, ProgramResult},
    Mollusk,
};
use mollusk_svm_programs_token::{associated_token, token};
use nock_interface::{
    constants::*,
    error::NockError,
    events::Event,
    instruction::{disc, Payload, *},
    state::{AsyncSwap, MakerBook, MakerLevel, Market},
    ID,
};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;
use std::collections::HashMap;

pub const SOL: u64 = 1_000_000_000;
pub const MINT_LEN: usize = 82;
pub const TOKEN_ACCOUNT_LEN: usize = 165;
pub const BASE_DECIMALS: u8 = 6;
pub const QUOTE_DECIMALS: u8 = 6;
pub const RICH: u64 = 1_000_000_000_000;

/// Market parameters every fixture uses: 1 base lot = 1_000 atoms, 1 quote lot = 10 atoms,
/// 1 tick = 1 quote atom per whole base unit.
pub const BASE_ATOMS_PER_LOT: u64 = 1_000;
pub const QUOTE_ATOMS_PER_LOT: u64 = 10;
pub const TICK: u64 = 1;
pub const MAKER_FEE_PPM: u32 = 200;
pub const TAKER_FEE_PPM: u32 = 500;
pub const MIN_ASYNC: u16 = 2;
pub const MAX_ASYNC: u16 = 100;
pub const START_SLOT: u64 = 1_000;

pub fn so_path() -> String {
    format!("{}/../target/deploy/nock.so", env!("CARGO_MANIFEST_DIR"))
}

pub fn custom(e: NockError) -> ProgramResult {
    ProgramResult::Failure(ProgramError::Custom(e as u32))
}

pub fn failure(e: ProgramError) -> ProgramResult {
    ProgramResult::Failure(e)
}

pub fn bytes_of<T: Payload>(payload: &T) -> &[u8] {
    // SAFETY: payloads are repr(C, packed) and exactly T::LEN bytes.
    unsafe { core::slice::from_raw_parts(payload as *const T as *const u8, T::LEN) }
}

pub fn ix_data<T: Payload>(tag: u8, payload: &T) -> Vec<u8> {
    let mut data = vec![tag];
    data.extend_from_slice(bytes_of(payload));
    data
}

pub fn wallet(lamports: u64) -> Account {
    Account::new(lamports, 0, &solana_sdk_ids::system_program::ID)
}

pub fn mint_bytes(decimals: u8, authority: Option<Pubkey>) -> Vec<u8> {
    let mut d = vec![0u8; MINT_LEN];
    if let Some(a) = authority {
        d[0..4].copy_from_slice(&1u32.to_le_bytes());
        d[4..36].copy_from_slice(a.as_ref());
    }
    d[44] = decimals;
    d[45] = 1;
    d
}

pub fn mint_account(decimals: u8) -> Account {
    Account {
        lamports: SOL,
        data: mint_bytes(decimals, None),
        owner: SPL_TOKEN,
        executable: false,
        rent_epoch: 0,
    }
}

pub fn token_bytes(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
    let mut d = vec![0u8; TOKEN_ACCOUNT_LEN];
    d[0..32].copy_from_slice(mint.as_ref());
    d[32..64].copy_from_slice(owner.as_ref());
    d[64..72].copy_from_slice(&amount.to_le_bytes());
    d[108] = 1;
    d
}

pub fn token_account(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Account {
    Account {
        lamports: SOL / 100,
        data: token_bytes(mint, owner, amount),
        owner: SPL_TOKEN,
        executable: false,
        rent_epoch: 0,
    }
}

pub fn ata(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[wallet.as_ref(), SPL_TOKEN.as_ref(), mint.as_ref()], &ATA).0
}

pub fn market_pda(base: &Pubkey, quote: &Pubkey, authority: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            SEED_MARKET,
            base.as_ref(),
            quote.as_ref(),
            authority.as_ref(),
        ],
        &ID,
    )
}

pub fn book_pda(market: &Pubkey, maker: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_MAKER_BOOK, market.as_ref(), maker.as_ref()], &ID)
}

pub fn async_pda(market: &Pubkey, user: &Pubkey, nonce: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            SEED_ASYNC_SWAP,
            market.as_ref(),
            user.as_ref(),
            &nonce.to_le_bytes(),
        ],
        &ID,
    )
}

/// A non-canonical bump for `seeds` whose address is still off-curve, with that address.
pub fn non_canonical(seeds: &[&[u8]]) -> (Pubkey, u8) {
    let (_, canonical) = Pubkey::find_program_address(seeds, &ID);
    for bump in (0..canonical).rev() {
        let mut with_bump: Vec<&[u8]> = seeds.to_vec();
        let b = [bump];
        with_bump.push(&b);
        if let Ok(address) = Pubkey::create_program_address(&with_bump, &ID) {
            return (address, bump);
        }
    }
    panic!("no second bump found");
}

fn tail(mut metas: Vec<AccountMeta>) -> Vec<AccountMeta> {
    metas.push(AccountMeta::new_readonly(EVENT_AUTHORITY, false));
    metas.push(AccountMeta::new_readonly(ID, false));
    metas
}

pub struct Env {
    pub mollusk: Mollusk,
    pub accounts: HashMap<Pubkey, Account>,
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}

impl Env {
    pub fn new() -> Self {
        let elf = std::fs::read(so_path()).unwrap_or_else(|e| {
            panic!("{}: {e}; run `cargo build-sbf` first", so_path());
        });
        let mut mollusk = Mollusk::default();
        mollusk.add_program_with_loader_and_elf(&ID, &loader_keys::LOADER_V3, &elf);
        token::add_program(&mut mollusk);
        associated_token::add_program(&mut mollusk);
        mollusk.warp_to_slot(START_SLOT);
        let mut accounts = HashMap::new();
        accounts.insert(ID, create_program_account_loader_v3(&ID));
        let (sys, sys_account) = keyed_account_for_system_program();
        accounts.insert(sys, sys_account);
        let (t, t_account) = token::keyed_account();
        accounts.insert(t, t_account);
        let (a, a_account) = associated_token::keyed_account();
        accounts.insert(a, a_account);
        accounts.insert(EVENT_AUTHORITY, wallet(0));
        Self { mollusk, accounts }
    }

    pub fn slot(&self) -> u64 {
        self.mollusk.sysvars.clock.slot
    }

    pub fn warp(&mut self, slot: u64) {
        self.mollusk.warp_to_slot(slot);
    }

    pub fn set(&mut self, key: Pubkey, account: Account) {
        self.accounts.insert(key, account);
    }

    pub fn get(&self, key: &Pubkey) -> Account {
        self.accounts.get(key).cloned().unwrap_or_else(|| wallet(0))
    }

    fn inputs(&self, ix: &Instruction) -> Vec<(Pubkey, Account)> {
        let mut seen = Vec::new();
        for meta in &ix.accounts {
            if !seen
                .iter()
                .any(|(k, _): &(Pubkey, Account)| *k == meta.pubkey)
            {
                seen.push((meta.pubkey, self.get(&meta.pubkey)));
            }
        }
        seen
    }

    /// Runs `ix` without committing anything.
    pub fn try_ix(&self, ix: &Instruction) -> InstructionResult {
        self.mollusk.process_instruction(ix, &self.inputs(ix))
    }

    /// Runs `ix`, commits the resulting accounts on success.
    pub fn run(&mut self, ix: &Instruction) -> InstructionResult {
        let result = self.try_ix(ix);
        if result.program_result.is_ok() {
            for (key, account) in &result.resulting_accounts {
                self.accounts.insert(*key, account.clone());
            }
        }
        result
    }

    pub fn ok(&mut self, ix: &Instruction) -> InstructionResult {
        let result = self.run(ix);
        assert_eq!(result.program_result, ProgramResult::Success, "logs above");
        result
    }

    pub fn expect(&self, ix: &Instruction, expected: ProgramResult) {
        let result = self.try_ix(ix);
        assert_eq!(result.program_result, expected);
    }

    pub fn token_amount(&self, key: &Pubkey) -> u64 {
        let data = self.get(key).data;
        u64::from_le_bytes(data[64..72].try_into().unwrap())
    }

    pub fn market(&self, key: &Pubkey) -> Market {
        read_state::<Market>(&self.get(key).data, MARKET_LEN)
    }

    pub fn book(&self, key: &Pubkey) -> MakerBook {
        read_state::<MakerBook>(&self.get(key).data, MAKER_BOOK_LEN)
    }

    pub fn async_swap(&self, key: &Pubkey) -> AsyncSwap {
        read_state::<AsyncSwap>(&self.get(key).data, ASYNC_SWAP_LEN)
    }

    /// Every account mutation the failure checks need: for each signer meta, the same
    /// instruction with that meta unsigned; for each writable meta, the same with it
    /// read-only. Each must fail; signers with exactly `MissingRequiredSignature`.
    pub fn assert_signers_and_writables_enforced(&self, ix: &Instruction) {
        assert_eq!(
            self.try_ix(ix).program_result,
            ProgramResult::Success,
            "baseline must pass"
        );
        for i in 0..ix.accounts.len() {
            if ix.accounts[i].is_signer {
                let mut bad = ix.clone();
                bad.accounts[i].is_signer = false;
                let result = self.try_ix(&bad);
                assert_eq!(
                    result.program_result,
                    failure(ProgramError::MissingRequiredSignature),
                    "account {i} unsigned"
                );
            }
            if ix.accounts[i].is_writable && ix.accounts[i].pubkey != ID {
                let mut bad = ix.clone();
                for m in bad
                    .accounts
                    .iter_mut()
                    .filter(|m| m.pubkey == ix.accounts[i].pubkey)
                {
                    m.is_writable = false;
                }
                let result = self.try_ix(&bad);
                assert!(
                    !result.program_result.is_ok(),
                    "account {i} read-only must fail"
                );
            }
        }
    }
}

pub fn read_state<T>(data: &[u8], len: usize) -> T {
    assert_eq!(data.len(), len);
    // SAFETY: state structs are plain integers and byte arrays.
    unsafe { core::ptr::read_unaligned(data.as_ptr() as *const T) }
}

/// Every event payload (bytes after the 0xff discriminator) carried by self-CPIs into
/// this program in `result`, in order.
pub fn events(result: &InstructionResult) -> Vec<Vec<u8>> {
    let message = result.message.as_ref().expect("inner-instructions feature");
    let keys = message.account_keys();
    result
        .inner_instructions
        .iter()
        .filter(|inner| keys.get(inner.instruction.program_id_index as usize) == Some(&ID))
        .map(|inner| {
            assert_eq!(
                inner.instruction.data[0],
                disc::EMIT_EVENT,
                "only events self-CPI"
            );
            inner.instruction.data[1..].to_vec()
        })
        .collect()
}

/// The single event tagged `tag` in `result`, decoded.
pub fn event<T: Event>(result: &InstructionResult, tag: [u8; 8]) -> T {
    let matching: Vec<Vec<u8>> = events(result)
        .into_iter()
        .filter(|e| e[..8] == tag)
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "expected one {:?} event",
        core::str::from_utf8(&tag)
    );
    let bytes = &matching[0];
    assert_eq!(bytes.len(), core::mem::size_of::<T>());
    // SAFETY: events are repr(C) without padding.
    unsafe { core::ptr::read_unaligned(bytes.as_ptr() as *const T) }
}

// ---------- instruction builders ----------

pub struct PoolArgs {
    pub tick: u64,
    pub base_lot: u64,
    pub quote_lot: u64,
    pub maker_fee: u32,
    pub taker_fee: u32,
    pub min_async: u16,
    pub max_async: u16,
}

impl Default for PoolArgs {
    fn default() -> Self {
        Self {
            tick: TICK,
            base_lot: BASE_ATOMS_PER_LOT,
            quote_lot: QUOTE_ATOMS_PER_LOT,
            maker_fee: MAKER_FEE_PPM,
            taker_fee: TAKER_FEE_PPM,
            min_async: MIN_ASYNC,
            max_async: MAX_ASYNC,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create_pool_ix(
    payer: Pubkey,
    authority: Pubkey,
    market: Pubkey,
    base_mint: Pubkey,
    quote_mint: Pubkey,
    base_vault: Pubkey,
    quote_vault: Pubkey,
    args: &PoolArgs,
) -> Instruction {
    let payload = CreatePoolData {
        tick_size_in_quote_atoms_per_base_unit: args.tick,
        base_atoms_per_base_lot: args.base_lot,
        quote_atoms_per_quote_lot: args.quote_lot,
        maker_fee_ppm: args.maker_fee,
        taker_fee_ppm: args.taker_fee,
        min_async_delay_slots: args.min_async,
        max_async_delay_slots: args.max_async,
    };
    Instruction {
        program_id: ID,
        accounts: tail(vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(market, false),
            AccountMeta::new_readonly(base_mint, false),
            AccountMeta::new_readonly(quote_mint, false),
            AccountMeta::new(base_vault, false),
            AccountMeta::new(quote_vault, false),
            AccountMeta::new_readonly(SYSTEM, false),
            AccountMeta::new_readonly(SPL_TOKEN, false),
            AccountMeta::new_readonly(ATA, false),
        ]),
        data: ix_data(disc::CREATE_POOL, &payload),
    }
}

pub fn admin_ix(tag: u8, market: Pubkey, admin: Pubkey, data: &[u8]) -> Instruction {
    let mut bytes = vec![tag];
    bytes.extend_from_slice(data);
    Instruction {
        program_id: ID,
        accounts: tail(vec![
            AccountMeta::new(market, false),
            AccountMeta::new_readonly(admin, true),
        ]),
        data: bytes,
    }
}

pub fn toggle_ix(market: Pubkey, admin: Pubkey, paused: u8) -> Instruction {
    admin_ix(
        disc::TOGGLE_MARKET_STATE,
        market,
        admin,
        bytes_of(&ToggleMarketStateData { paused }),
    )
}

pub fn update_config_ix(market: Pubkey, admin: Pubkey, maker: u32, taker: u32) -> Instruction {
    admin_ix(
        disc::UPDATE_CONFIG,
        market,
        admin,
        bytes_of(&UpdateConfigData {
            maker_fee_ppm: maker,
            taker_fee_ppm: taker,
        }),
    )
}

pub fn propose_admin_ix(market: Pubkey, admin: Pubkey, new_admin: Pubkey) -> Instruction {
    admin_ix(
        disc::PROPOSE_ADMIN,
        market,
        admin,
        bytes_of(&ProposeAdminData {
            new_admin: new_admin.to_bytes(),
        }),
    )
}

pub fn accept_admin_ix(market: Pubkey, new_admin: Pubkey) -> Instruction {
    admin_ix(disc::ACCEPT_ADMIN, market, new_admin, &[])
}

pub fn collect_fees_ix(
    market: Pubkey,
    book: Pubkey,
    quote_vault: Pubkey,
    destination: Pubkey,
) -> Instruction {
    Instruction {
        program_id: ID,
        accounts: tail(vec![
            AccountMeta::new_readonly(market, false),
            AccountMeta::new(book, false),
            AccountMeta::new(quote_vault, false),
            AccountMeta::new(destination, false),
            AccountMeta::new_readonly(SPL_TOKEN, false),
        ]),
        data: vec![disc::COLLECT_FEES],
    }
}

pub fn init_book_ix(maker: Pubkey, market: Pubkey, book: Pubkey) -> Instruction {
    Instruction {
        program_id: ID,
        accounts: tail(vec![
            AccountMeta::new(maker, true),
            AccountMeta::new_readonly(market, false),
            AccountMeta::new(book, false),
            AccountMeta::new_readonly(SYSTEM, false),
        ]),
        data: vec![disc::INIT_MAKER_BOOK],
    }
}

/// `[signer, book]` instructions.
pub fn book_ix(tag: u8, signer: Pubkey, book: Pubkey, data: &[u8]) -> Instruction {
    let mut bytes = vec![tag];
    bytes.extend_from_slice(data);
    Instruction {
        program_id: ID,
        accounts: tail(vec![
            AccountMeta::new_readonly(signer, true),
            AccountMeta::new(book, false),
        ]),
        data: bytes,
    }
}

pub fn update_book_ix(
    signer: Pubkey,
    book: Pubkey,
    sequence: u64,
    mid: u64,
    bids: &[(u64, i64)],
    asks: &[(u64, i64)],
) -> Instruction {
    let mut payload = UpdateMakerBookData {
        header: UpdateMakerBookHeader {
            sequence_number: sequence,
            mid_price_ticks: mid,
            num_bids: bids.len() as u8,
            num_asks: asks.len() as u8,
            _pad: [0; 6],
        },
        bid_levels: [MakerLevel::EMPTY; MAX_LEVELS],
        ask_levels: [MakerLevel::EMPTY; MAX_LEVELS],
    };
    let mut b = [MakerLevel::EMPTY; MAX_LEVELS];
    let mut a = [MakerLevel::EMPTY; MAX_LEVELS];
    for (i, (size, offset)) in bids.iter().enumerate() {
        b[i] = MakerLevel {
            size_in_base_lots: *size,
            price_offset_ticks: *offset,
        };
    }
    for (i, (size, offset)) in asks.iter().enumerate() {
        a[i] = MakerLevel {
            size_in_base_lots: *size,
            price_offset_ticks: *offset,
        };
    }
    payload.bid_levels = b;
    payload.ask_levels = a;
    book_ix(disc::UPDATE_MAKER_BOOK, signer, book, bytes_of(&payload))
}

pub fn seq_ix(tag: u8, signer: Pubkey, book: Pubkey, sequence: u64) -> Instruction {
    book_ix(
        tag,
        signer,
        book,
        bytes_of(&SequenceData {
            sequence_number: sequence,
        }),
    )
}

pub fn mid_ix(signer: Pubkey, book: Pubkey, sequence: u64, mid: u64) -> Instruction {
    book_ix(
        disc::UPDATE_MID_PRICE,
        signer,
        book,
        bytes_of(&UpdateMidPriceData {
            sequence_number: sequence,
            new_mid_ticks: mid,
        }),
    )
}

pub fn halt_ix(signer: Pubkey, book: Pubkey, sequence: u64, halted: u8) -> Instruction {
    book_ix(
        disc::SET_BOOK_HALT,
        signer,
        book,
        bytes_of(&SetBookHaltData {
            sequence_number: sequence,
            halted,
        }),
    )
}

pub fn spread_ix(signer: Pubkey, book: Pubkey, sequence: u64, spread: u16) -> Instruction {
    book_ix(
        disc::UPDATE_SYNC_SPREAD,
        signer,
        book,
        bytes_of(&UpdateSyncSpreadData {
            sequence_number: sequence,
            sync_spread_ticks: spread,
        }),
    )
}

pub fn expiry_ix(signer: Pubkey, book: Pubkey, sequence: u64, expiry: u64) -> Instruction {
    book_ix(
        disc::UPDATE_EXPIRY_IN_SLOTS,
        signer,
        book,
        bytes_of(&UpdateExpiryInSlotsData {
            sequence_number: sequence,
            expiry_in_slots: expiry,
        }),
    )
}

pub fn delegate_ix(
    maker: Pubkey,
    book: Pubkey,
    delegate: Pubkey,
    min: u64,
    max: u64,
    max_lots: u64,
) -> Instruction {
    Instruction {
        program_id: ID,
        accounts: tail(vec![
            AccountMeta::new_readonly(maker, true),
            AccountMeta::new(book, false),
            AccountMeta::new_readonly(delegate, false),
        ]),
        data: ix_data(
            disc::SET_BOOK_DELEGATE,
            &SetBookDelegateData {
                min_price_ticks: min,
                max_price_ticks: max,
                max_level_lots: max_lots,
            },
        ),
    }
}

pub fn close_book_ix(maker: Pubkey, book: Pubkey) -> Instruction {
    Instruction {
        program_id: ID,
        accounts: tail(vec![
            AccountMeta::new(maker, true),
            AccountMeta::new(book, false),
        ]),
        data: vec![disc::CLOSE_MAKER_BOOK],
    }
}

#[allow(clippy::too_many_arguments)]
pub fn funds_ix(
    tag: u8,
    maker: Pubkey,
    market: Pubkey,
    book: Pubkey,
    base_vault: Pubkey,
    quote_vault: Pubkey,
    maker_base: Pubkey,
    maker_quote: Pubkey,
    base_lots: u64,
    quote_lots: u64,
) -> Instruction {
    Instruction {
        program_id: ID,
        accounts: tail(vec![
            AccountMeta::new_readonly(maker, true),
            AccountMeta::new_readonly(market, false),
            AccountMeta::new(book, false),
            AccountMeta::new(base_vault, false),
            AccountMeta::new(quote_vault, false),
            AccountMeta::new(maker_base, false),
            AccountMeta::new(maker_quote, false),
            AccountMeta::new_readonly(SPL_TOKEN, false),
        ]),
        data: ix_data(
            tag,
            &MakerFundsData {
                base_lots,
                quote_lots,
            },
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn sync_swap_ix(
    user: Pubkey,
    market: Pubkey,
    book: Pubkey,
    base_vault: Pubkey,
    quote_vault: Pubkey,
    user_base: Pubkey,
    user_quote: Pubkey,
    args: SyncSwapData,
) -> Instruction {
    Instruction {
        program_id: ID,
        accounts: tail(vec![
            AccountMeta::new_readonly(user, true),
            AccountMeta::new_readonly(market, false),
            AccountMeta::new(book, false),
            AccountMeta::new(base_vault, false),
            AccountMeta::new(quote_vault, false),
            AccountMeta::new(user_base, false),
            AccountMeta::new(user_quote, false),
            AccountMeta::new_readonly(SPL_TOKEN, false),
        ]),
        data: ix_data(disc::SYNC_SWAP, &args),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn async_create_ix(
    user: Pubkey,
    market: Pubkey,
    async_swap: Pubkey,
    escrow: Pubkey,
    mint_in: Pubkey,
    user_src: Pubkey,
    args: AsyncSwapCreateData,
) -> Instruction {
    Instruction {
        program_id: ID,
        accounts: tail(vec![
            AccountMeta::new(user, true),
            AccountMeta::new_readonly(market, false),
            AccountMeta::new(async_swap, false),
            AccountMeta::new(escrow, false),
            AccountMeta::new_readonly(mint_in, false),
            AccountMeta::new(user_src, false),
            AccountMeta::new_readonly(SYSTEM, false),
            AccountMeta::new_readonly(SPL_TOKEN, false),
            AccountMeta::new_readonly(ATA, false),
        ]),
        data: ix_data(disc::ASYNC_SWAP_CREATE, &args),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn async_execute_ix(
    signer: Pubkey,
    market: Pubkey,
    book: Pubkey,
    async_swap: Pubkey,
    escrow: Pubkey,
    base_vault: Pubkey,
    quote_vault: Pubkey,
    user_dst: Pubkey,
    user_refund: Pubkey,
    user: Pubkey,
) -> Instruction {
    Instruction {
        program_id: ID,
        accounts: tail(vec![
            AccountMeta::new_readonly(signer, true),
            AccountMeta::new_readonly(market, false),
            AccountMeta::new(book, false),
            AccountMeta::new(async_swap, false),
            AccountMeta::new(escrow, false),
            AccountMeta::new(base_vault, false),
            AccountMeta::new(quote_vault, false),
            AccountMeta::new(user_dst, false),
            AccountMeta::new(user_refund, false),
            AccountMeta::new(user, false),
            AccountMeta::new_readonly(SPL_TOKEN, false),
        ]),
        data: vec![disc::ASYNC_SWAP_EXECUTE],
    }
}

pub fn async_cancel_ix(
    caller: Pubkey,
    async_swap: Pubkey,
    escrow: Pubkey,
    user_refund: Pubkey,
    user: Pubkey,
) -> Instruction {
    Instruction {
        program_id: ID,
        accounts: tail(vec![
            AccountMeta::new_readonly(caller, true),
            AccountMeta::new(async_swap, false),
            AccountMeta::new(escrow, false),
            AccountMeta::new(user_refund, false),
            AccountMeta::new(user, false),
            AccountMeta::new_readonly(SPL_TOKEN, false),
        ]),
        data: vec![disc::ASYNC_SWAP_CANCEL],
    }
}

// ---------- scenario fixtures ----------

/// A created market with funded actors.
pub struct World {
    pub env: Env,
    pub payer: Pubkey,
    pub authority: Pubkey,
    pub market: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
}

impl World {
    pub fn new() -> Self {
        Self::with_args(&PoolArgs::default())
    }

    pub fn with_args(args: &PoolArgs) -> Self {
        Self::in_env(Env::new(), args)
    }

    pub fn in_env(mut env: Env, args: &PoolArgs) -> Self {
        let payer = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        env.set(payer, wallet(RICH));
        env.set(authority, wallet(SOL));
        env.set(base_mint, mint_account(BASE_DECIMALS));
        env.set(quote_mint, mint_account(QUOTE_DECIMALS));
        let (market, _) = market_pda(&base_mint, &quote_mint, &authority);
        let base_vault = ata(&market, &base_mint);
        let quote_vault = ata(&market, &quote_mint);
        let ix = create_pool_ix(
            payer,
            authority,
            market,
            base_mint,
            quote_mint,
            base_vault,
            quote_vault,
            args,
        );
        env.ok(&ix);
        Self {
            env,
            payer,
            authority,
            market,
            base_mint,
            quote_mint,
            base_vault,
            quote_vault,
        }
    }

    /// A funded wallet with base and quote token accounts holding `amount` atoms each.
    pub fn actor(&mut self, amount: u64) -> Actor {
        let key = Pubkey::new_unique();
        let base = Pubkey::new_unique();
        let quote = Pubkey::new_unique();
        self.env.set(key, wallet(RICH));
        self.env
            .set(base, token_account(&self.base_mint, &key, amount));
        self.env
            .set(quote, token_account(&self.quote_mint, &key, amount));
        Actor { key, base, quote }
    }

    /// A maker with an initialized, funded book quoting `bids` / `asks` around `mid`.
    pub fn maker(
        &mut self,
        base_lots: u64,
        quote_lots: u64,
        mid: u64,
        bids: &[(u64, i64)],
        asks: &[(u64, i64)],
    ) -> Maker {
        let actor = self.actor(RICH);
        let (book, _) = book_pda(&self.market, &actor.key);
        self.env.ok(&init_book_ix(actor.key, self.market, book));
        self.env
            .ok(&self.deposit_ix(&actor, book, base_lots, quote_lots));
        self.env
            .ok(&update_book_ix(actor.key, book, 1, mid, bids, asks));
        Maker {
            actor,
            book,
            sequence: 1,
        }
    }

    pub fn deposit_ix(
        &self,
        actor: &Actor,
        book: Pubkey,
        base_lots: u64,
        quote_lots: u64,
    ) -> Instruction {
        funds_ix(
            disc::MAKER_DEPOSIT,
            actor.key,
            self.market,
            book,
            self.base_vault,
            self.quote_vault,
            actor.base,
            actor.quote,
            base_lots,
            quote_lots,
        )
    }

    pub fn withdraw_ix(
        &self,
        actor: &Actor,
        book: Pubkey,
        base_lots: u64,
        quote_lots: u64,
    ) -> Instruction {
        funds_ix(
            disc::MAKER_WITHDRAW,
            actor.key,
            self.market,
            book,
            self.base_vault,
            self.quote_vault,
            actor.base,
            actor.quote,
            base_lots,
            quote_lots,
        )
    }

    pub fn swap_ix(&self, user: &Actor, book: Pubkey, args: SyncSwapData) -> Instruction {
        sync_swap_ix(
            user.key,
            self.market,
            book,
            self.base_vault,
            self.quote_vault,
            user.base,
            user.quote,
            args,
        )
    }

    /// A second market on the same mints under a new authority, in the same env.
    pub fn other_market(&mut self) -> MarketKeys {
        let authority = Pubkey::new_unique();
        self.env.set(authority, wallet(SOL));
        let (market, _) = market_pda(&self.base_mint, &self.quote_mint, &authority);
        let base_vault = ata(&market, &self.base_mint);
        let quote_vault = ata(&market, &self.quote_mint);
        let ix = create_pool_ix(
            self.payer,
            authority,
            market,
            self.base_mint,
            self.quote_mint,
            base_vault,
            quote_vault,
            &PoolArgs::default(),
        );
        self.env.ok(&ix);
        MarketKeys {
            market,
            base_vault,
            quote_vault,
            authority,
        }
    }

    /// Async-create instruction for `user` with this market's mints.
    pub fn async_create(
        &self,
        user: &Actor,
        nonce: u64,
        args: AsyncSwapCreateData,
    ) -> (Instruction, Pubkey, Pubkey) {
        let (swap, _) = async_pda(&self.market, &user.key, nonce);
        let side = args.side;
        let (mint_in, src) = if side == 0 {
            (self.base_mint, user.base)
        } else {
            (self.quote_mint, user.quote)
        };
        let escrow = ata(&swap, &mint_in);
        (
            async_create_ix(user.key, self.market, swap, escrow, mint_in, src, args),
            swap,
            escrow,
        )
    }

    /// Async-execute instruction; output to the user's account of the output mint, refunds
    /// to the input mint account.
    pub fn async_execute(
        &self,
        signer: Pubkey,
        book: Pubkey,
        user: &Actor,
        swap: Pubkey,
        escrow: Pubkey,
        side: u8,
    ) -> Instruction {
        let (dst, refund) = if side == 0 {
            (user.quote, user.base)
        } else {
            (user.base, user.quote)
        };
        async_execute_ix(
            signer,
            self.market,
            book,
            swap,
            escrow,
            self.base_vault,
            self.quote_vault,
            dst,
            refund,
            user.key,
        )
    }

    pub fn toggle_paused(&self, paused: bool) -> Instruction {
        toggle_ix(self.market, self.authority, paused as u8)
    }

    pub fn admin_quote_account(&mut self) -> Pubkey {
        let key = Pubkey::new_unique();
        self.env
            .set(key, token_account(&self.quote_mint, &self.authority, 0));
        key
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MarketKeys {
    pub market: Pubkey,
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
    pub authority: Pubkey,
}

pub struct Actor {
    pub key: Pubkey,
    pub base: Pubkey,
    pub quote: Pubkey,
}

pub struct Maker {
    pub actor: Actor,
    pub book: Pubkey,
    pub sequence: u64,
}

impl Maker {
    pub fn next_seq(&mut self) -> u64 {
        self.sequence += 1;
        self.sequence
    }
}

pub fn sell(base_lots: u64, min_quote_lots: u64) -> SyncSwapData {
    SyncSwapData {
        side: 0,
        pin_sequence: PIN_NONE,
        base_lots,
        limit_quote_lots: min_quote_lots,
        price_limit_ticks: 0,
        expected_sequence: 0,
    }
}

pub fn buy(base_lots: u64, max_quote_lots: u64) -> SyncSwapData {
    SyncSwapData {
        side: 1,
        pin_sequence: PIN_NONE,
        base_lots,
        limit_quote_lots: max_quote_lots,
        price_limit_ticks: u64::MAX,
        expected_sequence: 0,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn async_args(
    side: u8,
    nonce: u64,
    base_lots: u64,
    limit: u64,
    price_limit: u64,
    expiry_slot: u64,
    maker: Pubkey,
) -> AsyncSwapCreateData {
    AsyncSwapCreateData {
        side,
        nonce,
        base_lots,
        limit_quote_lots: limit,
        price_limit_ticks: price_limit,
        expiry_slot,
        maker: maker.to_bytes(),
    }
}

/// The open-to-any-maker marker.
pub fn any_maker() -> Pubkey {
    Pubkey::new_from_array(NO_ADDRESS)
}

/// Sum of base and quote lots the vaults owe across `books`: (base, quote incl. fees).
pub fn owed(env: &Env, books: &[Pubkey]) -> (u64, u64) {
    books.iter().fold((0, 0), |(b, q), key| {
        let book = env.book(key);
        (
            b + book.base_free,
            q + book.quote_free + book.fees_quote_lots,
        )
    })
}
