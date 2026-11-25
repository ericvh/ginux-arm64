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
| `gate1-aarch64-guest` | The interception demonstration under `PTRACE_SYSEMU`: emulated `getpid` 4242, mediated `write` not executed, `uname` denied, `GATE1: PASS`. |
| `native-aarch64-guest` | `ginux native` under the seccomp trace filter, with a fixture that reads its own registers: `clone` denied with `-38`, a tracee `brk #0` failing the instance, virtual tid and PI-futex results, a `clone3` snapshot in parent and child, and a `close_range` across the protected descriptors, each with `x8` preserved. |
| `native-suite-aarch64-guest` | Upstream's `scripts/test-native-supervision.py`, unmodified, against the aarch64 daemon: 42 checks. |
| `integration-aarch64-guest` | That script, `test-gate1-setup`, and `test-process-native` in the same guest. `TESTS=` names others. |
| `gate1-x86_64-guest` | The x86_64 demonstration in a guest. Set `GINUX_X86_64_KERNEL` to a `bzImage`. |

### Upstream's integration tests

`make test-launch`, `test-native-supervision`, `test-gate1-setup`,
`test-tracee-kill`, and `test-process-native` are upstream's root-run
integration tests. Each has an `-arm64` target that runs the same script
against the aarch64 daemon and fixtures:

```sh
make build-arm64 build-fixtures-arm64

# On an arm64 host:
sudo make test-launch-arm64 test-native-supervision-arm64 test-gate1-setup-arm64 test-tracee-kill-arm64
make test-process-native-arm64

# On any other host, on the Rocky aarch64 guest (see the next section):
ROCKY_ARCH=aarch64 ./scripts/with-rocky-host.sh ./scripts/test-integration-host.sh

# Or the subset a minimal guest can run:
export GINUX_AARCH64_ROOTFS=/path/to/arm64-rootfs.tar
make integration-aarch64-guest
```

As root a workload drops to an unprivileged identity that must be able to
reach the fixtures, so `test-process-native-arm64` runs without root in a
private checkout, as upstream's CI runs it.

`build-fixtures-arm64` compiles upstream's C fixtures (`native-fixture.c`,
`launch-fixture.c`, `gate1-setup-fixture.c`, `process-review-fixture.c`),
the same sources the x86_64 targets compile, with `aarch64-linux-gnu-gcc`
(`ARM64_CC` overrides it; an arm64 host uses `cc`). The x86_64 fixtures are
unchanged: they compile to the same binaries as upstream's files.

The scenarios name every call by its x86-64 number, as the contract does.
`tests/fixtures/fixture-aarch64.h` issues each one the way an aarch64 C
library would. A number that exists on both tables is looked up in a header
generated from `crates/ginux-core/src/syscall_map.rs`. A call the aarch64
table lacks becomes its modern form: `open` is `openat`, `pipe` is `pipe2`,
`dup2` is `dup3`, `fork` is `clone`, `poll` is `ppoll`, `mkdir` is
`mkdirat`.

What differs on aarch64, each for a reason in the architecture:

- `int80` has no counterpart: a 64-bit aarch64 task has one syscall ABI. In
  `test-native-supervision` the scenario executes `brk #0` instead, and the
  check accepts the unexpected-trap failure it also accepts from an x86-64
  host without IA32 emulation. `test-launch` skips its `int80` case.
- `test-launch`'s contract checks name `openat` where they name `open` on
  x86-64, because the fixture can only issue `openat`.
- `juggled-pids` checks that `kill` and `pidfd_open` leave the caller's pid
  in the first argument register. On aarch64 that register returns the
  result, so those two checks pass by construction. The `waitid` checks in
  `null-wait` and `zero-effects` use the second argument register and are
  real on both.
- The seccomp filters that `gate1-setup-fixture.c` and
  `test-process-native.py` install name the aarch64 architecture and the
  aarch64 numbers of `ptrace` and `process_vm_writev`.

