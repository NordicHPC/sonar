// Run sacct, extract output and reformat as CSV or JSON on stdout.

use crate::json_tags::*;
use crate::nodelist;
use crate::output;
use crate::systemapi;
use crate::time;

use lazy_static::lazy_static;

#[cfg(test)]
use std::cmp::min;
#[cfg(feature = "daemon")]
use std::collections::HashMap;
use std::collections::HashSet;
use std::io;

lazy_static! {
    // The job states we are interested in collecting information about, notably PENDING and RUNNING
    // are not here by default but will be added if the `uncompleted` flag is set.

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

// The Jobber is an object that wraps the slurm job data extractor, but it adds state that
// implements a redundancy filter so that the daemon only needs to send data that have changed.  Not
// doing the filtering can result in a deluge of data being sent, especially for pending and running
// jobs.
//
// This is currently a prototype of that idea.
//
// - we need to determine what it means for something to be redundant.  These current output fields
//   seem relevant, if the job is in running or pending state:
//
//    - job_state (so, we push running after pending)
//    - requested_cpus
//    - requested_memory_per_node
//    - requested_node_count
//    - distribution
//    - time limit
//    - priority
//    - gres_detail or other resource stuff
//
// - Really anything that can be changed with scontrol or similar might be relevant, so if eg
//   reservation or account can be changed then yes, those too
//
// - fields that are *not* relevant might include
//    - job ids (all fields)
//    - user name
//    - account name
//    - reservation
//    - end
//    - exit code
//    - submit time
//    - suspended time (i think - discuss)
//    - partition
//    - node list
//    - job name?
//    - anything in the sacct subobject *including* elapsed time
//
// We want to do two things:
//
// - determine if any pertinent fields have changed
// - determine which pertient fields have changed
//
// We transmit only identifying data + pertinent fields that have changed.
//
// For this to work, this object holds job_state and a copy of all pertinent fields.
//
// If the job is not known to us then all fields are sent and the job is recorded with pertinent
// fields.
//
// If the job is known, then we check pertinent fields for equality and send the ones that have
// changed, if any (with identifying fields).
//
// Once a job reaches the completed stage we would expect for nothing to change, but we have to
// garbage collect the table every so often.  This GC can run off a timer or as part of the Jobber's
// normal workload.
//
// The best option for GC is probably that once we *stop* seeing data for a table entry *and* the
// table entry is in a completed state we can remove it.  To do this, every table entry can have a
// mark.  Suppose the mark is initially zero.  When we run sacct, we set the mark for every job we
// see information about.  Entries whose mark is clean will not have been seen.
//
// We might trigger this mark/sweep collector every hour, say, simply by comparing a timestamp to
// the previous time it was run - no timer is needed, it's driven entirely by the cadence of normal
// work.

#[cfg(feature = "daemon")]
pub struct Jobber<'a> {
    window: Option<u32>,
    span: Option<String>,
    uncompleted: bool,
    delta_coding: bool,
    system: &'a dyn systemapi::SystemAPI,
    // Map from (id, step) to data
    known: HashMap<(u64, String), Box<JobAll>>,
    // Earliest time of next garbage collection, UTC time in seconds
    next_collection: u64,
    // The delta we use for computing next collection
    delta: u64,
}

