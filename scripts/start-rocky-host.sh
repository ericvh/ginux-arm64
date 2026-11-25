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

if command -v lsof >/dev/null 2>&1; then
    listener=$(lsof -nP -t -iTCP:"$ROCKY_SSH_PORT" -sTCP:LISTEN 2>/dev/null | sed -n '1p')
    if [ -n "$listener" ]; then
        printf 'Rocky host SSH port %s is already used by PID %s.\n' \
            "$ROCKY_SSH_PORT" "$listener" >&2
        printf 'Run ./scripts/stop-rocky-host.sh or choose another port with ROCKY_SSH_PORT=<port>.\n' >&2
        exit 1
    fi
fi

base=${ROCKY_IMAGE_PATH:-build/cache/rocky/$ROCKY_IMAGE_NAME}
case $base in
    /*) ;;
    *) base=$(pwd -P)/$base ;;
esac
ROCKY_IMAGE_PATH=$base ./scripts/prepare-rocky-host.sh
network_restrict=${ROCKY_NETWORK_RESTRICT:-on}
vcpus=${ROCKY_VCPUS:-2}
memory_mib=${ROCKY_MEMORY_MIB:-2048}
case "$network_restrict" in
    on|off) ;;
    *)
        printf 'ROCKY_NETWORK_RESTRICT must be on or off\n' >&2
        exit 2
        ;;
esac
case "$vcpus:$memory_mib" in
    *[!0-9:]*|:*|*:) printf '%s\n' 'ROCKY_VCPUS and ROCKY_MEMORY_MIB must be positive integers' >&2; exit 2 ;;
esac
test "$vcpus" -gt 0 && test "$memory_mib" -gt 0 || {
    printf '%s\n' 'ROCKY_VCPUS and ROCKY_MEMORY_MIB must be positive integers' >&2
    exit 2
}
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

# The host is emulated, as upstream's is. ROCKY_ACCEL=kvm runs it on the
# machine's own CPU instead, which needs a machine of the host's
# architecture and access to /dev/kvm.
case ${ROCKY_ACCEL:-tcg} in
    tcg)
        accel=tcg,thread=multi
        accel_name=TCG
        aarch64_cpu=cortex-a72
        aarch64_gic=3
        ;;
    kvm)
        accel=kvm
        accel_name=KVM
        aarch64_cpu=host
        aarch64_gic=host
        ;;
    *)
        printf 'ROCKY_ACCEL must be tcg or kvm\n' >&2
        exit 2
        ;;
esac

case $ROCKY_ARCH in
    x86_64)
        host_progress "starting x86-64 QEMU ($accel_name, $vcpus vCPUs, $memory_mib MiB)"
        qemu-system-x86_64 \
            -machine q35 \
            -accel "$accel" \
            -cpu max \
            -smp "$vcpus" \
            -m "${memory_mib}M" \
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
        ;;
    aarch64)
        # The virt machine boots through Arm UEFI firmware and has no IDE
        # bus, so the seed is a SCSI CD-ROM. The firmware's variable store
        # is not kept: the image boots from its removable-media path.
        firmware=${GINUX_AAVMF_DIR:-/usr/share/AAVMF}
        if [ ! -f "$firmware/AAVMF_CODE.fd" ] || [ ! -f "$firmware/AAVMF_VARS.fd" ]; then
            printf 'Arm UEFI firmware not found in %s; install qemu-efi-aarch64 or set GINUX_AAVMF_DIR\n' \
                "$firmware" >&2
            exit 2
        fi
        host_progress "starting aarch64 QEMU ($accel_name, $vcpus vCPUs, $memory_mib MiB)"
        qemu-system-aarch64 \
            -machine "virt,gic-version=$aarch64_gic" \
            -accel "$accel" \
            -cpu "${ROCKY_AARCH64_CPU:-$aarch64_cpu}" \
            -smp "$vcpus" \
            -m "${memory_mib}M" \
            -display none \
            -daemonize \
            -pidfile "$ROCKY_PID_FILE" \
            -serial "file:$ROCKY_SERIAL_LOG" \
            -monitor none \
            -no-reboot \
            -drive "if=pflash,unit=0,format=raw,readonly=on,file=$firmware/AAVMF_CODE.fd" \
            -drive "if=pflash,unit=1,format=raw,snapshot=on,file=$firmware/AAVMF_VARS.fd" \
            -drive "if=none,id=rootdisk,format=qcow2,file=$overlay" \
            -device virtio-blk-pci,drive=rootdisk,bootindex=1 \
            -drive "if=none,id=cidata,media=cdrom,readonly=on,format=raw,file=build/rocky-seed.iso" \
            -device virtio-scsi-pci \
            -device scsi-cd,drive=cidata \
            -netdev "user,id=net0,restrict=$network_restrict,hostfwd=tcp:127.0.0.1:$ROCKY_SSH_PORT-:22" \
            -device virtio-net-pci,netdev=net0 \
            -device virtio-rng-pci
        # An emulated Arm guest takes longer to reach SSH on first boot.
        ssh_attempts=360
        ;;
esac

host_progress 'waiting for SSH'
attempt=0
while [ "$attempt" -lt "$ssh_attempts" ]; do
    if rocky_ssh true >/dev/null 2>&1; then
        host_progress 'SSH ready; waiting for cloud-init'
        cloud_attempt=0
        while ! rocky_ssh 'sudo cloud-init status --wait >/dev/null'; do
            cloud_attempt=$((cloud_attempt + 1))
            if [ "$cloud_attempt" -ge 12 ]; then
                printf '%s\n' 'Rocky cloud-init connection did not stabilize' >&2
                exit 1
            fi
            host_progress 'cloud-init SSH session reset; retrying'
            sleep 5
        done
        if [ "$network_restrict" = off ]; then
            host_progress 'cloud-init ready; enabling guest network access'
            # The virt machine names its network device differently.
            case $ROCKY_ARCH in
                x86_64) guest_device=enp0s2 ;;
                *) guest_device=$(rocky_ssh "ls /sys/class/net | grep -v '^lo\$' | sed -n 1p") ;;
            esac
            rocky_ssh "sudo ip route replace default via 10.0.2.2 dev $guest_device; printf '%s\n' 'nameserver 10.0.2.3' | sudo tee /etc/resolv.conf >/dev/null"
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
