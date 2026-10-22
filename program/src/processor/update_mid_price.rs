use super::{update_book, BookAuth};
use nock_interface::{
    events::tag,
    instruction::{parse, UpdateMidPriceData},
};
use pinocchio::{account::AccountView, error::ProgramError, ProgramResult};

/// Moves the mid, repricing every level, and refreshes the quote slot.
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let args = parse::<UpdateMidPriceData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let sequence = args.sequence_number;
    let new_mid = args.new_mid_ticks;

    update_book(
        accounts,
        tag::MID_UPDATED,
        BookAuth::MakerOrDelegate,
        Some(sequence),
        |b, role, slot| {
            b.mid_price_ticks = new_mid;
            b.last_updated_slot = slot;
            Ok(b.validate_quotes(role)?)
        },
    )
}
