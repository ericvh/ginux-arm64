#!/bin/sh
# Run upstream's native-supervision test scripts against the aarch64 daemon
# in an arm64 Linux guest under qemu-system-aarch64. On an arm64 host the
# same scripts run directly: `make test-native-supervision-arm64`.
#
#   GINUX_AARCH64_KERNEL   arm64 Linux Image with virtio-9p built in.
#   GINUX_AARCH64_ROOTFS   tar archive of an arm64 root filesystem that has
#                          /bin/sh and python3, for example
#                          `docker export` of python:3-alpine for linux/arm64.
#   GINUX_GUEST_TIMEOUT    seconds before the guest is stopped (default 3600).
#
# Arguments name the test scripts to run; the default is every script that
# `make test-native-supervision-arm64` runs.
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

ginux=target/aarch64-unknown-linux-gnu/release/ginux
fixtures=.artifacts/launch-arm64
for file in "$ginux" "$fixtures/native-fixture" "$fixtures/native-clone-collision-fixture"; do
    if [ ! -f "$file" ]; then
        echo "missing $file; run make build-arm64 build-launch-fixture-arm64" >&2
        exit 2
    fi
done

if [ "$#" -eq 0 ]; then
    set -- supervision
fi

work=$(mktemp -d)
trap 'chmod -R u+w "$work" 2>/dev/null; rm -rf "$work"' EXIT HUP INT TERM
share=$work/share
tree=$work/rootfs
log=$work/console.log
mkdir -p "$share" "$tree"

cp "$ginux" "$share/ginux"
cp "$fixtures/native-fixture" "$fixtures/native-clone-collision-fixture" "$share/"
mkdir "$share/scripts" "$share/config"
cp scripts/test-native-*.py "$share/scripts/"
cp config/instance-gate5.toml "$share/config/"

# One line per test, in the order the Makefile runs them.
{
    echo "set -u"
    echo "status=0"
    for name in "$@"; do
        case $name in
            clone-snapshot)
                extra=/share/native-clone-collision-fixture ;;
            *)
                extra= ;;
        esac
        echo "echo 'GINUX_GUEST_TEST: $name'"
        echo "python3 /share/scripts/test-native-$name.py /share/ginux /share/native-fixture $extra || status=1"
    done
    echo 'exit $status'
} > "$share/run.sh"

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
if mount -t 9p -o trans=virtio,version=9p2000.L,msize=262144 hostshare /share; then
    sh /share/run.sh </dev/null
    echo "GINUX_GUEST_STATUS: $?"
else
    echo "GINUX_GUEST_STATUS: no-share"
fi
sync
poweroff -f
INIT
chmod 755 "$tree/init"
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
    echo "aarch64 native-supervision suite: FAIL" >&2
    exit 1
fi
for name in "$@"; do
    if ! grep -q "^test-native-$name: PASS" "$log"; then
        echo "aarch64 guest did not report test-native-$name: PASS" >&2
        exit 1
    fi
done
echo "aarch64 native-supervision suite: PASS"
