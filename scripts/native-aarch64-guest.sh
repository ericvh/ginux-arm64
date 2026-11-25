#!/bin/sh
# Run ginux native supervision under qemu-system-aarch64.
# The guest mounts the daemon, config, and fixture over virtio-9p.
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$root"

kernel=${GINUX_AARCH64_KERNEL:-}
if [ -z "$kernel" ]; then
    echo "Set GINUX_AARCH64_KERNEL to an arm64 Linux Image (for example a v9fs/test kernel-v* Image)." >&2
    exit 2
fi
if [ ! -f "$kernel" ]; then
    echo "kernel image not found: $kernel" >&2
    exit 2
fi

ginux=target/aarch64-unknown-linux-gnu/release/ginux
if [ ! -f "$ginux" ]; then
    echo "missing $ginux; run make build-arm64" >&2
    exit 2
fi

mkdir -p "$root/.artifacts"
rustc --edition 2021 --target aarch64-unknown-linux-gnu \
    -C linker=rust-lld \
    -C link-arg=-nostdlib \
    -C link-arg=-static \
    -C link-arg=-no-pie \
    -C relocation-model=static \
    -C panic=abort \
    -C opt-level=2 \
    -o "$root/.artifacts/aarch64-native-fixture" \
    tests/fixtures/native-fixture-aarch64.rs
rustc --edition 2021 --target aarch64-unknown-linux-gnu \
    --cfg native_guest \
    -C linker=rust-lld \
    -C link-arg=-nostdlib \
    -C link-arg=-static \
    -C link-arg=-no-pie \
    -C relocation-model=static \
    -C panic=abort \
    -C opt-level=2 \
    -o "$root/.artifacts/aarch64-native-init" \
    tests/fixtures/aarch64-gate1-init.rs
rustc --edition 2021 --target aarch64-unknown-linux-gnu \
    --cfg native_guest \
    --cfg native_configured \
    -C linker=rust-lld \
    -C link-arg=-nostdlib \
    -C link-arg=-static \
    -C link-arg=-no-pie \
    -C relocation-model=static \
    -C panic=abort \
    -C opt-level=2 \
    -o "$root/.artifacts/aarch64-native-configured-init" \
    tests/fixtures/aarch64-gate1-init.rs

fixture=$root/.artifacts/aarch64-native-fixture

# run_scenario <scenario> [configured]
# `configured` runs the fixture as the instance's configured init, under a
# contract that grants it the filesystem and the virtual kernel views.
run_scenario() {
    scenario=$1
    mode=${2:-}
    init=$root/.artifacts/aarch64-native-init
    share=$(mktemp -d)
    initrd_dir=$(mktemp -d)
    log=$(mktemp)
    mkdir -p "$share/root/usr/bin"
    cp "$ginux" "$share/ginux"
    cp "$fixture" "$share/root/usr/bin/bash"
    if [ "$mode" = configured ]; then
        init=$root/.artifacts/aarch64-native-configured-init
        sed 's|init = "/bin/bash"|init = "/usr/bin/bash"|' config/instance-gate5.toml > "$share/native.toml"
        cat >> "$share/native.toml" <<'CONTRACT'

[contract.filesystem]
read = ["/"]
write = ["/"]
create = ["/"]
[contract.kernel]
proc_read = ["/"]
sys_read = ["/"]
CONTRACT
    else
        cp config/instance-gate5.toml "$share/native.toml"
    fi
    printf '%s\n' "$scenario" > "$share/root/scenario"
    chmod 755 "$share/ginux" "$share/root/usr/bin/bash" "$init"
    chmod 644 "$share/root/scenario" "$share/native.toml"
    mkdir -p "$initrd_dir"
    cp "$init" "$initrd_dir/init"
    chmod 755 "$initrd_dir/init"
    (cd "$initrd_dir" && echo init | cpio -o -H newc > "$root/.artifacts/aarch64-native-initrd.cpio")
    set +e
    timeout 30 qemu-system-aarch64 \
        -machine virt \
        -cpu cortex-a57 \
        -smp 1 \
        -m 1024 \
        -nographic \
        -display none \
        -kernel "$kernel" \
        -initrd "$root/.artifacts/aarch64-native-initrd.cpio" \
        -append "console=ttyAMA0 earlycon loglevel=4" \
        -fsdev "local,security_model=none,writeout=immediate,id=fsdev0,path=$share" \
        -device virtio-9p-pci,id=fs0,fsdev=fsdev0,mount_tag=hostshare \
        -no-reboot >"$log" 2>&1
    status=$?
    set -e
    cat "$log" >&2
    rm -rf "$share" "$initrd_dir"
    if [ "$status" -ne 0 ]; then
        echo "qemu-system-aarch64 exited $status for scenario $scenario" >&2
        exit "$status"
    fi
    printf '%s' "$log"
}

