#!/bin/sh

# Select the pinned Rocky host image. `make test` sets ROCKY_ARCH=aarch64.
if [ "${ROCKY_ARCH:-x86_64}" = aarch64 ]; then
    . ./config/rocky-host-image-aarch64.env
    ROCKY_CHECKSUM_FILE=config/rocky-host-image-aarch64.CHECKSUM
    ROCKY_ROOT_PART=3
else
    . ./config/rocky-host-image.env
    ROCKY_CHECKSUM_FILE=config/rocky-host-image.CHECKSUM
    ROCKY_ROOT_PART=4
fi
export ROCKY_CHECKSUM_FILE ROCKY_ROOT_PART
