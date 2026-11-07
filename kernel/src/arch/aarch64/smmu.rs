pub(crate) fn idr0(base: u64) -> Option<u32> {
    if base == 0 {
        return None;
    }
    Some(unsafe { (base as *const u32).read_volatile() })
}

/// Lets PCI DMA through while translation is off.
///
/// SMMUv3 reset often aborts transactions until software clears `GBPA.ABORT`.
pub(crate) fn bypass(base: u64) -> bool {
    if base == 0 {
        return false;
    }
    unsafe {
        let gbpa = (base + 0x44) as *mut u32;
        gbpa.write_volatile(1 << 31);
        for _ in 0..100_000 {
            if gbpa.read_volatile() & (1 << 31) == 0 {
                return true;
            }
            core::hint::spin_loop();
        }
    }
    false
}
