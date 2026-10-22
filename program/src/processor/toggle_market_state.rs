use super::parse_flag;
use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::event::{check_event_accounts, emit};
use nock_interface::{
    error::NockError,
    events::{tag, MarketStatusEvent},
    instruction::{parse, ToggleMarketStateData},
    state::{load_mut, Market},
    ID,
};
use pinocchio::{
    account::AccountView,
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [market, admin, event_authority, program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let args = parse::<ToggleMarketStateData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let paused = parse_flag(args.paused)?;

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
        m.set_paused(paused);
    }

    emit(
        &MarketStatusEvent {
            tag: tag::MARKET_STATUS,
            market: market.address().to_bytes(),
            admin: admin.address().to_bytes(),
            slot: Clock::get()?.slot,
            paused: paused as u8,
            _pad: [0; 7],
        },
        event_authority,
    )
}
