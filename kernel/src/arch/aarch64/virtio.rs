use super::memory::Frames;
use super::paging::{self, AddressSpace};
use super::uart;

const VIRTIO_BLK_TRANSITIONAL: u16 = 0x1001;
const VIRTIO_BLK_MODERN: u16 = 0x1042;
const SECTOR_BYTES: usize = 512;
const QUEUE_SIZE: u16 = 8;
const MARK: &[u8] = b"ginux-virtio-block";

const ACKNOWLEDGE: u8 = 1;
const DRIVER: u8 = 2;
const DRIVER_OK: u8 = 4;
const FEATURES_OK: u8 = 8;
const VIRTIO_F_VERSION_1: u32 = 1;

const DESC_NEXT: u16 = 1;
const DESC_WRITE: u16 = 2;

static mut SECTOR: [u8; SECTOR_BYTES] = [0; SECTOR_BYTES];
static mut READY: bool = false;

pub(crate) fn block_bytes() -> &'static [u8] {
    unsafe {
        if READY {
            &SECTOR[..SECTOR_BYTES]
        } else {
            &[]
        }
    }
}

pub(crate) fn drive_block(
    space: &AddressSpace,
    frames: &mut Frames,
    ecam: u64,
    start_bus: u8,
    bus: u8,
    device: u8,
    device_id: u16,
) -> bool {
    if device_id != VIRTIO_BLK_TRANSITIONAL && device_id != VIRTIO_BLK_MODERN {
        return false;
    }
    let Some(cfg) = config(ecam, start_bus, bus, device) else {
        uart::write(b"ginux: virtio-blk config missing\r\n");
        return false;
    };
    let mut command = cfg_read16(cfg, 0x04);
    command |= 0x6;
    cfg_write16(cfg, 0x04, command);
    let Some(caps) = caps(cfg) else {
        uart::write(b"ginux: virtio-blk has no modern cfg\r\n");
        return false;
    };
    let Some(common) = bar_address(cfg, caps.common_bar, caps.common_offset) else {
        uart::write(b"ginux: virtio-blk common bar missing\r\n");
        return false;
    };
    let Some(notify) = bar_address(cfg, caps.notify_bar, caps.notify_offset) else {
        uart::write(b"ginux: virtio-blk notify bar missing\r\n");
        return false;
    };
    let Some(device_cfg) = bar_address(cfg, caps.device_bar, caps.device_offset) else {
        uart::write(b"ginux: virtio-blk device bar missing\r\n");
        return false;
    };
    if paging::map_device(space, frames, common & !0x1f_ffff, 0x20_0000).is_none()
        || paging::map_device(space, frames, notify & !0x1f_ffff, 0x20_0000).is_none()
        || paging::map_device(space, frames, device_cfg & !0x1f_ffff, 0x20_0000).is_none()
    {
        uart::write(b"ginux: virtio-blk bar map failed\r\n");
        return false;
    }
    uart::write(b"ginux: virtio-blk bar ");
    uart::write_hex(common);
    uart::write(b"\r\n");
    if !setup(common, notify, caps.notify_multiplier, device_cfg, frames) {
        return false;
    }
    unsafe {
        if SECTOR.get(..MARK.len()) != Some(MARK) {
            uart::write(b"ginux: virtio-blk sector mismatch ");
            let mut word = [0_u8; 8];
            word.copy_from_slice(&SECTOR[..8]);
            uart::write_hex(u64::from_le_bytes(word));
            uart::write(b"\r\n");
            return false;
        }
        READY = true;
    }
    uart::write(b"ginux: virtio-blk read ok\r\n");
    true
}

struct Caps {
    common_bar: u8,
    common_offset: u32,
    notify_bar: u8,
    notify_offset: u32,
    notify_multiplier: u32,
    device_bar: u8,
    device_offset: u32,
}

