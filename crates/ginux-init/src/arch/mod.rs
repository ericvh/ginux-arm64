//! Architecture-specific daemon entry and host ABI.
//!
//! One binary targets one architecture. Each architecture file owns the
//! startup sequence, the syscall instruction, the daemon's own host syscall
//! numbers and flag values, and the ptrace register file.
//!
//! Instance decisions, the contract, and every supervision loop stay written
//! against the x86-64 Linux ABI: `UserRegs` is the x86-64 `user_regs_struct`
//! and syscall numbers in it are x86-64 numbers. On x86_64 that register file
//! is the host's own and passes through unchanged. On aarch64 it is a logical
//! frame: [`read_user_regs`] and [`write_user_regs`] translate `user_pt_regs`
//! and the generic syscall table at the ptrace boundary.
//!
//! Rules for code outside this module:
//!
//! - A number passed to `syscallN` is a host number and comes from here.
//! - A number stored in `UserRegs::orig_rax` is an x86-64 number.
//! - Re-executing a syscall instruction goes through [`replay_syscall`].
//! - A `struct stat` the daemon reads for itself goes through [`stat_mode`].

#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "aarch64")]
pub use aarch64::*;

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
compile_error!("the Ginux userspace daemon builds for x86_64 or aarch64 only");
