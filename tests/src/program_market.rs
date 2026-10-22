//! Market lifecycle on the built program: CreatePool, pause, config, admin rotation,
//! fee collection, events and rent.

use crate::harness::*;
use mollusk_svm::result::ProgramResult;
use nock_interface::{constants::*, error::NockError, events::*, instruction::disc, ID};
use solana_instruction::{AccountMeta, Instruction};
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;
use solana_rent::Rent;

/// An env with mints and actors ready for CreatePool, and the instruction.
struct Pre {
    env: Env,
    payer: Pubkey,
    authority: Pubkey,
    market: Pubkey,
    base_mint: Pubkey,
    quote_mint: Pubkey,
    base_vault: Pubkey,
    quote_vault: Pubkey,
}

impl Pre {
    fn new() -> Self {
        let mut env = Env::new();
        let payer = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        env.set(payer, wallet(RICH));
        env.set(authority, wallet(SOL));
        env.set(base_mint, mint_account(BASE_DECIMALS));
        env.set(quote_mint, mint_account(QUOTE_DECIMALS));
        let (market, _) = market_pda(&base_mint, &quote_mint, &authority);
        Self {
            env,
            payer,
            authority,
            market,
            base_mint,
            quote_mint,
            base_vault: ata(&market, &base_mint),
            quote_vault: ata(&market, &quote_mint),
        }
    }

    fn ix(&self, args: &PoolArgs) -> Instruction {
        create_pool_ix(
            self.payer,
            self.authority,
            self.market,
            self.base_mint,
            self.quote_mint,
            self.base_vault,
            self.quote_vault,
            args,
        )
    }
}

#[test]
fn create_pool_creates_the_market_and_its_vaults() {
    let mut pre = Pre::new();
    let result = pre.env.ok(&pre.ix(&PoolArgs::default()));

    let account = pre.env.get(&pre.market);
    assert_eq!(account.owner, ID);
    assert_eq!(
        account.lamports,
        pre.env.mollusk.sysvars.rent.minimum_balance(MARKET_LEN)
    );
    let m = pre.env.market(&pre.market);
    assert_eq!(m.tag, MARKET_TAG);
    assert_eq!(m.admin, pre.authority.to_bytes());
    assert_eq!(m.seed_authority, pre.authority.to_bytes());
    assert_eq!(m.pending_admin, NO_ADDRESS);
    assert_eq!(m.base_vault, pre.base_vault.to_bytes());
    assert_eq!(m.quote_vault, pre.quote_vault.to_bytes());
    assert_eq!(m.base_atoms_per_base_unit, 10u64.pow(BASE_DECIMALS as u32));
    assert_eq!(
        (m.maker_fee_ppm, m.taker_fee_ppm),
        (MAKER_FEE_PPM, TAKER_FEE_PPM)
    );
    assert_eq!(
        m.market_bump,
        market_pda(&pre.base_mint, &pre.quote_mint, &pre.authority).1,
        "canonical bump stored"
    );
    for (vault, mint) in [
        (pre.base_vault, pre.base_mint),
        (pre.quote_vault, pre.quote_mint),
    ] {
        let data = pre.env.get(&vault).data;
        assert_eq!(&data[0..32], mint.as_ref());
        assert_eq!(&data[32..64], pre.market.as_ref());
    }

    let e: PoolCreatedEvent = event(&result, tag::POOL_CREATED);
    assert_eq!(e.market, pre.market.to_bytes());
    assert_eq!(e.admin, pre.authority.to_bytes());
    assert_eq!(
        (e.maker_fee_ppm, e.taker_fee_ppm),
        (MAKER_FEE_PPM, TAKER_FEE_PPM)
    );
}

#[test]
fn create_pool_enforces_signers_and_writables() {
    let pre = Pre::new();
    pre.env
        .assert_signers_and_writables_enforced(&pre.ix(&PoolArgs::default()));
}

/// Audit P0: anyone could create a market under someone else's authority seed.
#[test]
fn regression_market_squatting_requires_authority_signature() {
    let pre = Pre::new();
    let mut ix = pre.ix(&PoolArgs::default());
    ix.accounts[1].is_signer = false;
    pre.env
        .expect(&ix, failure(ProgramError::MissingRequiredSignature));
}

#[test]
fn create_pool_accepts_a_separate_payer_and_survives_a_prefunded_address() {
    let mut pre = Pre::new();
    pre.env.set(pre.market, wallet(1_000));
    pre.env.ok(&pre.ix(&PoolArgs::default()));
    let account = pre.env.get(&pre.market);
    assert_eq!(account.owner, ID);
    assert_eq!(
        account.lamports,
        pre.env.mollusk.sysvars.rent.minimum_balance(MARKET_LEN)
    );
}

