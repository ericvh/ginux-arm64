use super::acpi::Platform;
use super::memory::MapRange;
use super::{
    acpi, gic, memory, paging, pci, psci, smmu, syscall, timer, uart, user, virtio, Handoff,
};

static mut RESET: Option<super::ResetSystem> = None;

pub(crate) fn bring_up(handoff: Handoff) -> ! {
    unsafe { RESET = Some(handoff.reset) };
    uart::write(b"ginux: arm uefi sbsa handoff\r\n");
    let el = current_el();
    uart::write(b"ginux: exception level ");
    uart::write_u64(el);
    uart::write(b"\r\n");
    if el != 1 {
        halt(b"ginux: arm uefi did not enter at el1\r\n");
    }
    let Some(platform) = acpi::discover(handoff.rsdp) else {
        halt(b"ginux: acpi sbsa tables missing\r\n");
    };
    uart::write(b"ginux: acpi xsdt tables: ");
    uart::write_u64(platform.xsdt_entries as u64);
    uart::write(b"\r\n");
    uart::write(b"ginux: gic version ");
    uart::write_u64(u64::from(platform.gic_version));
    uart::write(b" distributor ");
    uart::write_hex(platform.gicd);
    uart::write(b"\r\n");
    if platform.uart_kind == "pl011" || platform.uart_kind == "sbsa" {
        uart::set_base(platform.uart);
    }
    uart::write(b"ginux: uart ");
    uart::write(platform.uart_kind.as_bytes());
    uart::write(b" ");
    uart::write_hex(platform.uart);
    uart::write(b"\r\n");
    uart::write(b"ginux: generic timer gsiv ");
    uart::write_u64(u64::from(platform.timer_gsiv));
    uart::write(b"\r\n");

    let Some(mut frames) = memory::conventional_frames(
        handoff.memory_map,
        handoff.memory_map_size,
        handoff.descriptor_size,
        handoff.image_base,
        handoff.image_size,
    ) else {
        halt(b"ginux: no conventional memory\r\n");
    };
    let Some(test) = frames.alloc() else {
        halt(b"ginux: physical frame allocation failed\r\n");
    };
    unsafe {
        (test as *mut u64).write_volatile(0x4749_4e55_5800_0001);
        if (test as *const u64).read_volatile() != 0x4749_4e55_5800_0001 {
            halt(b"ginux: physical frame allocator mismatch\r\n");
        }
    }
    uart::write(b"ginux: physical frame allocator active\r\n");

    let Some(mut ranges) = memory::firmware_ranges(
        handoff.memory_map,
        handoff.memory_map_size,
        handoff.descriptor_size,
    ) else {
        halt(b"ginux: firmware memory map is too fragmented\r\n");
    };
    push_device(&mut ranges, platform.gicd, 0x2_0000);
    push_device(
        &mut ranges,
        platform.gicr,
        platform.gicr_length.max(0x2_0000),
    );
    push_device(&mut ranges, platform.uart, 0x1000);
    if platform.ecam != 0 {
        let buses = u64::from(
            platform
                .ecam_end_bus
                .saturating_sub(platform.ecam_start_bus),
        ) + 1;
        push_device(
            &mut ranges,
            platform.ecam,
            buses.saturating_mul(1024 * 1024),
        );
    }
    if platform.smmu != 0 {
        push_device(&mut ranges, platform.smmu, 0x2_0000);
    }
    uart::write(b"ginux: installing aarch64 page tables\r\n");
    let Some(space) = paging::install(&mut frames, ranges.as_slice()) else {
        halt(b"ginux: aarch64 page tables failed\r\n");
    };
    uart::write(b"ginux: aarch64 page tables on\r\n");

    match psci::version(platform.psci_hvc) {
        Some(version) => {
            uart::write(b"ginux: psci version ");
            uart::write_u64(u64::from(version.major));
            uart::write(b".");
            uart::write_u64(u64::from(version.minor));
            uart::write(if version.hvc {
                b" hvc\r\n"
            } else {
                b" smc\r\n"
            });
            start_secondary(&platform, version.hvc, &mut frames);
        }
        None => halt(b"ginux: psci unavailable\r\n"),
    }

    let Some(rd) = gic::init(platform.gicd, platform.gicr, platform.gicr_length) else {
        halt(b"ginux: gic redistributor wakeup failed\r\n");
    };
    gic::enable_ppi(rd, platform.timer_gsiv);
    if !syscall::install() {
        halt(b"ginux: vector table is misaligned\r\n");
    }
    let hz = timer::frequency();
    uart::write(b"ginux: generic timer hz ");
    uart::write_u64(hz);
    uart::write(b"\r\n");
    timer::arm_one_shot((hz / 100).max(1));
    syscall::wait_for_timer();
    match timer::consumed() {
        Some(intid) => {
            uart::write(b"ginux: timer-irq=");
            uart::write_u64(u64::from(intid));
            uart::write(b"\r\n");
        }
        None => halt(b"ginux: generic timer interrupt missing\r\n"),
    }

    let mut virtio = [pci::Virtio {
        bus: 0,
        device: 0,
        device_id: 0,
    }; 4];
    let found = pci::virtio_devices(
        platform.ecam,
        platform.ecam_start_bus,
        platform.ecam_end_bus,
        &mut virtio,
    );
    if found == 0 {
        halt(b"ginux: virtio pci device missing\r\n");
    }
    uart::write(b"ginux: pcie ecam ");
    uart::write_hex(platform.ecam);
    uart::write(b"\r\n");
    let mut drove_block = false;
    for device in &virtio[..found] {
        uart::write(b"ginux: virtio pci device ");
        uart::write_hex(u64::from(device.device_id));
        uart::write(b"\r\n");
    }
    if platform.smmu == 0 {
        halt(b"ginux: smmu not described\r\n");
    }
    let Some(idr0) = smmu::idr0(platform.smmu) else {
        halt(b"ginux: smmu unreadable\r\n");
    };
    uart::write(if platform.smmu_v3 {
        b"ginux: smmuv3 "
    } else {
        b"ginux: smmu "
    });
    uart::write_hex(platform.smmu);
    uart::write(b" idr0 ");
    uart::write_hex(u64::from(idr0));
    uart::write(b"\r\n");
    if !smmu::bypass(platform.smmu) {
        halt(b"ginux: smmu bypass failed\r\n");
    }
    for device in &virtio[..found] {
        if device.device_id == 0x1001 || device.device_id == 0x1042 {
            if !virtio::drive_block(
                &space,
                &mut frames,
                platform.ecam,
                platform.ecam_start_bus,
                device.bus,
                device.device,
                device.device_id,
            ) {
                halt(b"ginux: virtio-blk setup failed\r\n");
            }
            drove_block = true;
        }
    }
    if !drove_block {
        halt(b"ginux: virtio-blk missing\r\n");
    }

    let Some(loaded) = user::load(frames, &space, handoff.controller, handoff.controller_size)
    else {
        halt(b"ginux: controller elf invalid\r\n");
    };
    uart::write(b"ginux: process=started\r\n");
    unsafe { syscall::enter(loaded.entry, loaded.stack) }
}

