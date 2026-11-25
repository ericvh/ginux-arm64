use super::uart;
use super::virtio;

const SUPERBLOCK: u64 = 1024;
const EXT4_MAGIC: u16 = 0xEF53;
const EXTENT_MAGIC: u16 = 0xF30A;
const EXTENTS_FL: u32 = 0x80000;
const FILETYPE_CSUM: u8 = 0xDE;
const S_IFDIR: u16 = 0x4000;
const S_IFREG: u16 = 0x8000;
const ROOT_INODE: u32 = 2;
const FILE_MARK: &[u8] = b"ginux-ext4-file\n";

#[derive(Clone, Copy)]
struct Super {
    block_size: u64,
    inode_size: u64,
    inodes_per_group: u32,
    blocks_count: u32,
    inode_table: u64,
    block_bitmap: u64,
    desc_size: u64,
    gd_block: u64,
}

struct Inode {
    number: u32,
    mode: u16,
    size: u64,
    blocks: u64,
    extents: [Extent; 4],
    extent_count: u16,
    extent_max: u16,
}

#[derive(Clone, Copy)]
struct Extent {
    block: u32,
    len: u32,
    start: u64,
}

static mut SUPER: Option<Super> = None;

pub(crate) struct OpenFile {
    pub(crate) number: u32,
    pub(crate) mode: u16,
    pub(crate) size: u64,
}

pub(crate) fn mount() -> bool {
    let Some(superblock) = read_super() else {
        uart::write(b"ginux: ext4 superblock missing\r\n");
        return false;
    };
    unsafe { SUPER = Some(superblock) };
    uart::write(b"ginux: ext4 block ");
    uart::write_u64(superblock.block_size);
    uart::write(b"\r\n");
    match lookup(b"/ginux.txt") {
        Ok(file) => {
            let mut found = [0_u8; FILE_MARK.len()];
            if read(file.number, 0, &mut found) != Some(FILE_MARK.len()) || found != FILE_MARK {
                uart::write(b"ginux: ext4 file mismatch\r\n");
                return false;
            }
        }
        Err(_) => {
            uart::write(b"ginux: ext4 file missing\r\n");
            return false;
        }
    }
    uart::write(b"ginux: ext4 file ok\r\n");
    true
}

pub(crate) fn lookup(path: &[u8]) -> Result<OpenFile, i64> {
    const EINVAL: i64 = -22;
    const EISDIR: i64 = -21;
    const ENOTDIR: i64 = -20;
    const ENOENT: i64 = -2;
    const EIO: i64 = -5;
    if path.first() != Some(&b'/') {
        return Err(EINVAL);
    }
    let Some(mut current) = read_inode(ROOT_INODE) else {
        return Err(EIO);
    };
    let mut rest = &path[1..];
    if rest.is_empty() {
        return Err(EISDIR);
    }
    loop {
        let (name, next) = match rest.iter().position(|byte| *byte == b'/') {
            Some(index) => (&rest[..index], &rest[index + 1..]),
            None => (rest, &[][..]),
        };
        if name.is_empty() {
            return Err(EINVAL);
        }
        if current.mode & 0xF000 != S_IFDIR {
            return Err(ENOTDIR);
        }
        let Some(child) = find_entry(&current, name) else {
            return Err(ENOENT);
        };
        current = read_inode(child).ok_or(EIO)?;
        if next.is_empty() {
            if current.mode & 0xF000 == S_IFDIR {
                return Err(EISDIR);
            }
            if current.mode & 0xF000 != S_IFREG {
                return Err(EINVAL);
            }
            return Ok(OpenFile {
                number: current.number,
                mode: current.mode,
                size: current.size,
            });
        }
        rest = next;
    }
}

