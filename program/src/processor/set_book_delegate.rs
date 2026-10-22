use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::event::{check_event_accounts, emit};
use nock_interface::{
    error::NockError,
    events::{tag, DelegateSetEvent},
    instruction::{parse, SetBookDelegateData},
    state::{load_mut, MakerBook, Role},
    ID,
};
use pinocchio::{account::AccountView, error::ProgramError, ProgramResult};

/// Maker names a delegate (the system program id clears it) and the bounds its quotes
/// must respect: every level price inside `[min_price_ticks, max_price_ticks]` and every
/// level at most `max_level_lots`. A delegate can never move funds.
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [maker, book, delegate, event_authority, program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let args = parse::<SetBookDelegateData>(data).ok_or(ProgramError::InvalidInstructionData)?;
    let min_price_ticks = args.min_price_ticks;
    let max_price_ticks = args.max_price_ticks;
    let max_level_lots = args.max_level_lots;
    if min_price_ticks > max_price_ticks {
        return Err(ProgramError::InvalidInstructionData);
    }

    check_signer(maker)?;
    check_writable(book)?;
    check_owner(book, &ID)?;
    check_event_accounts(event_authority, program)?;

    {
        let mut book_data = book.try_borrow_mut()?;
        let b = load_mut::<MakerBook>(&mut book_data[..])?;
        if b.role_of(maker.address().as_ref())? != Role::Maker {
            return Err(NockError::UnauthorizedMaker.into());
        }
        b.delegate = delegate.address().to_bytes();
        b.delegate_min_price_ticks = min_price_ticks;
        b.delegate_max_price_ticks = max_price_ticks;
        b.delegate_max_level_lots = max_level_lots;
    }

    emit(
        &DelegateSetEvent {
            tag: tag::DELEGATE_SET,
            book: book.address().to_bytes(),
            maker: maker.address().to_bytes(),
            delegate: delegate.address().to_bytes(),
            min_price_ticks,
            max_price_ticks,
            max_level_lots,
        },
        event_authority,
    )
}
