//! x86_64 host ABI for the Ginux daemon process.
//!
//! Host numbers match `asm/unistd_64.h`. The logical register file is the
//! host's own `user_regs_struct`, so every helper here is the plain x86-64
//! operation the supervisor performed before the architecture split.

use core::arch::{asm, global_asm};

use crate::supervisor::UserRegs;

global_asm!(
    ".global _start",
    ".type _start,@function",
    "_start:",
    "xor %rbp, %rbp",
    "mov %rsp, %rdi",
    "andq $-16, %rsp",
    "call rust_main",
    "ud2",
    options(att_syntax)
);

pub const SYS_READ_HOST: usize = 0;
pub const SYS_WRITE_HOST: usize = 1;
pub const SYS_CLOSE_HOST: usize = 3;
pub const SYS_FSTAT: usize = 5;
const SYS_POLL: usize = 7;
pub const SYS_MMAP_HOST: usize = 9;
pub const SYS_RT_SIGACTION_HOST: usize = 13;
pub const SYS_RT_SIGPROCMASK: usize = 14;
pub const SYS_IOCTL_HOST: usize = 16;
pub const SYS_PREAD64: usize = 17;
pub const SYS_PWRITE64: usize = 18;
const SYS_PAUSE: usize = 34;
pub const SYS_GETPID_HOST: usize = 39;
pub const SYS_BIND_HOST: usize = 49;
const SYS_FORK: usize = 57;
pub const SYS_EXECVE: usize = 59;
pub const SYS_EXIT_HOST: usize = 60;
pub const SYS_WAIT4: usize = 61;
pub const SYS_KILL: usize = 62;
pub const SYS_FCNTL_HOST: usize = 72;
pub const SYS_FSYNC: usize = 74;
pub const SYS_CHDIR: usize = 80;
pub const SYS_PTRACE: usize = 101;
pub const SYS_GETUID: usize = 102;
pub const SYS_GETGID: usize = 104;
pub const SYS_SETUID: usize = 105;
pub const SYS_SETGID: usize = 106;
pub const SYS_SETGROUPS: usize = 116;
pub const SYS_PRCTL: usize = 157;
pub const SYS_SETRLIMIT_HOST: usize = 160;
pub const SYS_CHROOT: usize = 161;
pub const SYS_FUTEX_HOST: usize = 202;
pub const SYS_SCHED_SETAFFINITY_HOST: usize = 203;
#[allow(dead_code)]
pub const SYS_GETDENTS64_HOST: usize = 217;
pub const SYS_CLOCK_GETTIME: usize = 228;
pub const SYS_OPENAT_HOST: usize = 257;
pub const SYS_MKDIRAT_HOST: usize = 258;
pub const SYS_NEWFSTATAT_HOST: usize = 262;
pub const SYS_LINKAT_HOST: usize = 265;
pub const SYS_READLINKAT_HOST: usize = 267;
pub const SYS_SIGNALFD4: usize = 289;
pub const SYS_PROCESS_VM_READV: usize = 310;
pub const SYS_PROCESS_VM_WRITEV: usize = 311;
pub const SYS_SECCOMP: usize = 317;
pub const SYS_PIDFD_OPEN: usize = 434;
pub const SYS_CLOSE_RANGE_HOST: usize = 436;
pub const SYS_PIDFD_GETFD: usize = 438;

/// Host `O_DIRECTORY` for the daemon's own opens.
pub const O_DIRECTORY_HOST: usize = 0x1_0000;

/// `AUDIT_ARCH_X86_64`: the only syscall ABI an instance task may use.
pub const AUDIT_ARCH: u32 = 0xc000_003e;
/// Host number of `sched_yield`, the one call the trace filter routes away
/// from ptrace.
pub const SECCOMP_YIELD_NUMBER: u32 = 24;
pub const ABI_REJECT_PROCESS: &[u8] = b"process requires the x86-64 syscall ABI";
pub const ABI_REJECT_NATIVE: &[u8] = b"native requires the x86-64 syscall ABI";
pub const ABI_REJECT_LAUNCH: &[u8] = b"launch requires the x86-64 syscall ABI";
pub const ARCH_NAME: &[u8] = b"x86_64";

/// Length of the `syscall` instruction.
const SYSCALL_INSN_LEN: u64 = 2;