fn start_secondary(platform: &Platform, hvc: bool, frames: &mut memory::Frames) {
    let boot = current_mpidr();
    let Some(cpu) = platform.cpus[..platform.cpu_count]
        .iter()
        .find(|cpu| affinity(cpu.mpidr) != affinity(boot))
    else {
        uart::write(b"ginux: psci secondary not described\r\n");
        return;
    };
    let Some(page) = frames.alloc() else {
        halt(b"ginux: secondary stub frame unavailable\r\n");
    };
    let flag = page + 0x100;
    unsafe {
        (flag as *mut u32).write_volatile(0);
        let stub = [
            0x1000_0801u32,
            0x5280_0020,
            0xb900_0020,
            0xd503_3f9f,
            0xd503_209f,
            0xd503_205f,
            0x17ff_ffff,
        ];
        for (index, instruction) in stub.iter().enumerate() {
            (page as *mut u32).add(index).write_volatile(*instruction);
        }
        clean_executable(page, 0x120);
    }
    if !psci::cpu_on(hvc, affinity(cpu.mpidr), page) {
        uart::write(b"ginux: psci cpu_on rejected\r\n");
        return;
    }
    for _ in 0..5_000_000 {
        if unsafe { (flag as *const u32).read_volatile() } == 1 {
            uart::write(b"ginux: psci secondary started\r\n");
            return;
        }
        core::hint::spin_loop();
    }
    uart::write(b"ginux: psci secondary did not start\r\n");
}

fn push_device(ranges: &mut memory::RangeList, start: u64, length: u64) {
    if start == 0 || length == 0 {
        return;
    }
    let Some(end) = start.checked_add(length) else {
        return;
    };
    if !ranges.push(MapRange {
        start,
        end,
        device: true,
    }) {
        halt(b"ginux: too many device ranges\r\n");
    }
}

fn current_el() -> u64 {
    let el: u64;
    unsafe {
        core::arch::asm!("mrs {v}, CurrentEL", v = out(reg) el, options(nostack, nomem));
    }
    (el >> 2) & 3
}

fn current_mpidr() -> u64 {
    let mpidr: u64;
    unsafe {
        core::arch::asm!("mrs {v}, mpidr_el1", v = out(reg) mpidr, options(nostack, nomem));
    }
    mpidr
}

fn affinity(mpidr: u64) -> u64 {
    mpidr & 0xff_00ff_ffff
}

pub(crate) unsafe fn clean_executable(page: u64, length: u64) {
    let mut address = page & !63;
    let end = page + length;
    while address < end {
        core::arch::asm!(
            "dc cvau, {address}",
            "ic ivau, {address}",
            address = in(reg) address,
            options(nostack),
        );
        address += 64;
    }
    core::arch::asm!("dsb ish", "isb", options(nostack));
}

pub(crate) fn finish() -> ! {
    uart::write(b"ginux: sbsa arm services ready\r\n");
    unsafe {
        if let Some(reset) = RESET {
            reset(2, 0, 0, core::ptr::null());
        }
    }
    uart::write(b"ginux: firmware shutdown returned unexpectedly\r\n");
    loop {
        core::hint::spin_loop();
    }
}

pub(crate) fn halt(message: &[u8]) -> ! {
    uart::write(message);
    uart::write(b"GINUX_KERNEL_PANIC\r\n");
    loop {
        core::hint::spin_loop();
    }
}
