#![no_std]

#[cfg(target_arch = "x86_64")]
include!("x86_64_kernel.rs");

#[cfg(target_arch = "aarch64")]
include!("aarch64_kernel.rs");

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
compile_error!("ginux-kernel supports x86_64 and aarch64");
