use crate::helpers::assertions::{check_owner, check_signer, check_writable};
use crate::helpers::event::{check_event_accounts, emit};
use crate::helpers::pda::close_pda;
use nock_interface::{
    error::NockError,
    events::{tag, BookClosedEvent},
    state::{load, MakerBook, Role},
    ID,
};
use pinocchio::{account::AccountView, error::ProgramError, ProgramResult};

/// Closes an empty book and refunds its rent to the maker. Inventory must be withdrawn and
/// fees collected first, so no token in the vault is left without an owner.
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [maker, book, event_authority, program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }
    check_signer(maker)?;
    check_writable(maker)?;
    check_writable(book)?;
    check_owner(book, &ID)?;
    check_event_accounts(event_authority, program)?;

    let market = {
        let book_data = book.try_borrow()?;
        let b = load::<MakerBook>(&book_data[..])?;
        if b.role_of(maker.address().as_ref())? != Role::Maker {
            return Err(NockError::UnauthorizedMaker.into());
        }
        if b.base_free != 0 || b.quote_free != 0 || b.fees_quote_lots != 0 {
            return Err(NockError::BookNotEmpty.into());
        }
        b.market
    };

    let book_key = book.address().to_bytes();
    let refunded_lamports = close_pda(book, maker)?;

    emit(
        &BookClosedEvent {
            tag: tag::BOOK_CLOSED,
            book: book_key,
            market,
            maker: maker.address().to_bytes(),
            refunded_lamports,
        },
        event_authority,
    )
}
