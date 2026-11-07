//! aarch64 virt kernel for qemu-system-aarch64.
//!
//! The x86_64 UEFI kernel remains on `bootable-kernel`. This binary is the
//! EL1 kernel for the virt machine: drop from EL2, install exception vectors,
//! identity-map RAM and the virt MMIO devices, enable the MMU, and take one
//! ARM generic-timer interrupt through the GICv2 before exiting.

#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicU32, Ordering};

global_asm!(
    ".section .text.boot",
    ".global _start",
    ".type _start, %function",
    "_start:",
    "mrs x0, CurrentEL",
    "cmp x0, #8",
    "b.ne 1f",
    // EL1 is AArch64, EL1 may use the physical timer, and the virtual
    // counter offset is zero.
    "mov x0, #(1 << 31)",
    "msr hcr_el2, x0",
    "mrs x0, cnthctl_el2",
    "orr x0, x0, #3",
    "msr cnthctl_el2, x0",
    "msr cntvoff_el2, xzr",
    "mov x0, #0x3c5",
    "msr spsr_el2, x0",
    "adr x0, 1f",
    "msr elr_el2, x0",
    "eret",
    "1:",
    "ldr x1, =__bss_start",
    "ldr x2, =__bss_end",
    "2:",
    "cmp x1, x2",
    "b.hs 3f",
    "str xzr, [x1], #8",
    "b 2b",
    "3:",
    "ldr x0, =stack_top",
    "mov sp, x0",
    "bl kernel_main",
    "b ."
);

global_asm!(
    ".section .text.vectors",
    ".balign 2048",
    ".global vector_table",
    "vector_table:",
    "b sync_entry",
    ".balign 128",
    "b sync_entry",
    ".balign 128",
    "b sync_entry",
    ".balign 128",
    "b sync_entry",
    ".balign 128",
    "b sync_entry",
    ".balign 128",
    // Current EL, SP_ELx, IRQ. kernel_main runs in EL1h.
    "b irq_entry",
    ".balign 128",
    "b sync_entry",
    ".balign 128",
    "b sync_entry",
    ".balign 128",
    "b sync_entry",
    ".balign 128",
    "b sync_entry",
    ".balign 128",
    "b sync_entry",
    ".balign 128",
    "b sync_entry",
    ".balign 128",
    "b sync_entry",
    ".balign 128",
    "b sync_entry",
    ".balign 128",
    "b sync_entry",
    "sync_entry:",
    "bl rust_sync",
    "eret",
    "irq_entry:",
    "sub sp, sp, #256",
    "stp x0, x1, [sp, #0]",
    "stp x2, x3, [sp, #16]",
    "stp x4, x5, [sp, #32]",
    "stp x6, x7, [sp, #48]",
    "stp x8, x9, [sp, #64]",
    "stp x10, x11, [sp, #80]",
    "stp x12, x13, [sp, #96]",
    "stp x14, x15, [sp, #112]",
    "stp x16, x17, [sp, #128]",
    "stp x18, x19, [sp, #144]",
    "stp x20, x21, [sp, #160]",
    "stp x22, x23, [sp, #176]",
    "stp x24, x25, [sp, #192]",
    "stp x26, x27, [sp, #208]",
    "stp x28, x29, [sp, #224]",
    "str x30, [sp, #240]",
    "bl rust_irq",
    "ldr x30, [sp, #240]",
    "ldp x28, x29, [sp, #224]",
    "ldp x26, x27, [sp, #208]",
    "ldp x24, x25, [sp, #192]",
    "ldp x22, x23, [sp, #176]",
    "ldp x20, x21, [sp, #160]",
    "ldp x18, x19, [sp, #144]",
    "ldp x16, x17, [sp, #128]",
    "ldp x14, x15, [sp, #112]",
    "ldp x12, x13, [sp, #96]",
    "ldp x10, x11, [sp, #80]",
    "ldp x8, x9, [sp, #64]",
    "ldp x6, x7, [sp, #48]",
    "ldp x4, x5, [sp, #32]",
    "ldp x2, x3, [sp, #16]",
    "ldp x0, x1, [sp, #0]",
    "add sp, sp, #256",
    "eret"
);