#[derive(Default, Clone)]
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
    gres_detail: String,
    requested_cpus: u64,
    minimum_cpus_per_node: u64,
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
impl<'a> Jobber<'a> {
    pub fn new(
        window: &Option<u32>, // Minutes
        span: &Option<String>,
        uncompleted: bool,
        delta_coding: bool,
        system: &'a dyn systemapi::SystemAPI,
    ) -> Jobber<'a> {
        let delta = if let Some(w) = window {
            std::cmp::min(3600, w * 60 * 2) as u64
        } else {
            3600
        };
        Jobber {
            window: *window,
            span: span.clone(),
            uncompleted,
            delta_coding,
            system,
            known: HashMap::new(),
            next_collection: time::unix_now() + delta,
            delta,
        }
    }

    pub fn show_jobs(&mut self, token: String, writer: &mut dyn io::Write) {
        match collect_sacct_jobs_newfmt(self.system, &self.window, &self.span, self.uncompleted) {
            Ok(mut jobs) => {
                if self.delta_coding {
                    jobs = self.filter_jobs(jobs);
                }

                // This will push out a record even if the jobs array is empty.  This is probably
                // the right thing, it serves as a heartbeat.
                let mut envelope = output::newfmt_envelope(self.system, token, &[]);
                let (mut data, mut attrs) = output::newfmt_data(self.system, DATA_TAG_JOBS);
                attrs.push_a(JOBS_ATTRIBUTES_SLURM_JOBS, render_jobs_newfmt(jobs));
                data.push_o(JOBS_DATA_ATTRIBUTES, attrs);
                envelope.push_o(JOBS_ENVELOPE_DATA, data);
                output::write_json(writer, &output::Value::O(envelope));
            }
            Err(error) => {
                let mut envelope = output::newfmt_envelope(self.system, token, &[]);
                envelope.push_a(
                    JOBS_ENVELOPE_ERRORS,
                    output::newfmt_one_error(self.system, error),
                );
                output::write_json(writer, &output::Value::O(envelope));
            }
        }
    }

    fn filter_jobs(&mut self, jobs: Vec<Box<JobAll>>) -> Vec<Box<JobAll>> {
        let mut new_jobs = vec![];
        let mut marked = HashSet::new();
        let collecting = time::unix_now() >= self.next_collection;
        for j in jobs {
            let key = (j.job_id, j.job_step.clone());
            if collecting {
                marked.insert(key.clone());
            }
            let (pertinent, found) = match self.known.get(&key) {
                Some(p) => (
                    p.job_state != j.job_state
                        || p.requested_cpus != j.requested_cpus
                        || p.requested_memory_per_node != j.requested_memory_per_node
                        || p.requested_node_count != j.requested_node_count
                        || p.distribution != j.distribution
                        || p.time_limit != j.time_limit
                        || p.priority != j.priority
                        || p.gres_detail != j.gres_detail,
                    true,
                ),
                None => (true, false),
            };
            if !pertinent {
                continue;
            }
            let mut new_j = j.clone();
            if found {
                // Here we must first update new_j so that redundant data are not transmitted,
                // by clearing fields that have not changed.
                let prev = self.known.get(&key).unwrap();

                // In the interest of brevity, this makes some assumptions about what can change
                new_j.job_name = "".to_string();
                if prev.job_state == j.job_state {
                    new_j.job_state = "".to_string();
                }
                new_j.user_name = "".to_string();
                new_j.account = "".to_string();
                new_j.submit_time = "".to_string();
                if prev.time_limit == j.time_limit {
                    new_j.time_limit = 0;
                }
                new_j.partition = "".to_string();
                new_j.reservation = "".to_string();
                new_j.nodes = vec![];
                if prev.priority == j.priority {
                    new_j.priority = 0;
                }
                if prev.distribution == j.distribution {
                    new_j.distribution = "".to_string();
                }
                if prev.gres_detail == j.gres_detail {
                    new_j.gres_detail = "".to_string();
                }
                if prev.minimum_cpus_per_node == j.minimum_cpus_per_node {
                    new_j.minimum_cpus_per_node = 0;
                }
                if prev.requested_memory_per_node == j.requested_memory_per_node {
                    new_j.requested_memory_per_node = 0;
                }
                if prev.requested_node_count == j.requested_node_count {
                    new_j.requested_node_count = 0;
                }
                if prev.start_time == j.start_time {
                    new_j.start_time = "".to_string();
                }
                if prev.suspend_time == j.suspend_time {
                    new_j.suspend_time = 0;
                }
                if prev.end_time == j.end_time {
                    new_j.end_time = "".to_string();
                }
                if prev.exit_code == j.exit_code {
                    new_j.exit_code = 0;
                }
                if prev.sacct_alloc_tres == j.sacct_alloc_tres {
                    new_j.sacct_alloc_tres = "".to_string();
                }
            }

            // Expose these only for completed states
            if !is_completed_state(&j.job_state) {
                new_j.sacct_min_cpu = 0;
                new_j.sacct_ave_cpu = 0;
                new_j.sacct_ave_disk_read = 0;
                new_j.sacct_ave_disk_write = 0;
                new_j.sacct_ave_rss = 0;
                new_j.sacct_ave_vmsize = 0;
                new_j.sacct_elapsed_raw = 0;
                new_j.sacct_system_cpu = 0;
                new_j.sacct_user_cpu = 0;
                new_j.sacct_max_rss = 0;
                new_j.sacct_max_vmsize = 0;
            }

            // Whether found or not, store the new object as the known object for the job.
            self.known.insert(key, j);

            new_jobs.push(new_j);
        }
        if collecting {
            let mut old_known = HashMap::new();
            std::mem::swap(&mut old_known, &mut self.known);
            for (k, v) in old_known {
                if marked.contains(&k) || !is_completed_state(&v.job_state) {
                    self.known.insert(k, v);
                }
            }
            self.next_collection = time::unix_now() + self.delta;
        }
        new_jobs
    }
}

