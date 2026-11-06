//! aarch64 virt kernel for qemu-system-aarch64.
//!
//! The x86_64 UEFI kernel remains on `bootable-kernel`. This binary is the
//! EL1 kernel for the virt machine: drop from EL2, install exception vectors,
//! identity-map RAM and the virt MMIO devices, enable the MMU, and take one
//! ARM generic-timer interrupt through the GICv2. It then starts an EL0
//! process. That process enters with `svc #0`, and the kernel dispatches
//! the number in `x8`.

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
    // Lower EL, AArch64, synchronous. EL0 svc lands here.
    "b svc_entry",
    ".balign 128",
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
    "eret",
    "svc_entry:",
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
    "mov x0, sp",
    "bl rust_svc",
    "cbz x0, process_done",
    "msr elr_el1, x0",
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
    "eret",
    "process_done:",
    "bl rust_process_done",
    "b ."
);

global_asm!(
    ".section .usertext, \"ax\"",
    ".global user_process",
    "user_process:",
    "mov x8, #64",
    "adr x0, user_msg",
    "mov x1, #(user_msg_end - user_msg)",
    "svc #0",
    "mov x8, #93",
    "mov x0, #0",
    "svc #0",
    "b .",
    "user_msg:",
    ".ascii \"ginux-legacy: process-write ok\\n\"",
    "user_msg_end:"
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
static mut L2_RAM: PageTable = PageTable([0; 512]);

unsafe extern "C" {
    static user_process: u8;
    static user_stack_top: u8;
}

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
    unsafe {
        asm!("msr daifset, #2", options(nomem, nostack));
        start_process();
    }
}

unsafe fn start_process() -> ! {
    write_uart(b"ginux-legacy: process=started\n");
    let pc = core::ptr::addr_of!(user_process) as u64;
    let sp = core::ptr::addr_of!(user_stack_top) as u64;
    asm!(
        "msr sp_el0, {sp}",
        "msr elr_el1, {pc}",
        "msr spsr_el1, {spsr}",
        "eret",
        sp = in(reg) sp,
        pc = in(reg) pc,
        // EL0t with DAIF masked. The process reaches the kernel through svc.
        spsr = in(reg) 0x3c0u64,
        options(noreturn)
    );
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
extern "C" fn rust_svc(frame: *mut u64) -> u64 {
    let esr: u64;
    let elr: u64;
    unsafe {
        asm!("mrs {v}, esr_el1", v = out(reg) esr, options(nomem, nostack));
        asm!("mrs {v}, elr_el1", v = out(reg) elr, options(nomem, nostack));
    }
    let class = (esr >> 26) & 0x3f;
    if class != 0x15 || esr & 0xffff != 0 {
        write_uart(b"ginux-legacy: svc=not-svc esr=");
        write_hex(esr);
        write_uart(b"\n");
        boot_exit(1);
    }
    let number = unsafe { frame.add(8).read() };
    match number {
        64 => {
            write_uart(b"ginux-legacy: svc=dispatch nr=64\n");
            let ptr = unsafe { frame.read() };
            let len = unsafe { frame.add(1).read() };
            let Some(bytes) = user_bytes(ptr, len) else {
                write_uart(b"ginux-legacy: svc=bad-pointer\n");
                boot_exit(1);
            };
            write_uart(bytes);
            unsafe { frame.write(len) };
            // SVC records the next instruction in ELR, unlike a breakpoint.
            elr
        }
        93 => {
            write_uart(b"ginux-legacy: svc=dispatch nr=93\n");
            0
        }
        _ => {
            write_uart(b"ginux-legacy: svc=unknown nr=");
            write_hex(number);
            write_uart(b"\n");
            unsafe { frame.write((-38i64) as u64) };
            elr
        }
    }
}

fn user_bytes(ptr: u64, len: u64) -> Option<&'static [u8]> {
    if len == 0 || len > 128 {
        return None;
    }
    let end = ptr.checked_add(len)?;
    let text = ptr >= 0x4020_0000 && end <= 0x4040_0000;
    let stack = ptr >= 0x4040_0000 && end <= 0x4060_0000;
    if !text && !stack {
        return None;
    }
    Some(unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) })
}

#[no_mangle]
extern "C" fn rust_process_done() -> ! {
    write_uart(b"ginux-legacy: process=exited\n");
    write_semihost(b"ginux-legacy: aarch64 virt boot ok\n\0");
    write_semihost(b"ginux-legacy: mmu=on\n\0");
    write_semihost(b"ginux-legacy: timer-irq=1\n\0");
    write_semihost(b"ginux-legacy: process=started\n\0");
    write_semihost(b"ginux-legacy: svc=dispatch nr=64\n\0");
    write_semihost(b"ginux-legacy: svc=dispatch nr=93\n\0");
    write_semihost(b"ginux-legacy: process=exited\n\0");
    boot_exit(0)
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
    let devices = core::ptr::addr_of_mut!(L2_DEVICES) as u64;
    let ram = core::ptr::addr_of_mut!(L2_RAM) as u64;
    let l1 = core::ptr::addr_of_mut!(L1).cast::<u64>();
    l1.write(table_descriptor(devices));
    // RAM starts at 0x40000000. Split it into 2MB blocks so EL0 can execute
    // only its own text and can write only its own stack.
    l1.add(1).write(table_descriptor(ram));
    let devices = core::ptr::addr_of_mut!(L2_DEVICES).cast::<u64>();
    devices
        .add(0x0800_0000 >> 21)
        .write(block_descriptor(0x0800_0000, Map::Device));
    devices
        .add(0x0900_0000 >> 21)
        .write(block_descriptor(0x0900_0000, Map::Device));
    let ram = core::ptr::addr_of_mut!(L2_RAM).cast::<u64>();
    ram.write(block_descriptor(0x4000_0000, Map::Kernel));
    ram.add(1)
        .write(block_descriptor(0x4020_0000, Map::UserCode));
    ram.add(2)
        .write(block_descriptor(0x4040_0000, Map::UserData));
}

enum Map {
    Device,
    Kernel,
    UserCode,
    UserData,
}

fn table_descriptor(next: u64) -> u64 {
    (next & 0x0000_ffff_ffff_f000) | 0b11
}

fn block_descriptor(phys: u64, map: Map) -> u64 {
    let (attr, share, access, exec_never) = match map {
        // EL1 only. Device memory is never executable.
        Map::Device => (1u64 << 2, 0, 0, (1u64 << 53) | (1u64 << 54)),
        // EL1 code and data. EL0 has no access.
        Map::Kernel => (0, 3u64 << 8, 0, 1u64 << 54),
        // EL0 may read and execute. EL1 may read the process text.
        Map::UserCode => (0, 3u64 << 8, 3u64 << 6, 1u64 << 53),
        // EL0 stack. Neither side executes it.
        Map::UserData => (0, 3u64 << 8, 1u64 << 6, (1u64 << 53) | (1u64 << 54)),
    };
    (phys & 0x0000_ffff_ffe0_0000)
        | attr
        | (1 << 5)
        | access
        | share
        | (1 << 10)
        | exec_never
        | 0b01
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
