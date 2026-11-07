use super::memory::Frames;
use super::paging::{self, AddressSpace, UserPage};
use super::{platform, uart, virtio};

const PAGE: u64 = 4096;
const USER_BASE: u64 = 0x2000_0000;
const IMAGE_LIMIT: u64 = 0x2010_0000;
const MMAP_BASE: u64 = 0x2010_0000;
const MMAP_LIMIT: u64 = 0x201F_0000;
const STACK_VA: u64 = 0x201F_F000;
const MAX_PAGES: usize = 64;
const MAX_ELF: usize = 64 * 1024;
const MAX_LOADS: usize = 4;
const MAX_WRITE: usize = 256;
const MAX_MMAP_PAGES: u64 = 8;

const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const EM_AARCH64: u16 = 183;
const PT_LOAD: u32 = 1;
const PF_EXECUTE: u32 = 1;
const PF_WRITE: u32 = 2;

const ENOENT: i64 = -2;
const EIO: i64 = -5;
const EBADF: i64 = -9;
const ENOMEM: i64 = -12;
const EFAULT: i64 = -14;
const EINVAL: i64 = -22;

const AT_FDCWD: u64 = (-100_i64) as u64;
const DISK_FD: u64 = 3;
const DISK_PATH: &[u8] = b"/dev/vda";

#[derive(Clone, Copy)]
struct Page {
    va: u64,
}

#[derive(Clone, Copy)]
struct Segment {
    offset: usize,
    va: u64,
    filesz: u64,
    memsz: u64,
    flags: u32,
}

struct State {
    frames: Frames,
    space: AddressSpace,
    pages: [Page; MAX_PAGES],
    page_count: usize,
    break_addr: u64,
    mmap_next: u64,
    disk_open: bool,
    disk_pos: u64,
}

static mut STATE: Option<State> = None;

pub(crate) struct Loaded {
    pub(crate) entry: u64,
    pub(crate) stack: u64,
}

pub(crate) fn load(
    mut frames: Frames,
    space: &AddressSpace,
    pointer: *const u8,
    size: usize,
) -> Option<Loaded> {
    if pointer.is_null() || !(64..=MAX_ELF).contains(&size) {
        uart::write(b"ginux: controller elf missing\r\n");
        return None;
    }
    // SAFETY: the bootloader measured this buffer before handoff and the
    // firmware map still covers the image that holds it.
    let bytes = unsafe { core::slice::from_raw_parts(pointer, size) };
    let image = parse(bytes)?;
    uart::write(b"ginux: controller elf aarch64\r\n");
    let mut pages = [Page { va: 0 }; MAX_PAGES];
    let mut page_count = 0;
    let mut image_end = USER_BASE;
    let mut entry_ok = false;
    for segment in image.segments.iter().take(image.segment_count) {
        let mut page = segment.va;
        let end = segment.va.checked_add(segment.memsz)?;
        if segment.va < USER_BASE || end > IMAGE_LIMIT {
            uart::write(b"ginux: controller elf is outside the user window\r\n");
            return None;
        }
        while page < end {
            let pa = frames.alloc()?;
            let page_index = (page - segment.va) / PAGE;
            let copied = copy_page(bytes, segment, page_index, pa)?;
            let executable = segment.flags & PF_EXECUTE != 0 && segment.flags & PF_WRITE == 0;
            let access = if executable {
                UserPage::Execute
            } else if segment.flags & PF_WRITE != 0 {
                UserPage::Write
            } else {
                UserPage::Read
            };
            paging::map_user_page(space, &mut frames, page, pa, access)?;
            if executable {
                unsafe {
                    platform::clean_executable(pa, copied);
                    platform::clean_executable(page, copied);
                }
            }
            remember(&mut pages, &mut page_count, page)?;
            page = page.checked_add(PAGE)?;
        }
        image_end = image_end.max(end);
        if segment.flags & PF_EXECUTE != 0 && image.entry >= segment.va && image.entry < end {
            entry_ok = true;
        }
    }
    if !entry_ok {
        uart::write(b"ginux: controller elf entry is not executable\r\n");
        return None;
    }
    let stack_pa = frames.alloc()?;
    paging::map_user_page(space, &mut frames, STACK_VA, stack_pa, UserPage::Write)?;
    remember(&mut pages, &mut page_count, STACK_VA)?;
    let break_addr = align_up(image_end)?;
    if break_addr > IMAGE_LIMIT {
        return None;
    }
    unsafe {
        STATE = Some(State {
            frames,
            space: *space,
            pages,
            page_count,
            break_addr,
            mmap_next: MMAP_BASE,
            disk_open: false,
            disk_pos: 0,
        });
    }
    uart::write(b"ginux: controller elf loaded\r\n");
    uart::write(b"ginux: controller entry ");
    uart::write_hex(image.entry);
    uart::write(b"\r\n");
    Some(Loaded {
        entry: image.entry,
        stack: STACK_VA + PAGE,
    })
}

