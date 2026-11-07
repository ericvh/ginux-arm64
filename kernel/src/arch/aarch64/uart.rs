use core::sync::atomic::{AtomicUsize, Ordering};

const UART_DR: usize = 0x00;
const UART_FR: usize = 0x18;
const UART_TXFF: u32 = 1 << 5;

static BASE: AtomicUsize = AtomicUsize::new(0x0900_0000);

pub(crate) fn set_base(base: u64) {
    if base != 0 {
        BASE.store(base as usize, Ordering::Relaxed);
    }
}

pub(crate) fn write(bytes: &[u8]) {
    let base = BASE.load(Ordering::Relaxed) as *mut u32;
    for byte in bytes {
        unsafe {
            while base.add(UART_FR / 4).read_volatile() & UART_TXFF != 0 {
                core::hint::spin_loop();
            }
            base.add(UART_DR / 4).write_volatile(u32::from(*byte));
        }
    }
}

pub(crate) fn write_u64(mut value: u64) {
    let mut digits = [0_u8; 20];
    let mut cursor = digits.len();
    if value == 0 {
        write(b"0");
        return;
    }
    while value != 0 {
        cursor -= 1;
        digits[cursor] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    write(&digits[cursor..]);
}

pub(crate) fn write_hex(mut value: u64) {
    let mut digits = [0_u8; 16];
    let mut cursor = digits.len();
    write(b"0x");
    if value == 0 {
        write(b"0");
        return;
    }
    while value != 0 {
        cursor -= 1;
        digits[cursor] = b"0123456789abcdef"[(value & 0xf) as usize];
        value >>= 4;
    }
    write(&digits[cursor..]);
}
