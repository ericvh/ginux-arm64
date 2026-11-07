#!/bin/sh

ROCKY_SSH_PORT=${ROCKY_SSH_PORT:-22222}
ROCKY_SSH_USER=ginux
ROCKY_KEY=build/cache/rocky/gate1_ed25519
if [ "${ROCKY_ARCH:-x86_64}" = aarch64 ]; then
    ROCKY_PID_FILE=${ROCKY_PID_FILE:-build/rocky-host-aarch64.pid}
    ROCKY_SERIAL_LOG=${ROCKY_SERIAL_LOG:-build/rocky-host-aarch64-serial.log}
    ROCKY_OVERLAY_DISK=${ROCKY_OVERLAY_DISK:-build/rocky-host-aarch64-overlay.qcow2}
    ROCKY_VIRTIOFSD_PID=${ROCKY_VIRTIOFSD_PID:-build/rocky-virtiofsd.pid}
    ROCKY_VIRTIOFS_SOCK=${ROCKY_VIRTIOFS_SOCK:-build/rocky-virtiofs.sock}
else
    ROCKY_PID_FILE=${ROCKY_PID_FILE:-build/rocky-host.pid}
    ROCKY_SERIAL_LOG=${ROCKY_SERIAL_LOG:-build/rocky-host-serial.log}
    ROCKY_OVERLAY_DISK=${ROCKY_OVERLAY_DISK:-build/rocky-host-overlay.qcow2}
fi

# Directory of release binaries for the guest architecture. The aarch64
# runtime fixture stays on the x86_64 ELF the interpreter already audits.
ginux_release_dir() {
    if [ "${ROCKY_ARCH:-x86_64}" = aarch64 ]; then
        printf '%s\n' target/aarch64-unknown-linux-gnu/release
    else
        printf '%s\n' target/x86_64-unknown-linux-gnu/release
    fi
}

# Place a host file on the guest. aarch64 copies it from the virtiofs share
# mounted at /mnt/host. The Rocky 10 aarch64 kernel is built with
# CONFIG_NET_9P disabled, so the share is virtiofs rather than 9p.
# x86_64 keeps the SSH copy.
rocky_put() {
    src=$1
    dest=$2
    if [ "${ROCKY_ARCH:-x86_64}" = aarch64 ]; then
        rocky_ssh "cp -a -- '/mnt/host/${src}' '${dest}'"
    else
        rocky_scp "$src" "$ROCKY_SSH_USER@127.0.0.1:${dest}"
    fi
}

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
