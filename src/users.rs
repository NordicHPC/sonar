// Origin of this code: https://github.com/ogham/rust-users.  That library is not maintained as of
// 2024/02/22 and we need very little of it for Sonar, so the relevant bits have been moved here and
// heavily pruned to provide only what we need.

/*

MIT License

Copyright (c) 2019 Benjamin Sago

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

*/

use std::ffi::{CStr, OsStr, OsString};
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::ptr;

use libc::passwd as c_passwd;
use libc::uid_t;

/// Searches for a `User` with the given ID in the system’s user database.
/// Returns its name if one is found, otherwise returns `None`.
///
/// # libc functions used
///
/// - [`getpwuid_r`](https://docs.rs/libc/*/libc/fn.getpwuid_r.html)
pub fn get_user_by_uid(uid: uid_t) -> Option<OsString> {
    let mut passwd = unsafe { mem::zeroed::<c_passwd>() };
    let mut buf = vec![0; 2048];
    let mut result = ptr::null_mut::<c_passwd>();

    loop {
        let r =
            unsafe { libc::getpwuid_r(uid, &mut passwd, buf.as_mut_ptr(), buf.len(), &mut result) };

        if r != libc::ERANGE {
            break;
        }

        let newsize = buf.len().checked_mul(2)?;
        buf.resize(newsize, 0);
    }

    if result.is_null() {
        // There is no such user, or an error has occurred.
        // errno gets set if there’s an error.
        return None;
    }

    if result != &mut passwd {
        // The result of getpwuid_r should be its input passwd.
        return None;
    }

    Some(unsafe {
        OsString::from(OsStr::from_bytes(
            CStr::from_ptr(result.read().pw_name).to_bytes(),
        ))
    })
}
