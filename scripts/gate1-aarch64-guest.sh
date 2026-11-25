#!/bin/sh
# Run the aarch64 Ginux daemon's gate 1 path under qemu-system-aarch64.
# The daemon and payload are shared over virtio-9p; the initrd only mounts it.
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
payload=target/aarch64-unknown-linux-gnu/release/managed-payload
for built in "$ginux" "$payload"; do
    if [ ! -f "$built" ]; then
        echo "missing $built; run make build-arm64" >&2
        exit 2
    fi
done
mkdir -p "$root/.artifacts"

rustc --edition 2021 --target aarch64-unknown-linux-gnu \
    -C linker=rust-lld \
    -C link-arg=-nostdlib \
    -C link-arg=-static \
    -C link-arg=-no-pie \
    -C relocation-model=static \
    -C panic=abort \
    -C opt-level=2 \
    -o "$root/.artifacts/aarch64-gate1-init" \
    tests/fixtures/aarch64-gate1-init.rs

share=$(mktemp -d)
initrd_dir=$(mktemp -d)
log=$(mktemp)
trap 'rm -rf "$share" "$initrd_dir" "$log"' EXIT

cp "$ginux" "$share/ginux"
cp "$payload" "$share/ginux-managed-payload"
# The payload runs as the unprivileged host identity and must reach its file.
chmod 755 "$share" "$share/ginux" "$share/ginux-managed-payload" "$root/.artifacts/aarch64-gate1-init"

mkdir -p "$initrd_dir"
cp "$root/.artifacts/aarch64-gate1-init" "$initrd_dir/init"
chmod 755 "$initrd_dir/init"
(cd "$initrd_dir" && echo init | cpio -o -H newc > "$root/.artifacts/aarch64-gate1-initrd.cpio")

set +e
timeout 30 qemu-system-aarch64 \
    -machine virt \
    -cpu cortex-a57 \
    -smp 1 \
    -m 1024 \
    -nographic \
    -display none \
    -kernel "$kernel" \
    -initrd "$root/.artifacts/aarch64-gate1-initrd.cpio" \
    -append "console=ttyAMA0 earlycon loglevel=4" \
    -fsdev "local,security_model=none,writeout=immediate,id=fsdev0,path=$share" \
    -device virtio-9p-pci,id=fs0,fsdev=fsdev0,mount_tag=hostshare \
    -no-reboot >"$log" 2>&1
status=$?
set -e

cat "$log"
if grep -q "UNMEDIATED_PAYLOAD_WRITE" "$log"; then
    echo "gate 1 payload write reached the host" >&2
    exit 1
fi
for expected in \
    "GINUX: interception platform=ptrace-sysemu" \
    "GINUX: syscall=getpid decision=emulate result=4242" \
    "GINUX: syscall=write decision=mediate host-write=not-executed" \
    "GINUX: syscall=uname decision=deny errno=EPERM" \
    "GINUX: direct-host-syscall bypass=not-observed" \
    "GATE1: PASS"; do
    if ! grep -qF "$expected" "$log"; then
        echo "aarch64 gate 1 did not report: $expected" >&2
        exit 1
    fi
done
if grep -q "UNMEDIATED_PAYLOAD_WRITE" "$log"; then
    echo "aarch64 gate 1 let the payload write reach the host" >&2
    exit 1
fi
if ! grep -q "ptrace-init: wait=" "$log" || ! grep -q "status=0" "$log"; then
    echo "aarch64 gate 1 did not exit 0" >&2
    exit 1
fi
if [ "$status" -ne 0 ]; then
    echo "qemu-system-aarch64 exited $status" >&2
    exit "$status"
fi
