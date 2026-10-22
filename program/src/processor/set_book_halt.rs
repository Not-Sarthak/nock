use super::{parse_flag, update_book, BookAuth};
use nock_interface::{
    error::NockError,
    events::tag,
    instruction::{parse, SetBookHaltData},
    state::{maker_book::book_status, Role},
};
use pinocchio::{account::AccountView, error::ProgramError, ProgramResult};

/// Halts or resumes every quote on the book. Sequenced, so a stale resume can never land
/// after a newer halt. A delegate may halt, but only the maker may resume, so a maker who
/// halts against a misbehaving delegate cannot be overridden by it.
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let args = parse::<SetBookHaltData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let sequence = args.sequence_number;
    let halted = parse_flag(args.halted)?;

    update_book(
        accounts,
        tag::HALT_SET,
        BookAuth::MakerOrDelegate,
        Some(sequence),
        |b, role, _| {
            if !halted && role != Role::Maker {
                return Err(NockError::UnauthorizedMaker.into());
            }
            b.status = if halted {
                book_status::HALTED
            } else {
                book_status::ACTIVE
            };
            Ok(())
        },
    )
}