pub(crate) fn openat(dirfd: u64, path: u64) -> i64 {
    if dirfd != AT_FDCWD {
        return EBADF;
    }
    if !virtio::ready() {
        return EIO;
    }
    match path_eq(path, DISK_PATH) {
        Err(errno) => errno,
        Ok(false) => ENOENT,
        Ok(true) => {
            let Some(state) = state() else {
                return EIO;
            };
            state.disk_open = true;
            state.disk_pos = 0;
            DISK_FD as i64
        }
    }
}

pub(crate) fn uname(pointer: u64) -> i64 {
    if pointer == 0 {
        return EFAULT;
    }
    let mut name = [0_u8; 65 * 6];
    copy_field(&mut name[0..65], b"ginux");
    copy_field(&mut name[65..130], b"ginux");
    copy_field(&mut name[130..195], b"0.1.0");
    copy_field(&mut name[195..260], b"sbsa");
    copy_field(&mut name[260..325], b"aarch64");
    copy_field(&mut name[325..390], b"(none)");
    if !write_bytes(pointer, &name) {
        return EFAULT;
    }
    0
}

fn copy_field(field: &mut [u8], value: &[u8]) {
    field[..value.len()].copy_from_slice(value);
}

pub(crate) fn fstat(fd: u64, pointer: u64) -> i64 {
    if fd != DISK_FD || !disk_open() {
        return EBADF;
    }
    let sectors = virtio::capacity_sectors();
    let bytes = sectors.saturating_mul(512);
    let mut stat = [0_u8; 128];
    stat[16..20].copy_from_slice(&0x6180_u32.to_le_bytes());
    stat[20..24].copy_from_slice(&1_u32.to_le_bytes());
    stat[32..40].copy_from_slice(&0xfe00_u64.to_le_bytes());
    stat[48..56].copy_from_slice(&bytes.to_le_bytes());
    stat[56..60].copy_from_slice(&512_u32.to_le_bytes());
    stat[64..72].copy_from_slice(&sectors.to_le_bytes());
    if !write_bytes(pointer, &stat) {
        return EFAULT;
    }
    0
}

pub(crate) fn close(fd: u64) -> i64 {
    if fd == DISK_FD && disk_open() {
        if let Some(state) = state() {
            state.disk_open = false;
        }
        return 0;
    }
    EBADF
}

pub(crate) fn lseek(fd: u64, offset: u64, whence: u64) -> i64 {
    if fd != DISK_FD || !disk_open() {
        return EBADF;
    }
    let end = virtio::capacity_sectors().saturating_mul(SECTOR);
    let next = match whence {
        0 => offset as i64,
        1 => match (disk_pos() as i64).checked_add(offset as i64) {
            Some(value) => value,
            None => return EINVAL,
        },
        2 => match (end as i64).checked_add(offset as i64) {
            Some(value) => value,
            None => return EINVAL,
        },
        _ => return EINVAL,
    };
    if next < 0 || next as u64 > end {
        return EINVAL;
    }
    if let Some(state) = state() {
        state.disk_pos = next as u64;
    }
    next
}

