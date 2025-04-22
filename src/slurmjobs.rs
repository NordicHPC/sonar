// Run sacct, extract output and reformat as CSV or JSON on stdout.

use crate::json_tags::*;
use crate::nodelist;
use crate::output;
use crate::systemapi;
use crate::time;

use lazy_static::lazy_static;

#[cfg(test)]
use std::cmp::min;
use std::collections::HashSet;
use std::io;

lazy_static! {
    // The job states we are interested in collecting information about, notably PENDING and RUNNING
    // are not here by default but will be added if the --deluge option is given.

    //+ignore-strings
    static ref SACCT_STATES : Vec<&'static str> = vec![
        "CANCELLED",
        "COMPLETED",
        "DEADLINE",
        "FAILED",
        "OUT_OF_MEMORY",
        "TIMEOUT",
    ];
    //-ignore-strings

    // The fields we want to extract.  We can just pile it on here, but it's unlikely that
    // everything is of interest, hence we select.  The capitalization shall be exactly as it is in
    // the sacct man page, even though sacct appears to ignore capitalization.
    //
    // The order here MUST NOT change without updating both new and old formats and test cases.

    //+ignore-strings
    static ref SACCT_FIELDS : Vec<&'static str> = vec![
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
        // JobName must be last in case it contains `|`.
        "JobName",
    ];
    //-ignore-strings
}

// Default sacct reporting window.  Note this value is baked into the help message in main.rs too.
const DEFAULT_WINDOW: u32 = 90;

// Same output format as sacctd, which uses this version number.
const VERSION: &str = "0.1.0";

pub fn show_slurm_jobs(
    writer: &mut dyn io::Write,
    window: &Option<u32>,
    span: &Option<String>,
    deluge: bool,
    system: &dyn systemapi::SystemAPI,
    token: String,
    new_json: bool,
) {
    let embed_envelope = !new_json;
    match collect_sacct_jobs(system, window, span, deluge, embed_envelope) {
        Ok(jobs) => {
            if new_json {
                let mut envelope = output::newfmt_envelope(system, token, &[]);
                let (mut data, mut attrs) = output::newfmt_data(system, DATA_TAG_JOBS);
                attrs.push_a(JOBS_ATTRIBUTES_SLURM_JOBS, jobs);
                data.push_o(JOBS_DATA_ATTRIBUTES, attrs);
                envelope.push_o(JOBS_ENVELOPE_DATA, data);
                output::write_json(writer, &output::Value::O(envelope));
            } else {
                for i in 0..jobs.len() {
                    output::write_csv(writer, jobs.at(i));
                }
            }
        }
        Err(error) => {
            if new_json {
                let mut envelope = output::newfmt_envelope(system, token, &[]);
                envelope.push_a(
                    JOBS_ENVELOPE_ERRORS,
                    output::newfmt_one_error(system, error),
                );
                output::write_json(writer, &output::Value::O(envelope));
            } else {
                //+ignore-strings
                let mut envelope = output::Object::new();
                envelope.push_s("v", VERSION.to_string());
                envelope.push_s("error", error);
                envelope.push_s("timestamp", system.get_timestamp());
                output::write_csv(writer, &output::Value::O(envelope));
                //-ignore-strings
            }
        }
    }
}

// Run sacct, parse and collect data, and place in an array of jobs for output - it's the same array
// for the old and new formats.
//
// However, in the old "flat" CSV format, the "envelope" data (version, timestamp, error) are
// embedded in each record.  In collect_jobs() we only insert non-exceptional data, any errors are
// inserted into the first record before formatting, if they occur.

