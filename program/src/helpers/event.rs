use nock_interface::{
    constants::{EVENT_AUTHORITY, EVENT_AUTHORITY_BUMP, MAX_EVENT_LEN, SEED_EVENT_AUTHORITY},
    error::NockError,
    events::{BookEvent, Event},
    instruction::disc,
    state::MakerBook,
    ID,
};
use pinocchio::{
    account::AccountView,
    cpi::{invoke_signed, Seed, Signer},
    error::ProgramError,
    instruction::{InstructionAccount, InstructionView},
    ProgramResult,
};

/// Discriminator byte in front of the event bytes.
const DISC_LEN: usize = 1;

/// The two accounts every event-emitting instruction takes last: the event-authority PDA
/// and this program, which the self-CPI invokes.
#[inline]
pub fn check_event_accounts(event_authority: &AccountView, program: &AccountView) -> ProgramResult {
    if event_authority.address() != &EVENT_AUTHORITY {
        return Err(NockError::InvalidEventAuthority.into());
    }
    if program.address() != &ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// Emits `event` as the data of a self-CPI signed by the event authority.
pub fn emit<T: Event>(event: &T, event_authority: &AccountView) -> ProgramResult {
    let len = core::mem::size_of::<T>();
    let mut buf = [0u8; DISC_LEN + MAX_EVENT_LEN];
    buf[0] = disc::EMIT_EVENT;
    // SAFETY: `T: Event` is repr(C) without padding, so all `len` bytes are initialized.
    let bytes = unsafe { core::slice::from_raw_parts(event as *const T as *const u8, len) };
    buf[DISC_LEN..DISC_LEN + len].copy_from_slice(bytes);

    let accounts = [InstructionAccount::readonly_signer(
        event_authority.address(),
    )];
    let instruction = InstructionView {
        program_id: &ID,
        data: &buf[..DISC_LEN + len],
        accounts: &accounts,
    };
    let bump = [EVENT_AUTHORITY_BUMP];
    let seeds = [Seed::from(SEED_EVENT_AUTHORITY), Seed::from(&bump[..])];
    invoke_signed(&instruction, &[event_authority], &[Signer::from(&seeds)])
}

/// `disc::EMIT_EVENT`: accepts only the event authority as signer, which only this
/// program can produce, so a forged event fails here.
pub fn process_emit(accounts: &mut [AccountView]) -> ProgramResult {
    let [event_authority, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !event_authority.is_signer() || event_authority.address() != &EVENT_AUTHORITY {
        return Err(NockError::InvalidEventAuthority.into());
    }
    Ok(())
}

/// Snapshot of `b` after a book-level change.
pub fn book_event(
    tag: [u8; 8],
    book: [u8; 32],
    b: &MakerBook,
    signer: [u8; 32],
    slot: u64,
) -> BookEvent {
    let (num_bids, num_asks) = b.live_levels();
    BookEvent {
        tag,
        book,
        market: b.market,
        maker: b.maker,
        signer,
        sequence: b.sequence,
        mid_price_ticks: b.mid_price_ticks,
        expiry_in_slots: b.expiry_in_slots,
        last_updated_slot: b.last_updated_slot,
        slot,
        sync_spread_ticks: b.sync_spread_ticks,
        status: b.status,
        num_bids,
        num_asks,
        _pad: [0; 3],
    }
}
