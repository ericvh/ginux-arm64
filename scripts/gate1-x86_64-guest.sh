#!/bin/sh
# Run the x86_64 Ginux daemon's gate 1 path under qemu-system-x86_64.
# The daemon and payload are shared over virtio-9p; the initrd only mounts it.
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$root"

kernel=${GINUX_X86_64_KERNEL:-}
if [ -z "$kernel" ]; then
    echo "Set GINUX_X86_64_KERNEL to an x86_64 Linux bzImage." >&2
    exit 2
fi
if [ ! -f "$kernel" ]; then
    echo "kernel image not found: $kernel" >&2
    exit 2
fi

cargo build -p ginux-init --target x86_64-unknown-linux-gnu --release
cargo build -p managed-payload --target x86_64-unknown-linux-gnu --release

ginux=target/x86_64-unknown-linux-gnu/release/ginux
payload=target/x86_64-unknown-linux-gnu/release/managed-payload
mkdir -p "$root/.artifacts"

rustc --edition 2021 --target x86_64-unknown-linux-gnu \
    -C linker=rust-lld \
    -C link-arg=-nostdlib \
    -C link-arg=-static \
    -C link-arg=-no-pie \
    -C relocation-model=static \
    -C panic=abort \
    -C opt-level=2 \
    -o "$root/.artifacts/x86_64-gate1-init" \
    tests/fixtures/x86_64-gate1-init.rs

share=$(mktemp -d)
initrd_dir=$(mktemp -d)
log=$(mktemp)
trap 'rm -rf "$share" "$initrd_dir" "$log"' EXIT

cp "$ginux" "$share/ginux"
cp "$payload" "$share/ginux-managed-payload"
chmod 755 "$share/ginux" "$share/ginux-managed-payload" "$root/.artifacts/x86_64-gate1-init"

mkdir -p "$initrd_dir"
cp "$root/.artifacts/x86_64-gate1-init" "$initrd_dir/init"
chmod 755 "$initrd_dir/init"
(cd "$initrd_dir" && echo init | cpio -o -H newc > "$root/.artifacts/x86_64-gate1-initrd.cpio")

set +e
timeout 90 qemu-system-x86_64 \
    -machine q35,accel=tcg \
    -cpu qemu64 \
    -smp 1 \
    -m 1024 \
    -nographic \
    -display none \
    -serial mon:stdio \
    -kernel "$kernel" \
    -initrd "$root/.artifacts/x86_64-gate1-initrd.cpio" \
    -append "console=ttyS0,115200 earlyprintk=serial,ttyS0,115200 loglevel=4 rdinit=/init" \
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
if ! grep -q "GINUX: syscall=getpid decision=emulate result=4242" "$log"; then
    echo "x86_64 gate 1 did not emulate getpid" >&2
    exit 1
fi
if ! grep -q "GINUX: syscall=write decision=mediate host-write=not-executed" "$log"; then
    echo "x86_64 gate 1 executed the payload write" >&2
    exit 1
fi
if ! grep -q "GINUX: syscall=uname decision=deny errno=EPERM" "$log"; then
    echo "x86_64 gate 1 did not deny uname" >&2
    exit 1
fi
if ! grep -q "GINUX: direct-host-syscall bypass=not-observed" "$log"; then
    echo "x86_64 gate 1 observed a direct host syscall" >&2
    exit 1
fi
if ! grep -q "GATE1: PASS" "$log"; then
    echo "x86_64 gate 1 did not pass" >&2
    exit 1
fi
if ! grep -q "ptrace-init: wait=" "$log" || ! grep -q "status=0" "$log"; then
    echo "x86_64 gate 1 did not exit 0" >&2
    exit 1
fi
if [ "$status" -ne 0 ]; then
    echo "qemu-system-x86_64 exited $status" >&2
    exit "$status"
fi
