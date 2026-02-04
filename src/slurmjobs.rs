#![allow(clippy::comparison_to_empty)]
#![allow(clippy::vec_box)]
#![allow(clippy::len_zero)]
#![allow(clippy::comparison_to_empty)]

use crate::json_tags::*;
use crate::nodelist;
use crate::output;
use crate::systemapi;
use crate::time;

use once_cell::sync::Lazy;

use std::collections::HashMap;
use std::io;

// The job states we are interested in collecting information about, notably PENDING and RUNNING
// are not here by default but will be added if the `uncompleted` flag is set.

static SACCT_STATES: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        "CANCELLED",
        "COMPLETED",
        "DEADLINE",
        "FAILED",
        "OUT_OF_MEMORY",
        "TIMEOUT",
    ]
});

// The fields we want to extract.  We can just pile it on here, but it's unlikely that
// everything is of interest, hence we select.  The capitalization shall be exactly as it is in
// the sacct man page, even though sacct appears to ignore capitalization.
//
// The order here MUST NOT change without updating both new and old formats and test cases.

static SACCT_FIELDS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
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
    ]
});

// The State is an object that wraps the slurm job data extractor, potentially taking advantage of
// repeated invocations to avoid overhead or sending redundant data.

#[cfg(feature = "daemon")]
pub struct State<'a> {
    window: Option<u32>,
    token: String,
    uncompleted: bool,
    batch_size: Option<usize>,
    system: &'a dyn systemapi::SystemAPI,
}

#[derive(Default)]
struct JobAll {
    job_id: u64,      // "true" job ID, see doc/NEW-FORMAT.md
    job_step: String, // "true" job step, ditto
    job_name: String,
    job_state: String,
    array_job_id: u64,
    array_task_id: u64,
    het_job_id: u64,
    het_job_offset: u64,
    user_name: String,
    account: String,
    submit_time: String,
    time_limit: u64,
    partition: String,
    reservation: String,
    nodes: Vec<String>,
    priority: u64,
    distribution: String,
    requested_resources: String,
    allocated_resources: String,
    requested_cpus: u64,
    requested_memory_per_node: u64,
    requested_node_count: u64,
    start_time: String,
    suspend_time: u64,
    end_time: String,
    exit_code: u64,
    sacct_min_cpu: u64,
    sacct_alloc_tres: String,
    sacct_ave_cpu: u64,
    sacct_ave_disk_read: u64,
    sacct_ave_disk_write: u64,
    sacct_ave_rss: u64,
    sacct_ave_vmsize: u64,
    sacct_elapsed_raw: u64,
    sacct_system_cpu: u64,
    sacct_user_cpu: u64,
    sacct_max_rss: u64,
    sacct_max_vmsize: u64,
}

#[cfg(feature = "daemon")]
impl<'a> State<'a> {
    pub fn new(
        window: Option<u32>, // Minutes
        uncompleted: bool,
        batch_size: Option<usize>,
        system: &'a dyn systemapi::SystemAPI,
        token: String,
    ) -> State<'a> {
        State {
            window,
            token,
            uncompleted,
            batch_size,
            system,
        }
    }

    pub fn run(&mut self) -> Vec<Vec<u8>> {
        show_slurm_jobs_newfmt(
            &self.window,
            &None,
            self.uncompleted,
            self.batch_size,
            self.system,
            self.token.clone(),
        )
    }
}

// Default sacct reporting window.  Note this value is baked into the help message in main.rs too.
const DEFAULT_WINDOW: u32 = 90;

pub enum Format {
    // There used to be CSV here before, we might add eg Protobuf
    JSON,
}

pub fn show_slurm_jobs(
    writer: &mut dyn io::Write,
    window: &Option<u32>,
    span: &Option<String>,
    uncompleted: bool,
    batch_size: Option<usize>,
    system: &dyn systemapi::SystemAPI,
    token: String,
    fmt: Format,
) {
    match fmt {
        Format::JSON => {
            let output =
                show_slurm_jobs_newfmt(window, span, uncompleted, batch_size, system, token);
            for o in output {
                let _ = writer.write(&o);
            }
        }
    }
}

