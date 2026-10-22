use super::{update_book, BookAuth};
use nock_interface::{
    events::tag,
    instruction::{parse, SequenceData},
};
use pinocchio::{account::AccountView, error::ProgramError, ProgramResult};

/// Keeps the current quotes live for another `expiry_in_slots` slots.
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let args = parse::<SequenceData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let sequence = args.sequence_number;

    update_book(
        accounts,
        tag::HEARTBEAT,
        BookAuth::MakerOrDelegate,
        Some(sequence),
        |b, _, slot| {
            b.last_updated_slot = slot;
            Ok(())
        },
    )
}
