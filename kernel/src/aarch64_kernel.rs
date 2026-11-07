// Early aarch64 entry for the historical UEFI kernel.
//
// The x86_64 implementation stays in `x86_64_kernel.rs`. This file shares the
// PE loader, the measured-boot manifest, and the boot-information contract.
// It stops where that kernel depends on x86_64 page tables, PCI, the local
// APIC, or the Linux syscall ABI.

mod arch;
mod crypto;
mod manifest;
mod pe;

use core::ffi::c_void;

pub use pe::rebase_loaded_image;

const PL011: *mut u32 = 0x0900_0000 as *mut u32;
const UARTFR: usize = 0x18 / 4;
const UART_TXFF: u32 = 1 << 5;
const EFI_LOADER_CODE: u32 = 1;
const EFI_LOADER_DATA: u32 = 2;
const EFI_BOOT_SERVICES_CODE: u32 = 3;
const EFI_BOOT_SERVICES_DATA: u32 = 4;
const EFI_CONVENTIONAL_MEMORY: u32 = 7;
const UEFI_PAGES_PER_MIB: u64 = 256;
pub const BOOT_INFO_MAGIC: u64 = 0x0049_4258_554e_4947;
pub const BOOT_INFO_VERSION: u32 = 8;

pub type ResetSystem = unsafe extern "efiapi" fn(u32, usize, usize, *const c_void);

#[repr(C)]
pub struct BootInfo {
    pub magic: u64,
    pub version: u32,
    pub size: u32,
    pub memory_map: *const u8,
    pub memory_map_size: usize,
    pub memory_descriptor_size: usize,
    pub memory_descriptor_version: u32,
    pub acpi_rsdp: *const u8,
    pub manifest: *const u8,
    pub manifest_size: usize,
    pub manifest_sha256: [u8; 32],
    pub manifest_signature: [u8; 64],
    pub control_plane_bundle: *const u8,
    pub control_plane_bundle_size: usize,
    pub controller_runtime: *const u8,
    pub controller_runtime_size: usize,
    pub root_image: *const u8,
    pub root_image_size: usize,
    pub framebuffer_base: u64,
    pub framebuffer_size: usize,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub framebuffer_stride: u32,
    pub framebuffer_format: u32,
    pub oci_rootfs: *const u8,
    pub oci_rootfs_size: usize,
    pub unix_time: u64,
    pub reset_system: ResetSystem,
    pub image_base: *const u8,
    pub image_size: usize,
}

#[repr(C)]
struct UefiMemoryDescriptor {
    memory_type: u32,
    padding: u32,
    physical_start: u64,
    virtual_start: u64,
    page_count: u64,
    attributes: u64,
}

/// Takes control after UEFI boot services have ended.
///
/// # Safety
///
/// `boot_info` must identify a valid, immutable `BootInfo`. Its memory-map
/// buffer must remain allocated for the lifetime of the kernel. Firmware has
/// left the PL011 at `0x0900_0000` and the image mapped.
#[repr(C, align(16))]
struct KernelStack([u8; 64 * 1024]);

static mut KERNEL_STACK: KernelStack = KernelStack([0; 64 * 1024]);

pub unsafe extern "C" fn kernel_entry(boot_info: *const BootInfo) -> ! {
    let stack_top = core::ptr::addr_of_mut!(KERNEL_STACK).cast::<u8>().add(64 * 1024);
    core::arch::asm!(
        "msr daifset, #0xf",
        "msr spsel, #1",
        "mov sp, {stack}",
        "mov x0, {info}",
        "br {entry}",
        stack = in(reg) stack_top,
        info = in(reg) boot_info,
        entry = in(reg) kernel_main as *const () as usize,
        options(noreturn),
    );
}

