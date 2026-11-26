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

One test is this port's own. `sudo make test-page-size-arm64` runs
`tests/fixtures/page-size-fixture.c` as an instance's init and checks what
depends on the host kernel's page size: `getcwd` with a buffer of one page,
the virtual memory figures, and `clone3` and a send from a second task. On
a 4K-page kernel it passes whatever the supervisor assumes, so it only
means something on a 16K-page or 64K-page host; see "Page size". The
Rocky-host runner includes it for aarch64.

`make page-size-aarch64-guest` runs it where it does mean something: in
the minimal guest on a kernel with 64K pages, such as the `vmlinuz` of
Ubuntu's `linux-image-unsigned-*-generic-64k` package
(`GINUX_AARCH64_KERNEL`, with `GINUX_AARCH64_ROOTFS` as above). A
distribution's kernel has 9p only as a module, so for this target the
guest carries the daemon, the fixture, and the script in its initramfs.
The run fails unless the guest reports `GINUX_PAGE_SIZE`-byte pages (65536
by default). It takes about fifteen seconds under emulation, and the
aarch64 CI job runs it.

The same switch, `GINUX_AARCH64_SHARE=initrd`, lets
`make integration-aarch64-guest` run on a distribution's kernel, and the
guest runner unpacks an EFI zboot `vmlinuz` itself. The CI job runs
`test-gate1-setup` and `test-process-native` (48 checks) that way on the
64K-page kernel. `test-native-supervision` passes there too on a kernel
newer than 6.8 (42 checks on Ubuntu's 7.0 64K-page kernel), but not
reliably in a freshly booted guest, for a reason that has nothing to do
with the page size; both are under "Upstream defects this work ran into".

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

On an arm64 machine with `/dev/kvm`, `ROCKY_ACCEL=kvm` runs the guest on the
machine's own CPU instead of emulating it. The suite then takes minutes, not
the better part of an hour, and the time limits need no stretching:

```sh
ROCKY_ACCEL=kvm make test-arm64
ROCKY_ACCEL=kvm make test-instances-arm64 GINUX_EXPECT_TIME_SCALE=1
```

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

The host is emulated unless `ROCKY_ACCEL=kvm` is set
(`start-rocky-host.sh` uses TCG by default, as upstream's does), and an
emulated aarch64 host is slower than an emulated x86_64 one: systemd
reached its login in about 95 seconds here against 28.
`GINUX_EXPECT_TIME_SCALE` (10 in this target) multiplies the login driver's
50-second and 120-second limits. It is unset, and the limits are upstream's,
in every x86_64 target. Under KVM the limits need no stretching:
`GINUX_EXPECT_TIME_SCALE=1`.

An instance config names its root by image manifest, and upstream's files
name the amd64 ones. The daemon treats that as an identity and the tests
stage the arm64 root filesystem themselves, so those files work unchanged.
A recipe meant to pull its own image on an aarch64 host has to name the
arm64 manifest or the index.

### The package

```sh
make rpm-arm64        # builds in the Rocky aarch64 guest
make test-rpm-arm64   # checks its metadata, installs it in a fresh guest, and uses it
```

These are `ROCKY_ARCH=aarch64 make rpm`, `test-rpm-metadata`, and
`test-rpm`. The spec builds the runtime for the package's own
architecture. The build runs inside the guest: about an hour emulated, and
between one and a half and three minutes with `ROCKY_ACCEL=kvm` on an arm64
machine.

The example recipes name each image by its `linux/amd64` manifest.
`config/examples/arm64-manifests` lists the `linux/arm64` manifest of each
image from the same index, and the aarch64 package installs the recipes
with those, so `ginux import` of a packaged recipe fetches an image the
host can run. `docker.io/jrei/systemd-ubuntu`, which
`ubuntu24-network-service.toml` uses, is published for `linux/amd64` only:
that recipe has no arm64 image, and `test-rpm` runs its Ubuntu step on
x86_64 only. The recipes in the tree are upstream's, unchanged.

### The other Rocky-host targets

Each of upstream's remaining Rocky-host targets has an `-arm64` target that
runs the same script against the aarch64 daemon or package:

| Target | Upstream's target |
|--------|-------------------|
| `test-allocator-rocky-arm64` | `test-allocator-rocky`: the allocator built and checked on Rocky |
| `test-gate19-native-examples-arm64` | `test-gate19-native-examples`: the packaged Alpine and Rocky recipes imported and run. The Ubuntu recipe has no arm64 image and is skipped. |
| `test-systemd-interactive-arm64` | `test-systemd-interactive`: `run-host`'s script driven from a terminal, the package installed and a systemd instance used from its shell |
| `run-host-arm64` | `run-host`: the same host, left at a shell |
| `test-systemd-live-arm64` | `test-systemd-live`: `dnf update` in a systemd instance against the live mirrors |
| `bench-prepare-arm64`, `bench-baseline-arm64`, `bench-modes-arm64` | the sysbench baseline and the dispatch-mode matrix |
| `bench-block-arm64`, `bench-network-arm64`, `bench-systemd-arm64` | the device and boot benchmarks |

The benchmark host and the interactive test's host have their own files for
aarch64 (`build/gate11-host-aarch64-*`, `build/rocky-test-host-aarch64-*`),
as the other hosts do.

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

The translation needs to know what kind of stop it is reading: a call's
entry, its exit, a seccomp stop, or none of these. x86-64 code reads that
from the wait status and `rax`. The aarch64 layer is given the status too:
`supervisor::wait` and the native loops pass each one to
`arch::after_wait`, which is a no-op on x86-64. A seccomp stop is an event
in the status. A syscall stop is `SIGTRAP` with bit 7, and the kernel marks
its side in `x7` (0 at the entry, 1 at the exit). At the first stop of a
call `x8` is still the number the tracee issued, and at an exit the number
comes from the entry's record. The kernel is asked
(`PTRACE_GET_SYSCALL_INFO`) only where the status does not settle it: a
plain `SIGTRAP`, a seccomp stop that follows an entry stop (a tracer may
have changed the number there), or a stop whose status the layer was not
given.

The registers read at a stop are kept until the tracee can run again, so a
write does not read them a second time: the daemon's `ptrace` wrapper tells
the architecture layer about every request, and any request but a read or
write of a stopped tracee forgets them, and the status with them.

Together these leave the aarch64 supervisor making the requests the x86-64
one makes, plus one where it changes a call's number, which is a register
set of its own on arm64 (`NT_ARM_SYSTEM_CALL`). Counted with `strace` over
four scenarios of the native supervision test (`fork`, `descriptors`,
`many-reap`, `host-info`), which stop 104 times on either architecture:

| | `ptrace` requests |
|---|---:|
| x86_64 | 258 |
| aarch64, asking the kernel at every stop | 357 |
| aarch64, from the wait status | 269 |

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

### A tracee's own trap

x86-64 has `int3` for a breakpoint and `ud2` or `hlt` for a program that
aborts, and they raise different signals. The native loop fails the
instance at a tracee's `SIGTRAP` and delivers its `SIGILL` or `SIGSEGV`.
aarch64 has one trap instruction, `brk`, and every form raises `SIGTRAP`:
`brk #0` is the breakpoint, and the other immediates are how programs abort
(`brk #1000` from GCC's `__builtin_trap` and glibc's `abort`, `brk #1` from
LLVM). So on aarch64 a `brk #0` fails the instance, as `int3` does, and a
`brk` with any other immediate is delivered to the tracee as the `SIGTRAP`
the kernel raised.

This matters because the contract has no `tgkill`: glibc's `abort()` cannot
raise `SIGABRT` in an instance and falls back to its trap instruction, on
both architectures. The process then dies of `SIGSEGV` on x86-64 and of
`SIGTRAP` on aarch64. Before this rule it took the whole instance with it
on aarch64.

The two signals are the C library's, not the daemon's. Outside any
instance, a program that installs a seccomp filter failing `tgkill` with
`ENOSYS` and calls `abort()` is killed by signal 11 on x86-64 and by signal
5 on aarch64, on the two machines themselves and in the Rocky guests.

### Page size

arm64 kernels run with 4K, 16K, or 64K pages. The daemon and the C fixtures
are linked for 64K. The aarch64 managed payload is too: it was linked for
4K while the daemon interpreted it, and faulted when a 16K-page kernel
loaded it. The process fixture reads the page size from the auxiliary
vector where it needs the end of a page.

Three things in shared code assumed 4K pages, which only an arm64 kernel
can contradict. Two were found by running upstream's tests on Rocky's
64K-page kernel, and the third by reading the code after that.

The supervisor copies the arguments of `connect`, `bind`, the sends, and
`clone3` into a private read-only mapping in the tracee, at a fixed address
for the task's slot. The network slots were 16K apart and the `clone3`
slots 4K apart. A fixed mapping must start on a page boundary, so on a
64K-page kernel three network slots in four could not be mapped, and on a
16K-page kernel three `clone3` slots in four; the tracee's call came back
`EINVAL`. journald announces itself to PID 1 with such a send, so on the
64K-page kernel systemd in an instance waited for journald until it timed
out. The slots are now at least `arch::MAX_PAGE_SIZE` apart: that is 4K on
x86-64, where nothing moves, and 64K on aarch64.

`getcwd` was refused for a buffer above 4096 bytes. A C library's
`getcwd(NULL, 0)` asks with `PATH_MAX` or one page, whichever is larger,
so on 16K and 64K kernels Bash could not find its working directory. The
bound is now the larger of 4096 and the page size.

The virtual memory figures (`/proc/<pid>/statm` and `status`,
`/proc/meminfo`, the cgroup's `memory.current`) turn the kernel's page
counts into bytes. They did it with 4096, which is right on x86-64 and
wrong by a factor of 4 or 16 on the other arm64 kernels: on the 16K-page
M1 a process of 17 pages reported 68. The architecture layer now reads the
page size from the daemon's auxiliary vector (`arch::page_size`).

`test-page-size-arm64` checks all three. Against the daemon from before
these fixes it fails on `getcwd`, the memory figures, and the second
task's `clone3` on the 16K-page M1, and on those and the second task's
send in the 64K-page guest. Against the current daemon it passes on both.
The spacing is also asserted when the daemon is built. CI runs the test on
Ubuntu's 64K-page kernel (`make page-size-aarch64-guest`), where the old
daemon fails all four checks as well.

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

**On arm64 hardware.** An Apple M1 running Fedora Asahi Remix 40, kernel
6.14 with 16K pages, SELinux enforcing:

- Natively: `make check-arm64`, the arch smoke test, `test-launch` (68
  checks), all eight `test-native-supervision` scripts (57),
  `test-gate1-setup` (3), `test-tracee-kill` (5), `test-process-native`
  (48), `make test-unit`, and `test-allocator-linux`.
- In the Rocky Linux 10.2 aarch64 guest under KVM (kernel 6.12, 4K pages),
  every one at upstream's own time limits:
  - `make test-arm64` in under two minutes and `make test-instances-arm64`
    in under four. That is `test-image`, the host suite, the twelve
    integration scripts, `test-vfs`, `test-native-vfs`,
    `test-contract-runtime`, `test-gate125-lock`, `test-gate975-hooks`,
    `test-gate975-network`, `test-systemd`, and `test-multi-instance`.
  - `make test-rpm-arm64`: the package builds in about a minute and a half
    (rpmlint reports no errors), `test-rpm-metadata` passes, and upstream's
    lifecycle test passes.
  - `test-allocator-rocky-arm64`, `test-gate19-native-examples-arm64`, and
    `test-systemd-interactive-arm64`, which drives `run-host`'s script end
    to end.
  - The benchmarks: `bench-systemd-arm64`, `bench-block-arm64`,
    `bench-network-arm64`, `bench-baseline-arm64`, and `bench-modes-arm64`.
    Their figures are under "What supervision costs".

**On a 16K-page kernel in a guest.** The M1's own kernel
(`6.14.6-400.asahi.fc40.aarch64+16k`) installed into the Rocky aarch64
guest, under KVM: the host suite, the twelve integration scripts with the
same counts, `test-native-page-size`, and the eight instance tests.

**On a 64K-page kernel.** Rocky's `kernel-64k`
(`6.12.0-264.20.1.el10_3.aarch64+64k`) in the Rocky aarch64 guest, emulated
on an x86_64 machine, because the M1 cannot run 64K-page guests: the host
suite and all twelve integration scripts pass with the same counts, and so
do the instance tests: `test-vfs`, `test-native-vfs`,
`test-contract-runtime`, `test-gate125-lock`, `test-gate975-hooks`,
`test-gate975-network`, `test-systemd`, and `test-multi-instance`. The
last three boot systemd in an instance, which did not get past journald at
first; see "Page size". The same kernel build with 4K pages
(`6.12.0-264.20.1.el10_3`) passes `test-systemd` on both architectures
under KVM.

**On GitHub's `ubuntu-24.04-arm` runner**, in the `aarch64` CI job: the
same native list as above, and the two minimal-guest tests.

**Under emulation on an x86_64 machine**, in the Rocky aarch64 guest: the
two suites, with the login driver's limits stretched, and the package
(`make rpm-arm64`, `make test-rpm-arm64`).

**The x86_64 side** is unchanged in result: `make check`, `make test-unit`
(174 tests), the five integration tests (69, 57, 3, 5, and 48 checks), the
Rocky x86_64 host suite, and a systemd instance on the Rocky x86_64 host
all pass with the shared-code changes. The x86_64 C fixtures compile to the
same binaries as upstream's.

### Beside x86_64

The same targets on the Rocky x86_64 guest under KVM on an AMD Threadripper
1950X, and on the Rocky aarch64 guest under KVM on the M1. The machines
differ, so the figures compare nothing finer than the order of magnitude.

| | x86_64 | aarch64 |
|---|---|---|
| `test-systemd-interactive` | passes, 8 runs of 8 | passes, 12 runs of 12; before the process-group fix below it failed 6 runs of 19 |
| Package lifecycle (`test-rpm`) | passes | passes |
| `test-systemd-live` | fails: `dnf update` cannot unpack five packages | fails on the same five files with the same errors |
| Systemd instance, boot to login (`bench-systemd`) | 3540 ms | 2446 ms and 2592 ms in two runs |
| Block device model (`bench-block`) | 942 ns per operation | 291 ns and 1448 ns |
| Network device model (`bench-network`) | 527 ns per operation | 1267 ns and 1616 ns |

The two device rows measure the M1's two kinds of core more than anything
else. Each is a short single-threaded loop in the daemon, the same code on
both architectures, and the guest's virtual CPU lands on a performance core
or an efficiency core. Run natively, the network loop takes 502 ns per
operation on the Threadripper, 351 ns on an M1 performance core, and 798 ns
on an M1 efficiency core; the block loop takes about 1060 ns, 430 ns, and
1100 to 1750 ns.

### What supervision costs

**Requests.** The aarch64 supervisor makes the `ptrace` requests the
x86-64 one makes, plus one where it changes a call's number. The count is
under "What the aarch64 translation has to reconstruct".

**One supervised call.** `getppid` 200,000 times as the init of
`ginux native` with `config/instance-gate5.toml` (paranoid dispatch, every
call audited), the daemon and the workload on one CPU:

| | Natively | Supervised |
|---|---:|---:|
| Threadripper 1950X | 0.17 µs | 32 µs |
| M1 performance core | 0.13 µs | 12.6 µs (13.0 µs before the wait-status change) |

**Upstream's dispatch-mode matrix** (`bench-modes`: sysbench for five
seconds, five repetitions, with 1, 2, and 4 vCPUs). The figure is the
instance's rate over the native rate on the same guest, as the range across
the five vCPU and thread combinations:

| Workload | x86_64, selective | aarch64, selective | x86_64, paranoid | aarch64, paranoid |
|----------|---|---|---|---|
| cpu | 0.998x to 1.002x | 0.997x to 1.000x | 0.989x to 1.001x | 0.998x to 1.005x |
| memory | 0.991x to 1.003x | 0.956x to 1.007x | 0.980x to 1.000x | 0.991x to 1.185x |
| threads | 0.923x to 0.943x | 0.901x to 0.913x | 0.019x to 0.092x | 0.033x to 0.171x |
| mutex | 0.866x to 1.078x | 0.917x to 1.138x | 0.820x to 1.096x | 0.941x to 1.083x |

Compute-bound work is at native speed on both. `mutex` varies between
repetitions on both and shows nothing.

`threads` under selective dispatch looks like an aarch64 deficit and is
not one. Each of its events is a thousand `sched_yield` calls, and
selective dispatch lets that one call through in the kernel: the supervisor
never sees it. What the row measures is what a seccomp filter adds to a
call. Per yield:

| | Natively | In an instance | Added | With a bare seccomp filter, no daemon | Added |
|---|---:|---:|---:|---:|---:|
| x86_64 guest | 295 ns | 320 ns | 25 ns | 282 ns to 306 ns | 24 ns |
| aarch64 guest | 252 ns | 278 ns | 25 ns | 230 ns to 255 ns | 26 ns |

The fourth and fifth columns are a program that times the call, installs a
filter of the shape the daemon installs, and times it again, in the same
guests. The kernel adds the same 25 nanoseconds on both; the M1's yield is
quicker to begin with, so the same cost is a larger share of it. Under
paranoid dispatch the yield goes to the supervisor, and there it costs 2.8
to 16 microseconds on the Threadripper and 1.2 to 7.6 on the M1.

## Known gaps

What still separates the aarch64 daemon from the x86_64 one:

- **Upstream does not support it.** The approved target is x86-64. Nothing
  here has been reviewed by upstream or accepted by a human reviewer.
- **One arm64 machine.** The hardware results are from one Apple M1 and the
  CI runner, and the 64K-page results from an emulated guest. No
  server-class arm64 CPU has run it.
- **CI covers other page sizes in part.** On a 64K-page kernel the aarch64
  job runs the page-size test, which fails if one of the three 4K
  assumptions found so far comes back, and two of upstream's integration
  scripts. The build fails if the
  snapshot spacing stops being page-aligned. The Rocky host suite and the
  instance tests on 16K and 64K pages are runs made by hand: natively and
  in a guest on the M1, and in an emulated guest. They need a Rocky host,
  which upstream's CI does not boot for x86_64 either.
- **`test-systemd-live` fails on both architectures.** See "Upstream
  defects this work ran into" below.
- **Recipes in the tree name amd64 images.** The aarch64 package installs
  them naming arm64 ones. A recipe used straight from the tree on an
  aarch64 host has to be given the arm64 manifest from
  `config/examples/arm64-manifests`. The Ubuntu example has no arm64 image,
  so the Ubuntu steps of `test-rpm` and `test-gate19-native-examples` run
  on x86_64 only.
- **Checks that cannot be the same.** `int80` does not exist. The `kill`
  and `pidfd_open` selector checks pass by construction (see the
  integration tests). `test-runtime` and `test-instance` interpret the
  x86-64 runtime fixture on either host; the aarch64 interpreter is covered
  by `ginux-runtime`'s unit tests only. A process that aborts dies of
  `SIGTRAP` where it dies of `SIGSEGV` on x86-64, as it does outside an
  instance (see "A tracee's own trap").
- **Contract names.** `access()`, `chmod()`, `readlink()` and `getpgrp()`
  reach the contract under the names in "Calls shown in their x86-64 form".
  A rule that names `syscall.access` matches nothing an aarch64 C library
  issues, as a rule that names `syscall.faccessat` would not on x86-64.
- **Two calls work on aarch64 and not on x86-64.** The catalog lacks
  x86-64's legacy `link` and `fchmodat` numbers, so the calls an x86-64 C
  library issues for `link()` and for `fchmodat()` without flags get
  `ENOSYS` there. An aarch64 C library issues `linkat`, and its `fchmodat`
  is shown as `fchmodat2`; both are cataloged.
- **Structures.** `struct stat` is the only structure the supervisor reads
  or writes in a tracee's memory whose layout differs between the ABIs, and
  it is converted. A future supervisor path that touched `struct
  epoll_event` (packed on x86-64 only) or the flags in `open_how` would
  need the same.
- **The minimal guest** cannot run `test-launch`, `test-tracee-kill`, or
  the daemon-tracing scripts; the Rocky guest, the CI runner, and an arm64
  machine do.

Speed is not on this list. Nothing measured shows the aarch64 daemon
costing more than the x86_64 one; see "What supervision costs".

## Upstream defects this work ran into

These are in shared code or upstream's tests and show on x86_64 too.

Fixed in this tree, because each stood between an upstream test and any
result on either architecture:

- **`connect` with `AF_UNSPEC` was refused.** That call dissolves a
  datagram socket's association. The native network mediation answered it
  with `EINVAL`. glibc's `getaddrinfo` uses it between source-address
  probes; when the host has an IPv6 route, the next probe left an IPv6
  source on a socket glibc expects to be IPv4-mapped, and glibc aborted on
  an internal assertion. `dnf` died resolving its mirrors. A UDP socket may
  now dissolve its association, through the same snapshot of the address
  the other calls use.
- **`ginux start` of an instance whose init exits at once.** The package
  lifecycle test starts an instance that runs `ls /` and expects the start
  to succeed. Under KVM the workload had finished before the helper looked
  for it, and the helper reported `failed to start`. It now accepts an
  instance that has finished with status 0. `make test-rpm` passes under
  KVM on both architectures.
- **A dead child's process group.** The emulated `setpgid` updates the
  console broker's PID map and failed the instance when the entry was
  missing. A task that has exited has left that map but stays in the
  process table until it is reaped, and a shell with job control sets its
  child's group after every fork. On the M1 this ended
  `test-systemd-interactive` in about one run in three; on the slower
  x86_64 machine it did not show in eight runs. The update is now skipped
  for a task that has exited.
- **`close_range` flags.** See gate record A6.
- **Output shared between writers, inside the daemon.** The supervisor's
  two status lines could be split by its tracee's first line. They now go
  out before the tracee is resumed.

Not fixed here. Whether to report them upstream is the maintainer's call:

- **`dnf update` cannot unpack some packages in an instance**, so
  `test-systemd-live` fails. With name resolution working the update runs,
  and five files fail to unpack, the same five with the same errors on
  both architectures: `/lib64/libgcc_s-14-20260724.so.1` (`Unsafe
  symlink`), `/usr/bin/mount` and `/usr/bin/chfn` (`chmod failed -
  Operation not permitted`), `/usr/sbin/pam_timestamp_check` (`chmod
  failed - Directory not empty`), and `/usr/bin/chage` (`chmod failed - No
  such file or directory`). Two causes are visible. Only `fstat` results
  carry the instance's virtual owner; a lookup of the same file by path
  reports the host's, and rpm refuses a symbolic link whose owner differs
  from its target's. And a contract admits set-ID modes only for the paths
  on its `setuid` and `setgid` lists, which upstream's recipes leave empty
  by design, so `chmod` to a set-user-ID mode is refused. The test also
  moves: it updates the pinned 10.2 image against the mirrors of the day,
  which is now some two hundred packages, `util-linux` among them. Making
  it pass means changing what upstream's contract admits. That is not a
  difference between the architectures, and it is not this port's to
  decide.
- **The contract has no `tgkill`.** `raise()` and so `abort()` cannot signal
  the calling thread. See "A tracee's own trap".
- **On Linux 6.8 a reaped child does not free its place.** Upstream's
  check that `waitid` releases reaped children fails on Ubuntu's 6.8
  kernel on both architectures: after 127 children the instance ends with
  `native virtual child failed`. It passes on every later kernel tried,
  from 6.12, which is what upstream's Rocky host runs. The supervisor
  learns that a child was reaped from `POLLHUP` on the pidfd it holds; the
  likely cause is that 6.8 does not report that yet, which has not been
  checked. Found while looking for a 64K-page kernel that CI could boot.
- **A virtual pid that equals a host pid.** For `waitid(P_PID, n)` the
  supervisor looks for the task whose virtual pid is `n` or whose host pid
  is `n`, the second so that a restarted call still matches, and takes the
  first it finds. When a child's virtual pid equals the host pid of a task
  earlier in the table, such as the caller's own, the kernel is asked
  about the wrong process and answers `ECHILD`. Host pids are usually far
  above virtual ones, so it takes a pid namespace or a freshly booted
  guest to see it. Upstream's `null-wait` scenario then fails for about
  one starting pid in three. On x86_64:
  `sudo unshare --pid --fork --mount-proc` and run that scenario of
  `test-native-supervision.py` as the first thing in the namespace. This
  is what stopped `test-native-supervision` in the 64K-page guest in CI.
- **x86-64's legacy `link` and `fchmodat` are not in the catalog.** See
  "Known gaps".
- **One timing assumption in `test-native-supervision`.** The "shared
  descriptor mutations are serialized" check gives a supervised thread one
  million spin iterations to reach a blocked call. Under x86 emulation that
  is not long enough and the check fails; it passes natively.
- **Output shared between writers, in the tests.** The supervisor's audit
  lines and a workload's lines can interleave on one pipe, and a test that
  needs a line whole can miss it. One x86_64 run of
  `test-native-close-range-gap` did.
- **The pinned Rocky 10.2 x86_64 image URL** is intermittently unavailable
  now that 10.3 is current. The vault has it.

## Bare-metal kernel

The aarch64 port of the historical UEFI kernel is a separate workspace. See
`baremetal/README.md`.
