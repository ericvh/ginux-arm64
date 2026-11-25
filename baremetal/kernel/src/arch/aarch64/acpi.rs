const HEADER: usize = 36;
const MAX_TABLES: usize = 32;
const MAX_CPUS: usize = 8;

#[derive(Clone, Copy)]
pub(crate) struct Cpu {
    pub(crate) mpidr: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct Platform {
    pub(crate) xsdt_entries: usize,
    pub(crate) gic_version: u8,
    pub(crate) gicd: u64,
    pub(crate) gicr: u64,
    pub(crate) gicr_length: u64,
    pub(crate) cpus: [Cpu; MAX_CPUS],
    pub(crate) cpu_count: usize,
    pub(crate) timer_gsiv: u32,
    pub(crate) uart: u64,
    pub(crate) uart_kind: &'static str,
    pub(crate) psci_hvc: bool,
    pub(crate) psci_compliant: bool,
    pub(crate) ecam: u64,
    pub(crate) ecam_start_bus: u8,
    pub(crate) ecam_end_bus: u8,
    pub(crate) smmu: u64,
    pub(crate) smmu_v3: bool,
}

pub(crate) fn discover(rsdp: *const u8) -> Option<Platform> {
    let xsdt = xsdt_address(rsdp)?;
    let entries = table_entries(xsdt)?;
    let entries = entries.addresses();
    let mut platform = Platform {
        xsdt_entries: entries.len(),
        gic_version: 0,
        gicd: 0,
        gicr: 0,
        gicr_length: 0,
        cpus: [Cpu { mpidr: 0 }; MAX_CPUS],
        cpu_count: 0,
        timer_gsiv: 30,
        uart: 0,
        uart_kind: "none",
        psci_hvc: true,
        psci_compliant: false,
        ecam: 0,
        ecam_start_bus: 0,
        ecam_end_bus: 0,
        smmu: 0,
        smmu_v3: false,
    };
    for table in entries {
        let signature = table_signature(*table)?;
        match &signature {
            b"APIC" => parse_madt(*table, &mut platform)?,
            b"GTDT" => parse_gtdt(*table, &mut platform)?,
            b"MCFG" => parse_mcfg(*table, &mut platform)?,
            b"SPCR" => parse_spcr(*table, &mut platform)?,
            b"FACP" => parse_fadt(*table, &mut platform)?,
            b"IORT" => parse_iort(*table, &mut platform)?,
            _ => {}
        }
    }
    (platform.gicd != 0 && platform.gic_version >= 3).then_some(platform)
}

fn xsdt_address(rsdp: *const u8) -> Option<u64> {
    if rsdp.is_null() {
        return None;
    }
    unsafe {
        if rsdp.add(15).read() < 2 || checksum(rsdp, 36) != 0 {
            return None;
        }
        let xsdt = core::ptr::read_unaligned(rsdp.add(24).cast::<u64>());
        (xsdt != 0).then_some(xsdt)
    }
}

fn table_entries(xsdt: u64) -> Option<TableList> {
    let header = mapped(xsdt, HEADER)?;
    unsafe {
        if core::slice::from_raw_parts(header, 4) != b"XSDT" {
            return None;
        }
        let length = core::ptr::read_unaligned(header.add(4).cast::<u32>()) as usize;
        if length < HEADER || length > 1024 * 1024 || checksum(header, length) != 0 {
            return None;
        }
        let bytes = length - HEADER;
        if !bytes.is_multiple_of(8) {
            return None;
        }
        let mut entries = TableList {
            addresses: [0; MAX_TABLES],
            count: 0,
        };
        let pointers = bytes / 8;
        for index in 0..pointers.min(MAX_TABLES) {
            entries.addresses[index] =
                core::ptr::read_unaligned(header.add(HEADER + index * 8).cast::<u64>());
            entries.count += 1;
        }
        Some(entries)
    }
}

struct TableList {
    addresses: [u64; MAX_TABLES],
    count: usize,
}

impl TableList {
    fn addresses(&self) -> &[u64] {
        &self.addresses[..self.count]
    }
}

fn table_signature(address: u64) -> Option<[u8; 4]> {
    let header = mapped(address, HEADER)?;
    unsafe {
        let length = core::ptr::read_unaligned(header.add(4).cast::<u32>()) as usize;
        if !(HEADER..=1024 * 1024).contains(&length) || checksum(header, length) != 0 {
            return None;
        }
        let mut signature = [0; 4];
        signature.copy_from_slice(core::slice::from_raw_parts(header, 4));
        Some(signature)
    }
}

fn parse_madt(address: u64, platform: &mut Platform) -> Option<()> {
    let (header, length) = checked_table(address, b"APIC")?;
    if length < 44 {
        return None;
    }
    let mut offset = 44;
    while offset + 2 <= length {
        let entry = unsafe { header.add(offset) };
        let kind = unsafe { entry.read() };
        let entry_length = unsafe { entry.add(1).read() } as usize;
        if entry_length < 2 || offset + entry_length > length {
            return None;
        }
        match kind {
            0x0b if entry_length >= 76 => {
                let mpidr = unsafe { read_u64(entry, 68) };
                let flags = unsafe { read_u32(entry, 12) };
                if flags & 1 != 0 && platform.cpu_count < MAX_CPUS {
                    platform.cpus[platform.cpu_count] = Cpu { mpidr };
                    platform.cpu_count += 1;
                }
            }
            0x0c if entry_length >= 24 => {
                platform.gicd = unsafe { read_u64(entry, 8) };
                platform.gic_version = unsafe { entry.add(20).read() };
            }
            0x0e if entry_length >= 16 => {
                platform.gicr = unsafe { read_u64(entry, 4) };
                platform.gicr_length = u64::from(unsafe { read_u32(entry, 12) });
            }
            _ => {}
        }
        offset += entry_length;
    }
    Some(())
}

fn parse_gtdt(address: u64, platform: &mut Platform) -> Option<()> {
    let (header, length) = checked_table(address, b"GTDT")?;
    if length < 60 {
        return None;
    }
    let gsiv = unsafe { read_u32(header, 56) };
    if gsiv != 0 {
        platform.timer_gsiv = gsiv;
    }
    Some(())
}

fn parse_mcfg(address: u64, platform: &mut Platform) -> Option<()> {
    let (header, length) = checked_table(address, b"MCFG")?;
    if length < 44 + 16 {
        return None;
    }
    platform.ecam = unsafe { read_u64(header, 44) };
    platform.ecam_start_bus = unsafe { header.add(54).read() };
    platform.ecam_end_bus = unsafe { header.add(55).read() };
    Some(())
}

fn parse_spcr(address: u64, platform: &mut Platform) -> Option<()> {
    let (header, length) = checked_table(address, b"SPCR")?;
    if length < 52 {
        return None;
    }
    let interface = unsafe { header.add(36).read() };
    let base = unsafe { read_u64(header, 44) };
    platform.uart = base;
    platform.uart_kind = match interface {
        0x03 => "pl011",
        0x0e => "sbsa",
        _ => "other",
    };
    Some(())
}

fn parse_fadt(address: u64, platform: &mut Platform) -> Option<()> {
    let (header, length) = checked_table(address, b"FACP")?;
    if length < 132 {
        return Some(());
    }
    let flags = unsafe { core::ptr::read_unaligned(header.add(129).cast::<u16>()) };
    platform.psci_compliant = flags & 1 != 0;
    platform.psci_hvc = flags & 2 != 0;
    Some(())
}

fn parse_iort(address: u64, platform: &mut Platform) -> Option<()> {
    let (header, length) = checked_table(address, b"IORT")?;
    if length < 48 {
        return Some(());
    }
    let count = unsafe { read_u32(header, 36) } as usize;
    let mut offset = unsafe { read_u32(header, 40) } as usize;
    for _ in 0..count {
        if offset + 16 > length {
            return None;
        }
        let node = unsafe { header.add(offset) };
        let kind = unsafe { node.read() };
        let node_length = unsafe { core::ptr::read_unaligned(node.add(1).cast::<u16>()) } as usize;
        if node_length < 16 || offset + node_length > length {
            return None;
        }
        if (kind == 3 || kind == 4) && node_length >= 24 && platform.smmu == 0 {
            platform.smmu = unsafe { read_u64(node, 16) };
            platform.smmu_v3 = kind == 4;
        }
        offset += node_length;
    }
    Some(())
}

fn checked_table(address: u64, signature: &[u8; 4]) -> Option<(*const u8, usize)> {
    let found = table_signature(address)?;
    if &found != signature {
        return None;
    }
    let header = mapped(address, HEADER)?;
    let length = unsafe { core::ptr::read_unaligned(header.add(4).cast::<u32>()) } as usize;
    Some((header, length))
}

fn mapped(address: u64, length: usize) -> Option<*const u8> {
    let pointer = usize::try_from(address).ok()?;
    pointer.checked_add(length)?;
    Some(pointer as *const u8)
}

unsafe fn read_u32(base: *const u8, offset: usize) -> u32 {
    core::ptr::read_unaligned(base.add(offset).cast::<u32>())
}

unsafe fn read_u64(base: *const u8, offset: usize) -> u64 {
    core::ptr::read_unaligned(base.add(offset).cast::<u64>())
}

unsafe fn checksum(bytes: *const u8, length: usize) -> u8 {
    let mut sum = 0_u8;
    for offset in 0..length {
        sum = sum.wrapping_add(bytes.add(offset).read());
    }
    sum
}