fn is_completed_state(state: &str) -> bool {
    !(state == "PENDING" || state == "RUNNING")
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
    if new_json {
        show_slurm_jobs_newfmt(writer, window, span, deluge, system, token);
    } else {
        show_slurm_jobs_oldfmt(writer, window, span, system);
    }
}

pub fn show_slurm_jobs_newfmt(
    writer: &mut dyn io::Write,
    window: &Option<u32>,
    span: &Option<String>,
    deluge: bool,
    system: &dyn systemapi::SystemAPI,
    token: String,
) {
    match collect_sacct_jobs_newfmt(system, window, span, deluge) {
        Ok(jobs) => {
            let mut envelope = output::newfmt_envelope(system, token, &[]);
            let (mut data, mut attrs) = output::newfmt_data(system, DATA_TAG_JOBS);
            attrs.push_a(JOBS_ATTRIBUTES_SLURM_JOBS, render_jobs_newfmt(jobs));
            data.push_o(JOBS_DATA_ATTRIBUTES, attrs);
            envelope.push_o(JOBS_ENVELOPE_DATA, data);
            output::write_json(writer, &output::Value::O(envelope));
        }
        Err(error) => {
            let mut envelope = output::newfmt_envelope(system, token, &[]);
            envelope.push_a(
                JOBS_ENVELOPE_ERRORS,
                output::newfmt_one_error(system, error),
            );
            output::write_json(writer, &output::Value::O(envelope));
        }
    }
}

fn collect_sacct_jobs_newfmt(
    system: &dyn systemapi::SystemAPI,
    window: &Option<u32>,
    span: &Option<String>,
    deluge: bool,
) -> Result<Vec<Box<JobAll>>, String> {
    let local = time::now_local();
    let sacct_output = run_sacct(system, window, span, deluge)?;
    Ok(parse_sacct_jobs_newfmt(&sacct_output, &local))
}