pub(crate) fn write(number: u32, offset: u64, src: &[u8]) -> Result<usize, i64> {
    const EINVAL: i64 = -22;
    const EIO: i64 = -5;
    const ENOSPC: i64 = -28;
    if src.is_empty() {
        return Ok(0);
    }
    let mut inode = read_inode(number).ok_or(EIO)?;
    let end = offset.checked_add(src.len() as u64).ok_or(EINVAL)?;
    let block_size = superblock().ok_or(EIO)?.block_size;
    if block_size == 0 {
        return Err(EINVAL);
    }
    let mut logical = offset / block_size;
    let last = (end - 1) / block_size;
    let mut dirty = false;
    loop {
        if map_block(&inode, logical).is_none() {
            let Some(logical_block) = u32::try_from(logical).ok() else {
                return Err(ENOSPC);
            };
            let physical = allocate_block()?;
            if !add_extent(&mut inode, logical_block, physical, block_size) {
                return Err(ENOSPC);
            }
            dirty = true;
            uart::write(b"ginux: ext4 alloc ");
            uart::write_u64(physical);
            uart::write(b"\r\n");
        }
        if logical == last {
            break;
        }
        logical += 1;
    }
    let mut filled = 0;
    while filled < src.len() {
        let pos = offset + filled as u64;
        let logical = pos / block_size;
        let skip = (pos % block_size) as usize;
        let block = map_block(&inode, logical).ok_or(EIO)?;
        let mut data = [0_u8; 4096];
        let block_size_bytes = block_size as usize;
        if block_size_bytes > data.len() || !read_block(block, &mut data[..block_size_bytes]) {
            return Err(EIO);
        }
        let room = block_size_bytes - skip;
        let n = (src.len() - filled).min(room);
        data[skip..skip + n].copy_from_slice(&src[filled..filled + n]);
        if !write_block(block, &data[..block_size_bytes]) {
            return Err(EIO);
        }
        filled += n;
    }
    if end > inode.size {
        inode.size = end;
        dirty = true;
    }
    if dirty && !store_inode(&inode) {
        return Err(EIO);
    }
    Ok(filled)
}

pub(crate) fn read(inode: u32, offset: u64, dest: &mut [u8]) -> Option<usize> {
    let inode = read_inode(inode)?;
    if offset > inode.size {
        return None;
    }
    let available = (inode.size - offset) as usize;
    let size = dest.len().min(available);
    let mut filled = 0;
    while filled < size {
        let pos = offset + filled as u64;
        let logical = pos / superblock()?.block_size;
        let skip = (pos % superblock()?.block_size) as usize;
        let block = map_block(&inode, logical)?;
        let mut data = [0_u8; 4096];
        let block_size = superblock()?.block_size as usize;
        if block_size > data.len() || !read_block(block, &mut data[..block_size]) {
            return None;
        }
        let room = block_size - skip;
        let n = (size - filled).min(room);
        dest[filled..filled + n].copy_from_slice(&data[skip..skip + n]);
        filled += n;
    }
    Some(size)
}

fn read_super() -> Option<Super> {
    let mut raw = [0_u8; 256];
    if !read_at(SUPERBLOCK, &mut raw) {
        return None;
    }
    if u16_at(&raw, 0x38) != EXT4_MAGIC {
        return None;
    }
    let log_block = u32_at(&raw, 0x18);
    if log_block > 2 {
        return None;
    }
    let block_size = 1024_u64 << log_block;
    let inode_size = u64::from(u16_at(&raw, 0x58));
    if !(128..=256).contains(&inode_size) {
        return None;
    }
    let incompat = u32_at(&raw, 0x60);
    let desc_size = if incompat & 0x80 != 0 {
        u64::from(u16_at(&raw, 0xFE)).max(64)
    } else {
        32
    };
    let gd_block = if block_size == 1024 { 2 } else { 1 };
    let mut gd = [0_u8; 64];
    if desc_size as usize > gd.len()
        || !read_at(gd_block * block_size, &mut gd[..desc_size as usize])
    {
        return None;
    }
    let table_lo = u64::from(u32_at(&gd, 8));
    let table_hi = if desc_size >= 0x2C {
        u64::from(u32_at(&gd, 0x28))
    } else {
        0
    };
    let bitmap_lo = u64::from(u32_at(&gd, 0));
    let bitmap_hi = if desc_size >= 0x24 {
        u64::from(u32_at(&gd, 0x20))
    } else {
        0
    };
    Some(Super {
        block_size,
        inode_size,
        inodes_per_group: u32_at(&raw, 0x28),
        blocks_count: u32_at(&raw, 4),
        inode_table: (table_hi << 32) | table_lo,
        block_bitmap: (bitmap_hi << 32) | bitmap_lo,
        desc_size,
        gd_block,
    })
}

