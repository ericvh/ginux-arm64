#!/bin/sh

ROCKY_SSH_PORT=${ROCKY_SSH_PORT:-22222}
ROCKY_SSH_USER=ginux
ROCKY_KEY=build/cache/rocky/gate1_ed25519
# The guest architecture selects the binaries the tests copy to the host, the
# dynamic loader they expect in a Rocky root, and the host's files here. x86_64 is upstream's host and keeps upstream's file
# names. aarch64 is the fork's; see docs/arm64/README.md.
ROCKY_ARCH=${ROCKY_ARCH:-x86_64}
case $ROCKY_ARCH in
    x86_64)
        rocky_host=rocky-host
        GINUX_RELEASE=target/x86_64-unknown-linux-gnu/release
        GINUX_CLI_RELEASE=target/x86_64-unknown-linux-musl/release
        ROCKY_LOADER=usr/lib64/ld-linux-x86-64.so.2
        ;;
    aarch64)
        rocky_host=rocky-host-aarch64
        GINUX_RELEASE=target/aarch64-unknown-linux-gnu/release
        GINUX_CLI_RELEASE=target/aarch64-unknown-linux-musl/release
        ROCKY_LOADER=usr/lib/ld-linux-aarch64.so.1
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

# Write the instance config $1 to $2 as this host should get it. The configs
# name their image by its linux/amd64 manifest. An aarch64 host runs the
# linux/arm64 image of the same index, so its copy names that manifest.
rocky_instance_config() {
    cp "$1" "$2"
    if [ "$ROCKY_ARCH" = aarch64 ]; then
        grep '^sha256:' config/examples/arm64-manifests | while read -r amd64 arm64 _; do
            sed -i "s/$amd64/$arm64/" "$2"
        done
    fi
}