fn setup(common: u64, notify: u64, multiplier: u32, device_cfg: u64, frames: &mut Frames) -> bool {
    mmio_write8(common + 0x14, 0);
    if !wait_status(common, 0) {
        uart::write(b"ginux: virtio-blk reset failed\r\n");
        return false;
    }
    mmio_write8(common + 0x14, ACKNOWLEDGE);
    mmio_write8(common + 0x14, ACKNOWLEDGE | DRIVER);
    mmio_write32(common + 0x00, 1);
    let high = mmio_read32(common + 0x04);
    mmio_write32(common + 0x00, 0);
    let _low = mmio_read32(common + 0x04);
    if high & VIRTIO_F_VERSION_1 == 0 {
        uart::write(b"ginux: virtio-blk is not version 1\r\n");
        return false;
    }
    mmio_write32(common + 0x08, 1);
    mmio_write32(common + 0x0c, VIRTIO_F_VERSION_1);
    mmio_write32(common + 0x08, 0);
    mmio_write32(common + 0x0c, 0);
    mmio_write8(common + 0x14, ACKNOWLEDGE | DRIVER | FEATURES_OK);
    if mmio_read8(common + 0x14) & FEATURES_OK == 0 {
        uart::write(b"ginux: virtio-blk rejected features\r\n");
        return false;
    }
    mmio_write16(common + 0x16, 0);
    let max = mmio_read16(common + 0x18);
    if max < QUEUE_SIZE || !QUEUE_SIZE.is_power_of_two() {
        uart::write(b"ginux: virtio-blk queue is too small\r\n");
        return false;
    }
    mmio_write16(common + 0x18, QUEUE_SIZE);
    let Some(desc) = frames.alloc() else {
        return false;
    };
    let Some(avail) = frames.alloc() else {
        return false;
    };
    let Some(used) = frames.alloc() else {
        return false;
    };
    let Some(request) = frames.alloc() else {
        return false;
    };
    fill_request(request);
    fill_queue(desc, avail, used, request);
    clean(desc, 4096);
    clean(avail, 4096);
    clean(used, 4096);
    clean(request, 0x220);
    fence();
    write_u64(common + 0x20, desc);
    write_u64(common + 0x28, avail);
    write_u64(common + 0x30, used);
    let notify_off = u64::from(mmio_read16(common + 0x1e));
    mmio_write16(common + 0x1c, 1);
    fence();
    mmio_write8(
        common + 0x14,
        ACKNOWLEDGE | DRIVER | FEATURES_OK | DRIVER_OK,
    );
    fence();
    let capacity = read_u64(device_cfg);
    uart::write(b"ginux: virtio-blk capacity ");
    uart::write_u64(capacity);
    uart::write(b"\r\n");
    let notify_addr = notify + notify_off * u64::from(multiplier);
    mmio_write16(notify_addr, 0);
    fence();
    let used_idx = (used + 2) as *const u16;
    let mut done = false;
    for _ in 0..2_000_000 {
        fence();
        if unsafe { used_idx.read_volatile() } != 0 {
            done = true;
            break;
        }
        core::hint::spin_loop();
    }
    if !done {
        uart::write(b"ginux: virtio-blk request pending\r\n");
        return false;
    }
    clean(request + 0x10, 512);
    fence();
    let status = unsafe { ((request + 0x210) as *const u8).read_volatile() };
    if status != 0 {
        uart::write(b"ginux: virtio-blk status ");
        uart::write_u64(u64::from(status));
        uart::write(b"\r\n");
        return false;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(
            (request + 0x10) as *const u8,
            SECTOR.as_mut_ptr(),
            SECTOR_BYTES,
        );
    }
    true
}

fn fill_request(page: u64) {
    unsafe {
        core::ptr::write_bytes(page as *mut u8, 0, 0x220);
        ((page + 0x210) as *mut u8).write_volatile(0xff);
    }
}

fn fill_queue(desc: u64, avail: u64, used: u64, request: u64) {
    write_desc(desc, 0, request, 16, DESC_NEXT, 1);
    write_desc(desc, 1, request + 0x10, 512, DESC_NEXT | DESC_WRITE, 2);
    write_desc(desc, 2, request + 0x210, 1, DESC_WRITE, 0);
    unsafe {
        (avail as *mut u16).write_volatile(0);
        (avail as *mut u16).add(1).write_volatile(1);
        (avail as *mut u16).add(2).write_volatile(0);
        (used as *mut u16).write_volatile(0);
        (used as *mut u16).add(1).write_volatile(0);
    }
}

fn write_desc(base: u64, index: u16, addr: u64, len: u32, flags: u16, next: u16) {
    let entry = base + u64::from(index) * 16;
    write_u64(entry, addr);
    unsafe {
        ((entry + 8) as *mut u32).write_volatile(len);
        ((entry + 12) as *mut u16).write_volatile(flags);
        ((entry + 14) as *mut u16).write_volatile(next);
    }
}