fn read_inode(number: u32) -> Option<Inode> {
    if number == 0 {
        return None;
    }
    let superblock = superblock()?;
    if superblock.inodes_per_group == 0 {
        return None;
    }
    let index = number - 1;
    let group = index / superblock.inodes_per_group;
    if group != 0 {
        return None;
    }
    let local = u64::from(index % superblock.inodes_per_group);
    let offset = superblock.inode_table * superblock.block_size + local * superblock.inode_size;
    let mut raw = [0_u8; 256];
    let inode_size = superblock.inode_size as usize;
    if inode_size > raw.len() || !read_at(offset, &mut raw[..inode_size]) {
        return None;
    }
    let flags = u32_at(&raw, 32);
    if flags & EXTENTS_FL == 0 {
        return None;
    }
    let mut extents = [Extent {
        block: 0,
        len: 0,
        start: 0,
    }; 4];
    if u16_at(&raw, 40) != EXTENT_MAGIC || u16_at(&raw, 46) != 0 {
        return None;
    }
    let count = u16_at(&raw, 42);
    let extent_max = u16_at(&raw, 44);
    if count > 4 {
        return None;
    }
    for index in 0..count {
        let at = 52 + usize::from(index) * 12;
        extents[usize::from(index)] = Extent {
            block: u32_at(&raw, at),
            len: u32::from(u16_at(&raw, at + 4) & 0x7FFF),
            start: (u64::from(u16_at(&raw, at + 6)) << 32) | u64::from(u32_at(&raw, at + 8)),
        };
    }
    let size_lo = u64::from(u32_at(&raw, 4));
    let size_hi = if inode_size >= 112 {
        u64::from(u32_at(&raw, 108))
    } else {
        0
    };
    let blocks_lo = u64::from(u32_at(&raw, 28));
    let blocks_hi = if inode_size >= 0x76 {
        u64::from(u16_at(&raw, 0x74))
    } else {
        0
    };
    Some(Inode {
        number,
        mode: u16_at(&raw, 0),
        size: (size_hi << 32) | size_lo,
        blocks: (blocks_hi << 32) | blocks_lo,
        extents,
        extent_count: count,
        extent_max,
    })
}

fn find_entry(dir: &Inode, name: &[u8]) -> Option<u32> {
    let superblock = superblock()?;
    let mut offset = 0_u64;
    while offset < dir.size {
        let logical = offset / superblock.block_size;
        let block = map_block(dir, logical)?;
        let mut data = [0_u8; 4096];
        let block_size = superblock.block_size as usize;
        if block_size > data.len() || !read_block(block, &mut data[..block_size]) {
            return None;
        }
        let mut cursor = 0;
        while cursor + 8 <= block_size {
            let inode = u32_at(&data, cursor);
            let rec_len = usize::from(u16_at(&data, cursor + 4));
            let name_len = usize::from(data[cursor + 6]);
            let kind = data[cursor + 7];
            if rec_len < 8 || cursor + rec_len > block_size {
                break;
            }
            if inode != 0
                && kind != FILETYPE_CSUM
                && name_len > 0
                && cursor + 8 + name_len <= block_size
                && &data[cursor + 8..cursor + 8 + name_len] == name
            {
                return Some(inode);
            }
            cursor += rec_len;
        }
        offset += superblock.block_size;
    }
    None
}

