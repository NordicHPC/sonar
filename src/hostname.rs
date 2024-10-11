// Origin of this code: https://github.com/svartalf/hostname.

/*

MIT License

Copyright (c) 2016 fengcen
Copyright (c) 2019 svartalf

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

use std::ffi::OsString;
use std::io;
use std::os::unix::ffi::OsStringExt;

pub fn get() -> String {
    match primitive_get() {
        Ok(hn) => {
            match hn.into_string() {
                Ok(s) => s,
                Err(_) => "unknown-host".to_string()
            }
        }
        Err(_) => "unknown-host".to_string()
    }
}

pub fn primitive_get() -> io::Result<OsString> {
    // According to the POSIX specification,
    // host names are limited to `HOST_NAME_MAX` bytes
    //
    // https://pubs.opengroup.org/onlinepubs/9699919799/functions/gethostname.html
    let size = unsafe { libc::sysconf(libc::_SC_HOST_NAME_MAX) as libc::size_t };

    // Stack buffer OK: HOST_NAME_MAX is typically very small (64 on Linux).
    let mut buffer = vec![0u8; size];

    let result = unsafe { libc::gethostname(buffer.as_mut_ptr() as *mut libc::c_char, size) };

    if result != 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(wrap_buffer(buffer))
}

fn wrap_buffer(mut bytes: Vec<u8>) -> OsString {
    // Returned name might be truncated if it does not fit
    // and `buffer` will not contain the trailing \0 in that case.
    // Manually capping the buffer length here.
    let end = bytes
        .iter()
        .position(|&byte| byte == 0x00)
        .unwrap_or(bytes.len());
    bytes.resize(end, 0x00);

    OsString::from_vec(bytes)
}