pub(crate) fn pread(fd: u64, pointer: u64, length: u64, offset: u64) -> i64 {
    if fd != DISK_FD {
        return EBADF;
    }
    block_read(pointer, length, offset, false)
}

pub(crate) fn pwrite(fd: u64, pointer: u64, length: u64, offset: u64) -> i64 {
    if fd != DISK_FD {
        return EBADF;
    }
    block_write(pointer, length, offset, false)
}

pub(crate) fn write(fd: u64, pointer: u64, length: u64) -> i64 {
    if fd == DISK_FD {
        return block_write(pointer, length, disk_pos(), true);
    }
    if fd != 1 {
        return EBADF;
    }
    if length == 0 {
        return 0;
    }
    if length > MAX_WRITE as u64 {
        return EFAULT;
    }
    let Some(state) = state() else {
        return EFAULT;
    };
    if !covers(state, pointer, length) {
        return EFAULT;
    }
    let mut buffer = [0_u8; MAX_WRITE];
    let size = length as usize;
    unsafe {
        core::ptr::copy_nonoverlapping(pointer as *const u8, buffer.as_mut_ptr(), size);
    }
    uart::write(&buffer[..size]);
    length as i64
}

pub(crate) fn read(fd: u64, pointer: u64, length: u64) -> i64 {
    if fd == DISK_FD {
        return block_read(pointer, length, disk_pos(), true);
    }
    if fd != 0 {
        return EBADF;
    }
    if length == 0 {
        return 0;
    }
    let data = virtio::block_bytes();
    if data.is_empty() {
        return EIO;
    }
    let size = length.min(data.len() as u64).min(MAX_WRITE as u64) as usize;
    let Some(state) = state() else {
        return EFAULT;
    };
    if !covers(state, pointer, size as u64) {
        return EFAULT;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr(), pointer as *mut u8, size);
    }
    size as i64
}

const SECTOR: u64 = 512;

fn disk_open() -> bool {
    state().is_some_and(|state| state.disk_open)
}

fn disk_pos() -> u64 {
    state().map(|state| state.disk_pos).unwrap_or(0)
}

fn path_eq(pointer: u64, expected: &[u8]) -> Result<bool, i64> {
    if expected.len() + 1 > 64 {
        return Err(EINVAL);
    }
    let mut buffer = [0_u8; 64];
    let length = expected.len() + 1;
    if !read_bytes(pointer, &mut buffer[..length]) {
        return Err(EFAULT);
    }
    Ok(&buffer[..expected.len()] == expected && buffer[expected.len()] == 0)
}

fn block_span(offset: u64, length: u64) -> Result<Option<(u64, usize, usize)>, i64> {
    let end = virtio::capacity_sectors().saturating_mul(SECTOR);
    if offset > end {
        return Err(EINVAL);
    }
    if offset == end || length == 0 {
        return Ok(None);
    }
    let sector = offset / SECTOR;
    let skip = (offset % SECTOR) as usize;
    let room = SECTOR as usize - skip;
    let size = (length as usize).min(room).min(MAX_WRITE);
    Ok(Some((sector, skip, size)))
}

fn block_read(pointer: u64, length: u64, offset: u64, update: bool) -> i64 {
    if !disk_open() {
        return EBADF;
    }
    let Some((sector, skip, size)) = (match block_span(offset, length) {
        Ok(span) => span,
        Err(errno) => return errno,
    }) else {
        return 0;
    };
    let mut sector_buf = [0_u8; 512];
    if !virtio::sector_io(sector, false, &mut sector_buf) {
        return EIO;
    }
    let Some(state) = state() else {
        return EFAULT;
    };
    if !covers(state, pointer, size as u64) {
        return EFAULT;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(sector_buf[skip..].as_ptr(), pointer as *mut u8, size);
    }
    if update {
        state.disk_pos = offset + size as u64;
    }
    size as i64
}

