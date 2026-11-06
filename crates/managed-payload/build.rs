fn main() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").ok().as_deref() == Some("aarch64") {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        println!(
            "cargo:rerun-if-changed={manifest}/../../tests/fixtures/managed-payload-aarch64.ld"
        );
        println!(
            "cargo:rustc-link-arg=-T{manifest}/../../tests/fixtures/managed-payload-aarch64.ld"
        );
        // Keep the on-disk image inside the daemon's 64 KiB ELF buffer. The
        // virtual addresses stay page-aligned for the loader.
        println!("cargo:rustc-link-arg=-z");
        println!("cargo:rustc-link-arg=max-page-size=4096");
    }
}