pub fn show_slurm_jobs_newfmt(
    window: &Option<u32>,
    span: &Option<String>,
    uncompleted: bool,
    batch_size: Option<usize>,
    system: &dyn systemapi::SystemAPI,
    token: String,
) -> Vec<Vec<u8>> {
    let mut result = vec![];
    match collect_sacct_jobs_newfmt(system, window, span, uncompleted) {
        Ok(jobs) => {
            // This will push out a record even if the jobs array is empty.  It serves as a
            // heartbeat.
            let mut start = 0;
            loop {
                let xs = if let Some(n) = batch_size {
                    &jobs[start..std::cmp::min(start + n, jobs.len())]
                } else {
                    &jobs
                };
                start += xs.len();
                let mut envelope = output::newfmt_envelope(system, token.clone(), &[]);
                let (mut data, mut attrs) = output::newfmt_data(system, DATA_TAG_JOBS);
                attrs.push_a(JOBS_ATTRIBUTES_SLURM_JOBS, render_jobs_newfmt(xs));
                data.push_o(JOBS_DATA_ATTRIBUTES, attrs);
                envelope.push_o(JOBS_ENVELOPE_DATA, data);
                let mut writer = Vec::new();
                output::write_json(&mut writer, &output::Value::O(envelope));
                result.push(writer);
                if start == jobs.len() {
                    break;
                }
            }
        }
        Err(error) => {
            let mut envelope = output::newfmt_envelope(system, token, &[]);
            envelope.push_a(
                JOBS_ENVELOPE_ERRORS,
                output::newfmt_one_error(system, error),
            );
            let mut writer = Vec::new();
            output::write_json(&mut writer, &output::Value::O(envelope));
            result.push(writer);
        }
    }
    result
}

fn collect_sacct_jobs_newfmt(
    system: &dyn systemapi::SystemAPI,
    window: &Option<u32>,
    span: &Option<String>,
    uncompleted: bool,
) -> Result<Vec<Box<JobAll>>, String> {
    let local = time::now_local();
    let scontrol_output = system.run_scontrol()?;
    let resources = parse_scontrol_output(scontrol_output);
    let sacct_output = run_sacct(system, window, span, uncompleted)?;
    Ok(parse_sacct_jobs_newfmt(&sacct_output, &resources, &local))
}

fn parse_scontrol_output(scontrol_output: String) -> HashMap<String, String> {
    let mut resources = HashMap::<String, String>::new();
    for line in scontrol_output.lines() {
        let mut id = None;
        let mut res = None;
        'Fieldscan: for f in line.split(' ') {
            if let Some(s) = f.strip_prefix("JobId=") {
                id = Some(s.to_string());
                if res.is_some() {
                    break 'Fieldscan;
                }
            } else if let Some(s) = f.strip_prefix("ReqTRES=") {
                // Newer SLURM, notably v24
                res = Some(s.to_string());
                if id.is_some() {
                    break 'Fieldscan;
                }
            } else if let Some(s) = f.strip_prefix("TRES=") {
                // Older SLURM, notably v21
                res = Some(s.to_string());
                if id.is_some() {
                    break 'Fieldscan;
                }
            }
        }
        if let (Some(id), Some(res)) = (id, res) {
            resources.insert(id, res);
        }
    }
    resources
}

