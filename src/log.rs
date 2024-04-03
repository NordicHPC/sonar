// A trivial logging package, that can be replaced by something more interesting if necessary.

pub fn init() {
    // Currently nothing
}

pub fn info(s: &str) {
    eprintln!("Info: {s}");
}

pub fn error(s: &str) {
    eprintln!("Error: {s}");
}
