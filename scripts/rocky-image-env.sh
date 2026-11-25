#!/bin/sh

# Select the pinned Rocky host image for the guest architecture. x86_64 is
# upstream's host. aarch64 is the fork's; see docs/arm64/README.md.
case ${ROCKY_ARCH:-x86_64} in
    x86_64)
        . ./config/rocky-host-image.env
        ROCKY_CHECKSUM_FILE=config/rocky-host-image.CHECKSUM
        ROCKY_ROOT_PART=4
        ;;
    aarch64)
        . ./config/rocky-host-image-aarch64.env
        ROCKY_CHECKSUM_FILE=config/rocky-host-image-aarch64.CHECKSUM
        ROCKY_ROOT_PART=3
        ;;
    *)
        printf 'ROCKY_ARCH must be x86_64 or aarch64, not %s\n' "$ROCKY_ARCH" >&2
        exit 2
        ;;
esac
