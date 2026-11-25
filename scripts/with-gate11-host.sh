#!/bin/sh
set -eu

# The benchmark host keeps what bench-prepare installs. x86_64 is upstream's
# host and keeps upstream's file names; another guest architecture has its
# own disk, because it cannot boot this one.
case ${ROCKY_ARCH:-x86_64} in
    x86_64) gate11_host=gate11-host ;;
    *) gate11_host=gate11-host-$ROCKY_ARCH ;;
esac
export ROCKY_SSH_PORT="${ROCKY_SSH_PORT:-22411}"
export ROCKY_PID_FILE="${ROCKY_PID_FILE:-build/$gate11_host.pid}"
export ROCKY_SERIAL_LOG="${ROCKY_SERIAL_LOG:-build/$gate11_host-serial.log}"
export ROCKY_OVERLAY_DISK="${ROCKY_OVERLAY_DISK:-build/$gate11_host-overlay.qcow2}"
export ROCKY_NETWORK_RESTRICT="${ROCKY_NETWORK_RESTRICT:-on}"
export ROCKY_VCPUS="${ROCKY_VCPUS:-4}"
export ROCKY_MEMORY_MIB="${ROCKY_MEMORY_MIB:-4096}"

exec ./scripts/with-rocky-host.sh "$@"
