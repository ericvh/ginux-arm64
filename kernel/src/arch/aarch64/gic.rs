const GICD_CTLR: usize = 0x0000;
const GICR_WAKER: usize = 0x0014;
const GICR_TYPER: usize = 0x0008;
const GICR_IGROUPR0: usize = 0x0080;
const GICR_ISENABLER0: usize = 0x0100;

pub(crate) fn init(gicd: u64, gicr: u64, gicr_length: u64) -> Option<u64> {
    let rd = find_redistributor(gicr, gicr_length)?;
    let sgi = rd + 0x1_0000;
    unsafe {
        let waker = sgi_read(rd, GICR_WAKER);
        sgi_write(rd, GICR_WAKER, waker & !(1 << 1));
        let mut spins = 0;
        while sgi_read(rd, GICR_WAKER) & (1 << 2) != 0 {
            spins += 1;
            if spins > 100_000 {
                return None;
            }
            core::hint::spin_loop();
        }
        reg_write(sgi, GICR_IGROUPR0, 0xffff_ffff);
        let ctlr = reg_read(gicd, GICD_CTLR);
        let enable = if ctlr & (1 << 6) != 0 {
            ctlr | (1 << 1) | (1 << 4)
        } else {
            ctlr | (1 << 1) | (1 << 5)
        };
        reg_write(gicd, GICD_CTLR, enable);
        core::arch::asm!(
            "msr ICC_SRE_EL1, {sre}",
            "isb",
            "msr ICC_PMR_EL1, {pmr}",
            "msr ICC_IGRPEN1_EL1, {enable}",
            sre = in(reg) 1u64,
            pmr = in(reg) 0xf0u64,
            enable = in(reg) 1u64,
            options(nostack),
        );
    }
    Some(rd)
}

pub(crate) fn enable_ppi(rd: u64, intid: u32) {
    if intid >= 32 {
        return;
    }
    let sgi = rd + 0x1_0000;
    unsafe {
        reg_write(sgi, GICR_ISENABLER0, 1 << intid);
    }
}

fn find_redistributor(base: u64, length: u64) -> Option<u64> {
    let mpidr = current_affinity();
    let window = if length == 0 { 0x20_0000 } else { length };
    let mut address = base;
    let end = base.checked_add(window)?;
    while address + 0x2_0000 <= end {
        let typer = unsafe { (address as *const u64).add(GICR_TYPER / 8).read_volatile() };
        let affinity = typer >> 32;
        if affinity == mpidr {
            return Some(address);
        }
        if typer & (1 << 4) != 0 {
            break;
        }
        address += 0x2_0000;
    }
    (base != 0).then_some(base)
}

fn current_affinity() -> u64 {
    let mpidr: u64;
    unsafe {
        core::arch::asm!("mrs {v}, mpidr_el1", v = out(reg) mpidr, options(nostack, nomem));
    }
    let aff0 = mpidr & 0xff;
    let aff1 = (mpidr >> 8) & 0xff;
    let aff2 = (mpidr >> 16) & 0xff;
    let aff3 = (mpidr >> 32) & 0xff;
    aff0 | (aff1 << 8) | (aff2 << 16) | (aff3 << 24)
}

unsafe fn reg_read(base: u64, offset: usize) -> u32 {
    (base as *const u32).add(offset / 4).read_volatile()
}

unsafe fn reg_write(base: u64, offset: usize, value: u32) {
    (base as *mut u32).add(offset / 4).write_volatile(value);
}

unsafe fn sgi_read(rd: u64, offset: usize) -> u32 {
    reg_read(rd, offset)
}

unsafe fn sgi_write(rd: u64, offset: usize, value: u32) {
    reg_write(rd, offset, value);
}
