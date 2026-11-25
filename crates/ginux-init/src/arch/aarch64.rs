//! aarch64 host ABI for the Ginux daemon process.
//!
//! Host numbers match the generic Linux syscall table used by aarch64
//! (`include/uapi/asm-generic/unistd.h` as of Linux 6.12). That table has no
//! `fork`, `pause`, or `poll` entry, so the daemon uses `clone(SIGCHLD)` and
//! `ppoll`.
//!
//! The supervision loops are written against the x86-64 register file and
//! x86-64 syscall numbers. This file presents each aarch64 tracee stop in
//! that shape and writes the supervisor's changes back:
//!
//! | logical x86-64         | aarch64                                     |
//! |------------------------|---------------------------------------------|
//! | `orig_rax`             | `NT_ARM_SYSTEM_CALL`, name-matched number   |
//! | `rdi rsi rdx r10 r8 r9`| `x0`..`x5`                                  |
//! | `rax`                  | `x0` at a syscall exit, `-ENOSYS` at entry  |
//! | `rip`, `rsp`, `eflags` | `pc`, `sp`, `pstate`                        |
//! | `cs`                   | stop-phase tag owned by this file           |
//!
//! aarch64 returns a syscall result in the register that carried its first
//! argument. The entry arguments are therefore remembered per tracee, so an
//! exit stop still shows them, as an x86-64 exit stop does.
//!
//! Three argument encodings differ between the two ABIs and are translated
//! here, because the supervisor compares them against x86-64 values:
//! `clone` takes `tls` before `child_tid`, and `O_DIRECTORY`, `O_NOFOLLOW`,
//! `O_DIRECT`, and `O_LARGEFILE` have different bit positions. Structures
//! in tracee memory are not translated. See `docs/arm64/README.md`.

use core::arch::{asm, global_asm};

use crate::supervisor::UserRegs;

global_asm!(
    ".global _start",
    ".type _start, %function",
    "_start:",
    "mov x0, sp",
    "bic sp, x0, #15",
    "bl rust_main",
    "brk #1",
);

/// The compiler's out-of-line atomics ask libc whether the CPU has LSE. This
/// freestanding daemon has no libc and no auxiliary vector parser, so every
/// query answers "absent" and the atomics use the baseline exclusive-load
/// sequences, which every ARMv8.0 host executes.
#[no_mangle]
extern "C" fn getauxval(_kind: usize) -> usize {
    0
}

pub const SYS_READ_HOST: usize = 63;
pub const SYS_WRITE_HOST: usize = 64;
pub const SYS_CLOSE_HOST: usize = 57;
pub const SYS_FSTAT: usize = 80;
const SYS_PPOLL: usize = 73;
pub const SYS_MMAP_HOST: usize = 222;
pub const SYS_RT_SIGACTION_HOST: usize = 134;
pub const SYS_RT_SIGPROCMASK: usize = 135;
pub const SYS_IOCTL_HOST: usize = 29;
pub const SYS_PREAD64: usize = 67;
pub const SYS_PWRITE64: usize = 68;
pub const SYS_GETPID_HOST: usize = 172;
pub const SYS_BIND_HOST: usize = 200;
const SYS_CLONE: usize = 220;
pub const SYS_EXECVE: usize = 221;
pub const SYS_EXIT_HOST: usize = 93;
pub const SYS_WAIT4: usize = 260;
pub const SYS_KILL: usize = 129;
pub const SYS_FCNTL_HOST: usize = 25;
pub const SYS_FSYNC: usize = 82;
pub const SYS_CHDIR: usize = 49;
pub const SYS_PTRACE: usize = 117;
pub const SYS_GETUID: usize = 174;
pub const SYS_GETGID: usize = 176;
pub const SYS_SETUID: usize = 146;
pub const SYS_SETGID: usize = 144;
pub const SYS_SETGROUPS: usize = 159;
pub const SYS_PRCTL: usize = 167;
pub const SYS_SETRLIMIT_HOST: usize = 164;
pub const SYS_CHROOT: usize = 51;
pub const SYS_FUTEX_HOST: usize = 98;
pub const SYS_SCHED_SETAFFINITY_HOST: usize = 122;
#[allow(dead_code)]
pub const SYS_GETDENTS64_HOST: usize = 61;
pub const SYS_CLOCK_GETTIME: usize = 113;
pub const SYS_OPENAT_HOST: usize = 56;
pub const SYS_MKDIRAT_HOST: usize = 34;
pub const SYS_NEWFSTATAT_HOST: usize = 79;
pub const SYS_LINKAT_HOST: usize = 37;
pub const SYS_READLINKAT_HOST: usize = 78;
pub const SYS_SIGNALFD4: usize = 74;
pub const SYS_PROCESS_VM_READV: usize = 270;
pub const SYS_PROCESS_VM_WRITEV: usize = 271;
pub const SYS_SECCOMP: usize = 277;
pub const SYS_PIDFD_OPEN: usize = 434;
pub const SYS_CLOSE_RANGE_HOST: usize = 436;
pub const SYS_PIDFD_GETFD: usize = 438;

