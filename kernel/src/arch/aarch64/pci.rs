#[derive(Clone, Copy)]
pub(crate) struct Virtio {
    pub(crate) bus: u8,
    pub(crate) device: u8,
    pub(crate) device_id: u16,
}

pub(crate) fn virtio_devices(ecam: u64, start_bus: u8, end_bus: u8, found: &mut [Virtio]) -> usize {
    if ecam == 0 {
        return 0;
    }
    let last = end_bus.min(start_bus.saturating_add(1));
    let mut count = 0;
    for bus in start_bus..=last {
        for device in 0..32 {
            let offset = (u64::from(bus - start_bus) << 20) | (u64::from(device) << 15);
            let vendor = unsafe { ((ecam + offset) as *const u16).read_volatile() };
            if vendor != 0x1af4 {
                continue;
            }
            if count == found.len() {
                return count;
            }
            let device_id = unsafe { ((ecam + offset + 2) as *const u16).read_volatile() };
            found[count] = Virtio {
                bus,
                device,
                device_id,
            };
            count += 1;
        }
    }
    count
}
