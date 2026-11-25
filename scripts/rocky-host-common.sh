#!/bin/sh

ROCKY_SSH_PORT=${ROCKY_SSH_PORT:-22222}
ROCKY_SSH_USER=ginux
ROCKY_KEY=build/cache/rocky/gate1_ed25519
# The guest architecture selects the binaries the tests copy to the host and
# the host's files here. x86_64 is upstream's host and keeps upstream's file
# names. aarch64 is the fork's; see docs/arm64/README.md.
ROCKY_ARCH=${ROCKY_ARCH:-x86_64}
case $ROCKY_ARCH in
    x86_64)
        rocky_host=rocky-host
        GINUX_RELEASE=target/x86_64-unknown-linux-gnu/release
        GINUX_CLI_RELEASE=target/x86_64-unknown-linux-musl/release
        ;;
    aarch64)
        rocky_host=rocky-host-aarch64
        GINUX_RELEASE=target/aarch64-unknown-linux-gnu/release
        GINUX_CLI_RELEASE=target/aarch64-unknown-linux-musl/release
        ;;
    *)
        printf 'ROCKY_ARCH must be x86_64 or aarch64, not %s\n' "$ROCKY_ARCH" >&2
        exit 2
        ;;
esac
ROCKY_PID_FILE=${ROCKY_PID_FILE:-build/$rocky_host.pid}
ROCKY_SERIAL_LOG=${ROCKY_SERIAL_LOG:-build/$rocky_host-serial.log}
ROCKY_OVERLAY_DISK=${ROCKY_OVERLAY_DISK:-build/$rocky_host-overlay.qcow2}

rocky_ssh() {
    ssh \
        -i "$ROCKY_KEY" \
        -p "$ROCKY_SSH_PORT" \
        -o BatchMode=yes \
        -o ConnectTimeout=5 \
        -o IdentitiesOnly=yes \
        -o LogLevel=ERROR \
        -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        "$ROCKY_SSH_USER@127.0.0.1" "$@"
}

rocky_scp() {
    scp \
        -i "$ROCKY_KEY" \
        -P "$ROCKY_SSH_PORT" \
        -o BatchMode=yes \
        -o ConnectTimeout=5 \
        -o IdentitiesOnly=yes \
        -o LogLevel=ERROR \
        -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        "$@"
}

rocky_ssh_tty() {
    ssh -tt \
        -i "$ROCKY_KEY" \
        -p "$ROCKY_SSH_PORT" \
        -o BatchMode=yes \
        -o ConnectTimeout=5 \
        -o IdentitiesOnly=yes \
        -o LogLevel=ERROR \
        -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        "$ROCKY_SSH_USER@127.0.0.1" "$@"
}
