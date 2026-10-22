use super::{update_book, BookAuth};
use nock_interface::{
    constants::MAX_LEVELS,
    events::tag,
    instruction::{parse, SequenceData},
    state::MakerLevel,
};
use pinocchio::{account::AccountView, error::ProgramError, ProgramResult};

/// Empties both ladders.
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let args = parse::<SequenceData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let sequence = args.sequence_number;

    update_book(
        accounts,
        tag::BOOK_CLEARED,
        BookAuth::MakerOrDelegate,
        Some(sequence),
        |b, _, _| {
            b.bid_levels = [MakerLevel::EMPTY; MAX_LEVELS];
            b.ask_levels = [MakerLevel::EMPTY; MAX_LEVELS];
            Ok(())
        },
    )
}