#[test]
fn create_pool_rejects_bad_config() {
    let pre = Pre::new();
    let cases: [(PoolArgs, NockError); 7] = [
        (
            PoolArgs {
                tick: 0,
                ..PoolArgs::default()
            },
            NockError::InvalidMarketConfig,
        ),
        (
            PoolArgs {
                base_lot: 0,
                ..PoolArgs::default()
            },
            NockError::InvalidMarketConfig,
        ),
        (
            PoolArgs {
                quote_lot: 0,
                ..PoolArgs::default()
            },
            NockError::InvalidMarketConfig,
        ),
        (
            PoolArgs {
                maker_fee: MAX_FEE_PPM + 1,
                ..PoolArgs::default()
            },
            NockError::InvalidFee,
        ),
        (
            PoolArgs {
                taker_fee: MAX_FEE_PPM + 1,
                ..PoolArgs::default()
            },
            NockError::InvalidFee,
        ),
        (
            PoolArgs {
                min_async: 0,
                ..PoolArgs::default()
            },
            NockError::InvalidMarketConfig,
        ),
        (
            PoolArgs {
                min_async: 5,
                max_async: 4,
                ..PoolArgs::default()
            },
            NockError::InvalidMarketConfig,
        ),
    ];
    for (args, err) in cases {
        pre.env.expect(&pre.ix(&args), custom(err));
    }
    let at_cap = PoolArgs {
        maker_fee: MAX_FEE_PPM,
        taker_fee: MAX_FEE_PPM,
        ..PoolArgs::default()
    };
    assert!(pre.env.try_ix(&pre.ix(&at_cap)).program_result.is_ok());

    let mut short = pre.ix(&PoolArgs::default());
    short.data.pop();
    pre.env
        .expect(&short, failure(ProgramError::InvalidInstructionData));
}

#[test]
fn create_pool_rejects_bad_mints() {
    // same mint on both sides
    let mut pre = Pre::new();
    pre.quote_mint = pre.base_mint;
    pre.market = market_pda(&pre.base_mint, &pre.base_mint, &pre.authority).0;
    pre.env.expect(
        &pre.ix(&PoolArgs::default()),
        custom(NockError::InvalidMarketConfig),
    );

    // Token-2022 mint
    let mut pre = Pre::new();
    let mut m = mint_account(BASE_DECIMALS);
    m.owner = TOKEN_2022;
    pre.env.set(pre.base_mint, m);
    pre.env.expect(
        &pre.ix(&PoolArgs::default()),
        custom(NockError::Token2022Unsupported),
    );

    // uninitialized mint
    let mut pre = Pre::new();
    let mut m = mint_account(BASE_DECIMALS);
    m.data[45] = 0;
    pre.env.set(pre.base_mint, m);
    pre.env.expect(
        &pre.ix(&PoolArgs::default()),
        custom(NockError::InvalidMint),
    );

    // a token account passed as a mint
    let mut pre = Pre::new();
    pre.env.set(
        pre.base_mint,
        token_account(&Pubkey::new_unique(), &Pubkey::new_unique(), 0),
    );
    pre.env.expect(
        &pre.ix(&PoolArgs::default()),
        custom(NockError::InvalidMint),
    );

    // decimals whose unit overflows u64
    let mut pre = Pre::new();
    pre.env
        .set(pre.base_mint, mint_account(MAX_MINT_DECIMALS + 1));
    pre.env.expect(
        &pre.ix(&PoolArgs::default()),
        custom(NockError::InvalidMint),
    );

    // a mint owned by some other program
    let mut pre = Pre::new();
    let mut m = mint_account(BASE_DECIMALS);
    m.owner = Pubkey::new_unique();
    pre.env.set(pre.quote_mint, m);
    pre.env.expect(
        &pre.ix(&PoolArgs::default()),
        custom(NockError::InvalidMint),
    );
}

