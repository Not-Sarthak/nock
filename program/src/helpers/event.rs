#[cfg(target_os = "solana")]
use core::mem::size_of;

#[cfg(target_os = "solana")]
extern "C" {
    fn sol_log_data(data: *const u8, data_len: u64);
}

#[inline(always)]
pub fn emit<T: Sized>(event: &T) {
    #[cfg(target_os = "solana")]
    unsafe {
        let bytes = core::slice::from_raw_parts(
            event as *const T as *const u8,
            size_of::<T>(),
        );
        let slices: [&[u8]; 1] = [bytes];
        sol_log_data(
            slices.as_ptr() as *const u8,
            slices.len() as u64,
        );
    }
    #[cfg(not(target_os = "solana"))]
    let _ = event;
}
