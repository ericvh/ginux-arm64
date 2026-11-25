# Closing the gap between aarch64 and x86_64

> Fork-track record. Not an upstream gate. See `hdae/arm64/README.md` for
> its standing.

## Source

The human asked on 2026-11-25 whether the aarch64 daemon was on parity with
the x86_64 one, was told it was not and why, and then set the goal "close
the gap between aarch64 and x86_64". The scope below is the agent's reading
of that goal against the gaps it had listed. It has not been reviewed.

## Goal

Everything upstream tests on x86_64 that can be run against the aarch64
daemon is run against it, and what fails is fixed. What cannot be run, or
was not, is named.

## Scope

- Upstream's root-run integration tests: `test-launch`,
  `test-native-supervision` (all eight scripts), `test-gate1-setup`,
  `test-tracee-kill`, `test-process-native`.
- Upstream's instance tests on the Rocky host: the VFS, contract
  enforcement, the contract lock, hooks, shared networking, systemd in an
  instance, and two instances at once.
- The code gaps `docs/arm64/README.md` listed: structure layouts,
  synthesized status flags, calls rewritten into x86-64-only ones, the vDSO
  list.
- The RPM.

## Non-goals

- Any change to what the x86_64 daemon does, except where a fault is in
  shared code and shows on both.
- A change to the contract catalog. Calls an aarch64 C library issues that
  the catalog lacks are handled at the architecture boundary.
- The bare-metal kernel line under `baremetal/`.

## Delivery

Branch `arm64/main` on the fork. No pull request.

## Faults found and fixed

Each of these was found by running an upstream test against the aarch64
daemon.

| Fault | Found by | Fix |
|-------|----------|-----|
| Gate 1 interpreted its payload instead of tracing it, on the premise that arm64 has no `PTRACE_SYSEMU`. There was no tracee, so the setup-failure and kill properties were untestable. | `test-gate1-setup` | Gate 1 runs under `PTRACE_SYSEMU` on both architectures. |
| The native loop matched `getdents64` against the daemon's host number for it. A listing of a virtual `/proc` directory went to the host and failed. | `test-systemd` (systemd: "Failed to allocate fd set") | Matched by x86-64 number, like every other call. |
| `access()` and `chmod()` are `faccessat` and `fchmodat` on aarch64, which the catalog lacks. Every `access()` in an instance was denied with `ENOSYS`. | `test-systemd` (137 denials in one boot; `ldconfig.service` failed) | Shown to the supervisor as `faccessat2`, `chmod`, `fchmodat2`. |
| The virtual poweroff compared `reboot`'s magic numbers as 64-bit. An aarch64 C library passes the first sign-extended. A root init could not power its instance off. | `test-systemd` | Compared as the 32-bit values the kernel reads. Shared code. |
| `getpgrp()` is `getpgid(0)` on aarch64, which the balanced surface lacks. Bash's job control failed and the probes after it never ran. | `test-contract-runtime` | Shown as `getpgrp`. `readlinkat` on the working directory is shown as `readlink` for the same reason. |
| The index tool required the x86-64 loader path in a root filesystem. No aarch64 root could be prepared. | `make systemd-image-prepare` | Accepts either architecture's loader. |
| The image puller and `ensure-oci-image.sh` selected the `linux/amd64` manifest on every host. | Reading the code | Select the host's platform. |
| The RPM spec named the x86_64 target triple. | Reading the spec | Takes the triple from the build architecture. |
| The daemon did not link with Rocky's Rust toolchain on aarch64: its `compiler_builtins` carries a C routine built with the stack protector, and the freestanding daemon has no C library to supply the protector's symbols. | `ROCKY_ARCH=aarch64 make rpm` | The architecture layer supplies them. |
| The packaged recipes named `linux/amd64` images, which an aarch64 host cannot run. | Reading `rpm-test-guest.sh` before running it | The aarch64 package installs them naming the `linux/arm64` manifests of the same indexes. |
| The VFS probe reported the instance shell as an x86-64 ELF whatever it read. | `test-vfs` | Reports the machine it read. |
| The aarch64 managed payload was linked with 4K segment alignment. A 16K-page kernel loaded its three segments onto one page and it faulted. | `test-tracee-kill` on an Apple M1 | Linked for the largest arm64 page size. |
| The straddle scenarios of the process fixture found the end of the last mapped page by rounding to 4K. | `test-process-native` on an Apple M1 | The fixture reads the kernel's page size on aarch64. |
| The supervisor resumed its tracee before printing two status lines in several writes; the tracee's own line could land inside one. | `test-native-vfs` in the Rocky guest under KVM | The lines go out before the resume. Shared code. |
| `abort()` in any process of an instance ended the instance: glibc's fallback trap is `brk #1000`, which raises `SIGTRAP`, and a tracee's `SIGTRAP` failed the instance. On x86-64 the fallback is `hlt` and only the process dies. | `test-systemd-live` | A `brk` with a nonzero immediate is delivered to the tracee. `brk #0` still fails the instance, as `int3` does. |
| Launch treated `getcpu` and `time` as answered by the vDSO. aarch64's vDSO has neither, so a contract could not deny `getcpu`. | Code audit | The list is per architecture. |

