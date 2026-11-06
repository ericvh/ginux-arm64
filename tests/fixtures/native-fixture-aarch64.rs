//! aarch64 tracee for `ginux native`.
//!
//! Reads `/scenario` and issues aarch64 syscalls. `fork` is `clone(SIGCHLD)`
//! because aarch64 has no fork entry. `trap` is `brk #0`.
#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

global_asm!(
    r#"
    .section .text
    .balign 4
    .global _start
    _start:
        mov x0, sp
        bic sp, x0, #15
        bl rust_main
    1:
        wfi
        b 1b
    "#
);

const SYS_WRITE: usize = 64;
const SYS_EXIT_GROUP: usize = 94;
const SYS_FUTEX: usize = 98;
const SYS_SET_TID_ADDRESS: usize = 96;
const SYS_UNAME: usize = 160;
const SYS_GETPID: usize = 172;
const SYS_GETTID: usize = 178;
const SYS_CLONE: usize = 220;
const SYS_OPENAT: usize = 56;
const SYS_READ: usize = 63;
const AT_FDCWD: usize = (-100isize) as usize;
const FUTEX_LOCK_PI: usize = 6;
const SIGCHLD: usize = 17;

unsafe fn syscall0(n: usize) -> isize {
    let r: isize;
    asm!("svc #0", in("x8") n, lateout("x0") r, options(nostack));
    r
}

unsafe fn syscall1(n: usize, a: usize) -> isize {
    let r: isize;
    asm!("svc #0", in("x8") n, inlateout("x0") a => r, options(nostack));
    r
}

unsafe fn syscall3(n: usize, a: usize, b: usize, c: usize) -> isize {
    let r: isize;
    asm!("svc #0", in("x8") n, inlateout("x0") a => r, in("x1") b, in("x2") c, options(nostack));
    r
}

unsafe fn syscall4(n: usize, a: usize, b: usize, c: usize, d: usize) -> isize {
    let r: isize;
    asm!(
        "svc #0",
        in("x8") n,
        inlateout("x0") a => r,
        in("x1") b,
        in("x2") c,
        in("x3") d,
        options(nostack)
    );
    r
}

unsafe fn syscall5(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> isize {
    let r: isize;
    asm!(
        "svc #0",
        in("x8") n,
        inlateout("x0") a => r,
        in("x1") b,
        in("x2") c,
        in("x3") d,
        in("x4") e,
        options(nostack)
    );
    r
}

struct Line {
    buf: [u8; 160],
    len: usize,
}

impl Line {
    fn new() -> Self {
        Self {
            buf: [0; 160],
            len: 0,
        }
    }

    fn str(&mut self, text: &str) {
        for byte in text.as_bytes() {
            self.buf[self.len] = *byte;
            self.len += 1;
        }
    }

    fn dec(&mut self, mut n: isize) {
        if n < 0 {
            self.str("-");
            n = -n;
        }
        let mut tmp = [0u8; 20];
        let mut i = tmp.len();
        loop {
            i -= 1;
            tmp[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        while i < tmp.len() {
            self.buf[self.len] = tmp[i];
            self.len += 1;
            i += 1;
        }
    }

    fn write(&self) {
        unsafe { syscall3(SYS_WRITE, 1, self.buf.as_ptr() as usize, self.len); }
    }
}

fn write_str(s: &str) {
    let mut line = Line::new();
    line.str(s);
    line.write();
}

fn exit(code: usize) -> ! {
    unsafe { syscall1(SYS_EXIT_GROUP, code); }
    loop {}
}

fn starts(buf: &[u8], n: isize, prefix: &[u8]) -> bool {
    n >= prefix.len() as isize && buf[..prefix.len()] == *prefix
}

static mut TID: u32 = 0;
static mut PI: u32 = 0;
static mut UNAME: [u8; 390] = [0; 390];

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    let path = b"/scenario\0";
    let fd = unsafe { syscall4(SYS_OPENAT, AT_FDCWD, path.as_ptr() as usize, 0, 0) };
    if fd < 0 {
        let mut line = Line::new();
        line.str("GINUX_FIXTURE: open=");
        line.dec(fd);
        line.str("\n");
        line.write();
        exit(1);
    }
    let mut scenario = [0u8; 32];
    let n = unsafe { syscall3(SYS_READ, fd as usize, scenario.as_mut_ptr() as usize, scenario.len()) };
    if starts(&scenario, n, b"fork") {
        let child = unsafe { syscall5(SYS_CLONE, SIGCHLD, 0, 0, 0, 0) };
        if child == 0 {
            write_str("GINUX_FIXTURE: unsupervised-child\n");
            exit(2);
        }
        let mut line = Line::new();
        line.str("GINUX_FIXTURE: clone=");
        line.dec(child);
        line.str("\n");
        line.write();
        exit(0);
    }
    if starts(&scenario, n, b"trap") {
        let before = unsafe { syscall1(SYS_UNAME, core::ptr::addr_of_mut!(UNAME) as usize) };
        let mut line = Line::new();
        line.str("GINUX_FIXTURE: uname-before=");
        line.dec(before);
        line.str("\n");
        line.write();
        unsafe { asm!("brk #0", options(nomem, nostack)) };
        let after = unsafe { syscall1(SYS_UNAME, core::ptr::addr_of_mut!(UNAME) as usize) };
        let mut line = Line::new();
        line.str("GINUX_FIXTURE: uname-after=");
        line.dec(after);
        line.str("\n");
        line.write();
        exit(0);
    }
    if starts(&scenario, n, b"virtual-tid") {
        let tid = unsafe { syscall1(SYS_SET_TID_ADDRESS, core::ptr::addr_of_mut!(TID) as usize) };
        let pid = unsafe { syscall0(SYS_GETPID) };
        let gettid = unsafe { syscall0(SYS_GETTID) };
        let lock = unsafe {
            syscall4(
                SYS_FUTEX,
                core::ptr::addr_of_mut!(PI) as usize,
                FUTEX_LOCK_PI,
                0,
                0,
            )
        };
        let word = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(PI)) };
        let mut line = Line::new();
        line.str("GINUX_FIXTURE: set_tid_address=");
        line.dec(tid);
        line.str(" getpid=");
        line.dec(pid);
        line.str(" gettid=");
        line.dec(gettid);
        line.str(" futex_lock_pi=");
        line.dec(lock);
        line.str(" pi_word=");
        line.dec(word as isize);
        line.str("\n");
        line.write();
        exit(0);
    }
    write_str("GINUX_FIXTURE: unknown-scenario\n");
    exit(1);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    write_str("GINUX_FIXTURE: panic\n");
    exit(1);
}

#[no_mangle]
extern "C" fn rust_eh_personality() {}
