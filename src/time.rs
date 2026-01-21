// Miscellaneous time and date utilities that use libc to avoid pulling in Chrono.  These mostly
// panic on errors, there should never be any except where input could plausibly come from a user.
//
// NOTE: According to the web, localtime_r() is not required to initialize time zone information,
// and tzset() should be called before using it (at least once).  Need to figure that out somehow.
// So far it seems we've not needed to do this.

use crate::util::cstrdup;
use std::num::ParseIntError;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time precedes epoch")
        .as_secs()
}

// Get current time as an ISO time stamp: yyyy-mm-ddThh:mm:ss+hh:mm
//
//   t = time()
//   localtime_r(&t, timebuf)
//   strftime(strbuf, strbufsize, "%FT%T%z", timebuf)

pub fn now_iso8601() -> String {
    format_iso8601(&now_local())
}

// Get current local time with tz information.
//
//   t = time()
//   localtime_r(&t, timebuf)
//
// The tm that is returned here may have a non-null tm_zone but if so that should point to static
// data.

pub fn now_local() -> libc::tm {
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
    unsafe {
        let t = libc::time(std::ptr::null_mut());

        if libc::localtime_r(&t, &mut timebuf).is_null() {
            // There might be legitimate reasons for localtime_r to fail, but it's unclear what we
            // can do in that case.  We could return a dummy time?  Unclear if that's better than a
            // panic here.
            panic!("localtime_r");
        }
    }
    timebuf
}

// Parse a timestamp into components.  I guess we could use libc::strptime here but for now let's
// just handle yyyy-mm-ddThh:mm[:ss] and leave the localtime fields blank.  Here we must return a Result
// b/c this may depend on user input.

pub fn parse_date_and_time_no_tzo(s: &str) -> Result<libc::tm, String> {
    let components = s.split('T').collect::<Vec<&str>>();
    if components.len() != 2 {
        return Err("Expected ...T...".to_string());
    }
    let ymd = components[0].split('-').collect::<Vec<&str>>();
    if ymd.len() != 3 {
        return Err("Expected yyyy-mm-dd".to_string());
    }
    let hms = components[1].split(':').collect::<Vec<&str>>();
    if hms.len() != 2 && hms.len() != 3 {
        return Err("Expected hh:mm".to_string());
    }
    let yr = ymd[0].parse::<u32>().map_err(parse_int_err)?;
    let mo = ymd[1].parse::<u32>().map_err(parse_int_err)?;
    let dy = ymd[2].parse::<u32>().map_err(parse_int_err)?;
    let hr = hms[0].parse::<u32>().map_err(parse_int_err)?;
    let mi = hms[1].parse::<u32>().map_err(parse_int_err)?;
    let ss = if hms.len() == 3 {
        hms[2].parse::<u32>().map_err(parse_int_err)?
    } else {
        0
    };
    if yr < 1970
        || yr > 2100
        || mo < 1
        || mo > 12
        || dy < 1
        || (mo == 2 && dy > 29)
        || (mo == 1 || mo == 3 || mo == 5 || mo == 7 || mo == 8 || mo == 10 || mo == 12) && dy > 31
        || (mo == 2 || mo == 4 || mo == 6 || mo == 9 || mo == 11) && dy > 30
        || hr > 23
        || mi > 59
        || ss > 60
    {
        return Err("Date field out of range".to_string());
    }
    Ok(libc::tm {
        tm_sec: ss as i32,
        tm_min: mi as i32,
        tm_hour: hr as i32,
        tm_mday: dy as i32,
        tm_mon: (mo - 1) as i32,
        tm_year: (yr - 1900) as i32,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    })
}

fn parse_int_err(_e: ParseIntError) -> String {
    "Not an unsigned int value".to_string()
}

// Format a time as an ISO time stamp: yyyy-mm-ddThh:mm:ss+hh:mm
//
//   strftime(strbuf, strbufsize, "%FT%T%z", timebuf)

