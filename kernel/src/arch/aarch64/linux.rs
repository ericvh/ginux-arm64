use super::timer;
use super::uart;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub(crate) const CLASS_PID: u8 = 1;
pub(crate) const CLASS_PPID: u8 = 2;
pub(crate) const CLASS_ZERO: u8 = 3;
pub(crate) const CLASS_UMASK: u8 = 4;
pub(crate) const CLASS_BRK: u8 = 5;
pub(crate) const CLASS_MMAP: u8 = 6;
pub(crate) const CLASS_MEM: u8 = 7;
pub(crate) const CLASS_UNAME: u8 = 8;
pub(crate) const CLASS_CLOCK: u8 = 9;
pub(crate) const CLASS_SLEEP: u8 = 10;
pub(crate) const CLASS_IO: u8 = 11;
pub(crate) const CLASS_FD: u8 = 12;
pub(crate) const CLASS_PATH: u8 = 13;
pub(crate) const CLASS_EAGAIN: u8 = 14;
pub(crate) const CLASS_ECHILD: u8 = 15;
pub(crate) const CLASS_EXIT: u8 = 16;
pub(crate) const CLASS_SIGNAL: u8 = 17;
pub(crate) const CLASS_SIG: u8 = 18;
pub(crate) const CLASS_SCHED: u8 = 19;
pub(crate) const CLASS_INFO: u8 = 20;
pub(crate) const CLASS_CRED: u8 = 21;
pub(crate) const CLASS_NET: u8 = 22;
pub(crate) const CLASS_IPC: u8 = 23;
pub(crate) const CLASS_AIO: u8 = 24;
pub(crate) const CLASS_ADMIN: u8 = 25;
pub(crate) const CLASS_EINTR: u8 = 26;
pub(crate) const CLASS_RANDOM: u8 = 27;
pub(crate) const CLASS_GROUPS: u8 = 28;
pub(crate) const CLASS_EINVAL: u8 = 29;

struct LinuxCall {
    nr: u16,
    name: &'static str,
    class: u8,
}

include!("linux_table.inc.rs");

const EPERM: i64 = -1;
const ENOENT: i64 = -2;
const ESRCH: i64 = -3;
const EINTR: i64 = -4;
const EBADF: i64 = -9;
const ECHILD: i64 = -10;
const EAGAIN: i64 = -11;
const ENOMEM: i64 = -12;
const EFAULT: i64 = -14;
const EINVAL: i64 = -22;
const ENOSYS: i64 = -38;
const EAFNOSUPPORT: i64 = -97;

const PID: i64 = 4242;
static UMASK: AtomicU32 = AtomicU32::new(0o022);
static SEEN: [AtomicU64; 16] = [const { AtomicU64::new(0) }; 16];

pub(crate) fn note(number: u64) {
    if number >= 1024 {
        return;
    }
    let word = (number / 64) as usize;
    let bit = 1_u64 << (number % 64);
    SEEN[word].fetch_or(bit, Ordering::Relaxed);
}

pub(crate) fn unique() -> u32 {
    SEEN.iter()
        .map(|word| word.load(Ordering::Relaxed).count_ones())
        .sum()
}

pub(crate) fn table_len() -> usize {
    CALLS.len()
}

pub(crate) fn call(number: u64, args: [u64; 6]) -> Option<i64> {
    let entry = CALLS.iter().find(|call| u64::from(call.nr) == number)?;
    let result = behavior(entry, args);
    note(number);
    uart::write(b"ginux: svc=dispatch nr=");
    uart::write_u64(number);
    uart::write(b" ");
    uart::write(entry.name.as_bytes());
    uart::write(b" result=");
    write_result(result);
    uart::write(b"\r\n");
    Some(result)
}