unsafe extern "C" fn kernel_main(boot_info: *const BootInfo) -> ! {
    serial_write(b"ginux: booting\r\n");
    serial_write(b"ginux: kernel entered after ExitBootServices\r\n");
    serial_write(b"ginux: daif masked\r\n");
    serial_write(b"ginux: arm kernel stack active\r\n");

    if boot_info.is_null() {
        kernel_fail(b"ginux: missing boot information\r\n");
    }
    let boot_info = &*boot_info;
    if boot_info.magic != BOOT_INFO_MAGIC
        || boot_info.version != BOOT_INFO_VERSION
        || boot_info.size < core::mem::size_of::<BootInfo>() as u32
    {
        kernel_fail(b"ginux: incompatible boot information\r\n");
    }
    if boot_info.memory_map.is_null()
        || boot_info.memory_map_size == 0
        || boot_info.memory_descriptor_size < core::mem::size_of::<UefiMemoryDescriptor>()
        || !boot_info
            .memory_map_size
            .is_multiple_of(boot_info.memory_descriptor_size)
    {
        kernel_fail(b"ginux: invalid UEFI memory map\r\n");
    }
    if boot_info.unix_time < 1_577_836_800 {
        kernel_fail(b"ginux: invalid realtime clock handoff\r\n");
    }
    serial_write(b"ginux: realtime clock handoff accepted\r\n");

    if validate_acpi_rsdp(boot_info.acpi_rsdp).is_none() {
        kernel_fail(b"ginux: invalid ACPI root pointer\r\n");
    }
    serial_write(b"ginux: ACPI root pointer validated\r\n");

    if boot_info.image_base.is_null() || boot_info.image_size == 0 {
        kernel_fail(b"ginux: invalid loaded EFI image bounds\r\n");
    }
    let Some(image) = pe::inspect(boot_info.image_base, boot_info.image_size) else {
        kernel_fail(b"ginux: invalid loaded EFI PE image\r\n");
    };
    serial_write(b"ginux: versioned boot handoff accepted\r\n");
    serial_write(b"ginux: loaded EFI PE image accepted\r\nginux: PE sections: ");
    serial_write_usize(image.sections);
    serial_write(b"\r\nginux: executable PE sections: ");
    serial_write_usize(image.executable_sections);
    serial_write(b"\r\nginux: writable PE sections: ");
    serial_write_usize(image.writable_sections);
    serial_write(b"\r\n");
    serial_write(b"ginux: firmware page tables still installed\r\n");

    // Firmware already mapped the image. There is no Ginux identity map yet,
    // so the measured-boot checks use the full address space as their bound.
    let mapped_limit = usize::MAX;
    let Some(boot_manifest) = manifest::Manifest::parse_and_verify(
        boot_info.manifest,
        boot_info.manifest_size,
        &boot_info.manifest_sha256,
        &boot_info.manifest_signature,
        mapped_limit,
    ) else {
        kernel_fail(b"ginux: invalid measured boot manifest\r\n");
    };
    serial_write(b"ginux: signed boot manifest accepted\r\nginux: manifest sequence: ");
    serial_write_u64(boot_manifest.sequence());
    serial_write(b"\r\nginux: manifest authority grants: ");
    serial_write_usize(boot_manifest.grant_count());
    serial_write(b"\r\n");
    if !manifest::verify_signature_rejection(
        boot_info.manifest,
        boot_info.manifest_size,
        &boot_info.manifest_signature,
    ) {
        kernel_fail(b"ginux: invalid manifest signature was accepted\r\n");
    }
    serial_write(b"ginux: manifest signature rejection verified\r\n");
    if !manifest::verify_parser_rejections(boot_info.manifest, boot_info.manifest_size) {
        kernel_fail(b"ginux: malformed manifest parser test failed\r\n");
    }
    serial_write(b"ginux: malformed boot manifests rejected\r\n");
    if !boot_manifest.bundle_matches(
        boot_info.control_plane_bundle,
        boot_info.control_plane_bundle_size,
        mapped_limit,
    ) {
        kernel_fail(b"ginux: control-plane bundle measurement failed\r\n");
    }
    serial_write(b"ginux: control-plane bundle measurement accepted\r\n");
    if !boot_manifest.controller_matches(
        boot_info.controller_runtime,
        boot_info.controller_runtime_size,
        mapped_limit,
    ) {
        kernel_fail(b"ginux: controller runtime measurement failed\r\n");
    }
    serial_write(b"ginux: embedded controller runtime measurement accepted\r\n");
    if !boot_manifest.root_matches(
        boot_info.root_image,
        boot_info.root_image_size,
        mapped_limit,
    ) {
        kernel_fail(b"ginux: root-image measurement failed\r\n");
    }
    serial_write(b"ginux: root-image measurement accepted\r\n");
    if !boot_manifest.verify_root_rejection(
        boot_info.root_image,
        boot_info.root_image_size,
        mapped_limit,
    ) {
        kernel_fail(b"ginux: modified root-image measurement accepted\r\n");
    }
    serial_write(b"ginux: root-image measurement rejection verified\r\n");
    if boot_info.oci_rootfs_size != 0 || !boot_info.oci_rootfs.is_null() {
        kernel_fail(b"ginux: boot-embedded OCI images are unsupported\r\n");
    }
    serial_write(b"ginux: OCI images require runtime registry ingestion\r\n");

    let summary = summarize_memory(boot_info);
    serial_write(b"ginux: memory descriptors: ");
    serial_write_usize(summary.descriptors);
    serial_write(b"\r\nginux: usable memory MiB: ");
    serial_write_u64(summary.conventional_pages / UEFI_PAGES_PER_MIB);
    serial_write(b"\r\nginux: reclaimable boot-service pages: ");
    serial_write_u64(summary.boot_service_pages);
    serial_write(b"\r\nginux: reserved loader pages: ");
    serial_write_u64(summary.loader_pages);
    serial_write(b"\r\n");

    arch::aarch64::platform::bring_up(arch::aarch64::Handoff {
        rsdp: boot_info.acpi_rsdp,
        memory_map: boot_info.memory_map,
        memory_map_size: boot_info.memory_map_size,
        descriptor_size: boot_info.memory_descriptor_size,
        image_base: boot_info.image_base as u64,
        image_size: boot_info.image_size as u64,
        reset: boot_info.reset_system,
    });
}

