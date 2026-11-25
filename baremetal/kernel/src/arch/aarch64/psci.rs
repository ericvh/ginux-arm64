pub(crate) struct Version {
    pub(crate) major: u16,
    pub(crate) minor: u16,
    pub(crate) hvc: bool,
}

pub(crate) fn version(prefer_hvc: bool) -> Option<Version> {
    for hvc in [prefer_hvc, !prefer_hvc] {
        let result = call(hvc, 0x8400_0000, 0, 0, 0);
        if result >= 0 {
            let version = result as u32;
            return Some(Version {
                major: (version >> 16) as u16,
                minor: version as u16,
                hvc,
            });
        }
    }
    None
}

pub(crate) fn cpu_on(hvc: bool, mpidr: u64, entry: u64) -> bool {
    call(hvc, 0xc400_0003, mpidr, entry, 0) == 0
}

fn call(hvc: bool, function: u64, a1: u64, a2: u64, a3: u64) -> i64 {
    let result: u64;
    unsafe {
        if hvc {
            core::arch::asm!(
                "hvc #0",
                inlateout("x0") function => result,
                in("x1") a1,
                in("x2") a2,
                in("x3") a3,
                options(nostack),
            );
        } else {
            core::arch::asm!(
                "smc #0",
                inlateout("x0") function => result,
                in("x1") a1,
                in("x2") a2,
                in("x3") a3,
                options(nostack),
            );
        }
    }
    result as i64
}
