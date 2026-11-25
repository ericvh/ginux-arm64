#!/bin/sh
# Run the integration test scripts as root on the Rocky host, which has what
# they need of a distribution: /proc/<pid>/task/<tid>/children, util-linux,
# and Python.
#
#   ROCKY_ARCH=aarch64 ./scripts/with-rocky-host.sh ./scripts/test-integration-host.sh
#
# Arguments name the tests (see scripts/integration-tests.sh); the default is
# all of them. The tests bind a directory over /var/tmp, so the files go to
# /opt on the host.
set -eu

. ./scripts/rocky-host-common.sh
. ./scripts/integration-tests.sh

case $ROCKY_ARCH in
    x86_64) built= ;;
    *) built=-arm64 ;;
esac
launch=.artifacts/launch$built
gate1=.artifacts/gate1$built
process=.artifacts/process$built
remote=/opt/ginux-integration

if [ "$#" -eq 0 ]; then
    set -- $integration_default
fi
for file in "$GINUX_RELEASE/ginux" "$GINUX_RELEASE/managed-payload" \
    "$GINUX_RELEASE/process-native-fixture" "$launch/native-fixture" \
    "$gate1/payload" "$process/process-exec-target"; do
    if [ ! -f "$file" ]; then
        printf 'missing %s; build the daemon and its fixtures first\n' "$file" >&2
        exit 2
    fi
done

./scripts/check-rocky-host.sh
rocky_ssh "sudo rm -rf $remote && sudo install -d -m 755 -o $ROCKY_SSH_USER $remote"
tar -cf - scripts/test-*.py config/*.toml \
    "$GINUX_RELEASE/ginux" "$GINUX_RELEASE/managed-payload" \
    "$GINUX_RELEASE/process-native-fixture" "$launch" "$gate1" "$process" |
    rocky_ssh "tar -xf - -C $remote"

mkdir -p build
for name in "$@"; do
    log=build/integration-$ROCKY_ARCH-$name.log
    command=$(integration_command "$name" "$GINUX_RELEASE" "$launch" "$gate1" "$process")
    printf 'test-integration-host: %s\n' "$name"
    if ! rocky_ssh "cd $remote && sudo $command" > "$log" 2>&1; then
        tail -n 40 "$log" >&2
        printf 'test-integration-host: %s failed; see %s\n' "$name" "$log" >&2
        exit 1
    fi
    grep -Fqx "$(integration_pass_line "$name")" "$log"
    printf '%s (%s checks)\n' "$(integration_pass_line "$name")" "$(grep -c '^PASS' "$log")"
done
rocky_ssh "sudo rm -rf $remote"
printf 'test-integration-host: PASS (%s)\n' "$*"
