# Arm64 port of the historical UEFI kernel

## Source

Authorized by the human on 2026-11-06. Continue the arm64 stack from pull
request #12. The historical tree is `bootable-kernel` at `4d4a53bb`. Gate 17
remains the virt stub and is not edited.

## Goal

Boot the historical UEFI kernel as an aarch64 image under
`qemu-system-aarch64`, keep the x86_64 UEFI build working, and use
arch-specific files instead of a second kernel tree.

## Approved scope

- Bring `kernel/`, `boot/efi/`, their crate dependencies, and
  `specs/abi/control-v4.rs` onto the arm64 stack.
- Keep the x86_64 entry in `kernel/src/x86_64_kernel.rs`.
- Add `kernel/src/aarch64_kernel.rs` for the aarch64 entry.
- Accept both PE machine types in the shared PE parser.
- Boot `ginux-boot.efi` with AAVMF and record the log.

## Non-goals

- The x86_64 Linux syscall surface in `cpu.rs` beyond `write`, `getpid`, and
  `exit` on the Arm `svc` path.
- Merging to fork `main` or opening a pull request on `gmkurtzer/ginux`.
- Removing `macos-15-intel`.
- `qemu-system-aarch64 -machine sbsa-ref`. This environment has no SBSA flash
  image. The boot uses AAVMF on `virt` with GICv3, SMMUv3, and ACPI.

## Completed work

- [x] x86_64 `ginux-boot.efi` builds with `cargo +1.99.0` for
  `x86_64-unknown-uefi` (PE32+ x86-64).
- [x] aarch64 `ginux-boot.efi` builds for `aarch64-unknown-uefi`.
- [x] Arm UEFI handoff installs AArch64 page tables, a frame allocator,
  GICv3, the generic timer, PSCI, PCIe ECAM, and SMMUv3.
- [x] `scripts/uefi-aarch64-guest.sh` captured the boot log below.
- [x] `rustc --test kernel/src/pe.rs` passed 9 tests.

## Remaining work

- [ ] Human review of the pull request.
- [ ] The rest of the x86_64 Linux syscall surface.

## Verification plan

`make uefi-aarch64-guest` exits 0 and the log contains
`ginux: arm uefi build`, `ginux: aarch64 page tables on`,
`ginux: timer-irq=`, `ginux: virtio pci device`, `ginux: smmuv3`,
`ginux: svc=dispatch nr=172 result=4242`, and
`ginux: sbsa arm services ready`, with no `GINUX_KERNEL_PANIC` and no
`not ported`.

## Evidence

Observed on 2026-11-06 under
`qemu-system-aarch64 -machine virt,gic-version=3,iommu=smmuv3,acpi=on` with
AAVMF and an `aarch64-unknown-uefi` image. QEMU exited 0 after
`ResetSystem`.

```
ginux: arm uefi build
ginux: kernel entered after ExitBootServices
ginux: arm uefi sbsa handoff
ginux: exception level 1
ginux: acpi xsdt tables: 8
ginux: gic version 3 distributor 0x8000000
ginux: uart pl011 0x9000000
ginux: generic timer gsiv 30
ginux: physical frame allocator active
ginux: aarch64 page tables on
ginux: psci version 1.1 hvc
ginux: psci secondary started
ginux: timer-irq=30
ginux: pcie ecam 0x4010000000
ginux: virtio pci device 0x1001
ginux: smmuv3 0x9050000 idr0 0xd44101a
ginux: process=started
ginux: svc=dispatch nr=64
ginux-sbsa: process-write ok
ginux: svc=dispatch nr=172 result=4242
ginux: svc=dispatch nr=93
ginux: process=exited
ginux: sbsa arm services ready
```

## Decisions

- The aarch64 entry shares the PE parser, measured-boot manifest, and
  `BootInfo` contract. Arm services live under `kernel/src/arch/aarch64/`
  and are not linked into the x86_64 image.
- x86 serial stays on COM1. aarch64 serial follows the SPCR UART and falls
  back to the virt PL011 at `0x09000000`.
- A missing GOP does not stop the aarch64 boot. The x86 boot still requires it.
- The measured-boot blobs stay in the image. They are signed x86 bytes and
  are not executed on this path.

## Review policy

Checkpoint gate. It stays in `gates-todo/` until a human accepts it.