#[test]
fn create_pool_rejects_wrong_programs() {
    let pre = Pre::new();
    let base = pre.ix(&PoolArgs::default());
    let swap = |index: usize, key: Pubkey| {
        let mut ix = base.clone();
        ix.accounts[index].pubkey = key;
        ix
    };
    pre.env.expect(
        &swap(7, Pubkey::new_unique()),
        failure(ProgramError::InvalidArgument),
    );
    pre.env.expect(
        &swap(9, Pubkey::new_unique()),
        failure(ProgramError::InvalidArgument),
    );
    pre.env.expect(
        &swap(8, TOKEN_2022),
        custom(NockError::Token2022Unsupported),
    );
    pre.env.expect(
        &swap(8, Pubkey::new_unique()),
        custom(NockError::InvalidTokenProgram),
    );
    pre.env.expect(
        &swap(10, Pubkey::new_unique()),
        custom(NockError::InvalidEventAuthority),
    );
    pre.env.expect(
        &swap(11, Pubkey::new_unique()),
        failure(ProgramError::IncorrectProgramId),
    );
    pre.env.expect(
        &swap(5, Pubkey::new_unique()),
        failure(ProgramError::InvalidSeeds),
    );
}

/// Audit P1: a non-canonical bump let one seed set address several markets.
#[test]
fn regression_non_canonical_market_bump_rejected() {
    let mut pre = Pre::new();
    let (other, _) = non_canonical(&[
        SEED_MARKET,
        pre.base_mint.as_ref(),
        pre.quote_mint.as_ref(),
        pre.authority.as_ref(),
    ]);
    pre.market = other;
    pre.base_vault = ata(&other, &pre.base_mint);
    pre.quote_vault = ata(&other, &pre.quote_mint);
    pre.env
        .expect(&pre.ix(&PoolArgs::default()), custom(NockError::InvalidPda));
}

#[test]
fn create_pool_cannot_run_twice() {
    let mut pre = Pre::new();
    let ix = pre.ix(&PoolArgs::default());
    pre.env.ok(&ix);
    assert!(!pre.env.try_ix(&ix).program_result.is_ok());
}

/// Rent is read from the sysvar with its threshold, so creation is exactly rent-exempt
/// both before (rate per byte-year, threshold 2) and after SIMD-0194 (rate per byte,
/// threshold 1), and under a different rate.
#[test]
#[allow(deprecated)]
fn rent_matches_under_both_sysvar_layouts() {
    for (rate, threshold) in [(3_480u64, 2.0f64), (6_960, 1.0), (10_000, 1.0)] {
        let mut env = Env::new();
        env.mollusk.sysvars.rent = Rent {
            lamports_per_byte_year: rate,
            exemption_threshold: threshold,
            burn_percent: 50,
        };
        let w = World::in_env(env, &PoolArgs::default());
        let expected = ((128 + MARKET_LEN as u64) * rate) as f64 * threshold;
        assert_eq!(
            w.env.get(&w.market).lamports,
            expected as u64,
            "rate {rate} threshold {threshold}"
        );
        assert_eq!(
            w.env.get(&w.market).lamports,
            w.env.mollusk.sysvars.rent.minimum_balance(MARKET_LEN)
        );
    }
}

#[test]
fn toggle_pauses_and_resumes() {
    let mut w = World::new();
    let r = w.env.ok(&toggle_ix(w.market, w.authority, 1));
    assert!(w.env.market(&w.market).is_paused());
    let e: MarketStatusEvent = event(&r, tag::MARKET_STATUS);
    assert_eq!((e.paused, e.slot), (1, w.env.slot()));
    w.env.ok(&toggle_ix(w.market, w.authority, 0));
    assert!(!w.env.market(&w.market).is_paused());
}

#[test]
fn toggle_rejects() {
    let mut w = World::new();
    w.env
        .assert_signers_and_writables_enforced(&toggle_ix(w.market, w.authority, 1));
    let stranger = Pubkey::new_unique();
    w.env.set(stranger, wallet(SOL));
    w.env.expect(
        &toggle_ix(w.market, stranger, 1),
        custom(NockError::UnauthorizedAdmin),
    );
    w.env.expect(
        &toggle_ix(w.market, w.authority, 2),
        failure(ProgramError::InvalidInstructionData),
    );
    // a market-shaped account owned by another program
    let fake = Pubkey::new_unique();
    let mut account = w.env.get(&w.market);
    account.owner = Pubkey::new_unique();
    w.env.set(fake, account);
    w.env.expect(
        &toggle_ix(fake, w.authority, 1),
        failure(ProgramError::IllegalOwner),
    );
    // a program-owned account that is not a market
    let maker = w.maker(1, 1, 1_000_000, &[], &[]);
    w.env.expect(
        &toggle_ix(maker.book, w.authority, 1),
        failure(ProgramError::InvalidAccountData),
    );
}

