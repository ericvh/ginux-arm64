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
    # Page sizes other than 4K exist only on arm64, and so does this test.
    [ "$ROCKY_ARCH" = x86_64 ] || set -- "$@" page-size
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
tar -cf - scripts/test-*.py scripts/aarch64-trace.py config/*.toml \
    "$GINUX_RELEASE/ginux" "$GINUX_RELEASE/managed-payload" \
    "$GINUX_RELEASE/process-native-fixture" "$launch" "$gate1" "$process" |
    rocky_ssh "tar -xf - -C $remote"

mkdir -p build
for name in "$@"; do
    log=build/integration-$ROCKY_ARCH-$name.log
    command=$(integration_command "$name" "$GINUX_RELEASE" "$launch" "$gate1" "$process")
    printf 'test-integration-host: %s\n' "$name"
    if ! rocky_ssh "cd $remote && sudo env GINUX_TRACE_TIME_SCALE=${GINUX_TRACE_TIME_SCALE:-1} GINUX_TRACE_LOG=${GINUX_TRACE_LOG:-} timeout 1800 $command" > "$log" 2>&1; then
        tail -n 40 "$log" >&2
        printf 'test-integration-host: %s failed; see %s\n' "$name" "$log" >&2
        exit 1
    fi
    grep -q "^$(integration_pass_line "$name")" "$log"
    printf 'test-integration-host: %s passed (%s checks)\n' "$name" "$(grep -c '^PASS' "$log")"
done
rocky_ssh "sudo rm -rf $remote"
printf 'test-integration-host: PASS (%s)\n' "$*"
