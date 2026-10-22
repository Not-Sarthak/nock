use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::event::{check_event_accounts, emit};
use nock_interface::{
    constants::MAX_FEE_PPM,
    error::NockError,
    events::{tag, ConfigUpdatedEvent},
    instruction::{parse, UpdateConfigData},
    state::{load_mut, Market},
    ID,
};
use pinocchio::{account::AccountView, error::ProgramError, ProgramResult};

pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [market, admin, event_authority, program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let args = parse::<UpdateConfigData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let maker_fee_ppm = args.maker_fee_ppm;
    let taker_fee_ppm = args.taker_fee_ppm;
    if maker_fee_ppm > MAX_FEE_PPM || taker_fee_ppm > MAX_FEE_PPM {
        return Err(NockError::InvalidFee.into());
    }

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
        m.maker_fee_ppm = maker_fee_ppm;
        m.taker_fee_ppm = taker_fee_ppm;
    }

    emit(
        &ConfigUpdatedEvent {
            tag: tag::CONFIG_UPDATED,
            market: market.address().to_bytes(),
            admin: admin.address().to_bytes(),
            maker_fee_ppm,
            taker_fee_ppm,
        },
        event_authority,
    )
}
