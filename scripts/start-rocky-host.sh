#!/bin/sh
set -eu

. ./scripts/rocky-image-env.sh
. ./scripts/rocky-host-common.sh

start_epoch=$(date +%s)
host_progress() {
    now=$(date +%s)
    printf 'Rocky host: [%ss] %s\n' "$((now - start_epoch))" "$1"
}

if [ -f "$ROCKY_PID_FILE" ]; then
    existing=$(cat "$ROCKY_PID_FILE")
    if kill -0 "$existing" 2>/dev/null; then
        printf 'Rocky host already running with PID %s\n' "$existing"
        exit 0
    fi
    rm -f "$ROCKY_PID_FILE"
fi

base=${ROCKY_IMAGE_PATH:-build/cache/rocky/$ROCKY_IMAGE_NAME}
case $base in
    /*) ;;
    *) base=$(pwd -P)/$base ;;
esac
ROCKY_IMAGE_PATH=$base ./scripts/prepare-rocky-host.sh
network_restrict=${ROCKY_NETWORK_RESTRICT:-on}
case "$network_restrict" in
    on|off) ;;
    *)
        printf 'ROCKY_NETWORK_RESTRICT must be on or off\n' >&2
        exit 2
        ;;
esac
overlay=$ROCKY_OVERLAY_DISK
mkdir -p "$(dirname "$overlay")" "$(dirname "$ROCKY_PID_FILE")" \
    "$(dirname "$ROCKY_SERIAL_LOG")"
if [ -f "$overlay" ]; then
    host_progress "reusing persistent host disk overlay $overlay"
else
    host_progress "creating persistent host disk overlay $overlay"
    qemu-img create -q -f qcow2 -F qcow2 -b "$base" "$overlay"
fi
rm -f "$ROCKY_SERIAL_LOG"

if [ "${ROCKY_ARCH:-x86_64}" = aarch64 ]; then
    code=/usr/share/AAVMF/AAVMF_CODE.fd
    vars_src=/usr/share/AAVMF/AAVMF_VARS.fd
    vars=build/cache/rocky/aarch64-AAVMF_VARS.fd
    share=$(pwd -P)
    case $ROCKY_VIRTIOFS_SOCK in
        /*) ;;
        *) ROCKY_VIRTIOFS_SOCK=$share/$ROCKY_VIRTIOFS_SOCK ;;
    esac
    mkdir -p build/cache/rocky
    if [ ! -f "$vars" ]; then
        cp "$vars_src" "$vars"
    fi
    host_progress 'starting virtiofsd for the Ginux tree'
    rm -f "$ROCKY_VIRTIOFS_SOCK" "$ROCKY_VIRTIOFSD_PID"
    virtiofsd=/usr/libexec/virtiofsd
    if [ ! -x "$virtiofsd" ]; then
        printf '%s\n' 'aarch64 Rocky host needs /usr/libexec/virtiofsd' >&2
        exit 1
    fi
    nohup "$virtiofsd" \
        --socket-path="$ROCKY_VIRTIOFS_SOCK" \
        --shared-dir="$share" \
        --sandbox none \
        > build/rocky-virtiofsd.log 2>&1 &
    echo $! > "$ROCKY_VIRTIOFSD_PID"
    virtio_wait=0
    while [ ! -S "$ROCKY_VIRTIOFS_SOCK" ] && [ "$virtio_wait" -lt 50 ]; do
        virtio_wait=$((virtio_wait + 1))
        sleep 0.1
    done
    if [ ! -S "$ROCKY_VIRTIOFS_SOCK" ]; then
        printf '%s\n' 'virtiofsd did not create its socket' >&2
        kill "$(cat "$ROCKY_VIRTIOFSD_PID")" 2>/dev/null || true
        exit 1
    fi
    host_progress 'starting aarch64 QEMU (TCG, 2 vCPUs, virtiofs)'
    qemu-system-aarch64 \
        -machine virt,gic-version=3,acpi=on \
        -accel tcg,thread=multi \
        -cpu max \
        -smp 2 \
        -m 2048M \
        -object memory-backend-memfd,id=mem,size=2048M,share=on \
        -numa node,memdev=mem \
        -display none \
        -daemonize \
        -pidfile "$ROCKY_PID_FILE" \
        -serial "file:$ROCKY_SERIAL_LOG" \
        -monitor none \
        -no-reboot \
        -drive "if=pflash,unit=0,format=raw,readonly=on,file=$code" \
        -drive "if=pflash,unit=1,format=raw,file=$vars" \
        -drive "if=none,id=rootdisk,format=qcow2,file=$overlay" \
        -device virtio-blk-pci,drive=rootdisk,bootindex=1 \
        -drive "if=none,id=cidata,media=cdrom,readonly=on,format=raw,file=build/rocky-seed.iso" \
        -device virtio-scsi-pci \
        -device scsi-cd,drive=cidata,bootindex=2 \
        -chardev "socket,id=vfs,path=$ROCKY_VIRTIOFS_SOCK" \
        -device vhost-user-fs-pci,chardev=vfs,tag=hostshare \
        -netdev "user,id=net0,restrict=$network_restrict,hostfwd=tcp:127.0.0.1:$ROCKY_SSH_PORT-:22" \
        -device virtio-net-pci,netdev=net0 \
        -device virtio-rng-pci
    ssh_attempts=240
else
    host_progress 'starting x86-64 QEMU (TCG, 2 vCPUs)'
    qemu-system-x86_64 \
        -machine q35 \
        -accel tcg,thread=multi \
        -cpu max \
        -smp 2 \
        -m 2048M \
        -display none \
        -daemonize \
        -pidfile "$ROCKY_PID_FILE" \
        -serial "file:$ROCKY_SERIAL_LOG" \
        -monitor none \
        -no-reboot \
        -drive "if=virtio,format=qcow2,file=$overlay" \
        -drive "if=ide,media=cdrom,readonly=on,format=raw,file=build/rocky-seed.iso" \
        -netdev "user,id=net0,restrict=$network_restrict,hostfwd=tcp:127.0.0.1:$ROCKY_SSH_PORT-:22" \
        -device virtio-net-pci,netdev=net0
    ssh_attempts=120
fi

host_progress 'waiting for SSH'
attempt=0
while [ "$attempt" -lt "$ssh_attempts" ]; do
    if rocky_ssh true >/dev/null 2>&1; then
        host_progress 'SSH ready; waiting for cloud-init'
        cloud_attempt=0
        cloud_limit=12
        if [ "${ROCKY_ARCH:-x86_64}" = aarch64 ]; then
            cloud_limit=40
        fi
        while ! rocky_ssh 'status=$(sudo cloud-init status 2>/dev/null || true); case "$status" in *"status: done"*|*"status: error"*) exit 0 ;; *) exit 1 ;; esac'; do
            cloud_attempt=$((cloud_attempt + 1))
            if [ "$cloud_attempt" -ge "$cloud_limit" ]; then
                printf '%s\n' 'Rocky cloud-init connection did not stabilize' >&2
                exit 1
            fi
            host_progress 'cloud-init still running; retrying'
            sleep 5
        done
        if [ "$network_restrict" = off ]; then
            host_progress 'cloud-init ready; enabling guest network access'
            rocky_ssh "sudo ip route replace default via 10.0.2.2 dev enp0s2; printf '%s\n' 'nameserver 10.0.2.3' | sudo tee /etc/resolv.conf >/dev/null"
        fi
        if [ "${ROCKY_ARCH:-x86_64}" = aarch64 ]; then
            rocky_ssh 'sudo modprobe virtiofs && sudo mkdir -p /mnt/host && sudo mount -t virtiofs hostshare /mnt/host'
            rocky_ssh 'mountpoint -q /mnt/host && test -d /mnt/host/target'
        fi
        host_progress 'SSH and cloud-init ready'
        exit 0
    fi
    attempt=$((attempt + 1))
    sleep 5
done

printf '%s\n' 'Rocky host did not become ready' >&2
tail -n 80 "$ROCKY_SERIAL_LOG" >&2 || true
exit 1
