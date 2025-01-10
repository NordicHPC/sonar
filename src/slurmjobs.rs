// Run sacct, extract output and reformat as CSV on stdout.

use crate::command;
use crate::log;
use crate::output;
use crate::time;

#[cfg(test)]
use std::cmp::min;
use std::collections::HashSet;
use std::io;

// Default sacct reporting window.  Note this value is baked into the help message in main.rs too.
const DEFAULT_WINDOW: u32 = 90;

// 3 minutes ought to be enough for anyone.
const TIMEOUT_S: u64 = 180;

// Same output format as sacctd, which uses this version number.
const VERSION: &str = "0.1.0";

pub fn show_slurm_jobs(
    writer: &mut dyn io::Write,
    window: &Option<u32>,
    span: &Option<String>,
    json: bool,
) {
    let (job_states, field_names) = parameters();

    // Parse the options to compute the time range to pass to sacct.
    let (from, to) = if let Some(s) = span {
        let components = s.split(',').collect::<Vec<&str>>();
        if components.len() != 2 || !check_ymd(components[0]) || !check_ymd(components[1]) {
            log::error(&format!("Bad --span: {}", s));
            return;
        }
        (components[0].to_string(), components[1].to_string())
    } else {
        let mut minutes = DEFAULT_WINDOW;
        if let Some(w) = window {
            minutes = *w;
        }
        (format!("now-{minutes}minutes"), "now".to_string())
    };

    // Run sacct and parse the output.
    match command::safe_command(
        "sacct",
        &[
            "-aP",
            "-s",
            &job_states.join(","),
            "--noheader",
            "-o",
            &field_names.join(","),
            "-S",
            &from,
            "-E",
            &to,
        ],
        TIMEOUT_S,
    ) {
        Err(e) => {
            log::error(&format!("sacct failed: {:?}", e));
        }
        Ok(sacct_output) => {
            let local = time::now_local();
            format_jobs(writer, &sacct_output, &field_names, &local, json);
        }
    }
}

// This is a dumb hack.  These arrays are global and shared between production and testing code, but
// we don't want to depend on lazy_static.

fn parameters() -> (Vec<&'static str>, Vec<&'static str>) {
    // The job states we are interested in collecting information about, notably RUNNING is not
    // here.
    let job_states = vec![
        "CANCELLED",
        "COMPLETED",
        "DEADLINE",
        "FAILED",
        "OUT_OF_MEMORY",
        "TIMEOUT",
    ];

    // The fields we want to extract.  We can just pile it on here, but it's unlikely that
    // everything is of interest, hence we select.  The capitalization shall be exactly as it is in
    // the sacct man page, even though sacct appears to ignore capitalization.
    let field_names = vec![
        "JobID",
        "JobIDRaw",
        "User",
        "Account",
        "State",
        "Start",
        "End",
        "AveCPU",
        "AveDiskRead",
        "AveDiskWrite",
        "AveRSS",
        "AveVMSize",
        "ElapsedRaw",
        "ExitCode",
        "Layout",
        "MaxRSS",
        "MaxVMSize",
        "MinCPU",
        "ReqCPUS",
        "ReqMem",
        "ReqNodes",
        "Reservation",
        "Submit",
        "Suspended",
        "SystemCPU",
        "TimelimitRaw",
        "UserCPU",
        "NodeList",
        "Partition",
        "AllocTRES",
        "Priority",
        // JobName must be last in case it contains `|`, code below will clean that up.
        "JobName",
    ];

    (job_states, field_names)
}

fn check_ymd(s: &str) -> bool {
    let mut k = 0;
    for f in s.split('-') {
        k += 1;
        if f.parse::<u32>().is_err() {
            return false;
        }
    }
    k == 3
}

