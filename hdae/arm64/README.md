# aarch64 fork track

This directory is the record of the aarch64 port carried by the fork
https://github.com/ericvh/ginux-arm64. It is outside the upstream HDAE ledger
on purpose. Nothing here changes `hdae/STATUS.md`, `hdae/PLAN.md`,
`hdae/PROJECT.md`, or the upstream gate directories.

## Standing with upstream

Upstream closed pull requests #239 and #240 on 2026-11-13. The decision
recorded there:

- the approved architecture and validation target remain x86-64;
- the port's Gate 15 and Gate 16 collided with upstream gates of the same
  number (Gate 15 is dispatch performance; the separate Gate 16 enforcement
  direction was abandoned);
- the branches were built on an older supervisor shape;
- a future aarch64 effort would need a newly approved architecture milestone
  and a gate aligned with current `main`.

This track is therefore fork-only work. It makes no claim on an upstream
milestone, and nothing here is an upstream-approved gate.

## Gate records

The five records in `gates/` were written on the original branch stack as
gates 15 through 19. They are renumbered A1 through A5 so they cannot be read
as upstream gates. Their text is otherwise the historical record: commit ids
and branch names in them refer to the `cursor/*-2b05` branches, and where they
say an isolation unit is a "domain" they predate upstream's rename to
"instance".

| Record | Was | Subject |
|--------|-----|---------|
| `gates/A1-userspace-daemon.md` | 15 | Daemon builds and starts for aarch64 |
| `gates/A2-instance-mediation.md` | 16 | Mediation through the ptrace regset |
| `gates/A3-legacy-kernel.md` | 17 | Legacy-kernel entry on `qemu-system-aarch64` |
| `gates/A4-uefi-kernel.md` | 18 | UEFI kernel with SBSA services |
| `gates/A5-linux-svc.md` | 19 | AArch64 Linux syscall table on the UEFI boot |
| `gates/A6-native-supervision-suite.md` | none | Upstream's native-supervision suite on aarch64 |
| `gates/A7-rocky-host-suite.md` | none | Review of the fifteen original branches; upstream's host suite on Rocky 10 aarch64 |
| `gates/A8-parity-with-x86_64.md` | none | Upstream's integration and instance tests on aarch64, and the faults they found |

A6, A7 and A8 were written on `arm64/main`, after the rebase below. None of
the eight was accepted by a human reviewer. They stay open.

## Rebase onto current upstream (2026-11-25)

The stack was fifteen branches on upstream pull request #229. It was carried
forward to upstream `f4b85186` (pull request #333, 483 commits later) as one
linear series, restructured as follows.

- **Daemon.** Upstream's supervisor grew a seccomp trace filter, a
  user-notification broker, and replayed-syscall sequences for `clone3`,
  network sends, and `close_range`. The architecture layer in
  `crates/ginux-init/src/arch/` was rewritten for that shape rather than
  merged. `docs/arm64/README.md` describes the result and its known gaps.
- **Workspace.** The historical kernel, its crates, and its ABI spec moved
  from the repository root into `baremetal/`, a separate Cargo workspace. The
  root `Cargo.toml` and `Cargo.lock` are upstream's, unmodified.
- **Ledger.** The arm64 additions to `hdae/STATUS.md`, `hdae/PLAN.md`,
  `hdae/PROJECT.md`, and `hdae/gates-todo/` were removed. This directory
  replaces them.
- **Terminology.** Upstream renamed the isolation unit to "instance" and
  checks for the old word in `crates`, `docs`, `scripts`, `tests`, and
  `config`. The port's code and scripts follow.

Differences from the original stack worth knowing:

- `SYS_LAUNCH_MKDIRAT` is gone. Upstream fixed launch to issue `mkdirat`.
- The kernel's OCI crate is the package `ginux-kernel-oci`, because upstream
  added an unrelated `ginux-oci`.
- The freestanding link flags come from the Makefile's
  `FREESTANDING_RUSTFLAGS`, as upstream now does, not `.cargo/config.toml`.
- The fork-parent breakpoint on aarch64 replaces one instruction and keeps
  the neighbouring one, where the original overwrote both.
- The UEFI kernel merges adjacent firmware memory ranges. Without that it
  halts on current AAVMF with "firmware memory map is too fragmented".

The first carry-over left out one branch, `cursor/arm64-rocky-guest-2b05`,
which is not on the line from the first branch to the last. A7 reviews all
fifteen branches against `arm64/main` and integrates that one. Upstream
`85d53265` (pull request #334) is merged into `arm64/main` rather than
rebased under it.

## Next gate, if the track continues

What A8 left is in the "Known gaps" of `docs/arm64/README.md`. The items
that need a machine this track has not had: the instance tests and a
benchmark on arm64 hardware, and an aarch64 RPM built and installed there.