fn parse_sacct_jobs_newfmt(
    sacct_output: &str,
    requested_resources: &HashMap<String, String>,
    local: &libc::tm,
) -> Vec<Box<JobAll>> {
    let mut jobs = vec![];
    for line in sacct_output.lines() {
        // There are ways of making this table-driven but none that are not complicated.
        let fieldvals = compute_sacct_field_values(line);
        let mut output_line = Box::new(JobAll {
            ..Default::default()
        });
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
                        output_line.array_job_id = parse_uint(id);
                        let task = if let Some((task_id, _)) = task.split_once('.') {
                            task_id
                        } else {
                            task
                        };
                        output_line.array_task_id = parse_uint(task);
                    } else if let Some((id, offset)) = fieldvals[i].split_once('+') {
                        output_line.het_job_id = parse_uint(id);
                        let offset = if let Some((offset_id, _)) = offset.split_once('.') {
                            offset_id
                        } else {
                            offset
                        };
                        output_line.het_job_offset = parse_uint(offset);
                    }
                }
                "JobIDRaw" => {
                    let id = if let Some((id, step)) = fieldvals[i].split_once('.') {
                        output_line.job_id = parse_uint(id);
                        output_line.job_step = step.to_string();
                        id
                    } else {
                        output_line.job_id = parse_uint(&fieldvals[i]);
                        &fieldvals[i]
                    };
                    if let Some(r) = requested_resources.get(id) {
                        output_line.requested_resources = r.clone();
                    }
                }
                "User" => {
                    // User is empty for administrative records
                    output_line.user_name = fieldvals[i].clone();
                }
                "Account" => {
                    output_line.account = fieldvals[i].clone();
                }
                "State" => {
                    output_line.job_state = fieldvals[i].clone();
                }
                "Start" => {
                    output_line.start_time = parse_date(&fieldvals[i], local);
                }
                "End" => {
                    output_line.end_time = parse_date(&fieldvals[i], local);
                }
                "ExitCode" => {
                    if fieldvals[i] != "" {
                        // The format is code:signal
                        if let Some((code, _signal)) = fieldvals[i].split_once(':') {
                            output_line.exit_code = parse_uint(code);
                        }
                    }
                }
                "Layout" => {
                    output_line.distribution = fieldvals[i].to_string();
                }
                "ReqCPUS" => {
                    output_line.requested_cpus = parse_uint(&fieldvals[i]);
                }
                "ReqMem" => {
                    output_line.requested_memory_per_node = parse_volume_kb(&fieldvals[i]);
                }
                "ReqNodes" => {
                    output_line.requested_node_count = parse_uint(&fieldvals[i]);
                }
                "Reservation" => {
                    output_line.reservation = fieldvals[i].clone();
                }
                "Submit" => {
                    output_line.submit_time = parse_date(&fieldvals[i], local);
                }
                "Suspended" => {
                    output_line.suspend_time = parse_duration(&fieldvals[i]);
                }
                "TimelimitRaw" => {
                    output_line.time_limit = match fieldvals[i].as_str() {
                        "UNLIMITED" => EXTENDED_UINT_INFINITE,
                        "Partition_limit" => EXTENDED_UINT_UNSET,
                        limit => parse_uint_full(limit, 60, EXTENDED_UINT_BASE),
                    };
                }
                "NodeList" => {
                    if fieldvals[i] != "" {
                        if let Ok(nodes) = nodelist::parse(&fieldvals[i]) {
                            output_line.nodes = nodes;
                        }
                    }
                }
                "Partition" => {
                    output_line.partition = fieldvals[i].clone();
                }
                "Priority" => {
                    output_line.priority = parse_uint_full(&fieldvals[i], 1, EXTENDED_UINT_BASE);
                }
                "JobName" => {
                    output_line.job_name = fieldvals[i].clone();
                }

                "AveDiskRead" => {
                    output_line.sacct_ave_disk_read = parse_volume_kb(&fieldvals[i]);
                }
                "AveDiskWrite" => {
                    output_line.sacct_ave_disk_write = parse_volume_kb(&fieldvals[i]);
                }
                "AveRSS" => {
                    output_line.sacct_ave_rss = parse_volume_kb(&fieldvals[i]);
                }
                "AveVMSize" => {
                    output_line.sacct_ave_vmsize = parse_volume_kb(&fieldvals[i]);
                }
                "MaxRSS" => {
                    output_line.sacct_max_rss = parse_volume_kb(&fieldvals[i]);
                }
                "MaxVMSize" => {
                    output_line.sacct_max_vmsize = parse_volume_kb(&fieldvals[i]);
                }
                "AveCPU" => {
                    output_line.sacct_ave_cpu = parse_duration(&fieldvals[i]);
                }
                "MinCPU" => {
                    output_line.sacct_min_cpu = parse_duration(&fieldvals[i]);
                }
                "UserCPU" => {
                    output_line.sacct_user_cpu = parse_duration(&fieldvals[i]);
                }
                "SystemCPU" => {
                    output_line.sacct_system_cpu = parse_duration(&fieldvals[i]);
                }
                "ElapsedRaw" => {
                    output_line.sacct_elapsed_raw = parse_uint(&fieldvals[i]);
                }
                "AllocTRES" => {
                    output_line.sacct_alloc_tres = fieldvals[i].clone();
                    output_line.allocated_resources = fieldvals[i].clone();
                }
                _ => {
                    panic!("Bad field name {}", *field);
                }
            }
        }
        jobs.push(output_line);
    }
    jobs
}

