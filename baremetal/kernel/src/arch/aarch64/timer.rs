use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

static SEEN: AtomicBool = AtomicBool::new(false);
static SEEN_INTID: AtomicU32 = AtomicU32::new(1023);
static BOOT_UNIX: AtomicU64 = AtomicU64::new(0);
static BOOT_TICKS: AtomicU64 = AtomicU64::new(0);

pub(crate) fn capture_realtime(unix_time: u64) {
    BOOT_TICKS.store(counter(), Ordering::Relaxed);
    BOOT_UNIX.store(unix_time, Ordering::Relaxed);
}

pub(crate) fn counter() -> u64 {
    let ticks: u64;
    unsafe {
        core::arch::asm!("mrs {v}, cntpct_el0", v = out(reg) ticks, options(nostack, nomem));
    }
    ticks
}

pub(crate) fn timespec(clock: u64) -> Option<(u64, u64)> {
    let freq = frequency();
    if freq == 0 {
        return None;
    }
    let delta = counter().saturating_sub(BOOT_TICKS.load(Ordering::Relaxed));
    let seconds = delta / freq;
    let nanos = ((delta % freq) * 1_000_000_000) / freq;
    match clock {
        0 => {
            let unix = BOOT_UNIX.load(Ordering::Relaxed);
            if unix == 0 {
                return None;
            }
            Some((unix.saturating_add(seconds), nanos))
        }
        1 => Some((seconds, nanos)),
        _ => None,
    }
}

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
    SEEN.load(Ordering::Acquire)
        .then(|| SEEN_INTID.load(Ordering::Relaxed))
}