fn caps(cfg: u64) -> Option<Caps> {
    let mut found = Caps {
        common_bar: 0,
        common_offset: 0,
        notify_bar: 0,
        notify_offset: 0,
        notify_multiplier: 0,
        device_bar: 0,
        device_offset: 0,
    };
    let mut saw_common = false;
    let mut saw_notify = false;
    let mut saw_device = false;
    let mut next = cfg_read8(cfg, 0x34) & !3;
    let mut guard = 0;
    while next != 0 && guard < 32 {
        guard += 1;
        let id = cfg_read8(cfg, u64::from(next));
        let link = cfg_read8(cfg, u64::from(next) + 1);
        if id == 0x09 {
            let kind = cfg_read8(cfg, u64::from(next) + 3);
            let bar = cfg_read8(cfg, u64::from(next) + 4);
            let offset = cfg_read32(cfg, u64::from(next) + 8);
            match kind {
                1 => {
                    found.common_bar = bar;
                    found.common_offset = offset;
                    saw_common = true;
                }
                2 => {
                    found.notify_bar = bar;
                    found.notify_offset = offset;
                    found.notify_multiplier = cfg_read32(cfg, u64::from(next) + 16);
                    saw_notify = true;
                }
                4 => {
                    found.device_bar = bar;
                    found.device_offset = offset;
                    saw_device = true;
                }
                _ => {}
            }
        }
        next = link & !3;
    }
    (saw_common && saw_notify && saw_device).then_some(found)
}

fn bar_address(cfg: u64, bar: u8, offset: u32) -> Option<u64> {
    if bar > 5 {
        return None;
    }
    let raw = cfg_read32(cfg, 0x10 + u64::from(bar) * 4);
    if raw & 1 != 0 || raw == 0 {
        return None;
    }
    let low = u64::from(raw & !0xf);
    let base = if (raw >> 1) & 3 == 2 {
        let high = u64::from(cfg_read32(cfg, 0x10 + u64::from(bar + 1) * 4));
        (high << 32) | low
    } else {
        low
    };
    base.checked_add(u64::from(offset))
}

fn config(ecam: u64, start_bus: u8, bus: u8, device: u8) -> Option<u64> {
    if bus < start_bus {
        return None;
    }
    Some(ecam + ((u64::from(bus - start_bus) << 20) | (u64::from(device) << 15)))
}

fn wait_status(common: u64, expected: u8) -> bool {
    for _ in 0..100_000 {
        if mmio_read8(common + 0x14) == expected {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn cfg_read8(cfg: u64, offset: u64) -> u8 {
    unsafe { ((cfg + offset) as *const u8).read_volatile() }
}

fn cfg_read16(cfg: u64, offset: u64) -> u16 {
    unsafe { ((cfg + offset) as *const u16).read_volatile() }
}

fn cfg_read32(cfg: u64, offset: u64) -> u32 {
    unsafe { ((cfg + offset) as *const u32).read_volatile() }
}

fn cfg_write16(cfg: u64, offset: u64, value: u16) {
    unsafe { ((cfg + offset) as *mut u16).write_volatile(value) }
}

fn mmio_read8(addr: u64) -> u8 {
    unsafe { (addr as *const u8).read_volatile() }
}

fn mmio_read16(addr: u64) -> u16 {
    unsafe { (addr as *const u16).read_volatile() }
}

fn mmio_read32(addr: u64) -> u32 {
    unsafe { (addr as *const u32).read_volatile() }
}

fn mmio_write8(addr: u64, value: u8) {
    unsafe { (addr as *mut u8).write_volatile(value) }
}

fn mmio_write16(addr: u64, value: u16) {
    unsafe { (addr as *mut u16).write_volatile(value) }
}

fn mmio_write32(addr: u64, value: u32) {
    unsafe { (addr as *mut u32).write_volatile(value) }
}

fn read_u64(addr: u64) -> u64 {
    let low = u64::from(mmio_read32(addr));
    let high = u64::from(mmio_read32(addr + 4));
    (high << 32) | low
}

fn write_u64(addr: u64, value: u64) {
    mmio_write32(addr, value as u32);
    mmio_write32(addr + 4, (value >> 32) as u32);
}

fn clean(addr: u64, len: u64) {
    let mut line = addr & !63;
    let end = addr.saturating_add(len);
    while line < end {
        unsafe {
            core::arch::asm!("dc civac, {line}", line = in(reg) line, options(nostack));
        }
        line += 64;
    }
}

fn fence() {
    unsafe { core::arch::asm!("dsb sy", options(nostack)) }
}
