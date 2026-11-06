# Arm64 legacy bare-metal kernel

## Source

Authorized by the human on 2026-11-06 for the fork
https://github.com/ericvh/ginux-arm64. The x86_64 kernel history is upstream
`bootable-kernel` / tag `bootable-kernel-final` (`4d4a53bb`). That commit is
already an ancestor of `main`. This fork had no `bootable-kernel` branch ref
until this gate published it.

## Goal

The legacy kernel line can boot under `qemu-system-aarch64`, and the x86_64
UEFI kernel remains available on `bootable-kernel` without removing the
userspace tree from this branch.

## Approved scope

- Publish `bootable-kernel` and `bootable-kernel-final` on the fork.
- Add a freestanding aarch64 virt entry under `legacy-kernel/` that drops
  from EL2 to EL1, prints on the PL011, and exits through semihosting.
- `make legacy-kernel-aarch64` builds that entry and requires the banner.

## Non-goals

- Retargeting the historical UEFI kernel (APIC, ACPI, paging, Linux syscall
  ABI, Rocky userspace) onto aarch64. That crate is x86_64-only and stays on
  `bootable-kernel` unmodified.
- Running the x86_64 UEFI image in this environment.
- Dropping the userspace daemon or domain-mediation commits.

## Completed work

- [x] Pushed `bootable-kernel` and `bootable-kernel-final` to the fork.
- [x] Added `legacy-kernel/` and `make legacy-kernel-aarch64`.
- [x] Observed the boot banner under `qemu-system-aarch64`.

## Remaining work

- [ ] Human review of the pull request.
- [ ] A future gate if the historical UEFI kernel itself should execute on
  aarch64 rather than only the virt boot entry.

## Acceptance criteria

- `bootable-kernel` on the fork points at `4d4a53bb`, the same commit as
  `bootable-kernel-final`.
- `make legacy-kernel-aarch64` prints `ginux-legacy: aarch64 virt boot ok`
  and qemu exits 0.
- This branch still contains the userspace daemon sources.

## Verification plan

- `git rev-parse bootable-kernel bootable-kernel-final^{}` -> both
  `4d4a53bb`.
- `make legacy-kernel-aarch64` -> banner and exit 0.
- `git merge-base HEAD 4d4a53bb` -> `4d4a53bb`, so userspace history still
  contains the legacy kernel.

## Evidence

- Fork refs: `bootable-kernel` and `bootable-kernel-final` at `4d4a53bb`.
- `make legacy-kernel-aarch64` printed `ginux-legacy: aarch64 virt boot ok`
  twice (PL011 and semihosting) and qemu exited 0. QEMU 8.2.2,
  `-machine virt -cpu cortex-a57`.

## Decisions and limitations

- The aarch64 boot entry is a separate freestanding binary, not a `cfg` of
  `kernel/src/arch/x86_64.rs`. Restoring that crate into the userspace
  workspace would pull its x86_64 UEFI graph into the daemon build. The
  historical sources stay on `bootable-kernel`.
- qemu loads the ELF at `0x40080000`. The entry drops EL2 to EL1 before
  touching the PL011 at `0x09000000`.
- The x86_64 `make run` path was not executed here.

## Delivery strategy

- Working branch: `cursor/arm64-legacy-kernel-2b05`
- Intended base: `cursor/arm64-domain-mediation-2b05`
- Pull request: intended on https://github.com/ericvh/ginux-arm64
- Planned commits: aarch64 virt boot entry, make target, and this gate.
- Actual commits: pending

## Review policy and outcome

The human authorized gates 15 through 17 as one batch. Technical passage does
not close the gate. This is the last stage in the batch.
