use super::{update_book, BookAuth};
use nock_interface::{
    events::tag,
    instruction::{parse, UpdateSyncSpreadData},
};
use pinocchio::{account::AccountView, error::ProgramError, ProgramResult};

/// Sets the extra distance, in ticks, added to every level on both sides.
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let args = parse::<UpdateSyncSpreadData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let sequence = args.sequence_number;
    let spread = args.sync_spread_ticks;

    update_book(
        accounts,
        tag::SPREAD_UPDATED,
        BookAuth::MakerOrDelegate,
        Some(sequence),
        |b, role, _| {
            b.sync_spread_ticks = spread;
            Ok(b.validate_quotes(role)?)
        },
    )
}
