use std::env;

fn main() {
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    if let Ok(path) = env::var("SONAR_CUSTOM_GPUAPI") {
        println!("cargo:rustc-link-search={path}");
    } else {
        println!("cargo:rustc-link-search=gpuapi/{arch}");
    }
}
