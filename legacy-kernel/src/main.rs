//! aarch64 virt entry for the legacy Ginux kernel line.
//!
//! The x86_64 UEFI kernel remains on `bootable-kernel` at
//! `bootable-kernel-final`. This binary is the qemu-system-aarch64 bring-up:
//! drop to EL1 if firmware left us in EL2, write the banner on the virt
//! PL011, and exit through semihosting so the boot test can terminate.

#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

global_asm!(
    ".section .text.boot",
    ".global _start",
    ".type _start, %function",
    "_start:",
    "mrs x0, CurrentEL",
    "cmp x0, #8",
    "b.ne 1f",
    "mov x0, #(1 << 31)",
    "msr hcr_el2, x0",
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

const UART_DR: *mut u32 = 0x0900_0000 as *mut u32;
const UART_FR: *const u32 = 0x0900_0018 as *const u32;
const UART_TXFF: u32 = 1 << 5;
const SEMIHOST_WRITE0: u64 = 0x04;
const SEMIHOST_EXIT: u64 = 0x18;
const ADP_STOPPED_APPLICATION_EXIT: u64 = 0x20026;

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    boot_exit(1)
}

#[no_mangle]
extern "C" fn kernel_main() -> ! {
    write_uart(b"ginux-legacy: aarch64 virt boot ok\n");
    write_semihost(b"ginux-legacy: aarch64 virt boot ok\n\0");
    boot_exit(0)
}

fn write_uart(bytes: &[u8]) {
    for byte in bytes {
        unsafe {
            while core::ptr::read_volatile(UART_FR) & UART_TXFF != 0 {}
            core::ptr::write_volatile(UART_DR, u32::from(*byte));
        }
    }
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
