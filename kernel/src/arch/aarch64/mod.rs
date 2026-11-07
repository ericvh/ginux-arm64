pub(crate) mod acpi;
pub(crate) mod gic;
pub(crate) mod linux;
pub(crate) mod memory;
pub(crate) mod paging;
pub(crate) mod pci;
pub(crate) mod platform;
pub(crate) mod psci;
pub(crate) mod smmu;
pub(crate) mod syscall;
pub(crate) mod timer;
pub(crate) mod uart;
#[allow(static_mut_refs)]
pub(crate) mod user;

use core::ffi::c_void;

pub(crate) type ResetSystem = unsafe extern "efiapi" fn(u32, usize, usize, *const c_void);

pub(crate) struct Handoff {
    pub(crate) rsdp: *const u8,
    pub(crate) memory_map: *const u8,
    pub(crate) memory_map_size: usize,
    pub(crate) descriptor_size: usize,
    pub(crate) image_base: u64,
    pub(crate) image_size: u64,
    pub(crate) reset: ResetSystem,
    pub(crate) controller: *const u8,
    pub(crate) controller_size: usize,
}
