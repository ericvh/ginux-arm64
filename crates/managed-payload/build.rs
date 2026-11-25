fn main() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").ok().as_deref() == Some("aarch64") {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        println!(
            "cargo:rerun-if-changed={manifest}/../../tests/fixtures/managed-payload-aarch64.ld"
        );
        println!(
            "cargo:rustc-link-arg=-T{manifest}/../../tests/fixtures/managed-payload-aarch64.ld"
        );
        // The kernel loads this image, and arm64 kernels run with 4K, 16K,
        // or 64K pages: file offsets and addresses agree modulo the largest.
        println!("cargo:rustc-link-arg=-z");
        println!("cargo:rustc-link-arg=max-page-size=65536");
    }
}