fn summarize_memory(boot_info: &BootInfo) -> MemorySummary {
    let descriptors = boot_info.memory_map_size / boot_info.memory_descriptor_size;
    let mut summary = MemorySummary {
        descriptors,
        conventional_pages: 0,
        boot_service_pages: 0,
        loader_pages: 0,
    };
    for index in 0..descriptors {
        // SAFETY: BootInfo validation established a non-null map, a descriptor
        // stride large enough for the required prefix, and an integral count.
        let descriptor = unsafe {
            let address = boot_info
                .memory_map
                .add(index * boot_info.memory_descriptor_size);
            core::ptr::read_unaligned(address.cast::<UefiMemoryDescriptor>())
        };
        match descriptor.memory_type {
            EFI_CONVENTIONAL_MEMORY => {
                summary.conventional_pages = summary
                    .conventional_pages
                    .saturating_add(descriptor.page_count);
            }
            EFI_BOOT_SERVICES_CODE | EFI_BOOT_SERVICES_DATA => {
                summary.boot_service_pages = summary
                    .boot_service_pages
                    .saturating_add(descriptor.page_count);
            }
            EFI_LOADER_CODE | EFI_LOADER_DATA => {
                summary.loader_pages = summary.loader_pages.saturating_add(descriptor.page_count);
            }
            _ => {}
        }
    }
    summary
}

struct MemorySummary {
    descriptors: usize,
    conventional_pages: u64,
    boot_service_pages: u64,
    loader_pages: u64,
}

fn validate_acpi_rsdp(rsdp: *const u8) -> Option<usize> {
    const RSDP_V1_SIZE: usize = 20;
    const RSDP_V2_SIZE: usize = 36;
    const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";
    if rsdp.is_null() {
        return None;
    }
    unsafe {
        for (offset, expected) in RSDP_SIGNATURE.iter().enumerate() {
            if rsdp.add(offset).read() != *expected {
                return None;
            }
        }
        if checksum(rsdp, RSDP_V1_SIZE) != 0 {
            return None;
        }
        let revision = rsdp.add(15).read();
        let length = if revision >= 2 {
            let length = core::ptr::read_unaligned(rsdp.add(20).cast::<u32>()) as usize;
            if length < RSDP_V2_SIZE {
                return None;
            }
            length
        } else {
            RSDP_V1_SIZE
        };
        (rsdp as usize).checked_add(length.checked_sub(1)?)?;
        if checksum(rsdp, length) != 0 {
            return None;
        }
        Some(rsdp as usize + length - 1)
    }
}

unsafe fn checksum(bytes: *const u8, length: usize) -> u8 {
    let mut sum = 0_u8;
    for offset in 0..length {
        sum = sum.wrapping_add(bytes.add(offset).read());
    }
    sum
}

fn kernel_fail(message: &[u8]) -> ! {
    serial_write(message);
    serial_write(b"GINUX_KERNEL_PANIC\r\n");
    loop {
        core::hint::spin_loop();
    }
}

fn serial_write(bytes: &[u8]) {
    for byte in bytes {
        while unsafe { PL011.add(UARTFR).read_volatile() } & UART_TXFF != 0 {
            core::hint::spin_loop();
        }
        unsafe { PL011.write_volatile(u32::from(*byte)) };
    }
}

fn serial_write_u64(mut value: u64) {
    let mut digits = [0_u8; 20];
    let mut cursor = digits.len();
    if value == 0 {
        serial_write(b"0");
        return;
    }
    while value != 0 {
        cursor -= 1;
        digits[cursor] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    serial_write(&digits[cursor..]);
}

fn serial_write_usize(value: usize) {
    serial_write_u64(value as u64);
}
