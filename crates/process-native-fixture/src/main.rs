#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

#[cfg(target_arch = "x86_64")]
global_asm!(
    ".global _start",
    ".type _start,@function",
    "_start:",
    "xor %rbp, %rbp",
    "andq $-16, %rsp",
    "call fixture_main",
    "ud2",
    options(att_syntax)
);

#[cfg(target_arch = "aarch64")]
global_asm!(
    ".global _start",
    ".type _start, %function",
    "_start:",
    "mov x0, sp",
    "bic sp, x0, #15",
    "bl fixture_main",
    "brk #1",
);

#[cfg(target_arch = "x86_64")]
const SYS_WRITE: usize = 1;
#[cfg(target_arch = "x86_64")]
const SYS_GETPID: usize = 39;
#[cfg(target_arch = "x86_64")]
const SYS_FORK: usize = 57;
#[cfg(target_arch = "x86_64")]
const SYS_EXIT: usize = 60;
#[cfg(target_arch = "x86_64")]
const SYS_WAIT4: usize = 61;
#[cfg(target_arch = "x86_64")]
const SYS_GETPPID: usize = 110;

#[cfg(target_arch = "aarch64")]
const SYS_WRITE: usize = 64;
#[cfg(target_arch = "aarch64")]
const SYS_EXIT: usize = 93;
#[cfg(target_arch = "aarch64")]
const SYS_GETPID: usize = 172;
#[cfg(target_arch = "aarch64")]
const SYS_GETPPID: usize = 173;
#[cfg(target_arch = "aarch64")]
const SYS_CLONE: usize = 220;
#[cfg(target_arch = "aarch64")]
const SYS_WAIT4: usize = 260;
#[cfg(target_arch = "aarch64")]
const SIGCHLD: usize = 17;

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    exit(99)
}

#[no_mangle]
extern "C" fn fixture_main() -> ! {
    let initial_pid = unsafe { syscall0(SYS_GETPID) };
    if initial_pid != 1 {
        exit(10);
    }
    let mut expected_pid = 2;
    let mut round = 0;
    while round < 16 {
        let first = fork_child(expected_pid);
        expected_pid += 1;
        let second = fork_child(expected_pid);
        expected_pid += 1;
        wait_child(first);
        wait_child(second);
        round += 1;
    }
    write(b"GINUX_NATIVE_PARENT: PASS\n");
    exit(0)
}

fn fork_child(expected_pid: isize) -> isize {
    let child = host_fork();
    if child < 0 {
        exit(11);
    }
    if child == 0 {
        let pid = unsafe { syscall0(SYS_GETPID) };
        let parent = unsafe { syscall0(SYS_GETPPID) };
        if pid != expected_pid {
            exit(12);
        }
        if parent != 1 {
            exit(13);
        }
        exit(23);
    }
    if child != expected_pid {
        exit(15);
    }
    child
}

fn wait_child(child: isize) {
    let mut status = 0i32;
    let waited = unsafe {
        syscall4(
            SYS_WAIT4,
            child as usize,
            &mut status as *mut _ as usize,
            0,
            0,
        )
    };
    if waited != child {
        exit(16);
    }
}

fn write(bytes: &[u8]) {
    let result = unsafe { syscall3(SYS_WRITE, 1, bytes.as_ptr() as usize, bytes.len()) };
    if result != bytes.len() as isize {
        exit(14);
    }
}

fn exit(code: usize) -> ! {
    unsafe { syscall1(SYS_EXIT, code) };
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(target_arch = "x86_64")]
fn host_fork() -> isize {
    unsafe { syscall0(SYS_FORK) }
}

#[cfg(target_arch = "aarch64")]
fn host_fork() -> isize {
    // aarch64 has no fork entry. clone(SIGCHLD) is the fork shape.
    unsafe { syscall5(SYS_CLONE, SIGCHLD, 0, 0, 0, 0) }
}

#[cfg(target_arch = "x86_64")]
unsafe fn syscall0(number: usize) -> isize {
    let result: isize;
    asm!("syscall", inlateout("rax") number as isize => result, lateout("rcx") _, lateout("r11") _, options(nostack));
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn syscall0(number: usize) -> isize {
    let result: isize;
    asm!("svc #0", in("x8") number, lateout("x0") result, options(nostack));
    result
}

#[cfg(target_arch = "x86_64")]
unsafe fn syscall1(number: usize, a1: usize) -> isize {
    let result: isize;
    asm!("syscall", inlateout("rax") number as isize => result, in("rdi") a1, lateout("rcx") _, lateout("r11") _, options(nostack));
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn syscall1(number: usize, a1: usize) -> isize {
    let result: isize;
    asm!("svc #0", in("x8") number, inlateout("x0") a1 => result, options(nostack));
    result
}

#[cfg(target_arch = "x86_64")]
unsafe fn syscall3(number: usize, a1: usize, a2: usize, a3: usize) -> isize {
    let result: isize;
    asm!("syscall", inlateout("rax") number as isize => result, in("rdi") a1, in("rsi") a2, in("rdx") a3, lateout("rcx") _, lateout("r11") _, options(nostack));
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn syscall3(number: usize, a1: usize, a2: usize, a3: usize) -> isize {
    let result: isize;
    asm!(
        "svc #0",
        in("x8") number,
        inlateout("x0") a1 => result,
        in("x1") a2,
        in("x2") a3,
        options(nostack)
    );
    result
}

#[cfg(target_arch = "x86_64")]
unsafe fn syscall4(number: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> isize {
    let result: isize;
    asm!("syscall", inlateout("rax") number as isize => result, in("rdi") a1, in("rsi") a2, in("rdx") a3, in("r10") a4, lateout("rcx") _, lateout("r11") _, options(nostack));
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn syscall4(number: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> isize {
    let result: isize;
    asm!(
        "svc #0",
        in("x8") number,
        inlateout("x0") a1 => result,
        in("x1") a2,
        in("x2") a3,
        in("x3") a4,
        options(nostack)
    );
    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn syscall5(number: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize) -> isize {
    let result: isize;
    asm!(
        "svc #0",
        in("x8") number,
        inlateout("x0") a1 => result,
        in("x1") a2,
        in("x2") a3,
        in("x3") a4,
        in("x4") a5,
        options(nostack)
    );
    result
}
