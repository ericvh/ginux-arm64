fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir");
    println!("cargo:rustc-link-arg=-T{manifest}/kernel.ld");
    println!("cargo:rerun-if-changed=kernel.ld");
}
