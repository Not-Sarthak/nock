use core::ptr;

#[inline(always)]
pub unsafe fn pubkey_eq_32(a: *const u8, b: *const u8) -> bool {
    let a = a as *const u64;
    let b = b as *const u64;
    ptr::read_unaligned(a)         == ptr::read_unaligned(b)
        && ptr::read_unaligned(a.add(1)) == ptr::read_unaligned(b.add(1))
        && ptr::read_unaligned(a.add(2)) == ptr::read_unaligned(b.add(2))
        && ptr::read_unaligned(a.add(3)) == ptr::read_unaligned(b.add(3))
}

#[inline(always)]
pub unsafe fn pubkey_copy_32(src: *const u8, dst: *mut u8) {
    let s = src as *const u64;
    let d = dst as *mut u64;
    ptr::write_unaligned(d,         ptr::read_unaligned(s));
    ptr::write_unaligned(d.add(1),  ptr::read_unaligned(s.add(1)));
    ptr::write_unaligned(d.add(2),  ptr::read_unaligned(s.add(2)));
    ptr::write_unaligned(d.add(3),  ptr::read_unaligned(s.add(3)));
}

#[inline(always)]
pub unsafe fn tag_eq_8(a: *const u8, b_imm: u64) -> bool {
    ptr::read_unaligned(a as *const u64) == b_imm
}

#[inline(always)]
pub fn eq32(a: &[u8; 32], b: &[u8; 32]) -> bool {
    unsafe { pubkey_eq_32(a.as_ptr(), b.as_ptr()) }
}

#[inline(always)]
pub fn eq_addr(addr_bytes: &[u8], stored: &[u8; 32]) -> bool {
    debug_assert_eq!(addr_bytes.len(), 32);
    unsafe { pubkey_eq_32(addr_bytes.as_ptr(), stored.as_ptr()) }
}
