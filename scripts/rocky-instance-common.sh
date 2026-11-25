#!/bin/sh

. ./scripts/rocky-image-env.sh
. ./config/rocky-instance-image.env

# The aarch64 userspace is prepared beside the x86_64 one, under its own names.
if [ "${ROCKY_ARCH:-x86_64}" = aarch64 ]; then
    ROCKY_INSTANCE_ROOTFS=rocky-10.2-aarch64-rootfs.tar
    ROCKY_INSTANCE_MANIFEST=rocky-10.2-aarch64-rootfs.manifest
    ROCKY_INSTANCE_METADATA=rocky-10.2-aarch64-rootfs.env
fi

ROCKY_SOURCE_IMAGE=${ROCKY_IMAGE_PATH:-build/cache/rocky/$ROCKY_IMAGE_NAME}
ROCKY_INSTANCE_ROOTFS_PATH=$ROCKY_INSTANCE_CACHE_DIR/$ROCKY_INSTANCE_ROOTFS
ROCKY_INSTANCE_MANIFEST_PATH=$ROCKY_INSTANCE_CACHE_DIR/$ROCKY_INSTANCE_MANIFEST
ROCKY_INSTANCE_METADATA_PATH=$ROCKY_INSTANCE_CACHE_DIR/$ROCKY_INSTANCE_METADATA
ROCKY_INSTANCE_WORK_DIR=build/rocky-instance-work
