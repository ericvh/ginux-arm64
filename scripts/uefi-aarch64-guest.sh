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
mkdir -p "$work/src"
printf 'ginux-ext4-file\n' >"$work/src/ginux.txt"
dd if=/dev/zero of="$work/disk.img" bs=1M count=1 status=none
mkfs.ext4 -F -L ginux -b 4096 -O ^inline_data -d "$work/src" "$work/disk.img" >/dev/null
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
    "ginux: virtio-blk read ok" \
    "ginux: ext4 file ok" \
    "ginux: smmuv3" \
    "ginux: embedded controller runtime measurement accepted" \
    "ginux: controller elf aarch64" \
    "ginux: controller elf loaded" \
    "ginux: process=started" \
    "ginux-sbsa: controller write ok" \
    "ginux: svc=dispatch nr=64 write result=" \
    "ginux: svc=dispatch nr=214 brk result=" \
    "ginux: svc=dispatch nr=222 mmap result=" \
    "ginux-sbsa: controller memory ok" \
    "ginux: svc=dispatch nr=56 openat result=" \
    "ginux: svc=dispatch nr=80 fstat result=" \
    "ginux-sbsa: vda block 1048576" \
    "ginux-ext4-file" \
    "ginux-ext4-wrote" \
    "ginux: svc=dispatch nr=57 close result=" \
    "ginux: svc=dispatch nr=160 uname result=" \
    "ginux-sbsa: uname aarch64" \
    "ginux: svc=dispatch nr=113 clock_gettime result=" \
    "ginux-sbsa: clock realtime" \
    "ginux-sbsa: clock monotonic" \
    "ginux: svc=dispatch nr=63 read result=" \
    "ginux-sbsa: clone child ok" \
    "ginux: svc=dispatch nr=220 clone result=" \
    "ginux-sbsa: clone parent ok" \
    "ginux: waitid pid 4243 status 42" \
    "ginux-sbsa: waitid clone 42" \
    "ginux-sbsa: clone3 child ok" \
    "ginux: svc=dispatch nr=435 clone3 result=" \
    "ginux-sbsa: clone3 parent ok" \
    "ginux: waitid pid 4244 status 7" \
    "ginux-sbsa: waitid clone3 7" \
    "ginux: svc=dispatch nr=95 waitid result=" \
    "ginux: svc=dispatch nr=93" \
    "ginux: process=exited" \
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
for syscall in "nr=214 brk" "nr=222 mmap" "nr=64 write" "nr=63 read" "nr=220 clone" "nr=435 clone3" "nr=56 openat" "nr=80 fstat" "nr=62 lseek" "nr=57 close" "nr=160 uname" "nr=113 clock_gettime" "nr=95 waitid"; do
    if grep -F "$syscall" "$log" | grep -F -q "result=-"; then
        echo "uefi aarch64 kernel returned an error for $syscall" >&2
        fail=1
    fi
done
if [ "$fail" -ne 0 ]; then
    exit 1
fi
if [ "$qemu_status" -ne 0 ] && [ "$qemu_status" -ne 124 ]; then
    echo "qemu-system-aarch64 exited $qemu_status" >&2
    exit "$qemu_status"
fi
echo "aarch64 UEFI kernel: boot log captured"