Seven of the native-supervision scripts trace the daemon itself (the yield
broker, the two `clone3` scripts, the two `close_range` scripts, the two
send-snapshot scripts) and read what they find as x86-64.
`scripts/aarch64-trace.py <script> <arguments>` runs one unmodified on
aarch64 and supplies that view: `PTRACE_GETREGS` and `PTRACE_SETREGS` on the
traced daemon, its staging call shown as the `PTRACE_SETREGS` it is on
x86-64, `/proc/<pid>/syscall` with x86-64 numbers, and a call the kernel is
about to restart shown as x86-64 shows it at the same stop. On another
machine it only runs the script. `GINUX_TRACE_TIME_SCALE` multiplies a
script's own time limit.

The minimal guest (`GINUX_AARCH64_ROOTFS`, a tar of an arm64 root with
`/bin/sh` and `python3`) cannot run `test-launch`, `test-tracee-kill`, or the
seven tracing scripts: they need `/proc/<pid>/task/<tid>/children` and
util-linux `setpriv`, which that kernel and BusyBox lack. One way to make
the root filesystem:

```sh
docker pull --platform linux/arm64 python:3.13-alpine
docker export "$(docker create --platform linux/arm64 python:3.13-alpine)" -o arm64-rootfs.tar
```

### Upstream's host suite on Rocky Linux 10 aarch64

Upstream's `make test` boots a pinned Rocky Linux 10 x86_64 cloud image
under QEMU and runs its host scripts over SSH. `make test-arm64` runs the
same scripts against the aarch64 daemon on the aarch64 build of that image.

```sh
rustup target add aarch64-unknown-linux-gnu aarch64-unknown-linux-musl
make host-image-fetch-arm64   # the pinned Rocky 10.2 aarch64 image, 448 MiB
make image-helper-fetch       # upstream's extraction helper, for test-image
make test-arm64
```

The host needs `qemu-system-aarch64`, Arm UEFI firmware (`qemu-efi-aarch64`;
`GINUX_AAVMF_DIR` names its directory when it is not `/usr/share/AAVMF`),
`xorriso` or `genisoimage`, and Docker for the image test.

`ROCKY_ARCH` selects the guest. It defaults to `x86_64`, where every script
does what upstream's does. With `ROCKY_ARCH=aarch64`:

- `scripts/rocky-image-env.sh` selects `config/rocky-host-image-aarch64.env`
  and its checksum file. The image is the same Rocky 10.2 build as upstream's
  x86_64 pin. Rocky has since moved that release to its vault, so the URL
  points there.
- `scripts/start-rocky-host.sh` starts `qemu-system-aarch64 -machine virt`
  with the firmware, the disk as virtio-blk, and the cloud-init seed as a
  SCSI CD-ROM. `ROCKY_AARCH64_CPU` overrides the `cortex-a72` model.
- The host keeps its own overlay, pid file, and serial log
  (`build/rocky-host-aarch64-*`, `build/rocky-test-suite-host-aarch64-*`),
  because neither architecture can boot the other's disk.
- The test scripts copy the daemon, the managed payload, and the process
  fixture from `target/aarch64-unknown-linux-gnu/release`, and the CLI from
  `target/aarch64-unknown-linux-musl/release`. The runtime fixture stays the
  x86-64 ELF: `ginux runtime` interprets it on either host.
- `test-image` extracts root partition 3 (it is 4 on x86_64) and expects an
  aarch64 `systemd` and `ld-linux-aarch64.so.1`.

`make test-arm64` runs `test-image`, then boots the host once and runs
`test-intercept`, `test-runtime`, `test-config`, `test-instance` (with
`test-gate12-cli`), `test-devices`, `test-process`, and `test-kernel-views`
through `scripts/test-host-suite.sh`. One of them alone:

```sh
ROCKY_ARCH=aarch64 ./scripts/with-rocky-host.sh ./scripts/test-host-suite.sh process
```

