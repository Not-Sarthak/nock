use super::{update_book, BookAuth};
use nock_interface::{
    constants::MAX_QUOTE_AGE_SLOTS,
    error::NockError,
    events::tag,
    instruction::{parse, UpdateExpiryInSlotsData},
};
use pinocchio::{account::AccountView, error::ProgramError, ProgramResult};

/// Sets how many slots, counting the refresh slot itself, quotes stay live: 1 to
/// `MAX_QUOTE_AGE_SLOTS`.
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let args =
        parse::<UpdateExpiryInSlotsData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let sequence = args.sequence_number;
    let expiry = args.expiry_in_slots;
    if expiry == 0 || expiry > MAX_QUOTE_AGE_SLOTS {
        return Err(NockError::InvalidExpiry.into());
    }

    update_book(
        accounts,
        tag::EXPIRY_UPDATED,
        BookAuth::MakerOrDelegate,
        Some(sequence),
        |b, _, _| {
            b.expiry_in_slots = expiry;
            Ok(())
        },
    )
}
