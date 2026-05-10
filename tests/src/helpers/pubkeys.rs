use solana_pubkey::Pubkey;

pub fn pk(n: u8) -> Pubkey {
    let mut b = [0u8; 32];
    b[0] = n;
    Pubkey::new_from_array(b)
}

pub unsafe fn zeroed<T>() -> T {
    core::mem::zeroed()
}