#[test]
fn update_config_sets_fees_within_the_cap() {
    let mut w = World::new();
    let r = w
        .env
        .ok(&update_config_ix(w.market, w.authority, 7, MAX_FEE_PPM));
    let m = w.env.market(&w.market);
    assert_eq!((m.maker_fee_ppm, m.taker_fee_ppm), (7, MAX_FEE_PPM));
    let e: ConfigUpdatedEvent = event(&r, tag::CONFIG_UPDATED);
    assert_eq!((e.maker_fee_ppm, e.taker_fee_ppm), (7, MAX_FEE_PPM));

    w.env.expect(
        &update_config_ix(w.market, w.authority, MAX_FEE_PPM + 1, 0),
        custom(NockError::InvalidFee),
    );
    w.env.expect(
        &update_config_ix(w.market, w.authority, 0, MAX_FEE_PPM + 1),
        custom(NockError::InvalidFee),
    );
    w.env.expect(
        &update_config_ix(w.market, w.payer, 0, 0),
        custom(NockError::UnauthorizedAdmin),
    );
    w.env
        .assert_signers_and_writables_enforced(&update_config_ix(w.market, w.authority, 1, 1));
}

#[test]
fn two_step_admin_rotation() {
    let mut w = World::new();
    let next = Pubkey::new_unique();
    w.env.set(next, wallet(SOL));

    w.env.expect(
        &accept_admin_ix(w.market, next),
        custom(NockError::NoPendingAdmin),
    );
    w.env.expect(
        &propose_admin_ix(w.market, next, next),
        custom(NockError::UnauthorizedAdmin),
    );
    w.env
        .assert_signers_and_writables_enforced(&propose_admin_ix(w.market, w.authority, next));

    let r = w.env.ok(&propose_admin_ix(w.market, w.authority, next));
    let e: AdminEvent = event(&r, tag::ADMIN_PROPOSED);
    assert_eq!(e.pending_admin, next.to_bytes());
    assert_eq!(
        w.env.market(&w.market).admin,
        w.authority.to_bytes(),
        "nothing moves until accepted"
    );
    w.env.expect(
        &accept_admin_ix(w.market, w.payer),
        custom(NockError::UnauthorizedAdmin),
    );
    w.env
        .assert_signers_and_writables_enforced(&accept_admin_ix(w.market, next));

    let r = w.env.ok(&accept_admin_ix(w.market, next));
    let e: AdminEvent = event(&r, tag::ADMIN_ACCEPTED);
    assert_eq!(e.admin, next.to_bytes());
    let m = w.env.market(&w.market);
    assert_eq!((m.admin, m.pending_admin), (next.to_bytes(), NO_ADDRESS));
    assert_eq!(
        m.seed_authority,
        w.authority.to_bytes(),
        "the PDA seed never changes"
    );

    w.env.expect(
        &toggle_ix(w.market, w.authority, 1),
        custom(NockError::UnauthorizedAdmin),
    );
    w.env.ok(&toggle_ix(w.market, next, 1));
    w.env.ok(&toggle_ix(w.market, next, 0));

    // the market still signs for its vaults after rotation
    let maker = w.maker(10, 1_000, 1_000_000, &[], &[]);
    w.env
        .ok(&w.withdraw_ix(&maker.actor, maker.book, 10, 1_000));
}

#[test]
fn proposing_no_address_cancels_a_rotation() {
    let mut w = World::new();
    let next = Pubkey::new_unique();
    w.env.set(next, wallet(SOL));
    w.env.ok(&propose_admin_ix(w.market, w.authority, next));
    w.env
        .ok(&propose_admin_ix(w.market, w.authority, any_maker()));
    w.env.expect(
        &accept_admin_ix(w.market, next),
        custom(NockError::NoPendingAdmin),
    );
}

fn world_with_fees() -> (World, Maker) {
    let mut w = World::new();
    let maker = w.maker(1_000, 1_000_000, 1_000_000, &[(100, 1)], &[(100, 1)]);
    let user = w.actor(RICH);
    w.env.ok(&w.swap_ix(&user, maker.book, buy(50, u64::MAX)));
    w.env.ok(&w.swap_ix(&user, maker.book, sell(50, 0)));
    assert!(w.env.book(&maker.book).fees_quote_lots > 0);
    (w, maker)
}

