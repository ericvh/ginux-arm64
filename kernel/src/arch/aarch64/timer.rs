use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

static SEEN: AtomicBool = AtomicBool::new(false);
static SEEN_INTID: AtomicU32 = AtomicU32::new(1023);

pub(crate) fn frequency() -> u64 {
    let hz: u64;
    unsafe {
        core::arch::asm!("mrs {v}, cntfrq_el0", v = out(reg) hz, options(nostack, nomem));
    }
    hz
}

pub(crate) fn arm_one_shot(ticks: u64) {
    SEEN.store(false, Ordering::Relaxed);
    unsafe {
        core::arch::asm!(
            "msr cntp_tval_el0, {ticks}",
            "msr cntp_ctl_el0, {ctl}",
            ticks = in(reg) ticks,
            ctl = in(reg) 1u64,
            options(nostack),
        );
    }
}

pub(crate) fn disable() {
    unsafe {
        core::arch::asm!("msr cntp_ctl_el0, {ctl}", ctl = in(reg) 0u64, options(nostack));
    }
}

pub(crate) fn note_interrupt(intid: u32) {
    SEEN_INTID.store(intid, Ordering::Relaxed);
    SEEN.store(true, Ordering::Release);
    disable();
}

pub(crate) fn consumed() -> Option<u32> {
    SEEN.load(Ordering::Acquire).then(|| SEEN_INTID.load(Ordering::Relaxed))
}
