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

- aarch64 page tables, the physical frame allocator, virtio PCI, APIC, SMP,
  IOMMU, and the Linux syscall ABI.
- Merging to fork `main` or opening a pull request on `gmkurtzer/ginux`.
- Removing `macos-15-intel`.

## Completed work

- [x] x86_64 `ginux-boot.efi` builds with `cargo +1.99.0` for
  `x86_64-unknown-uefi` (PE32+ x86-64).
- [x] aarch64 `ginux-boot.efi` builds for `aarch64-unknown-uefi`.
- [x] `scripts/uefi-aarch64-guest.sh` captured the boot log below.
- [x] `rustc --test kernel/src/pe.rs` passed 9 tests.

## Remaining work

- [ ] Human review of the pull request.
- [ ] aarch64 paging, frame allocation, virtio discovery, and the Linux ABI.

## Verification plan

`make uefi-aarch64-guest` exits 0 and the log contains
`ginux: kernel entered after ExitBootServices`,
`ginux: signed boot manifest accepted`, and
`ginux: stopping before x86-only kernel path`, with no
`GINUX_KERNEL_PANIC`.

## Evidence

Observed on 2026-11-06 under `qemu-system-aarch64 -machine virt` with AAVMF:

```
ginux: EFI image loaded at 0x5c669000 size 0xf7000
ginux: kernel entered after ExitBootServices
ginux: ACPI root pointer validated
ginux: loaded EFI PE image accepted
ginux: PE sections: 4
ginux: signed boot manifest accepted
ginux: manifest sequence: 1
ginux: manifest authority grants: 12
ginux: memory descriptors: 29
ginux: usable memory MiB: 479
ginux: aarch64 page tables are not ported
ginux: aarch64 virtio PCI discovery is not ported
ginux: aarch64 Linux ABI is not ported
ginux: stopping before x86-only kernel path
```

## Decisions

- The aarch64 entry shares the PE parser, measured-boot manifest, and
  `BootInfo` contract. It stops before `arch::x86_64::pci`.
- x86 serial stays on COM1. aarch64 serial is the virt PL011 at `0x09000000`.
- A missing GOP does not stop the aarch64 boot. The x86 boot still requires it.

## Review policy

Checkpoint gate. It stays in `gates-todo/` until a human accepts it.
