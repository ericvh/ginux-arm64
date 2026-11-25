SHELL := /bin/sh

# The bare-metal kernel line builds in its own Cargo workspace. Nothing here
# is part of the userspace daemon's checks.
CARGO ?= cargo
# The UEFI image pins the toolchain the `bootable-kernel` branch pins.
UEFI_CARGO ?= cargo +1.99.0
UEFI_TARGET := aarch64-unknown-uefi

.PHONY: controller uefi-aarch64 uefi-aarch64-guest legacy-kernel-aarch64 fmt-check clean

# Rebuild the measured AArch64 controller ELF and the manifest that names it.
controller:
	@./scripts/aarch64-controller.sh

# SKIP_CONTROLLER=1 builds with the committed controller ELF and manifest,
# for hosts without clang.
uefi-aarch64: $(if $(SKIP_CONTROLLER),,controller)
	CARGO_PROFILE_RELEASE_LTO=off CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 $(UEFI_CARGO) build -p ginux-boot --target $(UEFI_TARGET) --release

uefi-aarch64-guest: uefi-aarch64
	@./scripts/uefi-aarch64-guest.sh

legacy-kernel-aarch64:
	@CARGO='$(CARGO)' ./scripts/legacy-kernel-aarch64.sh

fmt-check:
	$(UEFI_CARGO) fmt --all --check
	$(CARGO) fmt --manifest-path legacy-kernel/Cargo.toml --check

clean:
	rm -rf target legacy-kernel/target
