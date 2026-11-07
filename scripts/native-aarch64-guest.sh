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

cargo build -p ginux-init --target aarch64-unknown-linux-gnu --release

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

ginux=target/aarch64-unknown-linux-gnu/release/ginux
fixture=$root/.artifacts/aarch64-native-fixture

run_scenario() {
    scenario=$1
    share=$(mktemp -d)
    initrd_dir=$(mktemp -d)
    log=$(mktemp)
    mkdir -p "$share/root/usr/bin"
    cp "$ginux" "$share/ginux"
    cp "$fixture" "$share/root/usr/bin/bash"
    cp config/domain-gate5.toml "$share/native.toml"
    printf '%s\n' "$scenario" > "$share/root/scenario"
    chmod 755 "$share/ginux" "$share/root/usr/bin/bash" "$root/.artifacts/aarch64-native-init"
    chmod 644 "$share/root/scenario" "$share/native.toml"
    mkdir -p "$initrd_dir"
    cp "$root/.artifacts/aarch64-native-init" "$initrd_dir/init"
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
if ! grep -q "direct-unsupervised-syscalls=0" "$fork_log"; then
    echo "aarch64 native reported an unsupervised syscall" >&2
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
echo "aarch64 native supervision: PASS"
