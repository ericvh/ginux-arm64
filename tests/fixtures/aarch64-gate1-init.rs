//! Guest init for `make gate1-aarch64-guest`.
//! Mounts virtio-9p tag hostshare at /var/tmp and runs /var/tmp/ginux.
#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

global_asm!(
    r#"
    .section .text._start
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
const SYS_MKDIRAT: usize = 34;
const SYS_MOUNT: usize = 40;
const SYS_EXIT: usize = 93;
const SYS_REBOOT: usize = 142;
const SYS_CLONE: usize = 220;
const SYS_WAIT4: usize = 260;
const SYS_EXECVE: usize = 221;
const AT_FDCWD: usize = (-100isize) as usize;

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

fn write_str(s: &str) {
    unsafe { syscall3(SYS_WRITE, 1, s.as_ptr() as usize, s.len()); }
}

fn write_dec(mut n: isize) {
    if n < 0 {
        write_str("-");
        n = -n;
    }
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    unsafe { syscall3(SYS_WRITE, 1, buf[i..].as_ptr() as usize, buf.len() - i); }
}

fn poweroff() -> ! {
    unsafe {
        syscall4(SYS_REBOOT, 0xfee1dead, 672274793, 0x4321fedc, 0);
        syscall1(SYS_EXIT, 1);
    }
    loop {}
}

fn mkdir(path: &str) {
    let mut buf = [0u8; 64];
    let b = path.as_bytes();
    buf[..b.len()].copy_from_slice(b);
    unsafe { syscall3(SYS_MKDIRAT, AT_FDCWD, buf.as_ptr() as usize, 0o755); }
}

fn mount(src: &str, target: &str, fstype: &str, data: Option<&str>) -> isize {
    fn cstr(s: &str, buf: &mut [u8]) -> *const u8 {
        let b = s.as_bytes();
        buf[..b.len()].copy_from_slice(b);
        buf[b.len()] = 0;
        buf.as_ptr()
    }
    let mut a = [0u8; 32];
    let mut b = [0u8; 32];
    let mut c = [0u8; 32];
    let mut d = [0u8; 80];
    let src_p = cstr(src, &mut a);
    let tgt_p = cstr(target, &mut b);
    let type_p = cstr(fstype, &mut c);
    let data_p = if let Some(data) = data {
        cstr(data, &mut d) as usize
    } else {
        0
    };
    unsafe { syscall5(SYS_MOUNT, src_p as usize, tgt_p as usize, type_p as usize, 0, data_p) }
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    write_str("ptrace-init: boot\n");
    mkdir("/proc");
    mkdir("/sys");
    mkdir("/dev");
    mkdir("/var");
    mkdir("/var/tmp");
    let proc = mount("proc", "/proc", "proc", None);
    let sys = mount("sysfs", "/sys", "sysfs", None);
    let dev = mount("devtmpfs", "/dev", "devtmpfs", None);
    write_str("ptrace-init: mounts proc=");
    write_dec(proc);
    write_str(" sys=");
    write_dec(sys);
    write_str(" dev=");
    write_dec(dev);
    write_str("\n");
    let nine = mount(
        "hostshare",
        "/var/tmp",
        "9p",
        Some("trans=virtio,version=9p2000.L,cache=none"),
    );
    write_str("ptrace-init: 9p=");
    write_dec(nine);
    write_str("\n");
    if nine < 0 {
        write_str("ptrace-init: 9p mount failed\n");
        poweroff();
    }
    let pid = unsafe { syscall5(SYS_CLONE, 17, 0, 0, 0, 0) };
    if pid < 0 {
        write_str("ptrace-init: fork failed\n");
        poweroff();
    }
    if pid == 0 {
        let path = b"/var/tmp/ginux\0";
        #[cfg(not(native_guest))]
        let argv: [*const u8; 2] = [path.as_ptr(), core::ptr::null()];
        #[cfg(native_guest)]
        let argv: [*const u8; 5] = [
            path.as_ptr(),
            b"native\0".as_ptr(),
            b"/var/tmp/native.toml\0".as_ptr(),
            b"/var/tmp/root\0".as_ptr(),
            core::ptr::null(),
        ];
        let envp: [*const u8; 1] = [core::ptr::null()];
        let rc = unsafe { syscall3(SYS_EXECVE, path.as_ptr() as usize, argv.as_ptr() as usize, envp.as_ptr() as usize) };
        write_str("ptrace-init: execve=");
        write_dec(rc);
        write_str("\n");
        unsafe { syscall1(SYS_EXIT, 127); }
        loop {}
    }
    let mut status = 0i32;
    let waited = unsafe { syscall4(SYS_WAIT4, pid as usize, &mut status as *mut i32 as usize, 0, 0) };
    write_str("ptrace-init: wait=");
    write_dec(waited);
    write_str(" status=");
    write_dec(status as isize);
    write_str("\n");
    poweroff();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    write_str("ptrace-init: panic\n");
    poweroff()
}

#[no_mangle]
extern "C" fn rust_eh_personality() {}
