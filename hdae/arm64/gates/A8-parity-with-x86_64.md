# Closing the gap between aarch64 and x86_64

> Fork-track record. Not an upstream gate. See `hdae/arm64/README.md` for
> its standing.

## Source

The human asked on 2026-11-25 whether the aarch64 daemon was on parity with
the x86_64 one, was told it was not and why, and then set the goal "close
the gap between aarch64 and x86_64". The scope below is the agent's reading
of that goal against the gaps it had listed. It has not been reviewed.

The goal was set as a standing condition on the session, with the
instruction not to pause for direction until it held. So the work below ran
without the checkpoints this process otherwise asks for, including the
shared-code fixes under "Shared faults fixed here". Each such choice is
listed under "Decisions that need review".

## Goal

Everything upstream tests on x86_64 that can be run against the aarch64
daemon is run against it, and what fails is fixed. What cannot be run, or
was not, is named.

As written, this is not met for one test. `test-systemd-live` fails, and it
fails identically on x86_64, for reasons in upstream's contract (see
"Defects found outside the scope"). On 2026-11-26 the agent reworded the
goal to cover only failures that x86_64 does not share, and then put the
original back: narrowing a goal is the human's decision, not the agent's.
Its recommendation is that narrowing, because a test that fails the same
way on both architectures is not a gap between them.

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
- Upstream's remaining Rocky-host targets: the allocator's Rocky build, the
  native examples, the interactive systemd test (which drives `run-host`),
  the live test, and the benchmarks.
- What a supervised call costs on aarch64 beside x86_64.

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
| `getrusage(RUSAGE_CHILDREN)` was refused. The argument check accepted the selector as 0, 1, or `0xffffffff`, the -1 an x86-64 C library leaves in the register. An aarch64 C library sign-extends it. Bash's `time` printed what was on its stack. | `test-systemd-interactive` | Compared at the width the kernel reads it. Shared code. |
| The supervisor's argument snapshots were mapped at fixed addresses 16K apart (network calls) and 4K apart (`clone3`). A fixed mapping must start on a page boundary, so on a 64K-page kernel three network slots in four could not be mapped, and on a 16K-page one three `clone3` slots in four; the tracee's call returned `EINVAL`. journald's `READY=1` is such a send, so systemd in an instance never finished starting on Rocky's 64K-page kernel. | `test-systemd` on the 64K-page guest | The slots are spaced by the largest page size the architecture's kernels use: 4K on x86-64, 64K on aarch64. |
| `getcwd` was refused for a buffer above 4096 bytes. A C library asks with one page where a page is larger, so Bash complained at every prompt on 16K and 64K kernels. | `test-systemd` on the 64K-page guest | The bound is the larger of 4096 and the host's page size. |
| The virtual memory figures (`statm`, `status`, `meminfo`, `memory.current`) turned page counts into bytes with 4096. On a 16K-page or 64K-page kernel they were wrong by 4 or 16 times. | Reading the code after the 64K-page run | The architecture layer reads the page size from the auxiliary vector. |
| The register translation asked the kernel what kind of stop it was reading, at every stop: three `ptrace` requests where x86-64 makes two. | Counting the supervisor's requests after the `threads` benchmark ran further below native than on x86_64 | The daemon hands each wait status to the architecture layer, which reads the kind from it and asks the kernel only where the status does not settle it. |

Three more were in upstream's tests, not the daemon, and are recorded in
`docs/arm64/README.md`: the launch test's contract checks name `open`, which
an aarch64 fixture cannot issue; two seccomp filters in the tests name the
x86-64 architecture; and the interactive test looks for a `model name` line
in `/proc/cpuinfo`, which an arm64 kernel does not print.

## Shared faults fixed here

These are in shared code. The first two showed on x86_64 exactly as on
aarch64, and each stood between an upstream test and any result at all on
either architecture. The third is a race that only the faster of the two
machines lost, which happens to be the arm64 one.

| Fault | Found by | Fix |
|-------|----------|-----|
| `connect` with `AF_UNSPEC`, which dissolves a datagram socket's association, was refused with `EINVAL`. glibc's `getaddrinfo` uses it between source-address probes and then aborted on an internal assertion on a host with an IPv6 route. `dnf` died resolving its mirrors. | `test-systemd-live`, both architectures | The native network mediation lets a UDP socket dissolve its association, through the same snapshot of the address the other calls use. |
| `ginux start` reported `failed to start` for an instance whose init had already exited with status 0 when the command looked for it. | `make test-rpm` under KVM, both architectures (emulation was slow enough to hide it) | The start helper accepts an instance that has finished with status 0. |
| The emulated `setpgid` failed the whole instance when the console broker's PID map had no entry for the task. A task that has exited has left that map but is still in the process table. | `test-systemd-interactive` under KVM on the M1: 6 failures in 19 runs. 8 runs of 8 passed on the x86_64 guest. | The update is skipped for a task that has exited; 12 runs of 12 then passed. That the task involved is a dead child is read from the code and from that result, not captured. |

