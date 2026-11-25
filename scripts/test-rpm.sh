#!/bin/sh
set -eu

rpm_arch=${ROCKY_ARCH:-x86_64}
rpm_file=$(find build/rpm/RPMS -type f -name "ginux-*.$rpm_arch.rpm" -print | sed -n '1p')
test -n "$rpm_file" || { printf '%s\n' 'Ginux RPM is missing' >&2; exit 1; }

# An aarch64 test host has its own disk overlay.
case $rpm_arch in
    x86_64) test_host=rocky-rpm-test ;;
    *) test_host=rocky-rpm-test-$rpm_arch ;;
esac
rm -f "build/$test_host-overlay.qcow2"
ROCKY_NETWORK_RESTRICT=off \
    ROCKY_SSH_PORT=22427 \
    ROCKY_PID_FILE=build/$test_host.pid \
    ROCKY_SERIAL_LOG=build/$test_host-serial.log \
    ROCKY_OVERLAY_DISK=build/$test_host-overlay.qcow2 \
    ROCKY_VCPUS=4 ROCKY_MEMORY_MIB=4096 \
    ./scripts/with-rocky-host.sh ./scripts/test-rpm-rocky.sh "$rpm_file"