fn behavior(entry: &LinuxCall, args: [u64; 6]) -> i64 {
    let [a0, a1, a2, a3, _, _] = args;
    match entry.class {
        CLASS_PID => PID,
        CLASS_PPID => 0,
        CLASS_ZERO => 0,
        CLASS_UMASK => {
            let previous = UMASK.swap(a0 as u32 & 0o777, Ordering::Relaxed);
            i64::from(previous)
        }
        CLASS_BRK => super::user::brk(a0),
        CLASS_MMAP => super::user::mmap(a0, a1, a2, a3),
        CLASS_MEM => {
            if entry.name == "mremap" || entry.name == "remap_file_pages" || entry.name == "mseal" {
                EINVAL
            } else if a1 == 0 {
                0
            } else {
                ENOMEM
            }
        }
        CLASS_UNAME => {
            if a0 == 0 {
                EFAULT
            } else {
                EINVAL
            }
        }
        CLASS_CLOCK => {
            if a1 == 0 {
                EFAULT
            } else if a0 > 1 {
                EINVAL
            } else {
                EFAULT
            }
        }
        CLASS_SLEEP => EINVAL,
        CLASS_IO | CLASS_FD => match entry.name {
            "write" => super::user::write(a0, a1, a2),
            "read" => super::user::read(a0, a1, a2),
            "pread64" => super::user::pread(a0, a1, a2, a3),
            "pwrite64" => super::user::pwrite(a0, a1, a2, a3),
            "lseek" => super::user::lseek(a0, a1, a2),
            "close" => super::user::close(a0),
            "fstat" => super::user::fstat(a0, a1),
            _ => EBADF,
        },
        CLASS_PATH => {
            if entry.name == "openat" {
                super::user::openat(a0, a1)
            } else if path_pointer(entry.name, a0, a1) == 0 {
                EFAULT
            } else {
                ENOENT
            }
        }
        CLASS_EAGAIN => EAGAIN,
        CLASS_ECHILD => {
            if entry.name == "waitid" {
                super::syscall::waitid(a0, a1, a2, a3)
            } else {
                ECHILD
            }
        }
        CLASS_EXIT => 0,
        CLASS_SIGNAL => signal(entry.name, a0, a1, a2),
        CLASS_SIG => signal_mask(entry.name, a0, a1, a2, a3),
        CLASS_SCHED => sched(entry.name, a0, a1),
        CLASS_INFO => info(entry.name, a0, a1),
        CLASS_CRED => cred(entry.name, a0, a1, a2),
        CLASS_NET => {
            if entry.name == "socket" || entry.name == "socketpair" {
                if a0 == 0 {
                    EINVAL
                } else {
                    EAFNOSUPPORT
                }
            } else {
                EBADF
            }
        }
        CLASS_IPC | CLASS_AIO | CLASS_EINVAL => EINVAL,
        CLASS_ADMIN => admin(entry.name, a0),
        CLASS_EINTR => EINTR,
        CLASS_RANDOM => {
            if a1 == 0 {
                0
            } else if a0 == 0 {
                EFAULT
            } else {
                EINVAL
            }
        }
        CLASS_GROUPS => {
            if entry.name == "getgroups" {
                if (a0 as i64) < 0 {
                    EINVAL
                } else if a0 == 0 {
                    1
                } else if a1 == 0 {
                    EFAULT
                } else {
                    1
                }
            } else if a0 == 0 {
                0
            } else if a1 == 0 {
                EFAULT
            } else {
                EINVAL
            }
        }
        _ => ENOSYS,
    }
}

fn path_pointer(name: &str, first: u64, second: u64) -> u64 {
    match name {
        "openat" | "openat2" | "newfstatat" | "mkdirat" | "mknodat" | "unlinkat" | "readlinkat"
        | "faccessat" | "faccessat2" | "fchmodat" | "fchmodat2" | "fchownat" | "renameat"
        | "renameat2" | "linkat" | "symlinkat" | "execveat" | "statx" | "name_to_handle_at"
        | "utimensat" | "mount_setattr" | "fspick" | "move_mount" | "open_tree" => second,
        _ => first,
    }
}

fn signal(name: &str, pid: u64, signal: u64, third: u64) -> i64 {
    if signal > 64 {
        return EINVAL;
    }
    match name {
        "kill" => {
            if pid == 0 || pid == PID as u64 {
                0
            } else {
                ESRCH
            }
        }
        "tkill" => {
            if pid == PID as u64 {
                0
            } else {
                ESRCH
            }
        }
        "tgkill" => {
            if pid == PID as u64 && third == PID as u64 {
                0
            } else {
                ESRCH
            }
        }
        _ => EINVAL,
    }
}