Two more were in upstream's tests, not the daemon, and are recorded in
`docs/arm64/README.md`: the launch test's contract checks name `open`, which
an aarch64 fixture cannot issue, and two seccomp filters in the tests name
the x86-64 architecture.

## A decision that needs review

The trap rule is a choice, not a translation. Upstream fails an instance
closed at a tracee's own breakpoint trap, and its test asserts that. On
aarch64 the same signal also covers what x86-64 reports as `SIGILL` and
`SIGSEGV`, so applying the rule to every `brk` made any aborting process
fatal to its instance. The agent chose to deliver `brk` with a nonzero
immediate and keep `brk #0` fatal. The alternative is to keep every `brk`
fatal and accept that difference. The rule is one function,
`arch::abort_trap`.

## Defects found outside the scope

In shared code or in upstream's tests, present on x86_64 too, and not fixed
here. `docs/arm64/README.md` describes each under "Upstream defects this
work ran into":

- `connect` with `AF_UNSPEC` is refused with `EINVAL`, which makes glibc's
  `getaddrinfo` abort on a host with an IPv6 route. `test-systemd-live`
  fails on both architectures in the guests used here for that reason.
- The contract has no `tgkill`, so `abort()` cannot raise `SIGABRT`.
- One spin-count timing assumption in `test-native-supervision` fails under
  x86 emulation.
- Lines from two writers can interleave on a shared pipe and defeat a
  whole-line assertion.
- `ginux start` reports `failed to start` for an instance whose init has
  already exited, which fails the package lifecycle test under KVM on both
  architectures.
- The pinned Rocky 10.2 x86_64 image URL is intermittently unavailable.

Whether to report any of them upstream, or to fix them in this tree, is the
human's decision. The first is the one with user-visible effect: name
resolution fails in an instance on a dual-stack host.

## Audit of the listed code gaps

- **Structures in tracee memory.** The supervisor writes or reads these
  through a tracee's memory: `stat`, `sysinfo`, `utsname`, `rusage`, wait
  status words, `siginfo` for `waitid`, tid cells, `clone_args`, `msghdr`,
  `sockaddr`, `iovec`, `timespec`. Only `stat` is laid out differently on
  the two ABIs, and it is converted. `epoll_event` and `open_how` are not
  touched.
- **Status flags.** No supervisor path synthesizes an `F_GETFL` result.
- **Calls rewritten into x86-64-only ones.** The supervisor rewrites calls
  into, or replays, `fcntl`, `dup`, `close`, `close_range`, `exit_group`,
  `clone3`, `mmap`, `munmap`, and the network sends. All exist on aarch64.
- **Host numbers used as x86-64 numbers.** Every use of a host syscall
  constant outside a direct call was checked. `getdents64` was the one.

## Acceptance criteria and evidence

