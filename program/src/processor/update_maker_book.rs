use super::{update_book, BookAuth};
use nock_interface::{
    constants::MAX_LEVELS,
    events::tag,
    instruction::{parse, UpdateMakerBookData},
    state::MakerLevel,
};
use pinocchio::{account::AccountView, error::ProgramError, ProgramResult};

/// Replaces the mid and both ladders, and refreshes the quote slot. Levels past
/// `num_bids` / `num_asks` are cleared.
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let args = parse::<UpdateMakerBookData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let header_sequence = args.header.sequence_number;
    let mid_price_ticks = args.header.mid_price_ticks;
    let num_bids = args.header.num_bids as usize;
    let num_asks = args.header.num_asks as usize;
    if num_bids > MAX_LEVELS || num_asks > MAX_LEVELS {
        return Err(ProgramError::InvalidInstructionData);
    }
    let bids = args.bid_levels;
    let asks = args.ask_levels;

    update_book(
        accounts,
        tag::BOOK_UPDATED,
        BookAuth::MakerOrDelegate,
        Some(header_sequence),
        |b, role, slot| {
            b.mid_price_ticks = mid_price_ticks;
            b.last_updated_slot = slot;
            for i in 0..MAX_LEVELS {
                b.bid_levels[i] = if i < num_bids {
                    bids[i]
                } else {
                    MakerLevel::EMPTY
                };
                b.ask_levels[i] = if i < num_asks {
                    asks[i]
                } else {
                    MakerLevel::EMPTY
                };
            }
            Ok(b.validate_quotes(role)?)
        },
    )
}