`make test-arm64` then runs the integration tests of the previous section
as root on the same boot (`scripts/test-integration-host.sh`).

### Instances on Rocky Linux 10 aarch64

```sh
make test-instances-arm64
```

This runs upstream's instance tests against the aarch64 daemon on that
host: `test-vfs`, `test-native-vfs`, `test-contract-runtime`,
`test-gate125-lock`, `test-gate975-hooks`, `test-gate975-network`,
`test-systemd` (Rocky Linux 10 systemd booted to a login in an instance,
three times, then the dnf-failure and startup-failure cases), and
`test-multi-instance` (two systemd instances at once, on a second host).

The instance images are upstream's: `scripts/oci-common.sh` reads
`config/rocky-instance-oci-aarch64.env` and
`config/rocky-systemd-oci-aarch64.env` beside upstream's files, which name
the arm64 manifests of the same pinned image indexes.
`make instance-images-arm64` fetches and prepares them.

The host is emulated on every machine (`start-rocky-host.sh` uses TCG, as
upstream's does), and an emulated aarch64 host is slower than an emulated
x86_64 one: systemd reached its login in about 95 seconds here against 28.
`GINUX_EXPECT_TIME_SCALE` (10 in this target) multiplies the login driver's
50-second and 120-second limits. It is unset, and the limits are upstream's,
in every x86_64 target.

An instance config names its root by image manifest, and upstream's files
name the amd64 ones. The daemon treats that as an identity and the tests
stage the arm64 root filesystem themselves, so those files work unchanged.
A recipe meant to pull its own image on an aarch64 host has to name the
arm64 manifest or the index.

### The package

```sh
ROCKY_ARCH=aarch64 make rpm        # builds in the Rocky aarch64 guest
ROCKY_ARCH=aarch64 make test-rpm   # installs it in a fresh one and uses it
```

The spec builds the runtime for the package's own architecture. The build
runs inside the emulated guest and takes about an hour there.

The example recipes name each image by its `linux/amd64` manifest.
`config/examples/arm64-manifests` lists the `linux/arm64` manifest of each
image from the same index, and the aarch64 package installs the recipes
with those, so `ginux import` of a packaged recipe fetches an image the
host can run. `docker.io/jrei/systemd-ubuntu`, which
`ubuntu24-network-service.toml` uses, is published for `linux/amd64` only:
that recipe has no arm64 image, and `test-rpm` runs its Ubuntu step on
x86_64 only. The recipes in the tree are upstream's, unchanged.

The Rocky-host targets not covered are the benchmarks,
`test-allocator-rocky`, `test-gate19-native-examples`, `run-host`,
`test-systemd-live`, and `test-systemd-interactive`.

The original branch for this (`cursor/arm64-rocky-guest-2b05`) made
`make test` itself boot the aarch64 guest and copied the binaries from a
virtiofs share of the tree. Here `make test` stays upstream's x86_64 run and
the binaries go over SSH as upstream's do, which needs no `virtiofsd`.

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
  the exit write that ends the sequence gives it back;
- gives a restarted call the first argument the supervisor left at its exit
  stop. On x86-64 that is the `rdi` the supervisor wrote. On aarch64 the
  kernel restarts with the `x0` it last dispatched, which is the value the
  supervisor issued at the entry (a host pid, where the caller passed a
  virtual one). The entry stop of the restart puts the exit value back.
  Kernels before the tracer's `x0` became authoritative for the restart
  already pass the caller's own value, and nothing is changed there.

The registers read at a stop are kept until the tracee can run again, so a
write does not read them a second time: the daemon's `ptrace` wrapper tells
the architecture layer about every request, and any request but a read or
write of a stopped tracee forgets them. The syscall number comes with the
stop kind (`PTRACE_GET_SYSCALL_INFO`) at an entry, and from the entry's
record at an exit. A call with both stops costs six `ptrace` requests, where
x86-64 spends four.

Three argument encodings differ between the ABIs and are converted in both
directions: `clone` takes `tls` before `child_tid`; `O_DIRECTORY`,
`O_NOFOLLOW`, `O_DIRECT`, and `O_LARGEFILE` have different bit positions
(`openat`, `open_by_handle_at`, `pipe2`, `fcntl(F_SETFL)`); and a synthesized
`struct stat` is laid out for the tracee's architecture.

### Calls shown in their x86-64 form

A few calls have no aarch64 number, and an aarch64 C library issues another
call in their place that the contract does not catalog, or that the smaller
balanced surface does not include. The translation shows each one to the
supervisor as the call an x86-64 C library issues for the same function:

| The tracee issues | The supervisor sees |
|-------------------|---------------------|
| `faccessat(dir, path, mode)` | `faccessat2(dir, path, mode, 0)` |
| `fchmodat(AT_FDCWD, path, mode)` | `chmod(path, mode)` |
| `fchmodat(dir, path, mode)` | `fchmodat2(dir, path, mode, 0)` |
| `readlinkat(AT_FDCWD, path, buffer, size)` | `readlink(path, buffer, size)` |
| `getpgid(0)` | `getpgrp()` |

When the supervisor leaves the call as shown, the kernel runs the tracee's
own call with its own registers. When it skips the call, the result goes to
`x0` and the other registers are the tracee's. A contract rule and an audit
record name the shown call: `access()` on aarch64 is `syscall.faccessat2`.

Every other call an aarch64 C library issues in place of an x86-64-only one
(`openat`, `newfstatat`, `mkdirat`, `unlinkat`, `renameat`, `readlinkat`,
`fchownat`, `pipe2`, `dup3`, `ppoll`, `pselect6`, `epoll_pwait`, `clone`, and
the rest) is cataloged under its own name and needs nothing.

### The interception demonstration

With no arguments the daemon runs the managed payload as a tracee under
`PTRACE_SYSEMU` and answers each stopped call, on both architectures. arm64
Linux has had `PTRACE_SYSEMU` since 5.3. The kernel runs nothing at such a
stop and the result is what the tracer leaves, which on aarch64 is `x0`, so
the supervisor marks the call skipped before it writes the registers.

`ginux runtime` interprets ELF64 for either machine. An aarch64 entry point
must be 4-byte aligned.

### What a tracer of the daemon sees

Upstream's race tests ptrace the daemon and read the registers it stages for
a tracee out of the argument of its `PTRACE_SETREGS`. On aarch64 the daemon
stages with `PTRACE_SETREGSET`, and what it passes is `user_pt_regs`, already
translated. So `write_user_regs` also passes the address of the x86-64
register file it translated, as a fifth `ptrace` argument. The kernel takes
four and ignores it; nothing in the daemon reads it back.
`scripts/aarch64-trace.py` uses it to give those tests the view they have on
x86-64 (see the tests below).

## Verified

On upstream `85d53265` (pull request #334).

Directly on GitHub's `ubuntu-24.04-arm` runner, in the `aarch64` CI job:

- `make check-arm64`, the arch smoke test, the legacy-kernel boot, and the
  two guest tests.
- `test-launch` (68 checks; the x86_64 run has 69, the extra one being
  `int80`), all eight `test-native-supervision` scripts (57 checks),
  `test-gate1-setup` (3), `test-tracee-kill` (5), and `test-process-native`
  (48). These are the counts of the x86_64 job.
- `make test-unit` (every unit test of the workspace, built for and run on
  the arm64 host), `test-allocator`, and `test-allocator-linux` (the
  allocator library preloaded into its conformance and benchmark programs).

On an x86_64 machine, in the Rocky Linux 10.2 aarch64 guest (kernel 6.12)
under `qemu-system-aarch64`:

- `make test-arm64`: `test-image` 8/8; `test-intercept`, `test-runtime`,
  `test-config`, `test-instance`, `test-gate12-cli`, `test-devices`,
  `test-process`, `test-kernel-views`; then the twelve integration scripts
  above as root.
- `make test-instances-arm64`: `test-vfs`, `test-native-vfs`,
  `test-contract-runtime`, `test-gate125-lock`, `test-gate975-hooks`,
  `test-gate975-network`, `test-systemd` (cold, warm, and repeat boots to a
  login, the dnf-failure case, the startup-failure case, shutdown), and
  `test-multi-instance`.

- `ROCKY_ARCH=aarch64 make rpm` and `make test-rpm`: the package builds
  (rpmlint reports no errors, and the allocator's conformance program
  passes in the package's check step), installs, and passes upstream's
  lifecycle test. That test verifies the package, imports the Alpine and
  Rocky recipes, pulling their arm64 images, starts and stops them, boots
  the packaged Rocky systemd recipe to a login, runs a Rocky workload with
  the packaged allocator preloaded, imports from the cache with networking
  off, and reinstalls and erases the package.

In the minimal guest with the v9fs `kernel-v7.4-rc1` image:
`gate1-aarch64-guest`, `native-aarch64-guest`, and
`integration-aarch64-guest`.

The x86_64 side is unchanged in result: `make check`, `make test-unit` (174
tests), the five integration tests, the Rocky x86_64 host suite, and a
systemd instance on the Rocky x86_64 host all pass with the shared-code
changes. The x86_64 C fixtures compile to the same binaries as upstream's.

## Known gaps

What still separates the aarch64 daemon from the x86_64 one:

- **Upstream does not support it.** The approved target is x86-64. Nothing
  here has been reviewed by upstream or accepted by a human reviewer.
- **Hardware.** Only the integration tests have run on arm64 hardware (the
  CI runner). The Rocky host suite, every instance test, systemd included,
  and the package have run only under emulation on an x86_64 machine.
- **Speed.** Not measured on hardware. Under emulation a systemd instance
  reached its login in about 95 seconds on the aarch64 host and 28 on the
  x86_64 one; the two emulations are not comparable, so this says little.
  The aarch64 register translation spends six `ptrace` requests on a call
  with both stops, x86-64 four.
- **Not run on aarch64:** the benchmarks, `test-allocator-rocky`,
  `test-systemd-live`, `test-systemd-interactive` (its checks name
  `x86_64` in `/proc/version`), `test-gate19-native-examples`, `run-host`,
  and `test-rpm-metadata` (it needs `rpm` on the build machine, which this
  one lacks).
- **Recipes in the tree name amd64 images.** The aarch64 package installs
  them naming arm64 ones. A recipe used straight from the tree on an
  aarch64 host has to be given the arm64 manifest from
  `config/examples/arm64-manifests`. The Ubuntu example has no arm64 image.
- **Checks that cannot be the same.** `int80` does not exist. The `kill`
  and `pidfd_open` selector checks pass by construction (see the
  integration tests). `test-runtime` and `test-instance` interpret the
  x86-64 runtime fixture on either host; the aarch64 interpreter is covered
  by `ginux-runtime`'s unit tests only.
- **Contract names.** `access()`, `chmod()`, `readlink()` and `getpgrp()`
  reach the contract under the names in "Calls shown in their x86-64 form".
  A rule that names `syscall.access` matches nothing an aarch64 C library
  issues, as a rule that names `syscall.faccessat` would not on x86-64.
- **Structures.** `struct stat` is the only structure the supervisor reads
  or writes in a tracee's memory whose layout differs between the ABIs, and
  it is converted. A future supervisor path that touched `struct
  epoll_event` (packed on x86-64 only) or the flags in `open_how` would
  need the same.
- **The minimal guest** cannot run `test-launch`, `test-tracee-kill`, or
  the daemon-tracing scripts; the Rocky guest and the CI runner do.

## Bare-metal kernel

The aarch64 port of the historical UEFI kernel is a separate workspace. See
`baremetal/README.md`.
