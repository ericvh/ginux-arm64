pub(crate) fn idr0(base: u64) -> Option<u32> {
    if base == 0 {
        return None;
    }
    Some(unsafe { (base as *const u32).read_volatile() })
}