/// Host `O_DIRECTORY` for the daemon's own opens.
pub const O_DIRECTORY_HOST: usize = 0x4000;

/// `AUDIT_ARCH_AARCH64`: the only syscall ABI an instance task may use.
pub const AUDIT_ARCH: u32 = 0xc000_00b7;
/// Host number of `sched_yield`, the one call the trace filter routes away
/// from ptrace.
pub const SECCOMP_YIELD_NUMBER: u32 = 124;
pub const ABI_REJECT_PROCESS: &[u8] = b"process requires the aarch64 syscall ABI";
pub const ABI_REJECT_NATIVE: &[u8] = b"native requires the aarch64 syscall ABI";
pub const ABI_REJECT_LAUNCH: &[u8] = b"launch requires the aarch64 syscall ABI";
pub const ARCH_NAME: &[u8] = b"aarch64";

/// Length of the `svc` instruction.
const SYSCALL_INSN_LEN: u64 = 4;
const SIGCHLD: usize = 17;
const ESRCH: isize = 3;

pub fn host_fork() -> isize {
    // `clone` argument order on aarch64 is flags, newsp, parent_tid, tls,
    // child_tid. A zero stack keeps the parent's stack, which is fork.
    unsafe { syscall5(SYS_CLONE, SIGCHLD, 0, 0, 0, 0) }
}

pub fn host_pause() -> isize {
    // A null poll set with no timeout waits until a signal arrives.
    unsafe { syscall4(SYS_PPOLL, 0, 0, 0, 0) }
}

/// `poll(2)` on the daemon's own descriptors with a millisecond timeout.
pub fn host_poll(descriptors: usize, count: usize, timeout_ms: usize) -> isize {
    let timeout = [timeout_ms / 1000, (timeout_ms % 1000) * 1_000_000];
    unsafe {
        syscall5(
            SYS_PPOLL,
            descriptors,
            count,
            timeout.as_ptr() as usize,
            0,
            0,
        )
    }
}

/// `st_mode` of a host `struct stat` read as eighteen native words: the
/// `u32` at byte offset 16 of the generic layout.
pub fn stat_mode(stat: &[u64; 18]) -> u64 {
    stat[2] & 0xffff_ffff
}

/// Byte offset of `st_mode` in the host `struct stat`.
pub const STAT_MODE_OFFSET: usize = 16;
/// Byte offset of `st_uid` in the host `struct stat`; `st_gid` follows it.
pub const STAT_UID_OFFSET: usize = 24;

/// A `struct stat` in the layout an instance task expects, built from the
/// x86-64 layout the supervisor composes. The generic layout aarch64 uses
/// is 128 bytes, with a 32-bit link count after the mode and a 32-bit block
/// size.
pub struct GuestStat([u8; 128]);

