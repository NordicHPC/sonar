// A trivial logging package, that can be replaced by something more interesting if necessary.

#![allow(dead_code)]

pub fn init() {
    // Currently nothing
}

// verbose() is defined to always print to stderr.
pub fn verbose(s: &str) {
    eprintln!("Info: {s}");
}

pub fn info(s: &str) {
    eprintln!("Info: {s}");
}

pub fn error(s: &str) {
    eprintln!("Error: {s}");
}