fn parse_uint(val: &str) -> u64 {
    parse_uint_full(val, 1, 0)
}

fn parse_uint_full(val: &str, scale: u64, bias: u64) -> u64 {
    if val != "" {
        match val.parse::<u64>() {
            Ok(n) => {
                if n != 0 || bias != 0 {
                    bias + n * scale
                } else {
                    0
                }
            }
            Err(_) => 0,
        }
    } else {
        0
    }
}

fn parse_date(val: &str, local: &libc::tm) -> String {
    if val != "" && val != "Unknown" {
        // Reformat timestamps.  The slurm date format is localtime without a time zone offset.
        // This is bound to lead to problems eventually, so reformat with a time zone based on the
        // local time, which is the best available information.  (If parsing fails, just transmit
        // the date and let the consumer deal with it.)
        if let Ok(mut t) = time::parse_date_and_time_no_tzo(val) {
            t.tm_gmtoff = local.tm_gmtoff;
            t.tm_isdst = local.tm_isdst;
            // If t.tm_zone is not null then it must point to static data, so
            // copy it just to be safe.
            t.tm_zone = local.tm_zone;
            return time::format_iso8601(&t).to_string();
        }
    }
    "".to_string()
}

fn parse_duration(mut val: &str) -> u64 {
    // [DD-[hh:]]mm:ss
    let days = if let Some((dd, rest)) = val.split_once('-') {
        if let Ok(n) = dd.parse::<u64>() {
            val = rest;
            n
        } else {
            0
        }
    } else {
        0
    };
    let mut elts = val.split(':').collect::<Vec<&str>>();
    let mut hours = 0;
    if elts.len() == 3 {
        if let Ok(n) = elts[0].parse::<u64>() {
            elts.remove(0);
            hours = n;
        }
    }
    if elts.len() == 2 {
        let minutes = elts[0].parse::<u64>().unwrap_or(0);
        let seconds = elts[1].parse::<u64>().unwrap_or(0);
        days * (24 * 60 * 60) + hours * (60 * 60) + minutes * 60 + seconds
    } else {
        0
    }
}

fn parse_volume_kb(val: &str) -> u64 {
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
        (val.parse::<u64>().unwrap_or(0) * scale).div_ceil(1024)
    } else {
        0
    }
}