fn parse_sacct_jobs_newfmt(sacct_output: &str, local: &libc::tm) -> Vec<Box<JobAll>> {
    let mut jobs = vec![];
    for line in sacct_output.lines() {
        // There are ways of making this table-driven but none that are not complicated.
        let fieldvals = compute_field_values(line);
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
                    if let Some((id, step)) = fieldvals[i].split_once('.') {
                        output_line.job_id = parse_uint(id);
                        output_line.job_step = step.to_string();
                    } else {
                        output_line.job_id = parse_uint(&fieldvals[i]);
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
                    output_line.requested_memory_per_node = parse_uint(&fieldvals[i]);
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
                    output_line.sacct_ave_disk_read = parse_volume(&fieldvals[i]);
                }
                "AveDiskWrite" => {
                    output_line.sacct_ave_disk_write = parse_volume(&fieldvals[i]);
                }
                "AveRSS" => {
                    output_line.sacct_ave_rss = parse_volume(&fieldvals[i]);
                }
                "AveVMSize" => {
                    output_line.sacct_ave_vmsize = parse_volume(&fieldvals[i]);
                }
                "MaxRSS" => {
                    output_line.sacct_max_rss = parse_volume(&fieldvals[i]);
                }
                "MaxVMSize" => {
                    output_line.sacct_max_vmsize = parse_volume(&fieldvals[i]);
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
        // This is bound to lead to problems eventually, so reformat.  If parsing fails, just
        // transmit the date and let the consumer deal with it.
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

fn parse_volume(val: &str) -> u64 {
    if val != "" {
        let (val, _scale) = if let Some(suffix) = val.strip_suffix('K') {
            (suffix, 1024)
        } else if let Some(suffix) = val.strip_suffix('M') {
            (suffix, 1024 * 1024)
        } else if let Some(suffix) = val.strip_suffix('G') {
            (suffix, 1024 * 1024 * 1024)
        } else {
            (val, 1)
        };
        val.parse::<u64>().unwrap_or(0)
    } else {
        0
    }
}

fn render_jobs_newfmt(jobs: Vec<Box<JobAll>>) -> output::Array {
    let mut a = output::Array::new();
    for j in jobs {
        let mut o = output::Object::new();
        push_uint(&mut o, SLURM_JOB_JOB_ID, j.job_id);
        push_string(&mut o, SLURM_JOB_JOB_STEP, j.job_step);
        push_string_full(&mut o, SLURM_JOB_JOB_NAME, j.job_name, false);
        push_string(&mut o, SLURM_JOB_JOB_STATE, j.job_state);
        push_uint(&mut o, SLURM_JOB_ARRAY_JOB_ID, j.array_job_id);
        push_uint(&mut o, SLURM_JOB_ARRAY_TASK_ID, j.array_task_id);
        push_uint(&mut o, SLURM_JOB_HET_JOB_ID, j.het_job_id);
        push_uint(&mut o, SLURM_JOB_HET_JOB_OFFSET, j.het_job_offset);
        push_string_full(&mut o, SLURM_JOB_USER_NAME, j.user_name, false);
        push_string_full(&mut o, SLURM_JOB_ACCOUNT, j.account, false);
        push_string(&mut o, SLURM_JOB_SUBMIT_TIME, j.submit_time);
        push_uint(&mut o, SLURM_JOB_TIMELIMIT, j.time_limit);
        push_string(&mut o, SLURM_JOB_PARTITION, j.partition);
        push_string_full(&mut o, SLURM_JOB_RESERVATION, j.reservation, false);
        if j.nodes.len() > 0 {
            let mut ns = output::Array::new();
            for n in j.nodes {
                ns.push_s(n);
            }
            o.push_a(SLURM_JOB_NODE_LIST, ns);
        }
        push_uint(&mut o, SLURM_JOB_PRIORITY, j.priority);
        push_string(&mut o, SLURM_JOB_LAYOUT, j.distribution);
        push_string(&mut o, SLURM_JOB_GRESDETAIL, j.gres_detail);
        push_uint(&mut o, SLURM_JOB_REQ_CPUS, j.requested_cpus);
        push_uint(&mut o, SLURM_JOB_MIN_CPUSPER_NODE, j.minimum_cpus_per_node);
        push_uint(
            &mut o,
            SLURM_JOB_REQ_MEMORY_PER_NODE,
            j.requested_memory_per_node,
        );
        push_uint(&mut o, SLURM_JOB_REQ_NODES, j.requested_node_count);
        push_string(&mut o, SLURM_JOB_START, j.start_time);
        push_uint(&mut o, SLURM_JOB_SUSPENDED, j.suspend_time);
        push_string(&mut o, SLURM_JOB_END, j.end_time);
        push_uint(&mut o, SLURM_JOB_EXIT_CODE, j.exit_code);
        let mut s = output::Object::new();
        push_uint(&mut s, SACCT_DATA_MIN_CPU, j.sacct_min_cpu);
        push_string(&mut s, SACCT_DATA_ALLOC_TRES, j.sacct_alloc_tres);
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

//+ignore-strings
pub fn show_slurm_jobs_oldfmt(
    writer: &mut dyn io::Write,
    window: &Option<u32>,
    span: &Option<String>,
    system: &dyn systemapi::SystemAPI,
) {
    match collect_sacct_jobs_oldfmt(system, window, span) {
        Ok(jobs) => {
            for i in 0..jobs.len() {
                output::write_csv(writer, jobs.at(i));
            }
        }
        Err(error) => {
            let mut envelope = output::Object::new();
            envelope.push_s("v", VERSION.to_string());
            envelope.push_s("error", error);
            envelope.push_s("timestamp", system.get_timestamp());
            output::write_csv(writer, &output::Value::O(envelope));
        }
    }
}

fn collect_sacct_jobs_oldfmt(
    system: &dyn systemapi::SystemAPI,
    window: &Option<u32>,
    span: &Option<String>,
) -> Result<output::Array, String> {
    let local = time::now_local();
    let sacct_output = run_sacct(system, window, span, false)?;
    Ok(parse_sacct_jobs_oldfmt(&sacct_output, &local))
}

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

fn run_sacct(
    system: &dyn systemapi::SystemAPI,
    window: &Option<u32>,
    span: &Option<String>,
    deluge: bool,
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
    if deluge {
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