fn signal_mask(name: &str, first: u64, second: u64, third: u64, fourth: u64) -> i64 {
    match name {
        "rt_sigaction" => {
            if first == 0 || first > 64 {
                EINVAL
            } else if second == 0 && third == 0 {
                0
            } else {
                EFAULT
            }
        }
        "rt_sigprocmask" | "rt_sigtimedwait" | "rt_sigsuspend" => {
            if fourth != 8 && name != "rt_sigsuspend" {
                EINVAL
            } else if second == 0 && third == 0 && name == "rt_sigprocmask" {
                0
            } else {
                EINVAL
            }
        }
        "rt_sigpending" => {
            if second != 8 {
                EINVAL
            } else if first == 0 {
                EFAULT
            } else {
                EINVAL
            }
        }
        "rt_sigreturn" => EINVAL,
        "sigaltstack" => {
            if first == 0 && second == 0 {
                0
            } else {
                EFAULT
            }
        }
        "signalfd4" => EINVAL,
        _ => EINVAL,
    }
}

fn sched(name: &str, first: u64, second: u64) -> i64 {
    match name {
        "sched_getscheduler" => {
            if first == 0 || first == PID as u64 {
                0
            } else {
                ESRCH
            }
        }
        "sched_get_priority_max" => match first {
            0 => 0,
            1 | 2 => 99,
            _ => EINVAL,
        },
        "sched_get_priority_min" => {
            if first <= 2 {
                0
            } else {
                EINVAL
            }
        }
        "sched_setparam"
        | "sched_setscheduler"
        | "sched_setattr"
        | "sched_getparam"
        | "sched_getattr"
        | "sched_setaffinity"
        | "sched_getaffinity"
        | "sched_rr_get_interval" => {
            if second == 0 {
                EFAULT
            } else if first != 0 && first != PID as u64 {
                ESRCH
            } else {
                0
            }
        }
        _ => EINVAL,
    }
}

fn info(name: &str, first: u64, second: u64) -> i64 {
    match name {
        "times" => timer::frequency().max(1) as i64 / 1_000_000 + 1,
        "getcpu" => 0,
        "getpriority" => {
            if first > 2 {
                EINVAL
            } else {
                20
            }
        }
        "getrlimit" | "setrlimit" | "prlimit64" | "getrusage" | "sysinfo" => {
            if second == 0 {
                EFAULT
            } else if name == "getrlimit" && first > 15 {
                EINVAL
            } else {
                0
            }
        }
        _ => EINVAL,
    }
}

fn cred(name: &str, first: u64, second: u64, third: u64) -> i64 {
    match name {
        "getuid" | "geteuid" | "getgid" | "getegid" => 0,
        "setuid" | "setgid" | "setfsuid" | "setfsgid" => {
            if first == 0 {
                0
            } else {
                EPERM
            }
        }
        "setreuid" | "setregid" | "setresuid" | "setresgid" => {
            if first == 0
                && second == 0
                && (name != "setresuid" && name != "setresgid" || third == 0)
            {
                0
            } else {
                EPERM
            }
        }
        "getresuid" | "getresgid" => {
            if first == 0 || second == 0 || third == 0 {
                EFAULT
            } else {
                0
            }
        }
        "setsid" | "getpgid" | "getsid" | "getpgrp" => {
            if name != "setsid" && first != 0 && first != PID as u64 {
                ESRCH
            } else {
                PID
            }
        }
        "setpgid" => {
            if first == 0 || first == PID as u64 {
                0
            } else {
                ESRCH
            }
        }
        "personality" => 0,
        "capget" => {
            if first == 0 || second == 0 {
                EFAULT
            } else {
                0
            }
        }
        "capset" => EPERM,
        "prctl" => EINVAL,
        _ => EINVAL,
    }
}

fn admin(name: &str, first: u64) -> i64 {
    match name {
        "reboot" | "bpf" | "quotactl" | "quotactl_fd" => EINVAL,
        "acct" => 0,
        "syslog" => {
            if first == 0 {
                0
            } else {
                EINVAL
            }
        }
        "vhangup" => EBADF,
        _ => {
            if first == 0 {
                EFAULT
            } else {
                EPERM
            }
        }
    }
}

fn write_result(value: i64) {
    if value < 0 {
        uart::write(b"-");
        uart::write_u64((-value) as u64);
    } else {
        uart::write_u64(value as u64);
    }
}
