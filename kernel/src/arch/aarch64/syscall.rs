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
        let pid = current_pid();
        log_result(172, b"getpid", pid as i64);
        unsafe { frame.write(pid) };
        return;
    }
    if number == 220 || number == 435 {
        let name: &[u8] = if number == 220 { b"clone" } else { b"clone3" };
        let child_sp = if number == 220 {
            args[1]
        } else {
            match clone3_stack(args[0], args[1]) {
                Ok(top) => top,
                Err(errno) => {
                    log_result(number, name, errno);
                    unsafe { frame.write(errno as u64) };
                    return;
                }
            }
        };
        match spawn(frame, child_sp) {
            Ok(pid) => {
                log_result(number, name, pid as i64);
                return;
            }
            Err(errno) => {
                log_result(number, name, errno);
                unsafe { frame.write(errno as u64) };
                return;
            }
        }
    }
    if number == 93 {
        linux::note(93);
        uart::write(b"ginux: svc=dispatch nr=93\r\n");
        if resume_parent(frame) {
            uart::write(b"ginux: clone child returned\r\n");
            return;
        }
        uart::write(b"ginux: aarch64 svc table unique=");
        uart::write_u64(u64::from(linux::unique()));
        uart::write(b"\r\n");
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

const PARENT_PID: u64 = 4242;
const EFAULT: i64 = -14;
const EAGAIN: i64 = -11;
const EINVAL: i64 = -22;

struct Saved {
    elr: u64,
    spsr: u64,
    sp: u64,
    regs: [u64; 10],
}

static mut CURRENT: u64 = PARENT_PID;
static mut NEXT_PID: u64 = PARENT_PID + 1;
static mut SAVED: Option<Saved> = None;

fn current_pid() -> u64 {
    unsafe { CURRENT }
}

fn clone3_stack(pointer: u64, size: u64) -> Result<u64, i64> {
    if size < 64 {
        return Err(EINVAL);
    }
    let mut raw = [0_u8; 64];
    if !super::user::read_bytes(pointer, &mut raw) {
        return Err(EFAULT);
    }
    let stack = u64::from_le_bytes(raw[40..48].try_into().unwrap());
    let stack_size = u64::from_le_bytes(raw[48..56].try_into().unwrap());
    let Some(top) = stack.checked_add(stack_size) else {
        return Err(EINVAL);
    };
    if top == 0 {
        return Err(EINVAL);
    }
    Ok(top & !15)
}

#[allow(static_mut_refs)]
fn spawn(frame: *mut u64, child_sp: u64) -> Result<u64, i64> {
    if unsafe { SAVED.is_some() } {
        return Err(EAGAIN);
    }
    if !super::user::stack_ok(child_sp) {
        return Err(EFAULT);
    }
    let elr: u64;
    let spsr: u64;
    let sp: u64;
    unsafe {
        core::arch::asm!(
            "mrs {elr}, elr_el1",
            "mrs {spsr}, spsr_el1",
            "mrs {sp}, sp_el0",
            elr = out(reg) elr,
            spsr = out(reg) spsr,
            sp = out(reg) sp,
            options(nostack, nomem),
        );
    }
    let mut regs = [0_u64; 10];
    for index in 0..10 {
        regs[index] = unsafe { frame.add(index).read() };
    }
    let pid = unsafe { NEXT_PID };
    unsafe { NEXT_PID += 1 };
    regs[0] = pid;
    unsafe {
        SAVED = Some(Saved {
            elr,
            spsr,
            sp,
            regs,
        });
        CURRENT = pid;
        core::arch::asm!("msr sp_el0, {sp}", sp = in(reg) child_sp, options(nostack));
        frame.write(0);
    }
    Ok(pid)
}

#[allow(static_mut_refs)]
fn resume_parent(frame: *mut u64) -> bool {
    let Some(saved) = (unsafe { SAVED.take() }) else {
        return false;
    };
    unsafe {
        CURRENT = PARENT_PID;
        for index in 0..10 {
            frame.add(index).write(saved.regs[index]);
        }
        core::arch::asm!(
            "msr elr_el1, {elr}",
            "msr spsr_el1, {spsr}",
            "msr sp_el0, {sp}",
            elr = in(reg) saved.elr,
            spsr = in(reg) saved.spsr,
            sp = in(reg) saved.sp,
            options(nostack),
        );
    }
    true
}

fn log_result(number: u64, name: &[u8], result: i64) {
    linux::note(number);
    uart::write(b"ginux: svc=dispatch nr=");
    uart::write_u64(number);
    uart::write(b" ");
    uart::write(name);
    uart::write(b" result=");
    if result < 0 {
        uart::write(b"-");
        uart::write_u64((-result) as u64);
    } else {
        uart::write_u64(result as u64);
    }
    uart::write(b"\r\n");
}

fn fault(esr: u64) -> ! {
    uart::write(b"ginux: aarch64 exception esr=");
    uart::write_hex(esr);
    uart::write(b"\r\n");
    super::platform::halt(b"ginux: unexpected aarch64 exception\r\n");
}
