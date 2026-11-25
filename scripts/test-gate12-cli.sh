#!/bin/sh
set -eu

. ./scripts/rocky-host-common.sh

./scripts/check-rocky-host.sh
rocky_scp "$GINUX_RELEASE/ginux" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-runtime"
rocky_scp "$GINUX_CLI_RELEASE/ginux-cli" \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-cli"
rocky_scp scripts/ginux-host-instance-helper.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/ginux-instance-helper"
rocky_scp scripts/gate12-cli-guest.sh \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate12-cli-guest.sh"
rocky_scp config/examples/rocky10-systemd.toml \
    "$ROCKY_SSH_USER@127.0.0.1:/var/tmp/gate12-recipe.toml"

rocky_ssh 'sudo install -d -m 755 /etc/ginux/examples /usr/local/libexec/ginux-image /var/lib/ginux/images /var/lib/ginux/instances /var/lib/ginux/work; sudo install -m 600 /var/tmp/gate12-recipe.toml /etc/ginux/examples/rocky10-systemd.toml; sudo install -m 755 /var/tmp/ginux-runtime /usr/local/libexec/ginux-runtime; sudo install -m 755 /var/tmp/ginux-cli /usr/local/bin/ginux; sudo install -m 755 /var/tmp/ginux-instance-helper /usr/local/libexec/ginux-instance-helper; sudo chmod 0755 /var/tmp/gate12-cli-guest.sh; sudo /var/tmp/gate12-cli-guest.sh'

printf '%s\n' 'test-gate12-cli: PASS'