pub fn format_iso8601(timebuf: &libc::tm) -> String {
    const SIZE: usize = 32; // We need 25 unless something is greatly off
    let mut buffer = vec![0 as cty::c_char; SIZE];
    let s = unsafe {
        if libc::strftime(buffer.as_mut_ptr(), SIZE, c"%FT%T%z".as_ptr(), timebuf) == 0 {
            // strftime returns 0 if the buffer is too small for the result + NUL, but we should
            // have ensured above that this is never a problem.
            panic!("strftime");
        }
        cstrdup(&buffer)
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

// Taken from StackOverflow, converted to Rust, simplified.  The values returned are year, month,
// day, hour, minute, second, where month, day, hour, minute, second are all zero-based (normally in
// Unix timestamps, day and month are both 1-based).
//
// This iterates up from 1970 and is slowish.  For speed, we'd have a precomputed table of starting
// values for select years.

pub fn unix_time_components(t: u64) -> (u64, u64, u64, u64, u64, u64) {
    const SECONDS_PER_MINUTE: u64 = 60;
    const SECONDS_PER_HOUR: u64 = 60 * SECONDS_PER_MINUTE;
    const SECONDS_PER_DAY: u64 = 24 * SECONDS_PER_HOUR;

    let mut days = t / SECONDS_PER_DAY;
    let mut year = 1970;
    loop {
        let n = if is_leap_year(year) { 366 } else { 365 };
        if days < n {
            break;
        }
        days -= n;
        year += 1;
    }

    let days_per_month = [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0;
    loop {
        let n = days_per_month[month as usize];
        if days < n {
            break;
        }
        days -= n;
        month += 1;
    }

    let seconds_remaining = t % SECONDS_PER_DAY;
    let hour = seconds_remaining / SECONDS_PER_HOUR;
    let minute = seconds_remaining / SECONDS_PER_MINUTE % SECONDS_PER_MINUTE;
    let second = seconds_remaining % SECONDS_PER_MINUTE;

    (year, month, days, hour, minute, second)
}

pub fn unix_time_to_tm(t: u64) -> libc::tm {
    let (year, month, day, hour, minute, second) = unix_time_components(t);
    libc::tm {
        tm_sec: second as i32,
        tm_min: minute as i32,
        tm_hour: hour as i32,
        tm_mday: (day + 1) as i32,
        tm_mon: month as i32,
        tm_year: (year - 1900) as i32,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    }
}

fn is_leap_year(year: u64) -> bool {
    year % 400 == 0 || (year % 4 == 0 && year % 100 != 0)
}

// This also tests now_local() and format_iso8601
#[test]
pub fn test_now_iso8601() {
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

#[test]
pub fn test_parse_date_and_time_no_tzo() {
    let t = parse_date_and_time_no_tzo("2024-10-31T11:17").unwrap();
    assert!(t.tm_year == 2024 - 1900 && t.tm_mon == 10 - 1 && t.tm_mday == 31);
    assert!(t.tm_hour == 11 && t.tm_min == 17);
    let t = parse_date_and_time_no_tzo("2022-07-01T23:59:14").unwrap();
    assert!(t.tm_year == 2022 - 1900 && t.tm_mon == 7 - 1 && t.tm_mday == 1);
    assert!(t.tm_hour == 23 && t.tm_min == 59 && t.tm_sec == 14);

    assert!(parse_date_and_time_no_tzo("1969-07-01T23:59:14").is_err());
    assert!(parse_date_and_time_no_tzo("2105-07-01T23:59:14").is_err());
    assert!(parse_date_and_time_no_tzo("202207-01T23:59:14").is_err());
    assert!(parse_date_and_time_no_tzo("2022-07-01T23:5914").is_err());
    assert!(parse_date_and_time_no_tzo("2022-07-01T2359").is_err());
    assert!(parse_date_and_time_no_tzo("2022-07-01T23:59+03:30").is_err());
}

#[test]
pub fn test_unix_time_components() {
    // Test case from StackOverflow
    // 08/18/2016 @ 2:41am (UTC)
    let t = 1471488076;
    let (year, month, day, hour, minute, second) = unix_time_components(t);
    assert!(year == 2016);
    assert!(month == 7);
    assert!(day == 17);
    assert!(hour == 2);
    assert!(minute == 41);
    assert!(second == 16);

    // Sanity
    let (year, month, day, hour, minute, second) = unix_time_components(0);
    assert!(year == 1970);
    assert!(month == 0);
    assert!(day == 0);
    assert!(hour == 0);
    assert!(minute == 0);
    assert!(second == 0);

    // Generate test cases with `date -u +%s-%FT%T` - this yields both the
    // second count and the date/time.

    // 1740568588-2025-02-26T11:16:28
    let t = 1740568588;
    let (year, month, day, hour, minute, second) = unix_time_components(t);
    assert!(year == 2025);
    assert!(month == 1); // zero-based
    assert!(day == 25); // zero-based
    assert!(hour == 11);
    assert!(minute == 16);
    assert!(second == 28);
}

#[test]
pub fn test_unix_time_to_tm() {
    let t = 1740568588;
    let s = format_iso8601(&unix_time_to_tm(t));
    assert!(s == "2025-02-26T11:16:28+00:00");
}
