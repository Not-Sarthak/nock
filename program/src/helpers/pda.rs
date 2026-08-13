use pinocchio::{account::AccountView, cpi::Signer, Address, ProgramResult};
use pinocchio_system::instructions::{Allocate, Assign, CreateAccount, Transfer};

pub fn create_pda(
    payer: &AccountView,
    target: &AccountView,
    lamports: u64,
    space: u64,
    owner: &Address,
    signer: &[Signer],
) -> ProgramResult {
    let existing = target.lamports();

    if existing == 0 {
        return CreateAccount { from: payer, to: target, lamports, space, owner }
            .invoke_signed(signer);
    }

    if let Some(shortfall) = lamports.checked_sub(existing) {
        if shortfall > 0 {
            Transfer { from: payer, to: target, lamports: shortfall }.invoke()?;
        }
    }

    Allocate { account: target, space }.invoke_signed(signer)?;
    Assign { account: target, owner }.invoke_signed(signer)
}

pub fn close_pda(target: &mut AccountView, refund_to: &mut AccountView) -> ProgramResult {
    let refund = target.lamports();
    let credited = refund_to.lamports().saturating_add(refund);
    refund_to.set_lamports(credited);
    target.set_lamports(0);
    target.close()
}
