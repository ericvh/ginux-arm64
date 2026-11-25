//! aarch64 tracee for `ginux native`.
//!
//! Reads `/scenario` and issues aarch64 syscalls. `fork` is `clone(SIGCHLD)`
//! because aarch64 has no fork entry. `trap` is `brk #0`.
//!
//! `clone3` and `close-range` run under `ginux native ... configured`. The
//! supervisor answers each with a sequence of replayed syscalls, some with a
//! different number in `x8`. Both scenarios report the registers the kernel
//! preserves across `svc`, which the caller must get back unchanged.
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
const SYS_DUP3: usize = 24;
const SYS_FCNTL: usize = 25;
const SYS_WAIT4: usize = 260;
const SYS_CLONE3: usize = 435;
const SYS_CLOSE_RANGE: usize = 436;
/// aarch64 `O_DIRECTORY`. x86-64 uses this bit for `O_DIRECT`.
const O_DIRECTORY: usize = 0o40000;
const F_GETFD: usize = 1;
const CLONE_CLEAR_SIGHAND: u64 = 0x1_0000_0000;
const CLONE_INTO_CGROUP: u64 = 0x2_0000_0000;
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

/// Issue a three-argument syscall and also return `x1` and `x8` as they
/// are after the `svc`. The kernel preserves both.
unsafe fn observed3(n: usize, a: usize, b: usize, c: usize) -> (isize, usize, usize) {
    let r: isize;
    let x1: usize;
    let x8: usize;
    asm!(
        "svc #0",
        inlateout("x8") n => x8,
        inlateout("x0") a => r,
        inlateout("x1") b => x1,
        in("x2") c,
        options(nostack)
    );
    (r, x1, x8)
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
    if starts(&scenario, n, b"clone3") {
        // The virtual cgroup directory only opens with O_DIRECTORY, so this
        // also proves the supervisor reads the aarch64 flag encoding.
        let cgroup_path = b"/sys/fs/cgroup\0";
        let cgroup = unsafe {
            syscall4(SYS_OPENAT, AT_FDCWD, cgroup_path.as_ptr() as usize, O_DIRECTORY, 0)
        };
        // struct clone_args: flags, pidfd, child_tid, parent_tid, exit_signal,
        // stack, stack_size, tls, set_tid, set_tid_size, cgroup.
        let mut args = [0u64; 11];
        args[0] = CLONE_INTO_CGROUP | CLONE_CLEAR_SIGHAND;
        args[4] = SIGCHLD as u64;
        args[10] = cgroup as u64;
        let (pid, x1, x8) = unsafe { observed3(SYS_CLONE3, args.as_ptr() as usize, 88, 0) };
        if pid == 0 {
            // The child returns through the same replayed sequence.
            exit(if x1 == 88 && x8 == SYS_CLONE3 { 7 } else { 8 });
        }
        let mut status = 0i32;
        let waited = unsafe { syscall4(SYS_WAIT4, pid as usize, &mut status as *mut i32 as usize, 0, 0) };
        let mut line = Line::new();
        line.str("GINUX_FIXTURE: clone3 cgroup=");
        line.dec((cgroup >= 0) as isize);
        line.str(" child=");
        line.dec((pid > 0) as isize);
        line.str(" waited=");
        line.dec((waited == pid) as isize);
        line.str(" status=");
        line.dec(status as isize);
        line.str(" x1=");
        line.dec(x1 as isize);
        line.str(" x8=");
        line.dec(x8 as isize);
        line.str(" flags-kept=");
        line.dec((args[0] == CLONE_INTO_CGROUP | CLONE_CLEAR_SIGHAND) as isize);
        line.str("\n");
        line.write();
        exit(0);
    }
    if starts(&scenario, n, b"close-range") {
        // 250 is above the protected null (240) and console (241)
        // descriptors, so closing 3.. spans them and runs in two parts.
        let duplicate = unsafe { syscall3(SYS_DUP3, 1, 250, 0) };
        let before = unsafe { syscall3(SYS_FCNTL, 250, F_GETFD, 0) };
        let (result, x1, x8) = unsafe { observed3(SYS_CLOSE_RANGE, 3, u32::MAX as usize, 0) };
        let after = unsafe { syscall3(SYS_FCNTL, 250, F_GETFD, 0) };
        let mut line = Line::new();
        line.str("GINUX_FIXTURE: close-range dup=");
        line.dec(duplicate);
        line.str(" before=");
        line.dec(before);
        line.str(" result=");
        line.dec(result);
        line.str(" after=");
        line.dec(after);
        line.str(" last-kept=");
        line.dec((x1 == u32::MAX as usize) as isize);
        line.str(" x8=");
        line.dec(x8 as isize);
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
