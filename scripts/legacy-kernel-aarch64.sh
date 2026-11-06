#!/bin/sh
# Build the aarch64 legacy-kernel entry and boot it on qemu-system-aarch64.
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$root"

cargo build --manifest-path legacy-kernel/Cargo.toml \
    --target aarch64-unknown-none --release

kernel=$root/legacy-kernel/target/aarch64-unknown-none/release/ginux-legacy-kernel
log=$(mktemp)
trap 'rm -f "$log"' EXIT

# qemu stays alive if semihosting exit is ignored. Bound the run.
set +e
timeout 20 qemu-system-aarch64 \
    -machine virt \
    -cpu cortex-a57 \
    -m 128M \
    -nographic \
    -display none \
    -semihosting-config enable=on,target=native \
    -kernel "$kernel" >"$log" 2>&1
status=$?
set -e

cat "$log"
if ! grep -q "ginux-legacy: aarch64 virt boot ok" "$log"; then
    echo "legacy aarch64 kernel did not print its boot banner" >&2
    exit 1
fi
if [ "$status" -ne 0 ] && [ "$status" -ne 124 ]; then
    echo "qemu-system-aarch64 exited $status" >&2
    exit "$status"
fi
