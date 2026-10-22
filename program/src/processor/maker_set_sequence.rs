use super::{update_book, BookAuth};
use nock_interface::{
    events::tag,
    instruction::{parse, SequenceData},
};
use pinocchio::{account::AccountView, error::ProgramError, ProgramResult};

/// Bumps the sequence only, invalidating in-flight quote updates and pinned takers.
pub fn process_set(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let args = parse::<SequenceData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let sequence = args.sequence_number;

    update_book(
        accounts,
        tag::SEQUENCE_SET,
        BookAuth::MakerOrDelegate,
        Some(sequence),
        |_, _, _| Ok(()),
    )
}

/// Maker-only escape hatch: sets the sequence to any value, including a lower one, so a
/// counter pushed far ahead can be recovered without closing the book.
pub fn process_reset(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let args = parse::<SequenceData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let sequence = args.sequence_number;

    update_book(
        accounts,
        tag::SEQUENCE_RESET,
        BookAuth::MakerOnly,
        None,
        |b, _, _| {
            b.sequence = sequence;
            Ok(())
        },
    )
}
