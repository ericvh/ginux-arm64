# aarch64 port

This fork builds the Ginux userspace daemon for aarch64 as well as x86_64.
Upstream's approved architecture and validation target is x86-64 only. The
port is not an upstream milestone and none of it is part of the upstream HDAE
ledger. Its own records are in `hdae/arm64/`.

Each binary targets one architecture. `make build` is still the x86_64 daemon
and its x86_64 fixtures, unchanged.

## Build and test

```sh
rustup target add aarch64-unknown-linux-gnu
make build-arm64          # aarch64 daemon and managed payload
make check-arm64          # clippy for the aarch64 target, warnings denied
make arch-smoke           # x86_64: `ginux arch`, then usage on stderr
make arch-smoke-arm64     # the same probe under qemu-aarch64-static
```

`ginux arch` prints `GINUX_ARCH: <arch>` and `GINUX_ARCH: fork=ok`. It
exercises the daemon's own entry, write, exit, fork, and wait path. Set
`QEMU_AARCH64=` (empty) to run the probe natively on an arm64 host, or to
another qemu-user binary name.

Supervision needs ptrace and seccomp for the tracee's architecture, which
qemu-user on an x86_64 host cannot provide. Those tests run the aarch64
daemon under a real arm64 kernel in `qemu-system-aarch64`. The guest mounts
the daemon and fixtures with virtio-9p, tag `hostshare`, at `/var/tmp`.

```sh
export GINUX_AARCH64_KERNEL=/path/to/arm64/Image
make gate1-aarch64-guest
make native-aarch64-guest
```

`GINUX_AARCH64_KERNEL` is an arm64 Linux `Image` with virtio-9p built in. The
v9fs test harness publishes these on its `kernel-v*` releases.

| Target | What it proves |
|--------|----------------|
| `gate1-aarch64-guest` | The interception demonstration: emulated `getpid` 4242, mediated `write` not executed, `uname` denied, `GATE1: PASS`. |
| `native-aarch64-guest` | `ginux native` under the seccomp trace filter: `clone` denied with `-38`, a tracee `brk #0` stepped over, virtual tid and PI-futex results, a `clone3` snapshot in parent and child, and a `close_range` across the protected descriptors. |
| `gate1-x86_64-guest` | The x86_64 demonstration in a guest. Set `GINUX_X86_64_KERNEL` to a `bzImage`. |

## Design

The supervision loops, the contract, and the lane dispatch are written against
the x86-64 Linux ABI and stay that way. `UserRegs` is the x86-64
`user_regs_struct`; a syscall number in it is an x86-64 number.

`crates/ginux-init/src/arch/` holds everything that differs by architecture:

- the `_start` sequence and the syscall instruction;
- the daemon's own host syscall numbers and flag values;
- the ptrace register file.

On x86_64 the register file is the host's own and passes through unchanged.
On aarch64 it is a logical frame. `read_user_regs` and `write_user_regs`
translate `user_pt_regs` (`PTRACE_GETREGSET` with `NT_PRSTATUS` and
`NT_ARM_SYSTEM_CALL`) and the generic syscall table at the ptrace boundary.
`ginux_core::to_x86` and `to_aarch64` map numbers by syscall name.

Four rules keep code outside `arch/` portable:

1. A number passed to `syscallN` is a host number and comes from `arch`.
2. A number stored in `UserRegs::orig_rax` is an x86-64 number.
3. Re-executing a syscall instruction goes through `replay_syscall`.
4. A `struct stat` crosses through `stat_mode`, `STAT_MODE_OFFSET`,
   `STAT_UID_OFFSET`, or `guest_stat`.

### What the aarch64 translation has to reconstruct

aarch64 returns a syscall result in `x0`, the register that carried the first
argument, and has no separate `orig_rax`. The translation therefore:

- remembers each tracee's entry arguments, so an exit stop still shows them;
- reports `-ENOSYS` in `rax` at an entry stop, as x86-64 does;
- writes `x0` according to the stop: the first argument at an entry, `rax`
  when the call is skipped or at an exit, the first argument again for a
  replay;
- leaves the tracee's `x8` alone at an entry and changes the call through
  `NT_ARM_SYSTEM_CALL`. A replay that runs a different call borrows `x8` and
  the exit write that ends the sequence gives it back.

Three argument encodings differ between the ABIs and are converted in both
directions: `clone` takes `tls` before `child_tid`; `O_DIRECTORY`,
`O_NOFOLLOW`, `O_DIRECT`, and `O_LARGEFILE` have different bit positions
(`openat`, `open_by_handle_at`, `pipe2`, `fcntl(F_SETFL)`); and a synthesized
`struct stat` is laid out for the tracee's architecture.

### The interception demonstration

arm64 Linux has no `PTRACE_SYSEMU`. With no arguments, the aarch64 daemon
loads the managed payload into the Ginux runtime and decides each `svc #0`
there; the host executes none of them. `ginux runtime` loads ELF64 for either
machine. An aarch64 entry point must be 4-byte aligned.

## Verified

Run on an x86_64 host with QEMU 10.2 and the v9fs `kernel-v7.4-rc1` arm64
image, after rebasing onto upstream `f4b85186` (pull request #333):

- `make check` and `make check-arm64` pass; `make test-unit` passes.
- `make arch-smoke`, `make arch-smoke-arm64`, `make gate1-aarch64-guest`, and
  `make native-aarch64-guest` pass.
- The upstream x86_64 integration suite is unchanged in result:
  `test-launch`, `test-native-supervision` (all eight scripts),
  `test-process-native`, `test-gate1-setup`, and `test-tracee-kill` pass.

## Known gaps

The aarch64 daemon is a port of the mechanism, not a validated product
target. What is not covered:

- **Structures in tracee memory.** Only `struct stat` is converted. A
  supervisor path that reads or writes another structure whose layout differs
  between the ABIs would be wrong on aarch64. `struct epoll_event` (packed on
  x86-64 only) and the `open_how` flags of `openat2` are known examples; the
  supervisor does not touch either today.
- **`fcntl(F_GETFL)` results** are not converted. A passed-through result is
  correct; a synthesized one would carry x86-64 flag bits.
- **x86-64-only syscalls** (`open`, `stat`, `pipe`, `dup2`, `fork`, and the
  rest) have no aarch64 number. A supervisor path that rewrites a call into
  one of them is skipped on aarch64 and fails closed.
- **Launch's vDSO list** names the x86-64 vDSO entries. aarch64 has no vDSO
  `time` or `getcpu`.
- **Coverage.** The upstream native-supervision suite uses an x86-64 C
  fixture and has not been ported. The aarch64 guest test covers five
  scenarios. systemd instances, OCI images, networking, and the allocator
  have not been run on aarch64.
- **One behavioural difference.** A tracee's own `brk #0` is stepped over on
  aarch64. On x86-64 an unexpected trap still fails closed.
- `make gate1-x86_64-guest` was not run in this pass.

## Bare-metal kernel

The aarch64 port of the historical UEFI kernel is a separate workspace. See
`baremetal/README.md`.