const UART_DR: *mut u32 = 0x0900_0000 as *mut u32;
const UART_FR: *const u32 = 0x0900_0018 as *const u32;
const UART_TXFF: u32 = 1 << 5;
const SEMIHOST_WRITE0: u64 = 0x04;
const SEMIHOST_EXIT: u64 = 0x18;
const ADP_STOPPED_APPLICATION_EXIT: u64 = 0x20026;

const GICD: usize = 0x0800_0000;
const GICC: usize = 0x0801_0000;
const TIMER_IRQ: u32 = 30;

static TIMER_IRQS: AtomicU32 = AtomicU32::new(0);

#[repr(C, align(4096))]
struct PageTable([u64; 512]);

static mut L1: PageTable = PageTable([0; 512]);
static mut L2_DEVICES: PageTable = PageTable([0; 512]);

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    write_uart(b"ginux-legacy: panic\n");
    boot_exit(1)
}

#[no_mangle]
extern "C" fn kernel_main() -> ! {
    write_uart(b"ginux-legacy: aarch64 virt boot ok\n");
    unsafe {
        install_vectors();
        map_identity();
        enable_mmu();
    }
    write_uart(b"ginux-legacy: mmu=on\n");
    unsafe {
        enable_gic();
        arm_timer();
        asm!("msr daifclr, #2", options(nomem, nostack));
    }
    let mut spins = 0u32;
    while TIMER_IRQS.load(Ordering::Acquire) == 0 && spins < 50_000_000 {
        spins += 1;
        core::hint::spin_loop();
    }
    if TIMER_IRQS.load(Ordering::Acquire) == 0 {
        write_uart(b"ginux-legacy: timer-irq=missing\n");
        write_semihost(b"ginux-legacy: timer-irq=missing\n\0");
        boot_exit(1);
    }
    write_uart(b"ginux-legacy: timer-irq=1\n");
    write_semihost(b"ginux-legacy: aarch64 virt boot ok\n\0");
    write_semihost(b"ginux-legacy: mmu=on\n\0");
    write_semihost(b"ginux-legacy: timer-irq=1\n\0");
    boot_exit(0)
}

#[no_mangle]
extern "C" fn rust_irq() {
    unsafe {
        let iar = read32(GICC + 0x0c);
        let id = iar & 0x3ff;
        if id == TIMER_IRQ {
            asm!("msr cntp_ctl_el0, {v:x}", v = in(reg) 0u64, options(nomem, nostack));
            TIMER_IRQS.fetch_add(1, Ordering::Release);
        }
        if id < 1020 {
            write32(GICC + 0x10, iar);
        }
    }
}

#[no_mangle]
extern "C" fn rust_sync() -> ! {
    let esr: u64;
    let elr: u64;
    unsafe {
        asm!("mrs {v}, esr_el1", v = out(reg) esr, options(nomem, nostack));
        asm!("mrs {v}, elr_el1", v = out(reg) elr, options(nomem, nostack));
    }
    write_uart(b"ginux-legacy: sync esr=");
    write_hex(esr);
    write_uart(b" elr=");
    write_hex(elr);
    write_uart(b"\n");
    boot_exit(1)
}

unsafe fn install_vectors() {
    asm!(
        "adr {v}, vector_table",
        "msr vbar_el1, {v}",
        v = out(reg) _,
        options(nomem, nostack)
    );
}

unsafe fn map_identity() {
    let l2 = core::ptr::addr_of_mut!(L2_DEVICES) as u64;
    let l1 = core::ptr::addr_of_mut!(L1).cast::<u64>();
    // First gigabyte is a table of 2MB device blocks (GIC, PL011).
    l1.write(table_descriptor(l2));
    // RAM on the virt machine starts at 0x40000000. One normal 1GB block
    // covers the kernel, its stack, and the page tables.
    l1.add(1).write(block_descriptor(0x4000_0000, false));
    let devices = core::ptr::addr_of_mut!(L2_DEVICES).cast::<u64>();
    // 0x08000000 and 0x09000000 are distinct 2MB frames.
    devices
        .add(0x0800_0000 >> 21)
        .write(block_descriptor(0x0800_0000, true));
    devices
        .add(0x0900_0000 >> 21)
        .write(block_descriptor(0x0900_0000, true));
}