impl core::ops::Deref for GuestStat {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

pub fn guest_stat(x86_64: [u8; 144]) -> GuestStat {
    let mut stat = [0u8; 128];
    // st_dev, st_ino.
    stat[0..16].copy_from_slice(&x86_64[0..16]);
    // st_mode, then the low half of the 64-bit x86-64 st_nlink.
    stat[16..20].copy_from_slice(&x86_64[24..28]);
    stat[20..24].copy_from_slice(&x86_64[16..20]);
    // st_uid, st_gid.
    stat[24..32].copy_from_slice(&x86_64[28..36]);
    // st_rdev.
    stat[32..40].copy_from_slice(&x86_64[40..48]);
    // st_size.
    stat[48..56].copy_from_slice(&x86_64[48..56]);
    // The low half of the 64-bit x86-64 st_blksize.
    stat[56..60].copy_from_slice(&x86_64[56..60]);
    // st_blocks and the three timestamps.
    stat[64..120].copy_from_slice(&x86_64[64..120]);
    GuestStat(stat)
}

#[inline(always)]
pub unsafe fn syscall0(number: usize) -> isize {
    let result: isize;
    asm!(
        "svc #0",
        in("x8") number,
        lateout("x0") result,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall1(number: usize, a1: usize) -> isize {
    let result: isize;
    asm!(
        "svc #0",
        in("x8") number,
        inlateout("x0") a1 => result,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall2(number: usize, a1: usize, a2: usize) -> isize {
    let result: isize;
    asm!(
        "svc #0",
        in("x8") number,
        inlateout("x0") a1 => result,
        in("x1") a2,
        options(nostack)
    );
    result
}

#[inline(always)]
pub unsafe fn syscall3(number: usize, a1: usize, a2: usize, a3: usize) -> isize {
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

#[inline(always)]
pub unsafe fn syscall4(number: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> isize {
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
        "svc #0",
        in("x8") number,
        inlateout("x0") a1 => result,
        in("x1") a2,
        in("x2") a3,
        in("x3") a4,
        in("x4") a5,
        in("x5") a6,
        options(nostack)
    );
    result
}

// Stop-phase tags carried in the logical `cs` field. The supervisor never
// reads or writes `cs`, so the tag a read produced reaches the next write.
/// A syscall-entry or seccomp stop: `x0` still holds the first argument.
const ENTRY_PHASE: u64 = 1;
/// A syscall-exit stop: `x0` holds the result.
const EXIT_PHASE: u64 = 2;
/// Set by [`replay_syscall`]: `x0` must receive the first argument again.
const REPLAY_PHASE: u64 = 3;
/// A signal or ptrace-event stop: `x0` is an ordinary register.
const OTHER_PHASE: u64 = 4;

/// What x86-64 leaves in `rax` at a syscall entry: `-ENOSYS`.
const ENTRY_RAX: u64 = (-38i64) as u64;
/// Logical number for a host syscall with no x86-64 name match. It matches
/// no contract entry, so the supervisor fails closed on it.
const UNKNOWN_SYSCALL: u64 = u64::MAX - 1;
/// Logical number the supervisor writes to skip a syscall.
const NO_SYSCALL: u64 = u64::MAX;

const NT_PRSTATUS: usize = 1;
const NT_ARM_SYSTEM_CALL: usize = 0x404;
const PTRACE_GETREGSET: usize = 0x4204;
const PTRACE_SETREGSET: usize = 0x4205;
const PTRACE_GET_SYSCALL_INFO: usize = 0x420e;
const SYSCALL_INFO_ENTRY: u8 = 1;
const SYSCALL_INFO_EXIT: u8 = 2;
const SYSCALL_INFO_SECCOMP: u8 = 3;

// x86-64 numbers of the calls whose argument encoding differs.
const X86_CLONE: u64 = 56;
const X86_FCNTL: u64 = 72;
const X86_OPENAT: u64 = 257;
const X86_PIPE2: u64 = 293;
const X86_OPEN_BY_HANDLE_AT: u64 = 304;
const F_SETFL: u64 = 4;

#[repr(C)]
struct UserPtRegs {
    regs: [u64; 31],
    sp: u64,
    pc: u64,
    pstate: u64,
}

#[repr(C)]
struct IoVec {
    base: usize,
    len: usize,
}

/// The logical syscall a tracee is inside, as the supervisor last saw or
/// wrote it at the entry stop.
#[derive(Clone, Copy)]
struct SavedCall {
    pid: u32,
    live: bool,
    /// Address after the `svc`, which an exit stop reports again. A record
    /// left behind by a dead task that never reached its exit stop does not
    /// match a new task that reuses the pid.
    pc: u64,
    nr: u64,
    args: [u64; 6],
    /// The logical call the tracee itself issued. Its argument encoding
    /// still applies to the registers after the supervisor skips the call.
    issued: u64,
    /// The tracee's own `x8`, held while a replay runs another call.
    x8_held: bool,
    x8: u64,
    /// `x0` as the kernel last dispatched a call for this task.
    sent_x0: u64,
    /// A call that returned a restart result, as the supervisor left it at
    /// the exit stop. It outlives the calls a signal handler makes before
    /// the kernel runs the interrupted call again.
    restart: bool,
    restart_pc: u64,
    restart_nr: u64,
    restart_args: [u64; 6],
    restart_x0: u64,
}

const EMPTY_CALL: SavedCall = SavedCall {
    pid: 0,
    live: false,
    pc: 0,
    nr: 0,
    args: [0; 6],
    issued: 0,
    x8_held: false,
    x8: 0,
    sent_x0: 0,
    restart: false,
    restart_pc: 0,
    restart_nr: 0,
    restart_args: [0; 6],
    restart_x0: 0,
};

/// One record per tracee inside a syscall. The native supervisor tracks at
/// most 128 tasks; the rest is room for records of tasks that have exited.
const SAVED_LEN: usize = 256;
static mut SAVED_CALLS: [SavedCall; SAVED_LEN] = [EMPTY_CALL; SAVED_LEN];
static mut SAVED_VICTIM: usize = 0;

fn ptrace(request: usize, pid: usize, addr: usize, data: usize) -> isize {
    crate::supervisor::ptrace(request, pid, addr, data)
}

fn saved_slot(index: usize) -> *mut SavedCall {
    unsafe { (core::ptr::addr_of_mut!(SAVED_CALLS) as *mut SavedCall).add(index) }
}

/// The record for `pid`, if one exists.
fn find_call(pid: usize) -> Option<*mut SavedCall> {
    let pid32 = pid as u32;
    let mut index = 0;
    while index < SAVED_LEN {
        let slot = saved_slot(index);
        let current = unsafe { core::ptr::read(slot) };
        if current.live && current.pid == pid32 {
            return Some(slot);
        }
        index += 1;
    }
    None
}

/// The record for `pid`, created empty if it does not exist.
fn claim_call(pid: usize) -> *mut SavedCall {
    if let Some(slot) = find_call(pid) {
        return slot;
    }
    unsafe {
        let mut free = None;
        let mut index = 0;
        while index < SAVED_LEN && free.is_none() {
            if !core::ptr::read(saved_slot(index)).live {
                free = Some(index);
            }
            index += 1;
        }
        if free.is_none() {
            // Every slot is taken. Release the records of tasks that no
            // longer exist: their last entry stop never had an exit stop.
            index = 0;
            while index < SAVED_LEN {
                let current = core::ptr::read(saved_slot(index));
                if syscall2(SYS_KILL, current.pid as usize, 0) == -ESRCH {
                    core::ptr::write(saved_slot(index), EMPTY_CALL);
                    if free.is_none() {
                        free = Some(index);
                    }
                }
                index += 1;
            }
        }
        let index = free.unwrap_or_else(|| {
            let victim = core::ptr::read(core::ptr::addr_of!(SAVED_VICTIM));
            core::ptr::write(
                core::ptr::addr_of_mut!(SAVED_VICTIM),
                (victim + 1) % SAVED_LEN,
            );
            victim
        });
        let slot = saved_slot(index);
        core::ptr::write(
            slot,
            SavedCall {
                pid: pid as u32,
                live: true,
                ..EMPTY_CALL
            },
        );
        slot
    }
}

/// The record of the call `pid` is inside, if its entry stop was seen and
/// the program counter still names that call.
fn recall_call(pid: usize, pc: u64) -> Option<SavedCall> {
    let current = unsafe { core::ptr::read(find_call(pid)?) };
    (current.pc == pc).then_some(current)
}

/// Which kind of stop the tracee is in, from `PTRACE_GET_SYSCALL_INFO`.
fn stop_kind(pid: usize) -> u8 {
    let mut info = [0u64; 11];
    let read = ptrace(
        PTRACE_GET_SYSCALL_INFO,
        pid,
        core::mem::size_of_val(&info),
        info.as_mut_ptr() as usize,
    );
    if read < 0 {
        0
    } else {
        info[0] as u8
    }
}

fn read_raw(pid: usize) -> Option<UserPtRegs> {
    let mut raw = UserPtRegs {
        regs: [0; 31],
        sp: 0,
        pc: 0,
        pstate: 0,
    };
    let mut iovec = IoVec {
        base: &mut raw as *mut _ as usize,
        len: core::mem::size_of::<UserPtRegs>(),
    };
    if ptrace(
        PTRACE_GETREGSET,
        pid,
        NT_PRSTATUS,
        &mut iovec as *mut _ as usize,
    ) < 0
    {
        return None;
    }
    Some(raw)
}

/// Write the general registers. `logical` is the address of the x86-64
/// register file they were translated from, or zero when there is none.
///
/// `ptrace` takes four arguments and the kernel ignores a fifth. The logical
/// file rides there so that a tracer of this daemon can read what the
/// supervisor staged, as it reads the argument of `PTRACE_SETREGS` on
/// x86-64. `scripts/aarch64-trace.py` does, for upstream's race tests.
/// Nothing in the daemon or the kernel acts on it.
fn write_raw(pid: usize, raw: &UserPtRegs, logical: usize) -> bool {
    let mut iovec = IoVec {
        base: raw as *const _ as usize,
        len: core::mem::size_of::<UserPtRegs>(),
    };
    unsafe {
        syscall5(
            SYS_PTRACE,
            PTRACE_SETREGSET,
            pid,
            NT_PRSTATUS,
            &mut iovec as *mut _ as usize,
            logical,
        ) >= 0
    }
}

fn read_syscallno(pid: usize, raw: &UserPtRegs) -> i64 {
    let mut value = 0i32;
    let mut iovec = IoVec {
        base: &mut value as *mut _ as usize,
        len: core::mem::size_of::<i32>(),
    };
    if ptrace(
        PTRACE_GETREGSET,
        pid,
        NT_ARM_SYSTEM_CALL,
        &mut iovec as *mut _ as usize,
    ) < 0
    {
        raw.regs[8] as i64
    } else {
        i64::from(value)
    }
}

fn write_syscallno(pid: usize, number: i32) -> bool {
    let mut value = number;
    let mut iovec = IoVec {
        base: &mut value as *mut _ as usize,
        len: core::mem::size_of::<i32>(),
    };
    ptrace(
        PTRACE_SETREGSET,
        pid,
        NT_ARM_SYSTEM_CALL,
        &mut iovec as *mut _ as usize,
    ) >= 0
}

/// Host syscall number to the x86-64 number of the same name.
fn logical_number(native: i64) -> u64 {
    if native < 0 {
        NO_SYSCALL
    } else {
        ginux_core::to_x86(native as u64).unwrap_or(UNKNOWN_SYSCALL)
    }
}

/// x86-64 number to the host number of the same name. `None` leaves the
/// tracee's own number in place.
fn native_number(logical: u64) -> Option<i32> {
    if logical == NO_SYSCALL {
        return Some(-1);
    }
    match ginux_core::to_aarch64(logical) {
        Some(number) if number <= i32::MAX as u64 => Some(number as i32),
        // An x86-64 number with no aarch64 entry cannot run here. Skipping
        // it is the fail-closed reading of a supervisor that asked for it.
        None if logical != UNKNOWN_SYSCALL => Some(-1),
        _ => None,
    }
}

/// Exchange the open-flag bits that sit in different positions on the two
/// ABIs. x86-64 has `O_DIRECT`, `O_LARGEFILE`, `O_DIRECTORY`, `O_NOFOLLOW`
/// at `0o40000`, `0o100000`, `0o200000`, `0o400000`. aarch64 has
/// `O_DIRECTORY`, `O_NOFOLLOW`, `O_DIRECT`, `O_LARGEFILE` there. The
/// exchange is its own inverse.
fn exchange_open_flags(flags: u64) -> u64 {
    const LOW: u64 = 0o40000 | 0o100000;
    const HIGH: u64 = 0o200000 | 0o400000;
    (flags & !(LOW | HIGH)) | ((flags & LOW) << 2) | ((flags & HIGH) >> 2)
}

/// Convert the arguments of logical syscall `nr` between the two ABIs. Each
/// conversion is its own inverse, so one function serves both directions.
fn exchange_arguments(nr: u64, args: &mut [u64; 6]) {
    match nr {
        // x86-64: flags, stack, parent_tid, child_tid, tls.
        // aarch64: flags, stack, parent_tid, tls, child_tid.
        X86_CLONE => args.swap(3, 4),
        X86_OPENAT | X86_OPEN_BY_HANDLE_AT => args[2] = exchange_open_flags(args[2]),
        X86_PIPE2 => args[1] = exchange_open_flags(args[1]),
        X86_FCNTL if args[1] == F_SETFL => args[2] = exchange_open_flags(args[2]),
        _ => {}
    }
}

/// Whether a syscall result makes the kernel run the call again:
/// `ERESTARTSYS`, `ERESTARTNOINTR`, `ERESTARTNOHAND`, `ERESTART_RESTARTBLOCK`.
fn restart_result(result: u64) -> bool {
    matches!(result as i64, -512 | -513 | -514 | -516)
}

/// Give a restarted call the first argument the supervisor left at its exit.
///
/// On x86-64 the first argument has its own register, so what the supervisor
/// writes to `rdi` at the exit stop is what the kernel runs the call with
/// again. aarch64 has only `x0`, which holds the result at the exit stop,
/// and the kernel restarts with the first argument it last dispatched: the
/// value the supervisor issued at the entry. This puts the exit value back
/// in `x0`, in the tracee and in `raw`, when the stop is that restart.
fn restore_restarted_argument(pid: usize, raw: &mut UserPtRegs, nr: u64, args: &mut [u64; 6]) {
    let Some(slot) = find_call(pid) else {
        return;
    };
    let mut record = unsafe { core::ptr::read(slot) };
    if !record.restart || record.restart_pc != raw.pc || record.restart_nr != nr {
        return;
    }
    // The call at this address ran again or was issued anew; either way the
    // record is spent.
    record.restart = false;
    unsafe { core::ptr::write(slot, record) };
    if raw.regs[0] != record.restart_x0 || args[1..] != record.restart_args[1..] {
        return;
    }
    let mut wanted = record.restart_args;
    exchange_arguments(nr, &mut wanted);
    if wanted[0] == raw.regs[0] {
        return;
    }
    let dispatched = raw.regs[0];
    raw.regs[0] = wanted[0];
    if write_raw(pid, raw, 0) {
        args[0] = record.restart_args[0];
    } else {
        raw.regs[0] = dispatched;
    }
}

/// Fill the logical x86-64 register file from `user_pt_regs`.
pub fn read_user_regs(pid: usize, _addr: usize, data: usize) -> isize {
    let Some(mut raw) = read_raw(pid) else {
        return -1;
    };
    let kind = stop_kind(pid);
    let native = read_syscallno(pid, &raw);
    let mut args = [
        raw.regs[0],
        raw.regs[1],
        raw.regs[2],
        raw.regs[3],
        raw.regs[4],
        raw.regs[5],
    ];
    let mut nr = logical_number(native);
    let (rax, phase) = match kind {
        SYSCALL_INFO_EXIT => {
            if let Some(saved) = recall_call(pid, raw.pc) {
                nr = saved.nr;
                args = saved.args;
            } else {
                // No entry record: a new task returning from clone, or a
                // task whose image or signal frame replaced the program
                // counter. `x0` is the result and the first argument is
                // gone, as after `execve` on x86-64.
                exchange_arguments(nr, &mut args);
            }
            (raw.regs[0], EXIT_PHASE)
        }
        SYSCALL_INFO_ENTRY | SYSCALL_INFO_SECCOMP => {
            exchange_arguments(nr, &mut args);
            restore_restarted_argument(pid, &mut raw, nr, &mut args);
            let slot = claim_call(pid);
            unsafe {
                let mut record = core::ptr::read(slot);
                record.pc = raw.pc;
                record.nr = nr;
                record.args = args;
                record.issued = nr;
                record.sent_x0 = raw.regs[0];
                core::ptr::write(slot, record);
            }
            (ENTRY_RAX, ENTRY_PHASE)
        }
        _ => {
            exchange_arguments(nr, &mut args);
            (raw.regs[0], OTHER_PHASE)
        }
    };
    let logical = UserRegs {
        r15: 0,
        r14: 0,
        r13: 0,
        r12: 0,
        rbp: 0,
        rbx: 0,
        r11: 0,
        r10: args[3],
        r9: args[5],
        r8: args[4],
        rax,
        rcx: 0,
        rdx: args[2],
        rsi: args[1],
        rdi: args[0],
        orig_rax: nr,
        rip: raw.pc,
        cs: phase,
        eflags: raw.pstate,
        rsp: raw.sp,
        ss: 0,
        fs_base: 0,
        gs_base: 0,
        ds: 0,
        es: 0,
        fs: 0,
        gs: 0,
    };
    unsafe {
        core::ptr::write_unaligned(data as *mut UserRegs, logical);
    }
    0
}

/// Write the logical x86-64 register file back to `user_pt_regs`.
///
/// What `x0` receives depends on the stop:
///
/// - entry, syscall kept: the first argument.
/// - entry, syscall skipped: `rax`, which is `-ENOSYS` unless the supervisor
///   supplied a result. The kernel leaves `x0` alone when it skips.
/// - exit: `rax`, the result.
/// - replay: the first argument, for the re-executed `svc`.
/// - any other stop: `rax` if the supervisor changed it, else `rdi`.
///
/// `x8` is the tracee's own register and the kernel preserves it across a
/// syscall. A replay that runs a different call borrows it; the exit write
/// that ends the replay sequence gives it back.
pub fn write_user_regs(pid: usize, _addr: usize, data: usize) -> isize {
    let logical = unsafe { core::ptr::read_unaligned(data as *const UserRegs) };
    let Some(mut raw) = read_raw(pid) else {
        return -1;
    };
    let phase = logical.cs;
    let skip = logical.orig_rax == NO_SYSCALL;
    let syscall_stop = matches!(phase, ENTRY_PHASE | EXIT_PHASE | REPLAY_PHASE);
    let slot = if syscall_stop {
        Some(claim_call(pid))
    } else {
        None
    };
    let mut record = slot.map(|slot| unsafe { core::ptr::read(slot) });
    let logical_args = [
        logical.rdi,
        logical.rsi,
        logical.rdx,
        logical.r10,
        logical.r8,
        logical.r9,
    ];
    // A skipped call leaves the registers encoded for the call the tracee
    // issued; any other number names the encoding of what is in them.
    let encoding = match record {
        Some(record) if skip => record.issued,
        _ => logical.orig_rax,
    };
    let mut args = logical_args;
    exchange_arguments(encoding, &mut args);
    raw.regs[0] = match phase {
        ENTRY_PHASE if skip => logical.rax,
        ENTRY_PHASE | REPLAY_PHASE => args[0],
        EXIT_PHASE => logical.rax,
        _ if logical.rax != raw.regs[0] => logical.rax,
        _ => args[0],
    };
    raw.regs[1] = args[1];
    raw.regs[2] = args[2];
    raw.regs[3] = args[3];
    raw.regs[4] = args[4];
    raw.regs[5] = args[5];
    let number = native_number(logical.orig_rax);
    if let Some(record) = record.as_mut() {
        match phase {
            REPLAY_PHASE => {
                if let Some(number) = number.filter(|number| *number >= 0) {
                    if !record.x8_held && raw.regs[8] != number as u64 {
                        record.x8_held = true;
                        record.x8 = raw.regs[8];
                    }
                    raw.regs[8] = number as u64;
                }
            }
            EXIT_PHASE if record.x8_held => {
                raw.regs[8] = record.x8;
                record.x8_held = false;
            }
            _ => {}
        }
        match phase {
            ENTRY_PHASE | REPLAY_PHASE if !skip => record.sent_x0 = raw.regs[0],
            EXIT_PHASE if restart_result(logical.rax) => {
                record.restart = true;
                record.restart_pc = logical.rip;
                record.restart_nr = logical.orig_rax;
                record.restart_args = logical_args;
                record.restart_x0 = record.sent_x0;
            }
            _ => {}
        }
    }
    raw.pc = logical.rip;
    raw.sp = logical.rsp;
    if logical.eflags != 0 {
        raw.pstate = logical.eflags;
    }
    let current = read_syscallno(pid, &raw);
    if !write_raw(pid, &raw, data) {
        return -1;
    }
    // A replay re-enters the kernel through `svc`, which sets the number
    // again. Until then the task is not in a syscall, so a signal that
    // arrives first must not apply restart handling to its first argument.
    let wanted = if phase == REPLAY_PHASE {
        Some(-1)
    } else {
        number.filter(|_| logical_number(current) != logical.orig_rax)
    };
    if let Some(wanted) = wanted {
        if i64::from(wanted) != current && !write_syscallno(pid, wanted) {
            return -1;
        }
    }
    if let (Some(slot), Some(mut record)) = (slot, record) {
        if phase != REPLAY_PHASE {
            // The exit stop shows what the supervisor left in the
            // registers, as it does on x86-64, where they are the tracee's.
            record.pc = logical.rip;
            record.nr = logical.orig_rax;
            record.args = logical_args;
        }
        unsafe { core::ptr::write(slot, record) };
    }
    0
}

/// Replace one aligned instruction with `brk #0`.
pub fn breakpoint_trap_word(word: usize) -> usize {
    (word & !0xffff_ffff) | 0xd420_0000
}

/// Make the tracee execute `svc` again when it resumes.
///
/// `resume` is the address after the syscall instruction, as every syscall
/// stop reports it. The caller has already placed the arguments. The number
/// goes to `x8` through `orig_rax`, and the replay tag makes
/// [`write_user_regs`] load `x0` with the first argument instead of a result.
pub fn replay_syscall(regs: &mut UserRegs, resume: u64, syscall: u64) {
    regs.rip = resume - SYSCALL_INSN_LEN;
    regs.rax = syscall;
    regs.orig_rax = syscall;
    regs.cs = REPLAY_PHASE;
}

/// Prepare a register file for a call that `PTRACE_SYSEMU` stopped and the
/// kernel will not run. Marking the call skipped makes `write_user_regs`
/// put `rax` in `x0`, where an entry stop otherwise keeps the first argument.
pub fn emulated_result(regs: &mut UserRegs) {
    regs.orig_rax = NO_SYSCALL;
}
