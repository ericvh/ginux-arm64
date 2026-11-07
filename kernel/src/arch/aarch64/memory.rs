const CONVENTIONAL: u32 = 7;
const MAX_REGIONS: usize = 24;

pub(crate) struct Frames {
    regions: [(u64, u64); MAX_REGIONS],
    count: usize,
    index: usize,
    page: u64,
    allocated: u64,
}

impl Frames {
    pub(crate) fn allocated(&self) -> u64 {
        self.allocated
    }

    pub(crate) fn alloc(&mut self) -> Option<u64> {
        while self.index < self.count {
            let (base, pages) = self.regions[self.index];
            if self.page < pages {
                let address = base + self.page * 4096;
                self.page += 1;
                self.allocated += 1;
                unsafe {
                    core::ptr::write_bytes(address as *mut u8, 0, 4096);
                }
                return Some(address);
            }
            self.index += 1;
            self.page = 0;
        }
        None
    }
}

pub(crate) fn conventional_frames(
    memory_map: *const u8,
    memory_map_size: usize,
    descriptor_size: usize,
    image_base: u64,
    image_size: u64,
) -> Option<Frames> {
    if memory_map.is_null() || descriptor_size < 40 || !memory_map_size.is_multiple_of(descriptor_size)
    {
        return None;
    }
    let image_end = image_base.checked_add(image_size)?;
    let mut frames = Frames {
        regions: [(0, 0); MAX_REGIONS],
        count: 0,
        index: 0,
        page: 0,
        allocated: 0,
    };
    let descriptors = memory_map_size / descriptor_size;
    for index in 0..descriptors {
        let descriptor = unsafe { memory_map.add(index * descriptor_size) };
        let kind = unsafe { core::ptr::read_unaligned(descriptor.cast::<u32>()) };
        if kind != CONVENTIONAL {
            continue;
        }
        let start = unsafe { core::ptr::read_unaligned(descriptor.add(8).cast::<u64>()) };
        let pages = unsafe { core::ptr::read_unaligned(descriptor.add(24).cast::<u64>()) };
        let end = start.checked_add(pages.checked_mul(4096)?)?;
        push_excluding(&mut frames, start, end, image_base, image_end)?;
    }
    (frames.count != 0).then_some(frames)
}

fn push_excluding(
    frames: &mut Frames,
    start: u64,
    end: u64,
    hole_start: u64,
    hole_end: u64,
) -> Option<()> {
    if end <= hole_start || start >= hole_end {
        return push_aligned(frames, start, end);
    }
    if start < hole_start {
        push_aligned(frames, start, hole_start)?;
    }
    if hole_end < end {
        push_aligned(frames, hole_end, end)?;
    }
    Some(())
}

fn push_aligned(frames: &mut Frames, start: u64, end: u64) -> Option<()> {
    let start = start.checked_add(4095)? & !4095;
    let end = end & !4095;
    if start >= end {
        return Some(());
    }
    if frames.count == MAX_REGIONS {
        return None;
    }
    frames.regions[frames.count] = (start, (end - start) / 4096);
    frames.count += 1;
    Some(())
}

#[derive(Clone, Copy)]
pub(crate) struct MapRange {
    pub(crate) start: u64,
    pub(crate) end: u64,
    pub(crate) device: bool,
}

pub(crate) fn firmware_ranges(
    memory_map: *const u8,
    memory_map_size: usize,
    descriptor_size: usize,
) -> Option<RangeList> {
    let descriptors = memory_map_size / descriptor_size;
    let mut ranges = RangeList {
        items: [MapRange {
            start: 0,
            end: 0,
            device: false,
        }; 64],
        count: 0,
    };
    for index in 0..descriptors {
        let descriptor = unsafe { memory_map.add(index * descriptor_size) };
        let kind = unsafe { core::ptr::read_unaligned(descriptor.cast::<u32>()) };
        let start = unsafe { core::ptr::read_unaligned(descriptor.add(8).cast::<u64>()) };
        let pages = unsafe { core::ptr::read_unaligned(descriptor.add(24).cast::<u64>()) };
        let end = start.checked_add(pages.checked_mul(4096)?)?;
        let device = matches!(kind, 11 | 12);
        let normal = matches!(kind, 1 | 2 | 3 | 4 | 5 | 6 | 7 | 9);
        if !(device || normal) || start == end {
            continue;
        }
        if ranges.count == ranges.items.len() {
            return None;
        }
        ranges.items[ranges.count] = MapRange { start, end, device };
        ranges.count += 1;
    }
    Some(ranges)
}

pub(crate) struct RangeList {
    items: [MapRange; 64],
    count: usize,
}

impl RangeList {
    pub(crate) fn as_slice(&self) -> &[MapRange] {
        &self.items[..self.count]
    }

    pub(crate) fn push(&mut self, range: MapRange) -> bool {
        if self.count == self.items.len() {
            return false;
        }
        self.items[self.count] = range;
        self.count += 1;
        true
    }
}
