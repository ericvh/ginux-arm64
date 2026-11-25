# Bare-metal kernel line

This directory carries the historical Ginux UEFI kernel and its aarch64 port.
Upstream preserves the x86_64 kernel on the `bootable-kernel` branch; `main`
is the userspace daemon only. This fork keeps the kernel here, in its own
Cargo workspace, so the daemon workspace at the repository root keeps
upstream's manifest, lockfile, and checks unchanged.

The layout mirrors the root of `bootable-kernel`:

| Path | Contents |
|------|----------|
| `boot/efi/` | `ginux-boot`, the UEFI boot entry, and the measured AArch64 controller |
| `kernel/` | `ginux-kernel` |
| `crates/` | crates only the kernel uses |
| `specs/abi/` | the control ABI the kernel includes |
| `legacy-kernel/` | a small standalone `virt` kernel, its own package and lockfile |
| `scripts/` | the build and guest scripts for this directory |

`crates/oci` is the package `ginux-kernel-oci`. The daemon workspace has its
own, unrelated `ginux-oci`; the kernel still imports this one as `ginux_oci`.

## UEFI kernel

```sh
rustup toolchain install 1.99.0 --target aarch64-unknown-uefi
make uefi-aarch64-guest
```

The same target is available from the repository root. It rebuilds the
measured controller, builds `ginux-boot.efi` for `aarch64-unknown-uefi`, and
boots it with AAVMF under
`qemu-system-aarch64 -machine virt,gic-version=3,iommu=smmuv3,acpi=on`.

Requirements: `clang`, an `ld.lld` (the Rust toolchain's is found
automatically), Python with `cryptography`, `mkfs.ext4`, and the
`qemu-efi-aarch64` firmware. Three variables adjust the environment:

- `GINUX_AAVMF_DIR`: directory holding `AAVMF_CODE.fd` and `AAVMF_VARS.fd`,
  default `/usr/share/AAVMF`.
- `SKIP_CONTROLLER=1`: build with the committed controller ELF and manifest,
  for hosts without `clang`.
- `UEFI_CARGO`: the cargo invocation, default `cargo +1.99.0`.

The aarch64 image is the same bootloader and kernel crate as x86_64. It shares
the PE image check and the measured-boot manifest, then brings up Arm services
from the UEFI handoff: ACPI (MADT, GTDT, MCFG, SPCR, IORT), AArch64
translation tables, a physical frame allocator, GICv3, the generic timer,
PSCI, an SBSA or PL011 UART, PCIe ECAM, and SMMUv3.

The measured controller is an AArch64 ELF. The kernel loads it into user
memory and dispatches the AArch64 Linux syscall table for it: `write`, `brk`,
`mmap`, `openat`, `fstat`, `read`, `lseek`, `pread64`, `close`, `uname`,
`clock_gettime`, `clone`, `clone3`, `waitid`, and `exit`. `/dev/vda` is the
virtio-blk PCI disk; the same disk is an ext4 filesystem the controller reads,
overwrites, and grows by one block.

The guest script requires each of those lines in the boot log and fails on
`GINUX_KERNEL_PANIC` or an unported path.

The x86_64 UEFI target is unchanged:
`cargo +1.99.0 build -p ginux-boot --target x86_64-unknown-uefi`, run from
this directory.

## Legacy virt kernel

```sh
rustup target add aarch64-unknown-none
make legacy-kernel-aarch64
```

`legacy-kernel/` drops to EL1, installs exception vectors, identity-maps RAM
and the `virt` MMIO frame, enables the MMU, and takes one generic-timer
interrupt through the GICv2. It then starts one EL0 process, which enters the
kernel with `svc #0`; the kernel dispatches `write` (64) and `exit` (93). The
script expects the banner `ginux-legacy: aarch64 virt boot ok` from
`qemu-system-aarch64 -machine virt`.

## Notes

- `make fmt-check` checks formatting for both workspaces here. Nothing in
  this directory is covered by the root `make check`.
