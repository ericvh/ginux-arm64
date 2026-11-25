use super::memory::{Frames, MapRange};

const TWO_MEGABYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Copy)]
struct Tables {
    l0: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct AddressSpace {
    tables: Tables,
}

pub(crate) fn install(frames: &mut Frames, ranges: &[MapRange]) -> Option<AddressSpace> {
    let l0 = frames.alloc()?;
    let space = AddressSpace {
        tables: Tables { l0 },
    };
    for range in ranges {
        map_range(frames, l0, range.start, range.end, range.device)?;
    }
    unsafe { activate(l0) };
    Some(space)
}

#[derive(Clone, Copy)]
pub(crate) enum UserPage {
    Read,
    Write,
    Execute,
}

/// Maps one user page at `va` to an already-allocated physical frame.
///
/// A 2 MiB kernel block already covering `va` is left in place. The user
/// window is outside the firmware map, so that entry starts empty.
pub(crate) fn map_user_page(
    space: &AddressSpace,
    frames: &mut Frames,
    va: u64,
    pa: u64,
    access: UserPage,
) -> Option<()> {
    if va & 4095 != 0 || pa & 4095 != 0 {
        return None;
    }
    let l2 = l2_table(frames, space.tables.l0, va)?;
    let l2_index = ((va >> 21) & 511) as usize;
    let existing = unsafe { l2.add(l2_index).read_volatile() };
    let l3 = if existing & 0x3 == 0x3 {
        existing & 0x0000_ffff_ffff_f000
    } else if existing & 1 == 0 {
        let allocated = frames.alloc()?;
        unsafe { l2.add(l2_index).write_volatile(table_desc(allocated)) };
        allocated
    } else {
        return None;
    };
    let (ap, uxn) = match access {
        UserPage::Read => (3, true),
        UserPage::Write => (1, true),
        UserPage::Execute => (3, false),
    };
    let page = ((va >> 12) & 511) as usize;
    unsafe {
        (l3 as *mut u64)
            .add(page)
            .write_volatile(page_desc(pa, 0, ap, uxn, true));
        asm_barrier();
    }
    Some(())
}

pub(crate) fn map_device(
    space: &AddressSpace,
    frames: &mut Frames,
    start: u64,
    length: u64,
) -> Option<()> {
    let end = start.checked_add(length)?;
    map_range(frames, space.tables.l0, start, end, true)?;
    unsafe { asm_barrier() };
    Some(())
}

fn map_range(frames: &mut Frames, l0: u64, start: u64, end: u64, device: bool) -> Option<()> {
    if start >= end {
        return Some(());
    }
    let mut address = start & !(TWO_MEGABYTES - 1);
    let end = end.checked_add(TWO_MEGABYTES - 1)? & !(TWO_MEGABYTES - 1);
    while address < end {
        let l2 = l2_table(frames, l0, address)?;
        let l2_index = ((address >> 21) & 511) as usize;
        let desc = if device {
            block_desc(address, 1, false)
        } else {
            block_desc(address, 0, true)
        };
        unsafe { l2.add(l2_index).write_volatile(desc) };
        address += TWO_MEGABYTES;
    }
    Some(())
}

fn l2_table(frames: &mut Frames, l0: u64, va: u64) -> Option<*mut u64> {
    let l0_index = (va >> 39) as usize;
    if l0_index >= 512 {
        return None;
    }
    let l0_ptr = l0 as *mut u64;
    let mut l1 = unsafe { l0_ptr.add(l0_index).read_volatile() };
    if l1 & 1 == 0 {
        let allocated = frames.alloc()?;
        l1 = table_desc(allocated);
        unsafe { l0_ptr.add(l0_index).write_volatile(l1) };
    }
    let l1_ptr = (l1 & 0x0000_ffff_ffff_f000) as *mut u64;
    let l1_index = ((va >> 30) & 511) as usize;
    let mut l2 = unsafe { l1_ptr.add(l1_index).read_volatile() };
    if l2 & 1 == 0 {
        let allocated = frames.alloc()?;
        l2 = table_desc(allocated);
        unsafe { l1_ptr.add(l1_index).write_volatile(l2) };
    }
    Some((l2 & 0x0000_ffff_ffff_f000) as *mut u64)
}

fn table_desc(next: u64) -> u64 {
    (next & 0x0000_ffff_ffff_f000) | 0x3
}

fn block_desc(pa: u64, attr: u64, executable: bool) -> u64 {
    let mut desc = 0x1 | (attr << 2) | (1 << 10) | (1 << 54) | (pa & 0x0000_ffff_ffe0_0000);
    if attr == 0 {
        desc |= 3 << 8;
    }
    if !executable {
        desc |= 1 << 53;
    }
    desc
}

fn page_desc(pa: u64, attr: u64, ap: u64, uxn: bool, pxn: bool) -> u64 {
    let mut desc = 0x3 | (attr << 2) | (1 << 10) | ((ap & 3) << 6) | (3 << 8);
    desc |= pa & 0x0000_ffff_ffff_f000;
    if uxn {
        desc |= 1 << 54;
    }
    if pxn {
        desc |= 1 << 53;
    }
    desc
}

unsafe fn activate(l0: u64) {
    let tcr = 16u64 | (1 << 8) | (1 << 10) | (3 << 12) | (1 << 23) | (5 << 32);
    let mut sctlr = 0u64;
    core::arch::asm!(
        "mrs {sctlr}, sctlr_el1",
        "bic {sctlr}, {sctlr}, #1",
        "msr sctlr_el1, {sctlr}",
        "isb",
        "msr mair_el1, {mair}",
        "msr tcr_el1, {tcr}",
        "msr ttbr0_el1, {ttbr}",
        "tlbi vmalle1",
        "dsb ish",
        "isb",
        "mrs {sctlr}, sctlr_el1",
        "orr {sctlr}, {sctlr}, #1",
        "orr {sctlr}, {sctlr}, #4",
        "orr {sctlr}, {sctlr}, #0x1000",
        "bic {sctlr}, {sctlr}, #0x80000",
        "msr sctlr_el1, {sctlr}",
        "isb",
        sctlr = inout(reg) sctlr,
        mair = in(reg) 0xffu64,
        tcr = in(reg) tcr,
        ttbr = in(reg) l0,
        options(nostack),
    );
}

unsafe fn asm_barrier() {
    core::arch::asm!(
        "dsb ish",
        "tlbi vmalle1",
        "dsb ish",
        "isb",
        options(nostack)
    );
}
