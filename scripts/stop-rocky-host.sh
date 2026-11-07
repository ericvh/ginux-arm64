#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

if [ ! -f "$ROCKY_PID_FILE" ]; then
    printf '%s\n' 'Rocky host already stopped'
    exit 0
fi

pid=$(cat "$ROCKY_PID_FILE")
rocky_ssh 'sudo systemctl poweroff' >/dev/null 2>&1 || true

attempt=0
while kill -0 "$pid" 2>/dev/null && [ "$attempt" -lt 30 ]; do
    attempt=$((attempt + 1))
    sleep 2
done
if kill -0 "$pid" 2>/dev/null; then
    kill "$pid"
fi
rm -f "$ROCKY_PID_FILE"
if [ -n "${ROCKY_VIRTIOFSD_PID:-}" ] && [ -f "$ROCKY_VIRTIOFSD_PID" ]; then
    kill "$(cat "$ROCKY_VIRTIOFSD_PID")" 2>/dev/null || true
    rm -f "$ROCKY_VIRTIOFSD_PID" "${ROCKY_VIRTIOFS_SOCK:-}"
fi
printf '%s\n' 'Rocky host stopped'