fn collect_sacct_jobs(
    system: &dyn systemapi::SystemAPI,
    window: &Option<u32>,
    span: &Option<String>,
    deluge: bool,
    embed_envelope: bool,
) -> Result<output::Array, String> {
    // Parse the options to compute the time range to pass to sacct.
    let (from, to) = if let Some(s) = span {
        let components = s.split(',').collect::<Vec<&str>>();
        if components.len() != 2 || !check_ymd(components[0]) || !check_ymd(components[1]) {
            return Err(format!("Bad --span: {}", s));
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
    let mut states = SACCT_STATES.clone();
    if deluge {
        states.push("PENDING");
        states.push("RUNNING");
    }
    match system.run_sacct(&states, &SACCT_FIELDS, &from, &to) {
        Ok(sacct_output) => {
            let local = time::now_local();
            if embed_envelope {
                Ok(parse_sacct_jobs_oldfmt(&sacct_output, &local))
            } else {
                Ok(parse_sacct_jobs_newfmt(&sacct_output, &local))
            }
        }
        Err(s) => Err(s),
    }
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

fn compute_field_values(line: &str) -> Vec<String> {
    let mut field_store = line.split('|').collect::<Vec<&str>>();

    // If there are more fields than field names then that's because the job name
    // contains `|`.  The JobName field always comes last.  Catenate excess fields until
    // we have the same number of fields and names.  (Could just ignore excess fields
    // instead.)
    let n = SACCT_FIELDS.len();
    let jobname = field_store[n - 1..].join("");
    field_store[n - 1] = &jobname;
    field_store[..n]
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<String>>()
}

//+ignore-strings
fn parse_sacct_jobs_oldfmt(sacct_output: &str, local: &libc::tm) -> output::Array {
    // Fields that are dates that may be reinterpreted before transmission.
    let date_fields = HashSet::from(["Start", "End", "Submit"]);

    // These fields may contain zero values that don't mean zero.
    let uncontrolled_fields = HashSet::from(["JobName", "Account", "User"]);

    // Zero values in "controlled" fields.
    let zero_values = HashSet::from(["Unknown", "0", "00:00:00", "0:0", "0.00M"]);

    // For csv, push out records individually; if we add "common" fields (such as error information)
    // they will piggyback on the first record, as does `load` for `ps`.

    let mut jobs = output::Array::new();
    for line in sacct_output.lines() {
        let fields = compute_field_values(line);

        let mut output_line = output::Object::new();
        output_line.push_s("v", VERSION.to_string());

        for (i, name) in SACCT_FIELDS.iter().enumerate() {
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
        jobs.push_o(output_line);
    }
    jobs
}
//-ignore-strings

fn parse_sacct_jobs_newfmt(sacct_output: &str, local: &libc::tm) -> output::Array {
    let mut jobs = output::Array::new();
    for line in sacct_output.lines() {
        // There are ways of making this table-driven but none that are not complicated.
        let fieldvals = compute_field_values(line);
        let mut output_line = output::Object::new();
        let mut sacct = output::Object::new();
        for (i, field) in SACCT_FIELDS.iter().enumerate() {
            // Ideally keep these in the order of the SACCT_FIELDS array, but I've pushed all the
            // cases for the sacct object to the end.
            match *field {
                "JobID" => {
                    // The format here is (\d+)(?:([.+])(\d+)(?:\.(.*)) where $1 is the job ID, $2
                    // is the separator, $3 is the task/offset ID, and $4 is the step name.  The
                    // separator gives us the job type and if not a normal job gives us the array or
                    // het job ID and task or offset values.
                    if let Some((id, task)) = fieldvals[i].split_once('_') {
                        push_uint(&mut output_line, SLURM_JOB_ARRAY_JOB_ID, id);
                        let task = if let Some((task_id, _)) = task.split_once('.') {
                            task_id
                        } else {
                            task
                        };
                        push_uint(&mut output_line, SLURM_JOB_ARRAY_TASK_ID, task);
                    } else if let Some((id, offset)) = fieldvals[i].split_once('+') {
                        push_uint(&mut output_line, SLURM_JOB_HET_JOB_ID, id);
                        let offset = if let Some((offset_id, _)) = offset.split_once('.') {
                            offset_id
                        } else {
                            offset
                        };
                        push_uint(&mut output_line, SLURM_JOB_HET_JOB_OFFSET, offset);
                    }
                }
                "JobIDRaw" => {
                    if let Some((id, step)) = fieldvals[i].split_once('.') {
                        push_uint(&mut output_line, SLURM_JOB_JOB_ID, id);
                        push_string(&mut output_line, SLURM_JOB_JOB_STEP, step);
                    } else {
                        push_uint(&mut output_line, SLURM_JOB_JOB_ID, &fieldvals[i]);
                    }
                }
                "User" => {
                    // User is empty for administrative records
                    push_string(&mut output_line, SLURM_JOB_USER_NAME, &fieldvals[i]);
                }
                "Account" => {
                    output_line.push_s(SLURM_JOB_ACCOUNT, fieldvals[i].clone());
                }
                "State" => {
                    output_line.push_s(SLURM_JOB_JOB_STATE, fieldvals[i].clone());
                }
                "Start" => {
                    push_date(&mut output_line, SLURM_JOB_START, &fieldvals[i], local);
                }
                "End" => {
                    push_date(&mut output_line, SLURM_JOB_END, &fieldvals[i], local);
                }
                "ExitCode" => {
                    if fieldvals[i] != "" {
                        // The format is code:signal
                        if let Some((code, _signal)) = fieldvals[i].split_once(':') {
                            push_uint(&mut output_line, SLURM_JOB_EXIT_CODE, code);
                        }
                    }
                }
                "Layout" => {
                    push_string(&mut output_line, SLURM_JOB_LAYOUT, &fieldvals[i]);
                }
                "ReqCPUS" => {
                    push_uint(&mut output_line, SLURM_JOB_REQ_CPUS, &fieldvals[i]);
                }
                "ReqMem" => {
                    push_uint(
                        &mut output_line,
                        SLURM_JOB_REQ_MEMORY_PER_NODE,
                        &fieldvals[i],
                    );
                }
                "ReqNodes" => {
                    push_uint(&mut output_line, SLURM_JOB_REQ_NODES, &fieldvals[i]);
                }
                "Reservation" => {
                    if fieldvals[i] != "" {
                        output_line.push_s(SLURM_JOB_RESERVATION, fieldvals[i].clone());
                    }
                }
                "Submit" => {
                    push_date(
                        &mut output_line,
                        SLURM_JOB_SUBMIT_TIME,
                        &fieldvals[i],
                        local,
                    );
                }
                "Suspended" => {
                    push_duration(&mut output_line, SLURM_JOB_SUSPENDED, &fieldvals[i]);
                }
                "TimelimitRaw" => {
                    // Just make sure this name is referenced
                    assert!(EXTENDED_UINT_UNSET == 0);
                    if fieldvals[i] == "UNLIMITED" {
                        output_line.push_u(SLURM_JOB_TIMELIMIT, EXTENDED_UINT_INFINITE);
                    } else if fieldvals[i] != "Partition_limit" {
                        push_uint_full(
                            &mut output_line,
                            SLURM_JOB_TIMELIMIT,
                            &fieldvals[i],
                            60,
                            EXTENDED_UINT_BASE,
                            false,
                        );
                    }
                }
                "NodeList" => {
                    if fieldvals[i] != "" {
                        if let Ok(nodes) = nodelist::parse_and_render(&fieldvals[i]) {
                            output_line.push_a(SLURM_JOB_NODE_LIST, nodes);
                        }
                    }
                }
                "Partition" => {
                    push_string(&mut output_line, SLURM_JOB_PARTITION, &fieldvals[i]);
                }
                "Priority" => {
                    push_uint_full(
                        &mut output_line,
                        SLURM_JOB_PRIORITY,
                        &fieldvals[i],
                        1,
                        EXTENDED_UINT_BASE,
                        false,
                    );
                }
                "JobName" => {
                    output_line.push_s(SLURM_JOB_JOB_NAME, fieldvals[i].clone());
                }

                // Sacct fields
                "AveDiskRead" | "AveDiskWrite" | "AveRSS" | "AveVMSize" | "MaxRSS"
                | "MaxVMSize" => {
                    // NOTE - tags are the same as the fields
                    assert!(SACCT_DATA_AVE_DISK_READ == "AveDiskRead");
                    assert!(SACCT_DATA_AVE_DISK_WRITE == "AveDiskWrite");
                    assert!(SACCT_DATA_AVE_RSS == "AveRSS");
                    assert!(SACCT_DATA_AVE_VMSIZE == "AveVMSize");
                    assert!(SACCT_DATA_MAX_RSS == "MaxRSS");
                    assert!(SACCT_DATA_MAX_VMSIZE == "MaxVMSize");
                    push_volume(&mut sacct, field, &fieldvals[i]);
                }
                "AveCPU" | "MinCPU" | "UserCPU" | "SystemCPU" => {
                    // NOTE - tags are the same as the fields
                    assert!(SACCT_DATA_AVE_CPU == "AveCPU");
                    assert!(SACCT_DATA_MIN_CPU == "MinCPU");
                    assert!(SACCT_DATA_USER_CPU == "UserCPU");
                    assert!(SACCT_DATA_SYSTEM_CPU == "SystemCPU");
                    push_duration(&mut sacct, field, &fieldvals[i]);
                }
                "ElapsedRaw" => {
                    // NOTE - tags are the same as the fields
                    assert!(SACCT_DATA_ELAPSED_RAW == "ElapsedRaw");
                    push_uint(&mut sacct, field, &fieldvals[i]);
                }
                "AllocTRES" => {
                    // NOTE - tags are the same as the fields
                    assert!(SACCT_DATA_ALLOC_TRES == "AllocTRES");
                    push_string(&mut sacct, field, &fieldvals[i]);
                }

                _ => {
                    // A hack to make sure these names are used
                    assert!(SLURM_JOB_GRESDETAIL != "zappa");
                    assert!(SLURM_JOB_MIN_CPUSPER_NODE != "zappa");
                    panic!("Bad field name");
                }
            }
        }
        if !sacct.is_empty() {
            output_line.push_o(SLURM_JOB_SACCT, sacct);
        }
        jobs.push_o(output_line);
    }
    jobs
}

fn push_uint(obj: &mut output::Object, name: &str, val: &str) {
    push_uint_full(obj, name, val, 1, 0, false);
}

fn push_uint_full(
    obj: &mut output::Object,
    name: &str,
    val: &str,
    scale: u64,
    bias: u64,
    always: bool,
) {
    if val != "" {
        if let Ok(n) = val.parse::<u64>() {
            if n != 0 || bias != 0 || always {
                obj.push_u(name, bias + n * scale);
            }
        }
    }
}

fn push_duration(obj: &mut output::Object, name: &str, mut val: &str) {
    // [DD-[hh:]]mm:ss
    let days = if let Some((dd, rest)) = val.split_once('-') {
        if let Ok(n) = dd.parse::<u64>() {
            val = rest;
            n
        } else {
            return;
        }
    } else {
        0
    };
    let mut elts = val.split(':').collect::<Vec<&str>>();
    let hours = if elts.len() == 3 {
        if let Ok(n) = elts[0].parse::<u64>() {
            elts.remove(0);
            n
        } else {
            return;
        }
    } else {
        0
    };
    let minutes = if let Ok(n) = elts[0].parse::<u64>() {
        n
    } else {
        return;
    };
    let seconds = if let Ok(n) = elts[1].parse::<u64>() {
        n
    } else {
        return;
    };
    let t = days * (24 * 60 * 60) + hours * (60 * 60) + minutes * 60 + seconds;
    if t != 0 {
        obj.push_u(name, t);
    }
}

fn push_volume(obj: &mut output::Object, name: &str, val: &str) {
    if val != "" {
        let (val, scale) = if let Some(suffix) = val.strip_suffix('K') {
            (suffix, 1024)
        } else if let Some(suffix) = val.strip_suffix('M') {
            (suffix, 1024 * 1024)
        } else if let Some(suffix) = val.strip_suffix('G') {
            (suffix, 1024 * 1024 * 1024)
        } else {
            (val, 1)
        };
        if let Ok(n) = val.parse::<u64>() {
            if n != 0 {
                obj.push_u(name, n * scale);
            }
        }
    }
}

fn push_string(obj: &mut output::Object, name: &str, val: &str) {
    if val != "" && val != "Unknown" {
        obj.push_s(name, val.to_string())
    }
}

fn push_date(obj: &mut output::Object, name: &str, val: &str, local: &libc::tm) {
    if val != "" && val != "Unknown" {
        // Reformat timestamps.  The slurm date format is localtime without a time zone offset.
        // This is bound to lead to problems eventually, so reformat.  If parsing fails, just
        // transmit the date and let the consumer deal with it.
        if let Ok(mut t) = time::parse_date_and_time_no_tzo(val) {
            t.tm_gmtoff = local.tm_gmtoff;
            t.tm_isdst = local.tm_isdst;
            // If t.tm_zone is not null then it must point to static data, so
            // copy it just to be safe.
            t.tm_zone = local.tm_zone;
            obj.push_s(name, time::format_iso8601(&t).to_string());
        }
    }
}

// There is a test case that the "error" field is generated correctly in ../tests/slurm-no-sacct.sh.

// Test that known sacct output is formatted correctly.
#[test]
pub fn test_format_sacct_jobs() {
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
    let jobs = parse_sacct_jobs_oldfmt(sacct_output, &local);
    for i in 0..jobs.len() {
        output::write_csv(&mut output, jobs.at(i));
    }
    if output != expected.as_bytes() {
        let xs = &output;
        let ys = expected.as_bytes();
        if xs.len() != ys.len() {
            println!(
                "Lengths differ: output={} expected={}",
                xs.len(),
                ys.len()
            );
        }
        for i in 0..min(xs.len(), ys.len()) {
            if xs[i] != ys[i] {
                println!(
                    "Text differs first at {i}: output={} expected={}",
                    xs[i], ys[i]
                );
                break;
            }
        }
        println!("{} {}", xs.len(), ys.len());
        if xs.len() > ys.len() {
            println!(
                "`output` tail = {}",
                String::from_utf8_lossy(&xs[ys.len()..])
            );
        } else {
            println!(
                "`expected` tail = {}",
                String::from_utf8_lossy(&ys[xs.len()..])
            );
        }
        assert!(false);
    }
}