fn block_write(pointer: u64, length: u64, offset: u64, update: bool) -> i64 {
    if !disk_open() {
        return EBADF;
    }
    let Some((sector, skip, size)) = (match block_span(offset, length) {
        Ok(span) => span,
        Err(errno) => return errno,
    }) else {
        return EINVAL;
    };
    let mut sector_buf = [0_u8; 512];
    if !virtio::sector_io(sector, false, &mut sector_buf) {
        return EIO;
    }
    if !read_bytes(pointer, &mut sector_buf[skip..skip + size]) {
        return EFAULT;
    }
    if !virtio::sector_io(sector, true, &mut sector_buf) {
        return EIO;
    }
    if update {
        if let Some(state) = state() {
            state.disk_pos = offset + size as u64;
        }
    }
    size as i64
}

pub(crate) fn read_bytes(pointer: u64, dest: &mut [u8]) -> bool {
    let Some(state) = state() else {
        return false;
    };
    if dest.is_empty() || !covers(state, pointer, dest.len() as u64) {
        return false;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(pointer as *const u8, dest.as_mut_ptr(), dest.len());
    }
    true
}

pub(crate) fn write_bytes(pointer: u64, source: &[u8]) -> bool {
    let Some(state) = state() else {
        return false;
    };
    if source.is_empty() || !covers(state, pointer, source.len() as u64) {
        return false;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(source.as_ptr(), pointer as *mut u8, source.len());
    }
    true
}

pub(crate) fn stack_ok(top: u64) -> bool {
    let Some(state) = state() else {
        return false;
    };
    top >= 16 && top & 15 == 0 && covers(state, top - 16, 16)
}

pub(crate) fn brk(address: u64) -> i64 {
    let Some(state) = state() else {
        return ENOMEM;
    };
    if address == 0 || address == state.break_addr {
        return state.break_addr as i64;
    }
    if address < state.break_addr || address > IMAGE_LIMIT {
        return ENOMEM;
    }
    let mut page = align_up(state.break_addr).unwrap_or(state.break_addr);
    while page < address {
        let Some(pa) = state.frames.alloc() else {
            return ENOMEM;
        };
        if paging::map_user_page(&state.space, &mut state.frames, page, pa, UserPage::Write)
            .is_none()
        {
            return ENOMEM;
        }
        if remember(&mut state.pages, &mut state.page_count, page).is_none() {
            return ENOMEM;
        }
        page += PAGE;
    }
    state.break_addr = address;
    address as i64
}

pub(crate) fn mmap(_address: u64, length: u64, prot: u64, flags: u64) -> i64 {
    if length == 0 {
        return EINVAL;
    }
    if flags & 0x20 == 0 {
        return EBADF;
    }
    if prot & 0x7 == 0 {
        return EINVAL;
    }
    let Some(state) = state() else {
        return ENOMEM;
    };
    let pages = length.div_ceil(PAGE);
    if pages == 0 || pages > MAX_MMAP_PAGES {
        return ENOMEM;
    }
    let start = state.mmap_next;
    let Some(end) = start.checked_add(pages * PAGE) else {
        return ENOMEM;
    };
    if start < MMAP_BASE || end > MMAP_LIMIT {
        return ENOMEM;
    }
    let access = if prot & 2 != 0 {
        UserPage::Write
    } else if prot & 1 != 0 {
        UserPage::Read
    } else {
        UserPage::Execute
    };
    let mut page = start;
    while page < end {
        let Some(pa) = state.frames.alloc() else {
            return ENOMEM;
        };
        if paging::map_user_page(&state.space, &mut state.frames, page, pa, access).is_none() {
            return ENOMEM;
        }
        if remember(&mut state.pages, &mut state.page_count, page).is_none() {
            return ENOMEM;
        }
        page += PAGE;
    }
    state.mmap_next = end;
    start as i64
}

struct Image {
    entry: u64,
    segments: [Segment; MAX_LOADS],
    segment_count: usize,
}