fn map_block(inode: &Inode, logical: u64) -> Option<u64> {
    let logical = u32::try_from(logical).ok()?;
    inode.extents[..usize::from(inode.extent_count)]
        .iter()
        .find(|extent| logical >= extent.block && logical < extent.block + extent.len)
        .map(|extent| extent.start + u64::from(logical - extent.block))
}

fn add_extent(inode: &mut Inode, logical: u32, physical: u64, block_size: u64) -> bool {
    let count = usize::from(inode.extent_count);
    for extent in &mut inode.extents[..count] {
        let next_logical = extent.block.saturating_add(extent.len);
        let next_physical = extent.start.saturating_add(u64::from(extent.len));
        if logical == next_logical && physical == next_physical && extent.len < 0x7FFF {
            extent.len += 1;
            inode.blocks = inode.blocks.saturating_add(block_size / 512);
            return true;
        }
    }
    if inode.extent_count >= inode.extent_max || inode.extent_count >= 4 {
        return false;
    }
    let mut at = count;
    for (index, extent) in inode.extents[..count].iter().enumerate() {
        if logical < extent.block {
            at = index;
            break;
        }
    }
    if at < count {
        for index in (at..count).rev() {
            inode.extents[index + 1] = inode.extents[index];
        }
    }
    inode.extents[at] = Extent {
        block: logical,
        len: 1,
        start: physical,
    };
    inode.extent_count += 1;
    inode.blocks = inode.blocks.saturating_add(block_size / 512);
    true
}

fn allocate_block() -> Result<u64, i64> {
    const EIO: i64 = -5;
    const ENOSPC: i64 = -28;
    let superblock = superblock().ok_or(EIO)?;
    let mut bitmap = [0_u8; 4096];
    if superblock.block_size as usize > bitmap.len()
        || !read_block(superblock.block_bitmap, &mut bitmap)
    {
        return Err(EIO);
    }
    let mut found = None;
    for block in 0..superblock.blocks_count {
        let index = block as usize;
        let byte = index / 8;
        let bit = index % 8;
        if byte >= bitmap.len() {
            break;
        }
        if bitmap[byte] & (1 << bit) == 0 {
            bitmap[byte] |= 1 << bit;
            found = Some(u64::from(block));
            break;
        }
    }
    let Some(block) = found else {
        return Err(ENOSPC);
    };
    if !write_block(superblock.block_bitmap, &bitmap) || !note_allocated() {
        return Err(EIO);
    }
    bitmap.fill(0);
    if !write_block(block, &bitmap) {
        return Err(EIO);
    }
    Ok(block)
}

fn note_allocated() -> bool {
    let Some(superblock) = superblock() else {
        return false;
    };
    let mut count = [0_u8; 4];
    if !read_at(SUPERBLOCK + 0x0C, &mut count) {
        return false;
    }
    let free = u32::from_le_bytes(count);
    if free > 0 && !write_at(SUPERBLOCK + 0x0C, &(free - 1).to_le_bytes()) {
        return false;
    }
    let mut gd = [0_u8; 64];
    let desc = superblock.desc_size as usize;
    if desc > gd.len() || !read_at(superblock.gd_block * superblock.block_size, &mut gd[..desc]) {
        return false;
    }
    let lo = u32::from(u16_at(&gd, 12));
    let hi = if superblock.desc_size >= 46 {
        u32::from(u16_at(&gd, 44))
    } else {
        0
    };
    let free = (hi << 16) | lo;
    if free > 0 {
        let free = free - 1;
        gd[12..14].copy_from_slice(&(free as u16).to_le_bytes());
        if superblock.desc_size >= 46 {
            gd[44..46].copy_from_slice(&((free >> 16) as u16).to_le_bytes());
        }
    }
    write_at(superblock.gd_block * superblock.block_size, &gd[..desc])
}

