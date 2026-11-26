#!/bin/sh
# Run upstream's integration test scripts against the aarch64 daemon in an
# arm64 Linux guest under qemu-system-aarch64. On an arm64 host the same
# scripts run directly: `make test-integration-arm64`.
#
#   GINUX_AARCH64_KERNEL   arm64 Linux Image with virtio-9p built in. An
#                          EFI zboot image, which is what some
#                          distributions ship as vmlinuz, is unpacked.
#   GINUX_AARCH64_SHARE    `initrd` puts the daemon, fixtures, and scripts
#                          in the initramfs instead of sharing them over
#                          9p, for a kernel that has 9p only as a module,
#                          as a distribution's does. The default is 9p.
#   GINUX_AARCH64_ROOTFS   tar archive of an arm64 root filesystem that has
#                          /bin/sh and python3, for example
#                          `docker export` of python:3-alpine for linux/arm64.
#   GINUX_GUEST_TIMEOUT    seconds before the guest is stopped (default 3600).
#
# Arguments name the tests to run: launch, gate1-setup, tracee-kill,
# process-native, and the native-supervision scripts by the part of their
# file name after `test-native-` (supervision, yield-broker, ...). The
# default is supervision.
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$root"

kernel=${GINUX_AARCH64_KERNEL:-}
rootfs=${GINUX_AARCH64_ROOTFS:-}
if [ -z "$kernel" ] || [ ! -f "$kernel" ]; then
    echo "Set GINUX_AARCH64_KERNEL to an arm64 Linux Image (for example a v9fs/test kernel-v* Image)." >&2
    exit 2
fi
if [ -z "$rootfs" ] || [ ! -f "$rootfs" ]; then
    echo "Set GINUX_AARCH64_ROOTFS to a tar archive of an arm64 root filesystem with python3." >&2
    exit 2
fi

release=target/aarch64-unknown-linux-gnu/release
launch=.artifacts/launch-arm64
gate1=.artifacts/gate1-arm64
process=.artifacts/process-arm64

. ./scripts/integration-tests.sh
if [ "$#" -eq 0 ]; then
    set -- supervision
fi

work=$(mktemp -d)
trap 'chmod -R u+w "$work" 2>/dev/null; rm -rf "$work"' EXIT HUP INT TERM
share=$work/share
tree=$work/rootfs
log=$work/console.log
mkdir -p "$share" "$tree"

# QEMU loads a plain or gzip-compressed Image, and an EFI zboot image only
# for some compressions. Take the Image out of a zboot image here: its
# header gives the payload's offset, size, and compression.
if [ "$(dd if="$kernel" bs=1 skip=4 count=4 2>/dev/null)" = zimg ]; then
    python3 - "$kernel" "$work/Image" <<'UNPACK'
import struct
import subprocess
import sys

data = open(sys.argv[1], "rb").read()
offset, size = struct.unpack_from("<II", data, 8)
kind = data[24:32].rstrip(b"\0").decode()
tool = {"gzip": "gzip", "zstd": "zstd", "xz": "xz", "lz4": "lz4"}[kind]
image = subprocess.run([tool, "-dc"], input=data[offset:offset + size],
                       stdout=subprocess.PIPE, check=True).stdout
open(sys.argv[2], "wb").write(image)
UNPACK
    kernel=$work/Image
fi

# One command per test, in the order given. The share mirrors the paths of
# this tree, so the scripts find config/ beside scripts/ as they do here.
{
    echo "set -u"
    echo "cd /share"
    echo "status=0"
    for name in "$@"; do
        echo "echo 'GINUX_GUEST_TEST: $name'"
        echo "$(integration_command "$name" "$release" "$launch" "$gate1" "$process") || status=1"
    done
    echo 'exit $status'
} > "$share/run.sh"

if [ ! -f "$release/ginux" ] || [ ! -f "$launch/native-fixture" ]; then
    echo "missing the aarch64 daemon or fixtures; run make build-arm64 build-fixtures-arm64" >&2
    exit 2
fi
mkdir -p "$share/$release" "$share/.artifacts" "$share/scripts" "$share/config"
cp "$release/ginux" "$share/$release/"
for file in managed-payload process-native-fixture; do
    if [ -f "$release/$file" ]; then cp "$release/$file" "$share/$release/"; fi
done
for directory in "$launch" "$gate1" "$process"; do
    if [ -d "$directory" ]; then cp -R "$directory" "$share/.artifacts/"; fi
done
cp scripts/test-*.py scripts/aarch64-trace.py "$share/scripts/"
cp config/*.toml "$share/config/"

tar -xf "$rootfs" -C "$tree" --no-same-owner --exclude='dev/*' 2>/dev/null || {
    echo "could not unpack $rootfs" >&2
    exit 2
}
cat > "$tree/init" <<'INIT'
#!/bin/sh
export PATH=/usr/local/bin:/usr/local/sbin:/usr/bin:/usr/sbin:/bin:/sbin
mkdir -p /proc /sys /dev /tmp /share
mount -t proc proc /proc
mount -t sysfs sys /sys
mount -t devtmpfs dev /dev 2>/dev/null
mount -t tmpfs -o mode=1777 tmp /tmp
ip link set lo up 2>/dev/null || ifconfig lo 127.0.0.1 up
if [ -f /share/run.sh ] || mount -t 9p -o trans=virtio,version=9p2000.L,msize=262144 hostshare /share; then
    sh /share/run.sh </dev/null
    echo "GINUX_GUEST_STATUS: $?"
else
    echo "GINUX_GUEST_STATUS: no-share"
fi
sync
poweroff -f
INIT
chmod 755 "$tree/init"
if [ "${GINUX_AARCH64_SHARE:-9p}" = initrd ]; then
    mkdir -p "$tree/share"
    cp -R "$share/." "$tree/share/"
fi
(cd "$tree" && find . | cpio -o -H newc -R 0:0 2>/dev/null) > "$work/initrd.cpio"

set +e
timeout "${GINUX_GUEST_TIMEOUT:-3600}" qemu-system-aarch64 \
    -machine virt \
    -accel tcg,thread=multi \
    -cpu cortex-a72 \
    -smp 4 \
    -m 2048 \
    -nographic \
    -display none \
    -kernel "$kernel" \
    -initrd "$work/initrd.cpio" \
    -append "console=ttyAMA0 loglevel=4 rdinit=/init panic=-1" \
    -fsdev "local,security_model=none,writeout=immediate,id=fsdev0,path=$share" \
    -device virtio-9p-pci,id=fs0,fsdev=fsdev0,mount_tag=hostshare \
    -no-reboot 2>&1 | tee "$log"
set -e

if ! grep -q "^GINUX_GUEST_STATUS: 0" "$log"; then
    echo "aarch64 guest tests: FAIL" >&2
    exit 1
fi
for name in "$@"; do
    line=$(integration_pass_line "$name")
    if ! grep -q "^$line" "$log"; then
        echo "aarch64 guest did not report $line" >&2
        exit 1
    fi
done
echo "aarch64 guest tests: PASS ($*)"
