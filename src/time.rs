use std::ffi::CStr;

// Get current time as an ISO time stamp: yyyy-mm-ddThh:mm:ss+hh:mm
//
// Use libc to avoid pulling in all of Chrono for this:
//   t = time()
//   localtime_r(&t, timebuf)
//   strftime(strbuf, strbufsize, "%FT%T%z", timebuf)
//
// Panic on errors, there should never be any.

pub fn now_iso8601() -> String {
    let mut timebuf = libc::tm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 0,
        tm_mon: 0,
        tm_year: 0,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    };
    const SIZE: usize = 32; // We need 25 unless something is greatly off
    let mut buffer = vec![0i8; SIZE];
    let s = unsafe {
        let t = libc::time(std::ptr::null_mut());

        if libc::localtime_r(&t, &mut timebuf).is_null() {
            panic!("localtime_r");
        }

        // strftime returns 0 if the buffer is too small for the result + NUL.
        if libc::strftime(
            buffer.as_mut_ptr(),
            SIZE,
            CStr::from_bytes_with_nul_unchecked(b"%FT%T%z\0").as_ptr(),
            &timebuf,
        ) == 0
        {
            panic!("strftime");
        }

        CStr::from_ptr(buffer.as_ptr())
            .to_str()
            .unwrap()
            .to_string()
    };

    // We have +/-hhmm for the time zone but want +/-hh:mm for compatibility with older data and
    // consumers.  strftime() won't do that for us.  We could do the formatting ourselves from raw
    // data, here we fix up the string instead.  The code is conservative: it looks for the sign,
    // and does nothing to the string if the sign isn't found in the expected location.
    let bs = s.as_bytes();
    match bs[bs.len() - 5] {
        b'+' | b'-' => {
            format!(
                "{}:{}",
                std::str::from_utf8(&bs[..bs.len() - 2]).expect("Must have string"),
                std::str::from_utf8(&bs[bs.len() - 2..]).expect("Must have string")
            )
        }
        _ => s,
    }
}

#[test]
pub fn test_isotime() {
    let t = now_iso8601();
    let ts = t.as_str().chars().collect::<Vec<char>>();
    let expect = "dddd-dd-ddTdd:dd:dd+dd:dd";
    let mut i = 0;
    for c in expect.chars() {
        match c {
            'd' => {
                assert!(ts[i] >= '0' && ts[i] <= '9');
            }
            '+' => {
                assert!(ts[i] == '+' || ts[i] == '-');
            }
            _ => {
                assert!(ts[i] == c);
            }
        }
        i += 1;
    }
    assert!(i == ts.len());
}
