use std::ffi::CStr;

// Copy a C string.

pub fn cstrdup(s: &[cty::c_char]) -> String {
    unsafe { CStr::from_ptr(s.as_ptr()) }
        .to_str()
        .expect("Will always be utf8")
        .to_string()
}