fn render_jobs_newfmt(jobs: &[Box<JobAll>]) -> output::Array {
    let mut a = output::Array::new();
    for j in jobs {
        let mut o = output::Object::new();
        push_uint(&mut o, SLURM_JOB_JOB_ID, j.job_id);
        push_string(&mut o, SLURM_JOB_JOB_STEP, j.job_step.clone());
        push_string_full(&mut o, SLURM_JOB_JOB_NAME, j.job_name.clone(), false);
        push_string(&mut o, SLURM_JOB_JOB_STATE, j.job_state.clone());
        push_uint(&mut o, SLURM_JOB_ARRAY_JOB_ID, j.array_job_id);
        push_uint(&mut o, SLURM_JOB_ARRAY_TASK_ID, j.array_task_id);
        push_uint(&mut o, SLURM_JOB_HET_JOB_ID, j.het_job_id);
        push_uint(&mut o, SLURM_JOB_HET_JOB_OFFSET, j.het_job_offset);
        push_string_full(&mut o, SLURM_JOB_USER_NAME, j.user_name.clone(), false);
        push_string_full(&mut o, SLURM_JOB_ACCOUNT, j.account.clone(), false);
        push_string(&mut o, SLURM_JOB_SUBMIT_TIME, j.submit_time.clone());
        push_uint(&mut o, SLURM_JOB_TIMELIMIT, j.time_limit);
        push_string(&mut o, SLURM_JOB_PARTITION, j.partition.clone());
        push_string_full(&mut o, SLURM_JOB_RESERVATION, j.reservation.clone(), false);
        if j.nodes.len() > 0 {
            let mut ns = output::Array::new();
            for n in &j.nodes {
                ns.push_s(n.clone());
            }
            o.push_a(SLURM_JOB_NODE_LIST, ns);
        }
        push_uint(&mut o, SLURM_JOB_PRIORITY, j.priority);
        push_string(&mut o, SLURM_JOB_LAYOUT, j.distribution.clone());
        push_string(&mut o, SLURM_JOB_REQ_TRES, j.requested_resources.clone());
        push_string(&mut o, SLURM_JOB_ALLOC_TRES, j.allocated_resources.clone());
        push_uint(&mut o, SLURM_JOB_REQ_CPUS, j.requested_cpus);
        push_uint(
            &mut o,
            SLURM_JOB_REQ_MEMORY_PER_NODE,
            j.requested_memory_per_node,
        );
        push_uint(&mut o, SLURM_JOB_REQ_NODES, j.requested_node_count);
        push_string(&mut o, SLURM_JOB_START, j.start_time.clone());
        push_uint(&mut o, SLURM_JOB_SUSPENDED, j.suspend_time);
        push_string(&mut o, SLURM_JOB_END, j.end_time.clone());
        push_uint(&mut o, SLURM_JOB_EXIT_CODE, j.exit_code);
        let mut s = output::Object::new();
        push_uint(&mut s, SACCT_DATA_MIN_CPU, j.sacct_min_cpu);
        push_string(&mut s, SACCT_DATA_ALLOC_TRES, j.sacct_alloc_tres.clone());
        push_uint(&mut s, SACCT_DATA_AVE_CPU, j.sacct_ave_cpu);
        push_uint(&mut s, SACCT_DATA_AVE_DISK_READ, j.sacct_ave_disk_read);
        push_uint(&mut s, SACCT_DATA_AVE_DISK_WRITE, j.sacct_ave_disk_write);
        push_uint(&mut s, SACCT_DATA_AVE_RSS, j.sacct_ave_rss);
        push_uint(&mut s, SACCT_DATA_AVE_VMSIZE, j.sacct_ave_vmsize);
        push_uint(&mut s, SACCT_DATA_ELAPSED_RAW, j.sacct_elapsed_raw);
        push_uint(&mut s, SACCT_DATA_SYSTEM_CPU, j.sacct_system_cpu);
        push_uint(&mut s, SACCT_DATA_USER_CPU, j.sacct_user_cpu);
        push_uint(&mut s, SACCT_DATA_MAX_RSS, j.sacct_max_rss);
        push_uint(&mut s, SACCT_DATA_MAX_VMSIZE, j.sacct_max_vmsize);
        if !s.is_empty() {
            o.push_o(SLURM_JOB_SACCT, s)
        }
        a.push_o(o);
    }
    a
}

fn push_uint(o: &mut output::Object, k: &str, v: u64) {
    if v != 0 {
        o.push_u(k, v);
    }
}

fn push_string(o: &mut output::Object, k: &str, v: String) {
    push_string_full(o, k, v, true);
}

fn push_string_full(o: &mut output::Object, k: &str, v: String, filter_unknown: bool) {
    if v != "" && (v != "Unknown" || filter_unknown) {
        o.push_s(k, v);
    }
}

fn compute_sacct_field_values(line: &str) -> Vec<String> {
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

fn run_sacct(
    system: &dyn systemapi::SystemAPI,
    window: &Option<u32>,
    span: &Option<String>,
    uncompleted: bool,
) -> Result<String, String> {
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
    let mut states = SACCT_STATES.clone();
    if uncompleted {
        states.push("PENDING");
        states.push("RUNNING");
    }
    system.run_sacct(&states, &SACCT_FIELDS, &from, &to)
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

// There is a test case that the "error" field is generated correctly in ../tests/slurm-no-sacct.sh.
