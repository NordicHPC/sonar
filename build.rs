use std::env;

fn main() {
    let here = match env::current_dir() {
        Ok(path) => path.display().to_string(),
        Err(_) => panic!("No CWD"),
    };
    println!("cargo:rustc-link-lib={here}/gpuapi/sonar-nvidia.a");
}