pub fn host_fork() -> isize {
    unsafe { syscall0(SYS_FORK) }
}

pub fn host_pause() -> isize {
    unsafe { syscall0(SYS_PAUSE) }
}

/// `poll(2)` on the daemon's own descriptors with a millisecond timeout.
pub fn host_poll(descriptors: usize, count: usize, timeout_ms: usize) -> isize {
    unsafe { syscall3(SYS_POLL, descriptors, count, timeout_ms) }
}

/// `st_mode` of a host `struct stat` read as eighteen native words: the
/// `u32` at byte offset 24.
pub fn stat_mode(stat: &[u64; 18]) -> u64 {
    stat[3] & 0xffff_ffff
}

/// Byte offset of `st_mode` in the host `struct stat`.
pub const STAT_MODE_OFFSET: usize = 24;
/// Byte offset of `st_uid` in the host `struct stat`; `st_gid` follows it.
pub const STAT_UID_OFFSET: usize = 28;

/// A `struct stat` in the layout an instance task expects, built from the
/// x86-64 layout the supervisor composes. Here the two are the same.
pub struct GuestStat([u8; 144]);

impl core::ops::Deref for GuestStat {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

pub fn guest_stat(x86_64: [u8; 144]) -> GuestStat {
    GuestStat(x86_64)
}

pub fn read_user_regs(pid: usize, addr: usize, data: usize) -> isize {
    crate::supervisor::ptrace(crate::supervisor::PTRACE_GETREGS, pid, addr, data)
}

pub fn write_user_regs(pid: usize, addr: usize, data: usize) -> isize {
    crate::supervisor::ptrace(crate::supervisor::PTRACE_SETREGS, pid, addr, data)
}

/// Plant an `int3` in the low byte of a peeked instruction word.
pub fn breakpoint_trap_word(word: usize) -> usize {
    (word & !0xff) | 0xcc
}

/// Make the tracee execute `syscall` again when it resumes.
///
/// `resume` is the address after the syscall instruction, as every syscall
/// stop reports it. The caller has already placed the arguments, and sets
/// `orig_rax` itself where the kernel's restart bookkeeping needs it.
pub fn replay_syscall(regs: &mut UserRegs, resume: u64, syscall: u64) {
    regs.rip = resume - SYSCALL_INSN_LEN;
    regs.rax = syscall;
}

#[inline(always)]
pub unsafe fn syscall0(number: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall1(number: usize, a1: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall2(number: usize, a1: usize, a2: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall3(number: usize, a1: usize, a2: usize, a3: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall4(number: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall5(
    number: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        in("r8") a5,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall6(
    number: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
) -> isize {
    let result: isize;
    asm!(
        "syscall",
        inlateout("rax") number as isize => result,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        in("r8") a5,
        in("r9") a6,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result
}

/// Prepare a register file for a call that `PTRACE_SYSEMU` stopped and the
/// kernel will not run. `rax` already is the result on x86-64.
pub fn emulated_result(_regs: &mut UserRegs) {}

/// The calls the x86-64 vDSO answers without a syscall: gettimeofday, time,
/// clock_gettime, clock_getres and getcpu, by x86-64 number.
pub const VDSO_SYSCALLS: [u64; 5] = [96, 201, 228, 229, 309];

/// Called before every `ptrace` request the daemon makes. The x86-64
/// register file is the kernel's own and nothing is kept between requests.
#[inline(always)]
pub fn before_ptrace(_request: usize, _pid: usize) {}

/// Whether a tracee stopped by `SIGTRAP` executed a trap its compiler emits
/// to abort. On x86-64 those are `ud2` and `hlt`, which raise `SIGILL` and
/// `SIGSEGV`; `SIGTRAP` is `int3`, a breakpoint, and never this.
pub fn abort_trap(_pid: usize) -> bool {
    false
}

/// Called with every stop the daemon takes from `wait`. x86-64 reads the
/// kind of stop from the status where it is needed.
#[inline(always)]
pub fn after_wait(_pid: usize, _status: i32) {}

/// Called once with the stack the kernel started the daemon on.
#[inline(always)]
pub fn at_start(_initial_stack: *const usize) {}

/// The host kernel's page size in bytes. x86-64 Linux has one.
#[inline(always)]
pub fn page_size() -> u64 {
    4096
}