## Decisions that need review

**The three shared-code fixes above.** They change what the x86_64 daemon
and its helper do. They were made without the corrective-gate proposal this
process asks for when a defect is found outside a gate's scope. The human
may want them reported upstream, kept here, or taken out.

**The trap rule.** It is a choice, not a translation. Upstream fails an instance
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

- `dnf update` in an instance cannot unpack some packages, so
  `test-systemd-live` fails. The same five files fail with the same
  errors on both architectures. One cause is the contract's own: set-ID
  modes are admitted only for paths on its `setuid` and `setgid` lists,
  which upstream's recipes leave empty. Making the test pass means changing
  what the contract admits.
- The contract has no `tgkill`, so `abort()` cannot raise `SIGABRT`.
- On Linux 6.8 upstream's check that `waitid` releases reaped children
  fails, on both architectures (Ubuntu's 6.8 kernel in a minimal guest).
  It passes from 6.12 on.
- `waitid(P_PID, n)` selects the wrong task when a child's virtual pid
  equals the host pid of another managed task, and returns `ECHILD`.
  Reproduced on x86_64 in a pid namespace, where host pids are small:
  upstream's `null-wait` scenario fails for about one starting pid in
  three. It is why `test-native-supervision` is not in the 64K-page CI
  step.
- The catalog lacks x86-64's legacy `link` and `fchmodat` numbers. This one
  runs the other way: the calls work on aarch64 and get `ENOSYS` on x86_64.
- One spin-count timing assumption in `test-native-supervision` fails under
  x86 emulation.
- Lines from two writers can interleave on a shared pipe and defeat a
  whole-line assertion in a test.
- The pinned Rocky 10.2 x86_64 image URL is intermittently unavailable.

Whether to report any of them upstream, or to fix them in this tree, is the
human's decision.

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

"KVM" below is the Rocky 10.2 aarch64 guest (kernel 6.12, 4K pages) on an
Apple M1, and for x86_64 the Rocky 10.2 x86_64 guest on an AMD Threadripper
1950X.

| Criterion | Where it ran | Result |
|-----------|--------------|--------|
| Upstream's five integration tests pass against the aarch64 daemon | GitHub `ubuntu-24.04-arm` runner, natively (`aarch64` CI job) | `test-launch` 68 checks, `test-native-supervision` 57 across the eight scripts, `test-gate1-setup` 3, `test-tracee-kill` 5, `test-process-native` 48 |
| The same, on a second arm64 machine and page size | Apple M1, Fedora Asahi, kernel 6.14, 16K pages, natively | the same counts |
| The same, on a distribution kernel | `make test-arm64`, KVM (under two minutes) and emulated | the same counts; the main supervision script alone has 42 |
| The same, on 64K pages | Rocky `kernel-64k` 6.12 in the emulated aarch64 guest | the same counts, and the host suite |
| Upstream's Rocky host suite still passes | `make test-arm64`, KVM and emulated | `test-image` 8/8, `test-intercept`, `test-runtime`, `test-config`, `test-instance`, `test-gate12-cli`, `test-devices`, `test-process`, `test-kernel-views` |
| Upstream's instance tests pass | `make test-instances-arm64`: KVM at upstream's time limits (under four minutes), and emulated with the login driver's limits stretched | `test-vfs`, `test-native-vfs`, `test-contract-runtime`, `test-gate125-lock`, `test-gate975-hooks`, `test-gate975-network`, `test-systemd`, `test-multi-instance` |
| The suites and the instance tests on 16K pages | Rocky aarch64 guest on the M1's own 16K-page kernel (6.14 asahi), under KVM | the host suite, the twelve integration scripts with the same counts, and the eight instance tests |
| The page-size faults cannot come back unnoticed | `test-page-size-arm64` natively on the 16K-page M1 and in the 64K-page guest, against the daemon from before the fixes and the current one; a build of aarch64 with a 16K snapshot spacing | the old daemon fails three of the four checks on 16K pages and all four on 64K; the current one passes on both, and in the 16K-page guest; the build with a 16K spacing stops at the assertion. The aarch64 CI job runs the test on Ubuntu's 64K-page kernel (`make page-size-aarch64-guest`); there too the old daemon fails all four, and the target refuses a 4K-page kernel |
| The instance tests on 64K pages | the emulated 64K-page guest | `test-vfs`, `test-native-vfs`, `test-contract-runtime`, `test-gate125-lock`, `test-gate975-hooks`, `test-gate975-network`, `test-systemd`, `test-multi-instance`; the last three only after the snapshot spacing and `getcwd` fixes |
| The package builds and passes upstream's lifecycle test | `make test-rpm-arm64`, KVM and emulated; x86_64 `make test-rpm` under KVM | `Rocky Linux RPM build: PASS`, rpmlint 0 errors, `test-rpm-metadata: PASS`, `Rocky Linux RPM lifecycle: PASS` on both |
| Upstream's remaining Rocky-host tests pass | KVM: `test-allocator-rocky-arm64`, `test-gate19-native-examples-arm64`, `test-systemd-interactive-arm64`; x86_64 `test-systemd-interactive` under KVM | pass; the interactive test, which drives `run-host`, passed 12 runs of 12 on aarch64 after the process-group fix and 8 of 8 on x86_64 |
| Upstream's benchmarks run | KVM: `bench-systemd-arm64`, `bench-block-arm64`, `bench-network-arm64`, `bench-baseline-arm64`, `bench-modes-arm64`; the x86_64 targets under KVM | all complete on both; figures in `docs/arm64/README.md` |
| A supervised call costs no more requests than on x86_64 | `strace` of the supervisor over four native-supervision scenarios, 104 stops on either | 258 `ptrace` requests on x86_64, 269 on aarch64 (357 before); the 11 are writes of a changed call number |
| A supervised call costs no more time | `getppid` 200,000 times under `ginux native`, one CPU | 32 µs on the Threadripper, 12.6 µs on an M1 performance core |
| The `threads` benchmark's lower ratio on aarch64 is not the daemon's | sysbench `threads` under selective dispatch, and a program that times `sched_yield` with and without a bare seccomp filter, in the same guests | the instance adds 25 ns to a yield on both; the bare filter adds 24 ns on x86_64 and 26 ns on aarch64; the native yield is 295 ns and 252 ns |
| A process that aborts dies as it does outside an instance | `abort()` under a seccomp filter that fails `tgkill`, natively and in the Rocky guests | signal 11 on x86_64, signal 5 on aarch64 |
| The virtual memory figures use the host's page size | a process's `statm` natively and in an instance, on the 16K-page M1 | 17 pages natively; 68 in an instance before the change, 17 after |
| `test-systemd-live` behaves the same on both | KVM, both architectures | name resolution, HTTPS, and bind pass; `dnf update` runs and fails to unpack the same five files with the same errors (`/lib64/libgcc_s-14-20260724.so.1`, `/usr/bin/mount`, `/usr/sbin/pam_timestamp_check`, `/usr/bin/chage`, `/usr/bin/chfn`); the test fails on both |
| The workspace's unit tests and the allocator pass on an arm64 host | the CI runner | `make test-unit`, `test-allocator`, `test-allocator-linux` |
| The x86_64 fixtures are the same programs | compile upstream's files and these with the Makefile's flags, `cmp` | 38 binaries identical |
| The x86_64 daemon is unchanged in result | this x86_64 machine, and the `launch-integration` CI job | `make check`; `test-unit` 174 passed; `test-launch` 69, `test-native-supervision` 57, `test-gate1-setup` 3, `test-tracee-kill` 5, `test-process-native` 48 |
| Shared-code changes hold on upstream's reference host | Rocky 10.2 x86_64 guest | the host suite; `test-systemd` (also on the 10.3 kernel); `test-contract-runtime`; eleven of the twelve integration scripts (see limitations) |

## Limitations

- The arm64 hardware is one Apple M1 and the CI runner. The 64K-page
  results are from an emulated guest.
- In CI the page-size test and two of upstream's integration scripts are
  run on 64K pages. The Rocky host suite and the instance
  tests on 16K and 64K pages are runs made by hand; they need a Rocky host,
  which CI boots for neither architecture.
- The two machines compared are unlike. The figures show no aarch64 cost
  above x86_64's; they do not rank the architectures.
- `test-systemd-live` does not pass on either architecture; see the
  defects above.
- The Ubuntu steps of the package lifecycle test and of the native examples
  do not run on aarch64, because that image has no arm64 build.
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
- The register cache and the stop kind read from the wait status were
  verified by these suites, not by a test of their own.

## Review

Open. Not accepted by a human reviewer.
