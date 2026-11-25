# Upstream's native-supervision suite on aarch64

> Fork-track record. Not an upstream gate. See `hdae/arm64/README.md` for
> its standing.

## Source

The next gate named in `hdae/arm64/README.md` after the rebase onto upstream
`f4b85186`. The human asked on 2026-11-25 to continue implementation on
`arm64/main`. The scope below was chosen by the agent from that note and has
not been reviewed.

## Goal

`scripts/test-native-supervision.py`, as upstream ships it, passes against
the aarch64 daemon. It replaces the five-scenario guest test as the main
evidence that `ginux native` mediates an aarch64 workload.

## Scope

- An aarch64 build of `tests/fixtures/native-fixture.c` from the same source.
- A way to run the script on an arm64 host and in an arm64 guest.
- Fixes to the aarch64 architecture layer that the script exposes.
- A CI step on the arm64 runner.

## Non-goals

- The seven scripts that trace the daemon and decode its registers as x86-64
  `user_regs_struct`.
- `test-launch`, `test-process-native`, systemd, OCI, the allocator.
- Any change to what the x86_64 fixture or the test script does.

## Delivery

Branch `arm64/main` on the fork, pushed. No pull request.

## Completed work

- [x] `tests/fixtures/native-fixture-aarch64.h`: the calls, thread helpers,
  and entry point for aarch64. `native-fixture.c` selects it with
  `__aarch64__`; its x86-64 text is unchanged.
- [x] `scripts/aarch64-syscall-header.sh`: the x86-64 to aarch64 number table
  for the fixture, generated from `crates/ginux-core/src/syscall_map.rs`.
- [x] `make build-launch-fixture-arm64`, `test-native-supervision-arm64`,
  `native-suite-aarch64-guest`, and `scripts/native-suite-aarch64-guest.sh`.
- [x] CI: the arm64 job builds the fixture and runs the script as root. The
  workflow also runs on pushes to `arm64/main`.
- [x] Architecture layer: a restarted call gets the first argument the
  supervisor left at its exit stop.
- [x] Architecture layer: a tracee's own `brk #0` is no longer stepped over.
  The instance fails closed, as upstream's `trap` check requires.

## Defect found outside the scope

`close_range` with a flag the native supervisor does not model went to the
host unchanged. Linux 7.4-rc1 has `CLOSE_RANGE_EXCEPT` (8), which acts on
every descriptor outside the range, and `CLOSE_RANGE_CLOEXEC_ONLY` (16);
Linux 7.0 rejects both. On a kernel that has them `close_range(5, 5, 8)` from a workload closed its standard output and
the supervisor's view of the table went stale; the same call can close the
protected descriptors 240 and 241. This is in the shared supervisor, not in
the port, and applies to x86_64 on the same kernels.

The supervisor now answers `EINVAL` for any flag other than
`CLOSE_RANGE_UNSHARE` and `CLOSE_RANGE_CLOEXEC`, without a host call. On
older kernels the result is the one the kernel gave. The fix is its own
commit. Whether to report it upstream is the human's decision.

## Acceptance criteria and evidence

Run on an x86_64 host, QEMU 10.2, guest kernel v9fs `kernel-v7.4-rc1`,
Alpine arm64 userland with Python 3.13.

| Criterion | Command | Result |
|-----------|---------|--------|
| The script passes on aarch64 | `make native-suite-aarch64-guest` | 42 `PASS native` lines, `test-native-supervision: PASS` |
| The register-level guest test still passes | `make native-aarch64-guest` | `aarch64 native supervision: PASS` |
| The x86_64 fixture is the same program | compile upstream's file and this one with the Makefile's flags, `cmp` | identical |
| x86_64 results are unchanged | `sudo make test-launch test-native-supervision test-gate1-setup test-tracee-kill` | 69, 57, 3, 5 `PASS` lines; the same counts as before |
| Checks | `make check`, `make check-arm64`, `make test-unit` | pass; 174 unit tests |

Order of failures on the way, each fixed before the next appeared:

1. `translated pid selectors are restored`: the restarted `wait4` carried the
   host pid. Fixed in the architecture layer.
2. `socket aliases retain network authority`: `close_range(5, 5, 8)` closed
   standard output on the 7.4 guest kernel. Fixed in the supervisor.

## Limitations

- The guest kernel is 7.4-rc1, where a tracer's write to `x0` at an entry
  stop also sets the restart value. The older behaviour, where the restart
  uses the caller's own `x0`, is handled by the same code and is what the
  CI runner's kernel exercises; that job has not run yet at the time of
  writing.
- `kill` and `pidfd_open` selector preservation cannot be observed on
  aarch64, and `int80` is replaced by a trap. `docs/arm64/README.md` has the
  detail.
- Not run on arm64 hardware.

## Review

Open. Not accepted by a human reviewer.
