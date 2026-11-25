# Review of the original branches, and the Rocky aarch64 host suite

> Fork-track record. Not an upstream gate. See `hdae/arm64/README.md` for
> its standing.

## Source

The human asked on 2026-11-25: "review the cursor branches and see if they
can be rebased on the new code -- then integrate and test". The scope below
is the agent's reading of that request and has not been reviewed.

## Goal

Every one of the fifteen `cursor/*-2b05` branches is accounted for on
`arm64/main`: carried, superseded, or deliberately left out, with the reason
written down. What was missing is integrated and tested on current upstream.

## Scope

- A review of each branch against `arm64/main`, file by file.
- A merge of upstream `main` (`85d53265`, pull request #334).
- The one branch the earlier carry-over missed,
  `cursor/arm64-rocky-guest-2b05`: upstream's host suite on a Rocky Linux 10
  aarch64 guest.

## Non-goals

- Any change to the fifteen branches or their fifteen open pull requests on
  the fork. They are untouched.
- The Rocky-host targets outside `make test`: systemd, OCI, VFS, benchmarks,
  the RPM.
- The seven daemon-tracing scripts named as the next gate in A6.

## Delivery

Branch `arm64/main` on the fork. No pull request. Upstream is merged, not
rebased onto, so the published branch is not rewritten. Commits `63019713`
(the merge) and `9fcc071b` (the host suite), then this record.

## Review of the branches

Method: every file the two branch tips (`baremetal-arm64` and
`arm64-rocky-guest`) change against the fork point (`a9246c8d`,
upstream pull request #228) was compared with the same file on `arm64/main`,
at its original path or under `baremetal/`. They touch 194 files.

- 136 are byte-identical on `arm64/main`: 129 moved under `baremetal/`, 7 in
  place.
- 8 under `baremetal/` differ by the changes the rebase record lists (the
  `ginux-kernel-oci` package name, the generated-file header path, the
  firmware memory-range merge, and three script portability fixes).
- 37 in the main tree differ because upstream rewrote the code underneath
  them. The daemon's architecture layer, the fixtures, and the guest scripts
  were rewritten for upstream's current supervisor; the README text moved to
  `docs/arm64/README.md`; the ledger edits were removed on purpose.
- 13 are absent. 5 are the gate files, now `gates/A1` to `A5`. The other 8
  belong to the Rocky guest branch.

Eight commits sit on a lower branch only and are in neither tip. Each is
the same two-file formatting and cast fix, which `arm64/main` has.

No commit of the two tips applies to current upstream or to `arm64/main` with
`git cherry-pick`: each one conflicts or is already present. A mechanical
rebase of the branches is not possible; upstream moved 500 commits and
renamed the isolation unit throughout. `arm64/main` is the rebase, done by
restructuring.

| Branch | Pull request | On `arm64/main` |
|--------|--------------|-----------------|
| `arm64-userspace-daemon` | 1 | Carried: `crates/ginux-init/src/arch/`, `ginux arch` |
| `arm64-domain-mediation` | 2 | Carried, rewritten for the seccomp-trace supervisor |
| `arm64-legacy-kernel` | 3 | Carried under `baremetal/legacy-kernel` |
| `arm64-gate1-payload` | 4 | Carried: `make gate1-aarch64-guest` |
| `arm64-native-guest` | 6 | Carried: `make native-aarch64-guest` |
| `arm64-runtime-elf` | 7 | Carried: `crates/ginux-runtime` is identical |
| `arm64-ci-linux` | 8 | Carried: the `aarch64` CI job |
| `x86-gate1-guest` | 9 | Carried: `make gate1-x86_64-guest` |
| `arm64-native-brk` | 10 | Superseded. Upstream requires the instance to fail at a tracee's own trap, so A6 removed the step-over |
| `arm64-virt-kernel` | 11 | Carried under `baremetal/` |
| `arm64-virt-process` | 12 | Carried under `baremetal/` |
| `arm64-uefi-kernel` | 13 | Carried under `baremetal/` |
| `arm64-linux-svc` | 14 | Carried under `baremetal/` |
| `arm64-rocky-guest` | 15 | Was missing. Integrated by this gate |
| `baremetal-arm64` | 16 | Carried under `baremetal/` |

## Completed work

- [x] Upstream `85d53265` merged. No conflicts; upstream changed no crate.
- [x] `config/rocky-host-image-aarch64.env` and `.CHECKSUM`: the Rocky 10.2
  aarch64 image the branch pinned, same size and SHA-256, at its vault URL.
  Both URLs the branch recorded now return 404.
- [x] `scripts/rocky-image-env.sh` selects the image by `ROCKY_ARCH`.
  `fetch`, `verify`, `prepare`, and `start` use it.
- [x] `scripts/start-rocky-host.sh` starts `qemu-system-aarch64` for an
  aarch64 host. `rocky-host-common.sh`, `with-rocky-host.sh`, and
  `stop-rocky-host.sh` keep the two architectures' files apart.
- [x] The eight scripts of the `make test` host set take the daemon,
  payload, process fixture, and CLI from `$GINUX_RELEASE` and
  `$GINUX_CLI_RELEASE`.
- [x] The image scripts take the root partition number and the expected
  loader and machine from the architecture.
- [x] `crates/process-native-fixture` builds for aarch64. This is the
  branch's change to that file, applied unmodified.
- [x] `make build-arm64-host-suite`, `host-image-fetch-arm64`, `test-arm64`,
  and `scripts/test-host-suite.sh`. `check-arm64` lints the fixture.
- [x] `docs/arm64/README.md` describes the suite.

## Decisions that differ from the branch

- **`make test` is unchanged.** The branch made `make test` boot the aarch64
  guest. Here it stays upstream's x86_64 run and the aarch64 run is
  `make test-arm64`, as every other aarch64 target on this branch is
  separate from upstream's.
- **No virtiofs.** The branch shared the tree with `virtiofsd` and replaced
  every `rocky_scp` in the test scripts with a copy from the share. The
  scripts here keep upstream's SSH copy and change only the source path, so
  the host needs no `virtiofsd` and each script differs from upstream's by
  one or two lines.
- **Docker only for extraction.** The branch added an `unshare` fallback for
  hosts without Docker. Upstream requires its pinned helper image; so does
  this.
- **One boot.** `make test-arm64` boots the host once for all scripts.
  Upstream's `make test` boots it once per target.

## Acceptance criteria and evidence

Run on an x86_64 host with QEMU 10.2 (TCG), on the merged tree.

| Criterion | Command | Result |
|-----------|---------|--------|
| Upstream's host suite passes on Rocky 10.2 aarch64 (kernel 6.12.0-211.16.1.el10_2) | `make test-arm64` | `test-image: PASS (8/8 checks)`; `test-intercept`, `test-runtime`, `test-config`, `test-instance`, `test-gate12-cli`, `test-devices`, `test-process`, `test-kernel-views`: PASS |
| The same scripts still pass on upstream's x86_64 host | `./scripts/with-rocky-host.sh ./scripts/test-host-suite.sh`, `make test-image`, `make test-host-lifecycle` | all PASS |
| Static checks | `make check`, `make check-arm64` | pass |
| Unit tests | `make test-unit` | 174 passed, 0 failed |
| The earlier aarch64 guest tests still pass after the merge | `make arch-smoke arch-smoke-arm64 gate1-aarch64-guest native-aarch64-guest native-suite-aarch64-guest` | pass; 42 `PASS native` lines |
| The x86_64 integration suite is unchanged in result | `make test-launch test-native-supervision test-gate1-setup test-tracee-kill` as root, `test-process-native` | all exit 0; `test-process-native` 49 PASS lines as before |

## Limitations

- The aarch64 Rocky host runs under TCG on an x86_64 machine. It was not run
  with KVM on an arm64 machine, and it is not in CI: a boot and the suite
  take about seven minutes emulated.
- `ROCKY_NETWORK_RESTRICT=off` on aarch64 picks the guest's first network
  device by name. No test in this gate uses it.
- `test-runtime` and `test-instance` interpret the x86-64 runtime fixture on
  the aarch64 host, as the branch did. They do not exercise an aarch64
  managed ELF; `gate1-aarch64-guest` and `test-intercept` do.
- Upstream's pinned x86_64 image URL (`download.rockylinux.org/pub/rocky/10.2`)
  failed mid-transfer and returned 404 on some attempts during this work;
  Rocky 10.3 is current and mirrors are dropping 10.2. The vault copy has
  the pinned checksum. Upstream's file is not changed here.

## Review

Open. Not accepted by a human reviewer.
