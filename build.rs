use std::env;

fn main() {
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    println!("cargo:rustc-link-search=gpuapi/{arch}");
}