fork_log=$(run_scenario fork)
if ! grep -q "GINUX_FIXTURE: clone=-38" "$fork_log"; then
    echo "aarch64 native clone was not denied with -38" >&2
    exit 1
fi
if ! grep -q "syscall=56 action=deny result=-38" "$fork_log"; then
    echo "aarch64 native supervisor did not deny clone" >&2
    exit 1
fi
if ! grep -q "dispatch=paranoid direct-safe-syscalls=0" "$fork_log"; then
    echo "aarch64 native let a syscall run without supervision" >&2
    exit 1
fi
if ! grep -q "GINUX_NATIVE_CHECK: PASS" "$fork_log"; then
    echo "aarch64 native supervision check did not pass" >&2
    exit 1
fi
if grep -q "unsupervised-child" "$fork_log"; then
    echo "aarch64 native clone left an unsupervised child" >&2
    exit 1
fi
if ! grep -q "ptrace-init: wait=" "$fork_log" || ! grep -q "status=0" "$fork_log"; then
    echo "aarch64 native clone scenario did not exit 0" >&2
    exit 1
fi
rm -f "$fork_log"

trap_log=$(run_scenario trap)
if ! grep -q "GINUX_FIXTURE: uname-before=-38" "$trap_log"; then
    echo "aarch64 native trap did not deny uname before the breakpoint" >&2
    exit 1
fi
if ! grep -q "GINUX_FIXTURE: uname-after=-38" "$trap_log"; then
    echo "aarch64 native trap did not deny uname after the breakpoint" >&2
    exit 1
fi
if grep -q "unexpected native trace stop" "$trap_log"; then
    echo "aarch64 native trap stopped on brk before the second uname" >&2
    exit 1
fi
if ! grep -q "status=0" "$trap_log"; then
    echo "aarch64 native trap scenario did not exit 0" >&2
    exit 1
fi
rm -f "$trap_log"

tid_log=$(run_scenario virtual-tid)
if ! grep -q "GINUX_FIXTURE: set_tid_address=4242 getpid=4242 gettid=-38 futex_lock_pi=-1 pi_word=0" "$tid_log"; then
    echo "aarch64 native virtual tid scenario did not match" >&2
    exit 1
fi
if ! grep -q "status=0" "$tid_log"; then
    echo "aarch64 native virtual tid scenario did not exit 0" >&2
    exit 1
fi
rm -f "$tid_log"

# The supervisor answers clone3 with mmap, clone3, and munmap replayed at the
# caller's own svc, in the parent and again in the child. The caller must get
# its arguments and its x8 back, and the child must exit 7 (status 1792).
clone3_log=$(run_scenario clone3 configured)
if ! grep -q "GINUX_FIXTURE: clone3 cgroup=1 child=1 waited=1 status=1792 x1=88 x8=435 flags-kept=1" "$clone3_log"; then
    echo "aarch64 native clone3 replay did not preserve the caller" >&2
    exit 1
fi
if ! grep -q "status=0" "$clone3_log"; then
    echo "aarch64 native clone3 scenario did not exit 0" >&2
    exit 1
fi
rm -f "$clone3_log"

# A close_range across the protected descriptors runs as two replayed parts.
# Descriptor 250 must be open before (0), closed after (-9), and the caller's
# second argument and x8 unchanged.
range_log=$(run_scenario close-range configured)
if ! grep -q "GINUX_FIXTURE: close-range dup=250 before=0 result=0 after=-9 last-kept=1 x8=436" "$range_log"; then
    echo "aarch64 native close_range replay did not match" >&2
    exit 1
fi
if ! grep -q "status=0" "$range_log"; then
    echo "aarch64 native close_range scenario did not exit 0" >&2
    exit 1
fi
rm -f "$range_log"
echo "aarch64 native supervision: PASS"
