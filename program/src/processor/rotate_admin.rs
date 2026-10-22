use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::event::{check_event_accounts, emit};
use nock_interface::{
    constants::NO_ADDRESS,
    error::NockError,
    events::{tag, AdminEvent},
    instruction::{parse, ProposeAdminData},
    state::{load_mut, Market},
    ID,
};
use pinocchio::{account::AccountView, error::ProgramError, ProgramResult};

/// Admin names a successor; `NO_ADDRESS` withdraws a pending proposal. Nothing changes
/// hands until the successor signs `process_accept`.
pub fn process_propose(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [market, admin, event_authority, program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let args = parse::<ProposeAdminData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let new_admin = args.new_admin;

    check_signer(admin)?;
    check_writable(market)?;
    check_owner(market, &ID)?;
    check_event_accounts(event_authority, program)?;

    {
        let mut market_data = market.try_borrow_mut()?;
        let m = load_mut::<Market>(&mut market_data[..])?;
        if admin.address().as_array() != &m.admin {
            return Err(NockError::UnauthorizedAdmin.into());
        }
        m.pending_admin = new_admin;
    }

    emit(
        &AdminEvent {
            tag: tag::ADMIN_PROPOSED,
            market: market.address().to_bytes(),
            admin: admin.address().to_bytes(),
            pending_admin: new_admin,
        },
        event_authority,
    )
}

/// The proposed admin takes over by signing.
pub fn process_accept(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [market, new_admin, event_authority, program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    check_signer(new_admin)?;
    check_writable(market)?;
    check_owner(market, &ID)?;
    check_event_accounts(event_authority, program)?;

    {
        let mut market_data = market.try_borrow_mut()?;
        let m = load_mut::<Market>(&mut market_data[..])?;
        if m.pending_admin == NO_ADDRESS {
            return Err(NockError::NoPendingAdmin.into());
        }
        if new_admin.address().as_array() != &m.pending_admin {
            return Err(NockError::UnauthorizedAdmin.into());
        }
        m.admin = m.pending_admin;
        m.pending_admin = NO_ADDRESS;
    }

    emit(
        &AdminEvent {
            tag: tag::ADMIN_ACCEPTED,
            market: market.address().to_bytes(),
            admin: new_admin.address().to_bytes(),
            pending_admin: NO_ADDRESS,
        },
        event_authority,
    )
}