fn inode_offset(number: u32) -> Option<u64> {
    let superblock = superblock()?;
    if number == 0 || superblock.inodes_per_group == 0 {
        return None;
    }
    let index = number - 1;
    if index / superblock.inodes_per_group != 0 {
        return None;
    }
    let local = u64::from(index % superblock.inodes_per_group);
    Some(superblock.inode_table * superblock.block_size + local * superblock.inode_size)
}

fn store_inode(inode: &Inode) -> bool {
    let Some(offset) = inode_offset(inode.number) else {
        return false;
    };
    let Some(superblock) = superblock() else {
        return false;
    };
    let mut raw = [0_u8; 256];
    let inode_size = superblock.inode_size as usize;
    if inode_size > raw.len() || !read_at(offset, &mut raw[..inode_size]) {
        return false;
    }
    raw[4..8].copy_from_slice(&(inode.size as u32).to_le_bytes());
    if inode_size >= 112 {
        raw[108..112].copy_from_slice(&((inode.size >> 32) as u32).to_le_bytes());
    }
    raw[28..32].copy_from_slice(&(inode.blocks as u32).to_le_bytes());
    if inode_size >= 0x76 {
        raw[0x74..0x76].copy_from_slice(&((inode.blocks >> 32) as u16).to_le_bytes());
    }
    raw[42..44].copy_from_slice(&inode.extent_count.to_le_bytes());
    for index in 0..inode.extent_count {
        let at = 52 + usize::from(index) * 12;
        let extent = inode.extents[usize::from(index)];
        raw[at..at + 4].copy_from_slice(&extent.block.to_le_bytes());
        raw[at + 4..at + 6].copy_from_slice(&(extent.len as u16).to_le_bytes());
        raw[at + 6..at + 8].copy_from_slice(&((extent.start >> 32) as u16).to_le_bytes());
        raw[at + 8..at + 12].copy_from_slice(&(extent.start as u32).to_le_bytes());
    }
    write_at(offset, &raw[..inode_size])
}

fn read_block(block: u64, dest: &mut [u8]) -> bool {
    let Some(superblock) = superblock() else {
        return false;
    };
    read_block_bytes(block, superblock.block_size, dest)
}

fn read_block_bytes(block: u64, block_size: u64, dest: &mut [u8]) -> bool {
    if dest.len() < block_size as usize {
        return false;
    }
    read_at(block * block_size, &mut dest[..block_size as usize])
}

fn write_block(block: u64, src: &[u8]) -> bool {
    let Some(superblock) = superblock() else {
        return false;
    };
    let block_size = superblock.block_size as usize;
    if src.len() < block_size {
        return false;
    }
    write_at(block * superblock.block_size, &src[..block_size])
}

fn write_at(offset: u64, src: &[u8]) -> bool {
    let mut filled = 0;
    while filled < src.len() {
        let pos = offset + filled as u64;
        let mut sector = [0_u8; 512];
        if !virtio::sector_io(pos / 512, false, &mut sector) {
            return false;
        }
        let skip = (pos % 512) as usize;
        let n = (src.len() - filled).min(512 - skip);
        sector[skip..skip + n].copy_from_slice(&src[filled..filled + n]);
        if !virtio::sector_io(pos / 512, true, &mut sector) {
            return false;
        }
        filled += n;
    }
    true
}

fn read_at(offset: u64, dest: &mut [u8]) -> bool {
    let mut filled = 0;
    while filled < dest.len() {
        let pos = offset + filled as u64;
        let mut sector = [0_u8; 512];
        if !virtio::sector_io(pos / 512, false, &mut sector) {
            return false;
        }
        let skip = (pos % 512) as usize;
        let n = (dest.len() - filled).min(512 - skip);
        dest[filled..filled + n].copy_from_slice(&sector[skip..skip + n]);
        filled += n;
    }
    true
}

fn superblock() -> Option<Super> {
    unsafe { SUPER }
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