fn table_descriptor(next: u64) -> u64 {
    (next & 0x0000_ffff_ffff_f000) | 0b11
}

fn block_descriptor(phys: u64, device: bool) -> u64 {
    let attr = if device { 1u64 << 2 } else { 0 };
    let share = if device { 0 } else { 3u64 << 8 };
    let exec_never = if device {
        (1u64 << 53) | (1u64 << 54)
    } else {
        0
    };
    (phys & 0x0000_ffff_ffe0_0000) | attr | (1 << 5) | share | (1 << 10) | exec_never | 0b01
}

unsafe fn enable_mmu() {
    let l1 = core::ptr::addr_of!(L1) as u64;
    asm!(
        "msr mair_el1, {mair}",
        "msr tcr_el1, {tcr}",
        "msr ttbr0_el1, {ttbr}",
        "dsb sy",
        "tlbi vmalle1",
        "dsb sy",
        "isb",
        "mrs {sctlr}, sctlr_el1",
        "orr {sctlr}, {sctlr}, #1",
        "orr {sctlr}, {sctlr}, #4",
        "orr {sctlr}, {sctlr}, #(1 << 12)",
        "msr sctlr_el1, {sctlr}",
        "isb",
        mair = in(reg) 0xffu64,
        // 39-bit VA, 4KB granules, inner WB, outer WB, inner shareable, 40-bit PA.
        tcr = in(reg) (25u64 | (1 << 8) | (1 << 10) | (3 << 12) | (2 << 32)),
        ttbr = in(reg) l1,
        sctlr = out(reg) _,
        options(nostack)
    );
}

unsafe fn enable_gic() {
    write32(GICD + 0x100, 1 << TIMER_IRQ);
    core::ptr::write_volatile((GICD + 0x400 + TIMER_IRQ as usize) as *mut u8, 0xa0);
    write32(GICD, 1);
    write32(GICC + 0x04, 0xff);
    write32(GICC, 1);
}

unsafe fn arm_timer() {
    asm!(
        "msr cntp_tval_el0, {v}",
        "msr cntp_ctl_el0, {enable}",
        v = in(reg) 1000u64,
        enable = in(reg) 1u64,
        options(nomem, nostack)
    );
}

unsafe fn read32(addr: usize) -> u32 {
    core::ptr::read_volatile(addr as *const u32)
}

unsafe fn write32(addr: usize, value: u32) {
    core::ptr::write_volatile(addr as *mut u32, value);
}

fn write_uart(bytes: &[u8]) {
    for byte in bytes {
        unsafe {
            while core::ptr::read_volatile(UART_FR) & UART_TXFF != 0 {}
            core::ptr::write_volatile(UART_DR, u32::from(*byte));
        }
    }
}

fn write_hex(mut value: u64) {
    let mut buf = [0u8; 16];
    let mut index = buf.len();
    loop {
        index -= 1;
        let nibble = (value & 0xf) as u8;
        buf[index] = if nibble < 10 {
            b'0' + nibble
        } else {
            b'a' + (nibble - 10)
        };
        value >>= 4;
        if value == 0 {
            break;
        }
    }
    write_uart(&buf[index..]);
}

fn write_semihost(message: &[u8]) {
    unsafe {
        asm!(
            "hlt #0xf000",
            in("x0") SEMIHOST_WRITE0,
            in("x1") message.as_ptr(),
            options(nostack)
        );
    }
}

fn boot_exit(code: u32) -> ! {
    let args = [ADP_STOPPED_APPLICATION_EXIT, u64::from(code)];
    unsafe {
        asm!(
            "hlt #0xf000",
            in("x0") SEMIHOST_EXIT,
            in("x1") args.as_ptr(),
            options(noreturn)
        );
    }
}
