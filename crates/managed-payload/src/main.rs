#![no_std]
#![no_main]

use core::arch::global_asm;
use core::panic::PanicInfo;

#[cfg(target_arch = "x86_64")]
global_asm!(
    include_str!("../../../tests/fixtures/managed-payload.S"),
    options(att_syntax)
);

#[cfg(target_arch = "aarch64")]
global_asm!(include_str!(
    "../../../tests/fixtures/managed-payload-aarch64.S"
));

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
compile_error!("managed-payload supports x86_64 and aarch64");

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