| Criterion | Where it ran | Result |
|-----------|--------------|--------|
| Upstream's five integration tests pass against the aarch64 daemon | GitHub `ubuntu-24.04-arm` runner, natively (`aarch64` CI job) | `test-launch` 68 checks, `test-native-supervision` 57 across the eight scripts, `test-gate1-setup` 3, `test-tracee-kill` 5, `test-process-native` 48 |
| The same, on a second arm64 machine and page size | Apple M1, Fedora Asahi, kernel 6.14, 16K pages, natively | the same counts |
| The same, on a distribution kernel | Rocky 10.2 aarch64 guest, kernel 6.12 (`make test-arm64`), under KVM on the M1 and under emulation | the same counts; the main supervision script alone has 42 |
| Upstream's Rocky host suite still passes | the same guest (`make test-arm64`), KVM and emulated | `test-image` 8/8, `test-intercept`, `test-runtime`, `test-config`, `test-instance`, `test-gate12-cli`, `test-devices`, `test-process`, `test-kernel-views` |
| Upstream's instance tests pass | the same guest (`make test-instances-arm64`): under KVM on the M1 at upstream's time limits, and emulated with the login driver's limits stretched | `test-vfs`, `test-native-vfs`, `test-contract-runtime`, `test-gate125-lock`, `test-gate975-hooks`, `test-gate975-network`, `test-systemd`, `test-multi-instance` |
| A systemd instance boots in comparable time on both architectures | Rocky guests under KVM: aarch64 on an Apple M1, x86_64 on an AMD Threadripper 1950X | login after 3.3 s on aarch64; 3.4 s and 3.9 s on x86_64 |
| The package builds and passes upstream's lifecycle test | Rocky 10.2 aarch64 guests, emulated (`ROCKY_ARCH=aarch64 make rpm`, `make test-rpm`) | `Rocky Linux RPM build: PASS`, rpmlint 0 errors; `rpm-test-guest: PASS`, `Rocky Linux RPM lifecycle: PASS` |
| The package builds on arm64 hardware | Rocky 10.2 aarch64 guest under KVM on the M1 | built in 164 s; `test-rpm-metadata: PASS`; the lifecycle test fails at the step it fails at for the x86_64 package under KVM (see the defects) |
| The workspace's unit tests and the allocator pass on an arm64 host | the CI runner | `make test-unit`, `test-allocator`, `test-allocator-linux` |
| The x86_64 fixtures are the same programs | compile upstream's files and these with the Makefile's flags, `cmp` | 38 binaries identical |
| The x86_64 daemon is unchanged in result | this x86_64 machine, and the `launch-integration` CI job | `make check`; `test-unit` 174 passed; `test-launch` 69, `test-native-supervision` 57, `test-gate1-setup` 3, `test-tracee-kill` 5, `test-process-native` 48 |
| Shared-code changes hold on upstream's reference host | Rocky 10.2 x86_64 guest | the host suite; a systemd instance to login and shutdown; `test-contract-runtime`; eleven of the twelve integration scripts (see limitations) |

## Limitations

- The arm64 hardware is one Apple M1 and the CI runner. No 64K-page kernel
  has run any of this.
- `test-systemd-live` does not pass on either architecture in these guests;
  see the defects above.
- The package lifecycle test passed in an emulated guest only; under KVM
  it stops at a step that fails for x86_64 too. Its Ubuntu step does not
  run on aarch64, because that image has no arm64 build.
- In the Rocky x86_64 guest, one check of `test-native-supervision`
  ("shared descriptor mutations are serialized") fails. It allows a sibling
  thread one million spin iterations to reach a blocked call, which is not
  long enough for a ptrace-supervised thread under x86 emulation. It passes
  natively on x86_64 and on aarch64 in every environment tried. Upstream
  does not run that script in the Rocky guest.
- One x86_64 run of `test-native-close-range-gap` failed because a fixture
  line and an audit line interleaved on the shared pipe; three reruns
  passed. Upstream's script reads lines from that pipe.
- The time limits of the login driver and of the tracing scripts are
  stretched for the emulated aarch64 host (`GINUX_EXPECT_TIME_SCALE`). The
  integration scripts ran there at their own limits.
- The register cache in the architecture layer was verified by these
  suites, not by a test of its own.

## Review

Open. Not accepted by a human reviewer.