fn parse(bytes: &[u8]) -> Option<Image> {
    if bytes.get(..16)? != b"\x7fELF\x02\x01\x01\0\0\0\0\0\0\0\0\0" {
        uart::write(b"ginux: controller elf header rejected\r\n");
        return None;
    }
    let kind = read_u16(bytes, 16)?;
    if (kind != ET_EXEC && kind != ET_DYN)
        || read_u16(bytes, 18)? != EM_AARCH64
        || read_u32(bytes, 20)? != 1
        || usize::from(read_u16(bytes, 52)?) != 64
        || usize::from(read_u16(bytes, 54)?) != 56
    {
        uart::write(b"ginux: controller elf is not aarch64\r\n");
        return None;
    }
    let entry = read_u64(bytes, 24)?;
    let phoff = usize::try_from(read_u64(bytes, 32)?).ok()?;
    let phnum = usize::from(read_u16(bytes, 56)?);
    if phnum == 0 || phoff.checked_add(phnum.checked_mul(56)?)? > bytes.len() {
        return None;
    }
    let mut lowest = u64::MAX;
    let mut segments = [Segment {
        offset: 0,
        va: 0,
        filesz: 0,
        memsz: 0,
        flags: 0,
    }; MAX_LOADS];
    let mut segment_count = 0;
    for index in 0..phnum {
        let offset = phoff + index * 56;
        let kind = read_u32(bytes, offset)?;
        if kind != PT_LOAD {
            continue;
        }
        if segment_count == MAX_LOADS {
            return None;
        }
        let flags = read_u32(bytes, offset + 4)?;
        let file_offset = usize::try_from(read_u64(bytes, offset + 8)?).ok()?;
        let va = read_u64(bytes, offset + 16)?;
        let filesz = read_u64(bytes, offset + 32)?;
        let memsz = read_u64(bytes, offset + 40)?;
        if va & (PAGE - 1) != 0 || memsz == 0 || filesz > memsz {
            return None;
        }
        if file_offset.checked_add(usize::try_from(filesz).ok()?)? > bytes.len() {
            return None;
        }
        lowest = lowest.min(va);
        segments[segment_count] = Segment {
            offset: file_offset,
            va,
            filesz,
            memsz,
            flags,
        };
        segment_count += 1;
    }
    if segment_count == 0 {
        return None;
    }
    let bias = if kind == ET_DYN && lowest < USER_BASE {
        USER_BASE.checked_sub(lowest)?
    } else {
        0
    };
    if bias != 0 {
        for segment in segments.iter_mut().take(segment_count) {
            segment.va = segment.va.checked_add(bias)?;
        }
    }
    Some(Image {
        entry: entry.checked_add(bias)?,
        segments,
        segment_count,
    })
}

fn copy_page(bytes: &[u8], segment: &Segment, page_index: u64, pa: u64) -> Option<u64> {
    let start = page_index.checked_mul(PAGE)?;
    if start >= segment.filesz {
        return Some(0);
    }
    let remaining = segment.filesz - start;
    let size = usize::try_from(remaining.min(PAGE)).ok()?;
    let file_offset = segment.offset.checked_add(usize::try_from(start).ok()?)?;
    let source = bytes.get(file_offset..file_offset.checked_add(size)?)?;
    unsafe {
        core::ptr::copy_nonoverlapping(source.as_ptr(), pa as *mut u8, size);
    }
    Some(size as u64)
}

fn remember(pages: &mut [Page; MAX_PAGES], count: &mut usize, va: u64) -> Option<()> {
    if *count == pages.len() {
        return None;
    }
    pages[*count] = Page { va };
    *count += 1;
    Some(())
}

fn covers(state: &State, address: u64, length: u64) -> bool {
    let Some(end) = address.checked_add(length) else {
        return false;
    };
    let mut page = address & !(PAGE - 1);
    while page < end {
        if !state.pages[..state.page_count]
            .iter()
            .any(|mapped| mapped.va == page)
        {
            return false;
        }
        page += PAGE;
    }
    true
}

fn align_up(value: u64) -> Option<u64> {
    value
        .checked_add(PAGE - 1)
        .map(|aligned| aligned & !(PAGE - 1))
}

#[allow(static_mut_refs)]
fn state() -> Option<&'static mut State> {
    unsafe { STATE.as_mut() }
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        bytes.get(offset..offset + 8)?.try_into().ok()?,
    ))
}
