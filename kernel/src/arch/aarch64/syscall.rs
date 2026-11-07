use super::linux;
use super::timer;
use super::uart;
use core::arch::global_asm;

global_asm!(
    r#"
    .section .text.vectors, "ax"
    .balign 2048
    .global ginux_vectors
ginux_vectors:
    b ginux_unhandled
    .balign 0x80
    b ginux_unhandled
    .balign 0x80
    b ginux_unhandled
    .balign 0x80
    b ginux_unhandled
    .balign 0x80
    b ginux_unhandled
    .balign 0x80
    b ginux_irq
    .balign 0x80
    b ginux_unhandled
    .balign 0x80
    b ginux_unhandled
    .balign 0x80
    b ginux_svc
    .balign 0x80
    b ginux_irq
    .balign 0x80
    b ginux_unhandled
    .balign 0x80
    b ginux_unhandled
    .balign 0x80
    b ginux_unhandled
    .balign 0x80
    b ginux_unhandled
    .balign 0x80
    b ginux_unhandled
    .balign 0x80
    b ginux_unhandled
    .balign 0x80

ginux_irq:
    stp x0, x1, [sp, #-16]!
    bl ginux_on_irq
    ldp x0, x1, [sp], #16
    eret

ginux_svc:
    sub sp, sp, #128
    stp x0, x1, [sp, #0]
    stp x2, x3, [sp, #16]
    stp x4, x5, [sp, #32]
    stp x6, x7, [sp, #48]
    stp x8, x9, [sp, #64]
    mov x0, sp
    bl ginux_on_svc
    ldp x0, x1, [sp, #0]
    ldp x2, x3, [sp, #16]
    ldp x4, x5, [sp, #32]
    ldp x6, x7, [sp, #48]
    ldp x8, x9, [sp, #64]
    add sp, sp, #128
    eret

ginux_unhandled:
    bl ginux_on_fault
    eret
    "#
);

extern "C" {
    static ginux_vectors: u8;
}

pub(crate) fn install() -> bool {
    let vectors = core::ptr::addr_of!(ginux_vectors) as u64;
    if vectors & 2047 != 0 {
        return false;
    }
    unsafe {
        core::arch::asm!("msr vbar_el1, {v}", "isb", v = in(reg) vectors, options(nostack));
    }
    true
}

pub(crate) fn wait_for_timer() {
    unsafe {
        core::arch::asm!("msr daifclr, #2", options(nostack));
    }
    for _ in 0..2_000_000 {
        if timer::consumed().is_some() {
            break;
        }
        unsafe { core::arch::asm!("wfi", options(nostack)) };
    }
    unsafe {
        core::arch::asm!("msr daifset, #2", options(nostack));
    }
}

pub(crate) unsafe fn enter(pc: u64, stack: u64) -> ! {
    core::arch::asm!(
        "msr sp_el0, {stack}",
        "msr elr_el1, {pc}",
        "msr spsr_el1, {spsr}",
        "isb",
        "eret",
        stack = in(reg) stack,
        pc = in(reg) pc,
        spsr = in(reg) 0x3c0u64,
        options(noreturn),
    );
}

#[no_mangle]
extern "C" fn ginux_on_irq() {
    let intid: u64;
    unsafe {
        core::arch::asm!("mrs {v}, ICC_IAR1_EL1", v = out(reg) intid, options(nostack, nomem));
    }
    if intid < 1020 {
        timer::note_interrupt(intid as u32);
        unsafe {
            core::arch::asm!(
                "msr ICC_EOIR1_EL1, {v}",
                v = in(reg) intid,
                options(nostack),
            );
        }
    }
}

#[no_mangle]
extern "C" fn ginux_on_svc(frame: *mut u64) {
    let esr: u64;
    unsafe {
        core::arch::asm!("mrs {v}, esr_el1", v = out(reg) esr, options(nostack, nomem));
    }
    if esr >> 26 != 0x15 {
        fault(esr);
    }
    let number = unsafe { frame.add(8).read() };
    let args = [0, 1, 2, 3, 4, 5].map(|index| unsafe { frame.add(index).read() });
    if number == 172 {
        linux::note(172);
        uart::write(b"ginux: svc=dispatch nr=172 result=4242\r\n");
        unsafe { frame.write(4242) };
        return;
    }
    if number == 93 {
        linux::note(93);
        uart::write(b"ginux: aarch64 svc table unique=");
        uart::write_u64(u64::from(linux::unique()));
        uart::write(b"\r\n");
        uart::write(b"ginux: svc=dispatch nr=93\r\n");
        uart::write(b"ginux: process=exited\r\n");
        super::platform::finish();
    }
    match linux::call(number, args) {
        Some(result) => unsafe { frame.write(result as u64) },
        None => {
            uart::write(b"ginux: svc=unknown nr=");
            uart::write_u64(number);
            uart::write(b"\r\n");
            unsafe { frame.write((-38i64) as u64) };
        }
    }
}

#[no_mangle]
extern "C" fn ginux_on_fault() {
    let esr: u64;
    unsafe {
        core::arch::asm!("mrs {v}, esr_el1", v = out(reg) esr, options(nostack, nomem));
    }
    fault(esr);
}

fn fault(esr: u64) -> ! {
    uart::write(b"ginux: aarch64 exception esr=");
    uart::write_hex(esr);
    uart::write(b"\r\n");
    super::platform::halt(b"ginux: unexpected aarch64 exception\r\n");
}
