#!/bin/sh
# Run the scripts of `make test` that need the Rocky host, on one boot of it:
#
#   ROCKY_ARCH=aarch64 ./scripts/with-rocky-host.sh ./scripts/test-host-suite.sh
#
# Arguments name the scripts to run; the default is all of them.
set -eu

if [ "$#" -eq 0 ]; then
    set -- intercept runtime config instance-suite devices process kernel-views
fi
for name in "$@"; do
    printf 'test-host-suite: %s\n' "$name"
    "./scripts/test-$name.sh"
done
printf 'test-host-suite: PASS (%s)\n' "$*"