#[test]
fn collect_fees_pays_the_admin_and_zeroes_the_book() {
    let (mut w, maker) = world_with_fees();
    let dest = w.admin_quote_account();
    let fees = w.env.book(&maker.book).fees_quote_lots;
    let vault_before = w.env.token_amount(&w.quote_vault);
    let ix = collect_fees_ix(w.market, maker.book, w.quote_vault, dest);
    w.env.assert_signers_and_writables_enforced(&ix);
    let r = w.env.ok(&ix);
    assert_eq!(w.env.token_amount(&dest), fees * QUOTE_ATOMS_PER_LOT);
    assert_eq!(
        w.env.token_amount(&w.quote_vault),
        vault_before - fees * QUOTE_ATOMS_PER_LOT
    );
    assert_eq!(w.env.book(&maker.book).fees_quote_lots, 0);
    let e: FeesCollectedEvent = event(&r, tag::FEES_COLLECTED);
    assert_eq!(
        (e.fee_quote_lots, e.fee_quote_atoms),
        (fees, fees * QUOTE_ATOMS_PER_LOT)
    );
    assert_eq!(e.destination, dest.to_bytes());

    // nothing left: a no-op with no event
    let r = w.env.ok(&ix);
    assert!(events(&r).is_empty());
}

#[test]
fn collect_fees_goes_only_to_the_admin() {
    let (mut w, maker) = world_with_fees();
    let thief = Pubkey::new_unique();
    let thief_quote = Pubkey::new_unique();
    w.env
        .set(thief_quote, token_account(&w.quote_mint, &thief, 0));
    w.env.expect(
        &collect_fees_ix(w.market, maker.book, w.quote_vault, thief_quote),
        custom(NockError::IncorrectTokenAccount),
    );

    let admin_base = Pubkey::new_unique();
    w.env
        .set(admin_base, token_account(&w.base_mint, &w.authority, 0));
    w.env.expect(
        &collect_fees_ix(w.market, maker.book, w.quote_vault, admin_base),
        custom(NockError::IncorrectTokenAccount),
    );

    let dest = w.admin_quote_account();
    w.env.expect(
        &collect_fees_ix(w.market, maker.book, w.base_vault, dest),
        custom(NockError::IncorrectVault),
    );

    let other = w.other_market();
    w.env.expect(
        &collect_fees_ix(other.market, maker.book, other.quote_vault, dest),
        custom(NockError::IncorrectTokenAccount),
    );
    let other_admin_dest = Pubkey::new_unique();
    w.env.set(
        other_admin_dest,
        token_account(&w.quote_mint, &other.authority, 0),
    );
    w.env.expect(
        &collect_fees_ix(
            other.market,
            maker.book,
            other.quote_vault,
            other_admin_dest,
        ),
        custom(NockError::MarketMismatch),
    );

    // after rotation, only the new admin can be paid
    let next = Pubkey::new_unique();
    w.env.set(next, wallet(SOL));
    w.env.ok(&propose_admin_ix(w.market, w.authority, next));
    w.env.ok(&accept_admin_ix(w.market, next));
    w.env.expect(
        &collect_fees_ix(w.market, maker.book, w.quote_vault, dest),
        custom(NockError::IncorrectTokenAccount),
    );
    let next_dest = Pubkey::new_unique();
    w.env.set(next_dest, token_account(&w.quote_mint, &next, 0));
    w.env.ok(&collect_fees_ix(
        w.market,
        maker.book,
        w.quote_vault,
        next_dest,
    ));
}

#[test]
fn emit_event_cannot_be_called_from_outside() {
    let w = World::new();
    let forged = |signer: bool, key: Pubkey| Instruction {
        program_id: ID,
        accounts: vec![AccountMeta::new_readonly(key, signer)],
        data: vec![disc::EMIT_EVENT, 1, 2, 3],
    };
    w.env.expect(
        &forged(false, EVENT_AUTHORITY),
        custom(NockError::InvalidEventAuthority),
    );
    let mut env = Env::new();
    let impostor = Pubkey::new_unique();
    env.set(impostor, wallet(SOL));
    env.expect(
        &forged(true, impostor),
        custom(NockError::InvalidEventAuthority),
    );
}

#[test]
fn unknown_and_empty_instructions_are_rejected() {
    let w = World::new();
    for data in [vec![], vec![0x20], vec![0xfe]] {
        let ix = Instruction {
            program_id: ID,
            accounts: vec![],
            data,
        };
        w.env
            .expect(&ix, failure(ProgramError::InvalidInstructionData));
    }
    let _ = ProgramResult::Success;
}

#[test]
fn event_authority_is_the_canonical_pda() {
    let (address, bump) = Pubkey::find_program_address(&[SEED_EVENT_AUTHORITY], &ID);
    assert_eq!((address, bump), (EVENT_AUTHORITY, EVENT_AUTHORITY_BUMP));
}
