#!/bin/sh
set -eu

# Automated checks never share the operator's persistent interactive host.
# An aarch64 host has its own overlay; neither can boot the other's.
case ${ROCKY_ARCH:-x86_64} in
    x86_64) suite_host=rocky-test-suite-host ;;
    *) suite_host=rocky-test-suite-host-$ROCKY_ARCH ;;
esac
export ROCKY_SSH_PORT="${ROCKY_SSH_PORT:-22408}"
export ROCKY_PID_FILE="${ROCKY_PID_FILE:-build/$suite_host.pid}"
export ROCKY_SERIAL_LOG="${ROCKY_SERIAL_LOG:-build/$suite_host-serial.log}"
export ROCKY_OVERLAY_DISK="${ROCKY_OVERLAY_DISK:-build/$suite_host-overlay.qcow2}"

cleanup() {
    ./scripts/stop-rocky-host.sh
}
trap cleanup 0 HUP INT TERM

./scripts/start-rocky-host.sh
"$@"