fn format_jobs(
    writer: &mut dyn io::Write,
    sacct_output: &str,
    field_names: &[&str],
    local: &libc::tm,
    json: bool,
) {
    // Fields that are dates that may be reinterpreted before transmission.
    let date_fields = HashSet::from(["Start", "End", "Submit"]);

    // These fields may contain zero values that don't mean zero.
    let uncontrolled_fields = HashSet::from(["JobName", "Account", "User"]);

    // Zero values in "controlled" fields.
    let zero_values = HashSet::from(["Unknown", "0", "00:00:00", "0:0", "0.00M"]);

    // For csv, push out records individually; if we add "common" fields (such as error information)
    // they will piggyback on the first record, as does `load` for `ps`.
    //
    // For json, collect records in an array and then push out an envelope containing that array, as
    // this envelope can later be adapted to hold more fields.

    let mut jobs = output::Array::new();
    for line in sacct_output.lines() {
        let mut field_store = line.split('|').collect::<Vec<&str>>();

        // If there are more fields than field names then that's because the job name
        // contains `|`.  The JobName field always comes last.  Catenate excess fields until
        // we have the same number of fields and names.  (Could just ignore excess fields
        // instead.)
        let jobname = field_store[field_names.len() - 1..].join("");
        field_store[field_names.len() - 1] = &jobname;
        let fields = &field_store[..field_names.len()];

        let mut output_line = output::Object::new();
        if !json {
            output_line.push_s("v", VERSION.to_string());
        }
        for (i, name) in field_names.iter().enumerate() {
            let mut val = fields[i].to_string();
            let is_zero = val.is_empty()
                || (!uncontrolled_fields.contains(name) && zero_values.contains(val.as_str()));
            if !is_zero {
                if date_fields.contains(name) {
                    // The slurm date format is localtime without a time zone offset.  This
                    // is bound to lead to problems eventually, so reformat.  If parsing
                    // fails, just transmit the date and let the consumer deal with it.
                    if let Ok(mut t) = time::parse_date_and_time_no_tzo(&val) {
                        t.tm_gmtoff = local.tm_gmtoff;
                        t.tm_isdst = local.tm_isdst;
                        // If t.tm_zone is not null then it must point to static data, so
                        // copy it just to be safe.
                        t.tm_zone = local.tm_zone;
                        val = time::format_iso8601(&t).to_string()
                    }
                }
                output_line.push_s(name, val);
            }
        }
        if json {
            jobs.push_o(output_line);
        } else {
            output::write_csv(writer, &output::Value::O(output_line));
        }
    }
    if json {
        let mut envelope = output::Object::new();
        envelope.push_s("v", VERSION.to_string());
        envelope.push_a("jobs", jobs);
        output::write_json(writer, &output::Value::O(envelope));
    }
}

#[test]
pub fn test_format_jobs() {
    let (_, field_names) = parameters();

    // Actual sacct output from Fox, anonymized and with one command name replaced and Priority
    // added.
    let sacct_output = std::include_str!("testdata/sacct-output.txt");

    // The golang `sacctd` output for the above input, with Priority added.
    let expected = std::include_str!("testdata/sacctd-output.txt");

    let mut output = Vec::new();
    let mut local = time::now_local();
    // The output below depends on us being in UTC+01:00 and not in dst so mock that.
    local.tm_gmtoff = 3600;
    local.tm_isdst = 0;
    format_jobs(&mut output, sacct_output, &field_names, &local, false);
    if output != expected.as_bytes() {
        let xs = &output;
        let ys = expected.as_bytes();
        if xs.len() != ys.len() {
            print!(
                "Lengths differ: output={} expected={}\n",
                xs.len(),
                ys.len()
            );
        }
        for i in 0..min(xs.len(), ys.len()) {
            if xs[i] != ys[i] {
                print!(
                    "Text differs first at {i}: output={} expected={}\n",
                    xs[i], ys[i]
                );
                break;
            }
        }
        println!("{} {}", xs.len(), ys.len());
        if xs.len() > ys.len() {
            println!("{}", String::from_utf8_lossy(&xs[ys.len()..]));
        } else {
            println!("{}", String::from_utf8_lossy(&ys[xs.len()..]));
        }
        assert!(false);
    }
}
