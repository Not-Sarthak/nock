pub mod async_swap;
pub mod clear_book;
pub mod close_maker_book;
pub mod collect_fees;
pub mod create_pool;
pub mod init_maker_book;
pub mod maker_deposit_funds;
pub mod maker_heartbeat;
pub mod maker_set_sequence;
pub mod maker_withdraw_funds;
pub mod rotate_admin;
pub mod set_book_delegate;
pub mod set_book_halt;
pub mod sync_swap;
pub mod toggle_market_state;
pub mod update_config;
pub mod update_expiry_in_slots;
pub mod update_maker_book;
pub mod update_mid_price;
pub mod update_sync_spread;

use nock_interface::state::{load, Market};
use pinocchio::{account::AccountView, error::ProgramError};

/// Flag bytes in instruction data must be exactly 0 or 1.
#[inline]
pub(crate) fn parse_flag(byte: u8) -> Result<bool, ProgramError> {
    match byte {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

/// Copies the market state out so no borrow outlives the read.
#[inline]
pub(crate) fn read_market<R>(
    market: &AccountView,
    f: impl FnOnce(&Market) -> Result<R, ProgramError>,
) -> Result<R, ProgramError> {
    crate::helpers::assertions::check_owner(market, &nock_interface::ID)?;
    let data = market.try_borrow()?;
    f(load::<Market>(&data[..])?)
}

/// Signer authority a book instruction requires.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BookAuth {
    MakerOrDelegate,
    MakerOnly,
}

/// Shared shape of every book-level instruction, accounts `[signer, book, event_authority,
/// program]`: authorizes the signer, bumps the sequence when one is given, applies
/// `change`, and emits a `BookEvent` tagged `tag` with the resulting state.
pub(crate) fn update_book(
    accounts: &mut [AccountView],
    tag: [u8; 8],
    auth: BookAuth,
    sequence: Option<u64>,
    change: impl FnOnce(
        &mut nock_interface::state::MakerBook,
        nock_interface::state::Role,
        u64,
    ) -> Result<(), ProgramError>,
) -> pinocchio::ProgramResult {
    use crate::helpers::assertions::{check_owner, check_signer, check_writable};
    use crate::helpers::event::{book_event, check_event_accounts, emit};
    use nock_interface::{
        error::NockError,
        state::{load_mut, MakerBook, Role},
        ID,
    };
    use pinocchio::sysvars::{clock::Clock, Sysvar};

    let [signer, book, event_authority, program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    check_signer(signer)?;
    check_writable(book)?;
    check_owner(book, &ID)?;
    check_event_accounts(event_authority, program)?;

    let slot = Clock::get()?.slot;
    let book_key = book.address().to_bytes();
    let signer_key = signer.address().to_bytes();
    let event = {
        let mut book_data = book.try_borrow_mut()?;
        let b = load_mut::<MakerBook>(&mut book_data[..])?;
        let role = b.role_of(signer.address().as_ref())?;
        if auth == BookAuth::MakerOnly && role != Role::Maker {
            return Err(NockError::UnauthorizedMaker.into());
        }
        if let Some(seq) = sequence {
            b.try_bump_sequence(seq)?;
        }
        change(b, role, slot)?;
        book_event(tag, book_key, b, signer_key, slot)
    };
    emit(&event, event_authority)
}
