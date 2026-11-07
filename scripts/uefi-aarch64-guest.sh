#!/bin/sh
# Boot the aarch64 UEFI image with Arm UEFI firmware on a SystemReady-style SBSA virt machine.
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
efi="$root/target/aarch64-unknown-uefi/release/ginux-boot.efi"
code=/usr/share/AAVMF/AAVMF_CODE.fd
vars_src=/usr/share/AAVMF/AAVMF_VARS.fd
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

if [ ! -f "$efi" ]; then
    echo "missing $efi; build ginux-boot for aarch64-unknown-uefi first" >&2
    exit 1
fi

mkdir -p "$work/esp/EFI/BOOT"
cp "$efi" "$work/esp/EFI/BOOT/BOOTAA64.EFI"
cp "$vars_src" "$work/vars.fd"

log="$work/boot.log"
set +e
dd if=/dev/zero of="$work/disk.img" bs=1M count=1 status=none
timeout 90 qemu-system-aarch64 \
    -machine virt,gic-version=3,iommu=smmuv3,acpi=on \
    -cpu cortex-a57 \
    -smp 2 \
    -m 512 \
    -nographic \
    -display none \
    -serial mon:stdio \
    -drive if=pflash,format=raw,readonly=on,file="$code" \
    -drive if=pflash,format=raw,file="$work/vars.fd" \
    -device virtio-blk-device,drive=esp,bootindex=0 \
    -drive if=none,id=esp,file=fat:rw:"$work/esp",format=raw \
    -device virtio-blk-pci,drive=disk \
    -drive if=none,id=disk,file="$work/disk.img",format=raw \
    -net none \
    -no-reboot >"$log" 2>&1
qemu_status=$?
set -e

cat "$log"

fail=0
for line in \
    "ginux: arm uefi build" \
    "ginux: kernel entered after ExitBootServices" \
    "ginux: arm uefi sbsa handoff" \
    "ginux: acpi xsdt tables:" \
    "ginux: gic version 3" \
    "ginux: aarch64 page tables on" \
    "ginux: physical frame allocator active" \
    "ginux: psci version" \
    "ginux: timer-irq=" \
    "ginux: virtio pci device" \
    "ginux: smmuv3" \
    "ginux: svc=dispatch nr=64" \
    "ginux-sbsa: process-write ok" \
    "ginux: svc=dispatch nr=172 result=4242" \
    "ginux: svc=dispatch nr=93" \
    "ginux: sbsa arm services ready"
do
    if ! grep -F -q "$line" "$log"; then
        echo "uefi aarch64 kernel log missing: $line" >&2
        fail=1
    fi
done
if grep -F -q "not ported" "$log"; then
    echo "uefi aarch64 kernel stopped on an unported path" >&2
    fail=1
fi
if grep -F -q "GINUX_KERNEL_PANIC" "$log"; then
    echo "uefi aarch64 kernel panicked" >&2
    fail=1
fi
if [ "$fail" -ne 0 ]; then
    exit 1
fi
if [ "$qemu_status" -ne 0 ] && [ "$qemu_status" -ne 124 ]; then
    echo "qemu-system-aarch64 exited $qemu_status" >&2
    exit "$qemu_status"
fi
echo "aarch64 UEFI kernel: boot log captured"
