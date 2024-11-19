// Run sacct, extract output and reformat as CSV on stdout.

use crate::command;
use crate::log;
use crate::time;
use crate::util;

use std::collections::HashSet;
use std::io;

// Default sacct reporting window.  Note this value is baked into the help message in main.rs too.
const DEFAULT_WINDOW: u32 = 90;

// 3 minutes ought to be enough for anyone.
const TIMEOUT_S: u64 = 180;

// Same output format as sacctd, which uses this version number.
const VERSION: &str = "0.1.0";

pub fn show_slurm_jobs(window: &Option<u32>, span: &Option<String>) {
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
        &vec![
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
            let mut writer = io::stdout();
            let local = time::now_local();
            format_jobs(&mut writer, &sacct_output, &field_names, &local);
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
    return k == 3;
}

fn format_jobs(
    writer: &mut dyn io::Write,
    sacct_output: &str,
    field_names: &[&str],
    local: &libc::tm,
) {
    // Fields that are dates that may be reinterpreted before transmission.
    let date_fields = HashSet::from(["Start", "End", "Submit"]);

    // These fields may contain zero values that don't mean zero.
    let uncontrolled_fields = HashSet::from(["JobName", "Account", "User"]);

    // Zero values in "controlled" fields.
    let zero_values = HashSet::from(["Unknown", "0", "00:00:00", "0:0", "0.00M"]);

    for line in sacct_output.lines() {
        let mut field_store = line.split('|').collect::<Vec<&str>>();

        // If there are more fields than field names then that's because the job name
        // contains `|`.  The JobName field always comes last.  Catenate excess fields until
        // we have the same number of fields and names.  (Could just ignore excess fields
        // instead.)
        let jobname = field_store[field_names.len() - 1..].join("");
        field_store[field_names.len() - 1] = &jobname;
        let fields = &field_store[..field_names.len()];

        let mut output_line = "v=".to_string() + VERSION;
        for (i, name) in field_names.iter().enumerate() {
            let mut val = fields[i].to_string();
            let is_zero = val == ""
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
                output_line += ",";
                output_line += &util::csv_quote(&(name.to_string() + "=" + &val));
            }
        }
        output_line += "\n";
        let _ = writer.write(output_line.as_bytes());
    }
}

#[test]
pub fn test_format_jobs() {
    let (_, field_names) = parameters();

    // Actual sacct output from Fox, anonymized and with one command name replaced.
    let sacct_output = r#"973821|973821|ec-aaaaa|ec85|COMPLETED|2024-11-13T11:08:00|2024-11-13T13:07:24||||||7164|0:0|||||6|10000M|1||2024-11-13T08:30:40|00:00:00|22:53.824|400|11:06:33|c1-28|normal|billing=6,cpu=6,mem=10000M,node=1|command
973821.batch|973821.batch||ec85|COMPLETED|2024-11-13T11:08:00|2024-11-13T13:07:24|00:00:03|51.54M|0.30M|112488K|0|7164|0:0|Unknown|112488K|0|00:00:03|6||1||2024-11-13T11:08:00|00:00:00|00:01.062||00:02.806|c1-28||cpu=6,mem=10000M,node=1|batch
973821.extern|973821.extern||ec85|COMPLETED|2024-11-13T11:08:00|2024-11-13T13:07:24|00:00:00|0.01M|0.00M|0|0|7164|0:0|Unknown|0|0|00:00:00|6||1||2024-11-13T11:08:00|00:00:00|00:00:00||00:00:00|c1-28||billing=6,cpu=6,mem=10000M,node=1|extern
973821.0|973821.0||ec85|COMPLETED|2024-11-13T11:08:09|2024-11-13T13:07:23|11:29:20|10808.05M|3807.56M|10121984K|0|7154|0:0|Cyclic|10121984K|0|11:29:20|6||1||2024-11-13T11:08:09|00:00:00|22:52.760||11:06:30|c1-28||cpu=6,mem=10000M,node=1|python3.12
973947|973947|ec-aaaaa|ec85|COMPLETED|2024-11-13T11:49:04|2024-11-13T13:10:25||||||4881|0:0|||||6|10000M|1||2024-11-13T09:18:58|00:00:00|15:01.681|400|07:29:10|c1-17|normal|billing=6,cpu=6,mem=10000M,node=1|command
973947.batch|973947.batch||ec85|COMPLETED|2024-11-13T11:49:04|2024-11-13T13:10:25|00:00:04|51.51M|0.28M|111836K|0|4881|0:0|Unknown|111836K|0|00:00:04|6||1||2024-11-13T11:49:04|00:00:00|00:01.458||00:02.789|c1-17||cpu=6,mem=10000M,node=1|batch
973947.extern|973947.extern||ec85|COMPLETED|2024-11-13T11:49:04|2024-11-13T13:10:25|00:00:00|0.01M|0.00M|0|0|4881|0:0|Unknown|0|0|00:00:00|6||1||2024-11-13T11:49:04|00:00:00|00:00.001||00:00:00|c1-17||billing=6,cpu=6,mem=10000M,node=1|extern
973947.0|973947.0||ec85|COMPLETED|2024-11-13T11:49:19|2024-11-13T13:10:25|07:44:06|7708.71M|3091.87M|10124088K|0|4866|0:0|Cyclic|10124088K|0|07:44:06|6||1||2024-11-13T11:49:19|00:00:00|15:00.221||07:29:08|c1-17||cpu=6,mem=10000M,node=1|python3.12
973980|973980|ec-aaaaa|ec85|COMPLETED|2024-11-13T11:55:35|2024-11-13T13:09:31||||||4436|0:0|||||6|10000M|1||2024-11-13T09:27:02|00:00:00|13:30.976|400|06:48:00|c1-25|normal|billing=6,cpu=6,mem=10000M,node=1|command
973980.batch|973980.batch||ec85|COMPLETED|2024-11-13T11:55:35|2024-11-13T13:09:31|00:00:04|51.51M|0.28M|113872K|0|4436|0:0|Unknown|113872K|0|00:00:04|6||1||2024-11-13T11:55:35|00:00:00|00:01.029||00:02.964|c1-25||cpu=6,mem=10000M,node=1|batch
973980.extern|973980.extern||ec85|COMPLETED|2024-11-13T11:55:35|2024-11-13T13:09:31|00:00:00|0.01M|0.00M|0|0|4436|0:0|Unknown|0|0|00:00:00|6||1||2024-11-13T11:55:35|00:00:00|00:00.001||00:00:00|c1-25||billing=6,cpu=6,mem=10000M,node=1|extern
973980.0|973980.0||ec85|COMPLETED|2024-11-13T11:55:45|2024-11-13T13:09:31|07:01:25|7489.09M|3091.76M|10122528K|0|4426|0:0|Cyclic|10122528K|0|07:01:25|6||1||2024-11-13T11:55:45|00:00:00|13:29.945||06:47:57|c1-25||cpu=6,mem=10000M,node=1|python3.12
973997|973997|ec-aaaaa|ec85|FAILED|2024-11-13T12:27:39|2024-11-13T13:07:46||||||2407|1:0|||||6|10000M|1||2024-11-13T09:32:55|00:00:00|06:14.540|400|03:37:42|c1-11|normal|billing=6,cpu=6,mem=10000M,node=1|command
973997.batch|973997.batch||ec85|FAILED|2024-11-13T12:27:39|2024-11-13T13:07:46|00:00:04|51.51M|0.28M|124556K|0|2407|1:0|Unknown|124556K|0|00:00:04|6||1||2024-11-13T12:27:39|00:00:00|00:01.142||00:02.974|c1-11||cpu=6,mem=10000M,node=1|batch
973997.extern|973997.extern||ec85|COMPLETED|2024-11-13T12:27:39|2024-11-13T13:07:46|00:00:00|0.01M|0.00M|0|0|2407|0:0|Unknown|0|0|00:00:00|6||1||2024-11-13T12:27:39|00:00:00|00:00.001||00:00:00|c1-11||billing=6,cpu=6,mem=10000M,node=1|extern
973997.0|973997.0||ec85|OUT_OF_MEMORY|2024-11-13T12:27:57|2024-11-13T13:07:46|03:43:52|2777.94M|3270.57M|10108844K|0|2389|0:125|Cyclic|10108844K|0|03:43:52|6||1||2024-11-13T12:27:57|00:00:00|06:13.397||03:37:39|c1-11||cpu=6,mem=10000M,node=1|python3.12
974001|974001|ec-aaaaa|ec85|FAILED|2024-11-13T12:35:13|2024-11-13T13:06:46||||||1893|1:0|||||6|10000M|1||2024-11-13T09:33:56|00:00:00|04:29.591|400|02:52:03|c1-19|normal|billing=6,cpu=6,mem=10000M,node=1|command
974001.batch|974001.batch||ec85|FAILED|2024-11-13T12:35:13|2024-11-13T13:06:46|00:00:03|51.51M|0.28M|104300K|0|1893|1:0|Unknown|104300K|0|00:00:03|6||1||2024-11-13T12:35:13|00:00:00|00:01.999||00:02.686|c1-19||cpu=6,mem=10000M,node=1|batch
974001.extern|974001.extern||ec85|COMPLETED|2024-11-13T12:35:13|2024-11-13T13:06:46|00:00:00|0.01M|0.00M|0|0|1893|0:0|Unknown|0|0|00:00:00|6||1||2024-11-13T12:35:13|00:00:00|00:00.001||00:00:00|c1-19||billing=6,cpu=6,mem=10000M,node=1|extern
974001.0|974001.0||ec85|OUT_OF_MEMORY|2024-11-13T12:35:25|2024-11-13T13:06:46|02:56:29|2336.23M|3300.20M|10119756K|0|1881|0:125|Cyclic|10119756K|0|02:56:29|6||1||2024-11-13T12:35:25|00:00:00|04:27.590||02:52:01|c1-19||cpu=6,mem=10000M,node=1|python3.12
974563|974563|ec-aaaaa|ec85|COMPLETED|2024-11-13T13:10:06|2024-11-13T13:10:28||||||22|0:0|||||4|10000M|1||2024-11-13T11:55:36|00:00:00|00:03.329|5|00:09.162|c1-19|normal|billing=4,cpu=4,mem=10000M,node=1|command
974563.batch|974563.batch||ec85|COMPLETED|2024-11-13T13:10:06|2024-11-13T13:10:28|00:00:03|0|0.00M|348K|0|22|0:0|Unknown|348K|0|00:00:03|4||1||2024-11-13T13:10:06|00:00:00|00:00.945||00:02.379|c1-19||cpu=4,mem=10000M,node=1|batch
974563.extern|974563.extern||ec85|COMPLETED|2024-11-13T13:10:06|2024-11-13T13:10:28|00:00:00|0.01M|0.00M|0|0|22|0:0|Unknown|0|0|00:00:00|4||1||2024-11-13T13:10:06|00:00:00|00:00.001||00:00:00|c1-19||billing=4,cpu=4,mem=10000M,node=1|extern
974563.0|974563.0||ec85|COMPLETED|2024-11-13T13:10:15|2024-11-13T13:10:28|00:00:09|0|0.00M|884K|0|13|0:0|Cyclic|884K|0|00:00:09|4||1||2024-11-13T13:10:15|00:00:00|00:02.383||00:06.782|c1-19||cpu=4,mem=10000M,node=1|python3.12
974564|974564|ec-aaaaa|ec85|COMPLETED|2024-11-13T13:10:37|2024-11-13T13:11:03||||||26|0:0|||||4|10000M|1||2024-11-13T11:55:45|00:00:00|00:03.348|5|00:09.304|c1-19|normal|billing=4,cpu=4,mem=10000M,node=1|command
974564.batch|974564.batch||ec85|COMPLETED|2024-11-13T13:10:37|2024-11-13T13:11:03|00:00:03|0|0.00M|312K|0|26|0:0|Unknown|312K|0|00:00:03|4||1||2024-11-13T13:10:37|00:00:00|00:00.909||00:02.432|c1-19||cpu=4,mem=10000M,node=1|batch
974564.extern|974564.extern||ec85|COMPLETED|2024-11-13T13:10:37|2024-11-13T13:11:03|00:00:00|0.01M|0.00M|0|0|26|0:0|Unknown|0|0|00:00:00|4||1||2024-11-13T13:10:37|00:00:00|00:00:00||00:00.001|c1-19||billing=4,cpu=4,mem=10000M,node=1|extern
974564.0|974564.0||ec85|COMPLETED|2024-11-13T13:10:47|2024-11-13T13:11:03|00:00:09|0|0.00M|912K|0|16|0:0|Cyclic|912K|0|00:00:09|4||1||2024-11-13T13:10:47|00:00:00|00:02.438||00:06.871|c1-19||cpu=4,mem=10000M,node=1|python3.12
974598|974598|ec-bbbbb|ec201|COMPLETED|2024-11-13T12:17:06|2024-11-13T13:09:47||||||3161|0:0|||||10|160G|1||2024-11-13T12:04:48|00:00:00|00:36.549|1440|06:34:28|c1-13|normal|billing=40,cpu=10,mem=160G,node=1|complete_rankings_mixtures
974598.batch|974598.batch||ec201|COMPLETED|2024-11-13T12:17:06|2024-11-13T13:09:47|06:35:05|23.66M|0.20M|11790764K|0|3161|0:0|Unknown|11790764K|0|06:35:05|10||1||2024-11-13T12:17:06|00:00:00|00:36.548||06:34:28|c1-13||cpu=10,mem=160G,node=1|batch
974598.extern|974598.extern||ec201|COMPLETED|2024-11-13T12:17:06|2024-11-13T13:09:47|00:00:00|0.01M|0.00M|0|0|3161|0:0|Unknown|0|0|00:00:00|10||1||2024-11-13T12:17:06|00:00:00|00:00:00||00:00:00|c1-13||billing=40,cpu=10,mem=160G,node=1|extern
974615|974615|ec-bbbbb|ec201|COMPLETED|2024-11-13T12:53:03|2024-11-13T13:08:22||||||919|0:0|||||10|160G|1||2024-11-13T12:04:48|00:00:00|00:19.832|1440|01:55:32|c1-20|normal|billing=40,cpu=10,mem=160G,node=1|complete_rankings_mixtures
974615.batch|974615.batch||ec201|COMPLETED|2024-11-13T12:53:03|2024-11-13T13:08:22|01:55:51|23.66M|0.23M|6958164K|0|919|0:0|Unknown|6958164K|0|01:55:51|10||1||2024-11-13T12:53:03|00:00:00|00:19.831||01:55:32|c1-20||cpu=10,mem=160G,node=1|batch
974615.extern|974615.extern||ec201|COMPLETED|2024-11-13T12:53:03|2024-11-13T13:08:22|00:00:00|0.01M|0.00M|0|0|919|0:0|Unknown|0|0|00:00:00|10||1||2024-11-13T12:53:03|00:00:00|00:00.001||00:00:00|c1-20||billing=40,cpu=10,mem=160G,node=1|extern
974620|974620|ec-bbbbb|ec201|COMPLETED|2024-11-13T12:57:58|2024-11-13T13:11:00||||||782|0:0|||||10|160G|1||2024-11-13T12:04:48|00:00:00|00:18.078|1440|01:38:09|c1-13|normal|billing=40,cpu=10,mem=160G,node=1|complete_rankings_mixtures
974620.batch|974620.batch||ec201|COMPLETED|2024-11-13T12:57:58|2024-11-13T13:11:00|01:38:28|23.66M|0.25M|6314188K|0|782|0:0|Unknown|6314188K|0|01:38:28|10||1||2024-11-13T12:57:58|00:00:00|00:18.077||01:38:09|c1-13||cpu=10,mem=160G,node=1|batch
974620.extern|974620.extern||ec201|COMPLETED|2024-11-13T12:57:58|2024-11-13T13:11:00|00:00:00|0.01M|0.00M|0|0|782|0:0|Unknown|0|0|00:00:00|10||1||2024-11-13T12:57:58|00:00:00|00:00.001||00:00:00|c1-13||billing=40,cpu=10,mem=160G,node=1|extern
974724|974724|ec313-autotekst|ec313|COMPLETED|2024-11-13T13:02:56|2024-11-13T13:06:50||||||234|0:0|||||8|64G|1||2024-11-13T12:37:07|00:00:00|00:10.278|22|05:06.252|gpu-10|mig|billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|nsk-transcription-job
974724.batch|974724.batch||ec313|COMPLETED|2024-11-13T13:02:56|2024-11-13T13:06:50|00:05:16|3365.99M|115.41M|4173944K|0|234|0:0|Unknown|4173944K|0|00:05:16|8||1||2024-11-13T13:02:56|00:00:00|00:10.277||05:06.252|gpu-10||cpu=8,gres/gpu=1,mem=64G,node=1|batch
974724.extern|974724.extern||ec313|COMPLETED|2024-11-13T13:02:56|2024-11-13T13:06:50|00:00:00|0.01M|0.00M|0|0|234|0:0|Unknown|0|0|00:00:00|8||1||2024-11-13T13:02:56|00:00:00|00:00.001||00:00:00|gpu-10||billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|extern
974725|974725|ec313-autotekst|ec313|COMPLETED|2024-11-13T13:04:25|2024-11-13T13:08:41||||||256|0:0|||||8|64G|1||2024-11-13T12:37:46|00:00:00|00:12.758|28|04:44.632|gpu-10|mig|billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|nsk-transcription-job
974725.batch|974725.batch||ec313|COMPLETED|2024-11-13T13:04:25|2024-11-13T13:08:41|00:04:57|3401.69M|146.66M|4246808K|0|256|0:0|Unknown|4246808K|0|00:04:57|8||1||2024-11-13T13:04:25|00:00:00|00:12.757||04:44.632|gpu-10||cpu=8,gres/gpu=1,mem=64G,node=1|batch
974725.extern|974725.extern||ec313|COMPLETED|2024-11-13T13:04:25|2024-11-13T13:08:41|00:00:00|0.01M|0.00M|0|0|256|0:0|Unknown|0|0|00:00:00|8||1||2024-11-13T13:04:25|00:00:00|00:00.001||00:00:00|gpu-10||billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|extern
974728|974728|ec313-autotekst|ec313|COMPLETED|2024-11-13T13:04:55|2024-11-13T13:07:03||||||128|0:0|||||8|64G|1||2024-11-13T12:39:01|00:00:00|00:11.880|7|02:07.245|gpu-10|mig|billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|uio-transcription-job
974728.batch|974728.batch||ec313|COMPLETED|2024-11-13T13:04:55|2024-11-13T13:07:03|00:02:19|3596.31M|63.43M|5212664K|0|128|0:0|Unknown|5212664K|0|00:02:19|8||1||2024-11-13T13:04:55|00:00:00|00:11.878||02:07.245|gpu-10||cpu=8,gres/gpu=1,mem=64G,node=1|batch
974728.extern|974728.extern||ec313|COMPLETED|2024-11-13T13:04:55|2024-11-13T13:07:03|00:00:00|0.01M|0.00M|0|0|128|0:0|Unknown|0|0|00:00:00|8||1||2024-11-13T13:04:55|00:00:00|00:00.001||00:00:00|gpu-10||billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|extern
974729|974729|ec313-autotekst|ec313|COMPLETED|2024-11-13T13:05:22|2024-11-13T13:08:02||||||160|0:0|||||8|64G|1||2024-11-13T12:39:07|00:00:00|00:10.466|16|02:56.332|gpu-10|mig|billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|nsk-transcription-job
974729.batch|974729.batch||ec313|COMPLETED|2024-11-13T13:05:22|2024-11-13T13:08:02|00:03:06|3325.18M|82.02M|4110232K|0|160|0:0|Unknown|4110232K|0|00:03:06|8||1||2024-11-13T13:05:22|00:00:00|00:10.464||02:56.332|gpu-10||cpu=8,gres/gpu=1,mem=64G,node=1|batch
974729.extern|974729.extern||ec313|COMPLETED|2024-11-13T13:05:22|2024-11-13T13:08:02|00:00:00|0.01M|0.00M|0|0|160|0:0|Unknown|0|0|00:00:00|8||1||2024-11-13T13:05:22|00:00:00|00:00.001||00:00:00|gpu-10||billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|extern
974730|974730|ec313-autotekst|ec313|COMPLETED|2024-11-13T13:05:55|2024-11-13T13:08:30||||||155|0:0|||||8|64G|1||2024-11-13T12:39:32|00:00:00|00:10.839|9|02:40.646|gpu-10|mig|billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|uio-transcription-job
974730.batch|974730.batch||ec313|COMPLETED|2024-11-13T13:05:55|2024-11-13T13:08:30|00:02:51|3617.03M|77.50M|4001088K|0|155|0:0|Unknown|4001088K|0|00:02:51|8||1||2024-11-13T13:05:55|00:00:00|00:10.837||02:40.646|gpu-10||cpu=8,gres/gpu=1,mem=64G,node=1|batch
974730.extern|974730.extern||ec313|COMPLETED|2024-11-13T13:05:55|2024-11-13T13:08:30|00:00:00|0.01M|0.00M|0|0|155|0:0|Unknown|0|0|00:00:00|8||1||2024-11-13T13:05:55|00:00:00|00:00.001||00:00:00|gpu-10||billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|extern
974735|974735|ec313-autotekst|ec313|COMPLETED|2024-11-13T13:07:06|2024-11-13T13:07:46||||||40|0:0|||||8|64G|1||2024-11-13T12:42:53|00:00:00|00:10.826|3|00:27.934|gpu-10|mig|billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|nsk-transcription-job
974735.batch|974735.batch||ec313|COMPLETED|2024-11-13T13:07:06|2024-11-13T13:07:46|00:00:38|3245.38M|9.27M|3955768K|0|40|0:0|Unknown|3955768K|0|00:00:38|8||1||2024-11-13T13:07:06|00:00:00|00:10.824||00:27.934|gpu-10||cpu=8,gres/gpu=1,mem=64G,node=1|batch
974735.extern|974735.extern||ec313|COMPLETED|2024-11-13T13:07:06|2024-11-13T13:07:46|00:00:00|0.01M|0.00M|0|0|40|0:0|Unknown|0|0|00:00:00|8||1||2024-11-13T13:07:06|00:00:00|00:00.001||00:00:00|gpu-10||billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|extern
974737|974737|ec313-autotekst|ec313|COMPLETED|2024-11-13T13:07:49|2024-11-13T13:08:18||||||29|0:0|||||8|64G|1||2024-11-13T12:43:17|00:00:00|00:10.440|2|00:14.088|gpu-10|mig|billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|nsk-transcription-job
974737.batch|974737.batch||ec313|COMPLETED|2024-11-13T13:07:49|2024-11-13T13:08:18|00:00:24|0|0.00M|3152K|0|29|0:0|Unknown|3152K|0|00:00:24|8||1||2024-11-13T13:07:49|00:00:00|00:10.438||00:14.088|gpu-10||cpu=8,gres/gpu=1,mem=64G,node=1|batch
974737.extern|974737.extern||ec313|COMPLETED|2024-11-13T13:07:49|2024-11-13T13:08:18|00:00:00|0.01M|0.00M|0|0|29|0:0|Unknown|0|0|00:00:00|8||1||2024-11-13T13:07:49|00:00:00|00:00.001||00:00:00|gpu-10||billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|extern
974738|974738|ec313-autotekst|ec313|COMPLETED|2024-11-13T13:08:03|2024-11-13T13:10:31||||||148|0:0|||||8|64G|1||2024-11-13T12:43:25|00:00:00|00:08.654|15|02:44.072|gpu-10|mig|billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|nsk-transcription-job
974738.batch|974738.batch||ec313|COMPLETED|2024-11-13T13:08:03|2024-11-13T13:10:31|00:02:52|3318.92M|73.20M|4108448K|0|148|0:0|Unknown|4108448K|0|00:02:52|8||1||2024-11-13T13:08:03|00:00:00|00:08.653||02:44.072|gpu-10||cpu=8,gres/gpu=1,mem=64G,node=1|batch
974738.extern|974738.extern||ec313|COMPLETED|2024-11-13T13:08:03|2024-11-13T13:10:31|00:00:00|0.01M|0.00M|0|0|148|0:0|Unknown|0|0|00:00:00|8||1||2024-11-13T13:08:03|00:00:00|00:00.001||00:00:00|gpu-10||billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|extern
974739|974739|ec313-autotekst|ec313|COMPLETED|2024-11-13T13:08:18|2024-11-13T13:08:47||||||29|0:0|||||8|64G|1||2024-11-13T12:43:27|00:00:00|00:10.226|2|00:14.978|gpu-10|mig|billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|nsk-transcription-job
974739.batch|974739.batch||ec313|COMPLETED|2024-11-13T13:08:18|2024-11-13T13:08:47|00:00:24|0|0.00M|1448K|0|29|0:0|Unknown|1448K|0|00:00:24|8||1||2024-11-13T13:08:18|00:00:00|00:10.225||00:14.978|gpu-10||cpu=8,gres/gpu=1,mem=64G,node=1|batch
974739.extern|974739.extern||ec313|COMPLETED|2024-11-13T13:08:18|2024-11-13T13:08:47|00:00:00|0.01M|0.00M|0|0|29|0:0|Unknown|0|0|00:00:00|8||1||2024-11-13T13:08:18|00:00:00|00:00.001||00:00:00|gpu-10||billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|extern
974742|974742|ec313-autotekst|ec313|COMPLETED|2024-11-13T13:08:35|2024-11-13T13:09:00||||||25|0:0|||||8|64G|1||2024-11-13T12:44:21|00:00:00|00:06.631|2|00:10.521|gpu-10|mig|billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|nsk-transcription-job
974742.batch|974742.batch||ec313|COMPLETED|2024-11-13T13:08:35|2024-11-13T13:09:00|00:00:17|0|0.00M|1240K|0|25|0:0|Unknown|1240K|0|00:00:17|8||1||2024-11-13T13:08:35|00:00:00|00:06.629||00:10.521|gpu-10||cpu=8,gres/gpu=1,mem=64G,node=1|batch
974742.extern|974742.extern||ec313|COMPLETED|2024-11-13T13:08:35|2024-11-13T13:09:00|00:00:00|0.01M|0.00M|0|0|25|0:0|Unknown|0|0|00:00:00|8||1||2024-11-13T13:08:35|00:00:00|00:00.001||00:00:00|gpu-10||billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|extern
974745|974745|ec-ccccc|ec35|CANCELLED by 2101477|2024-11-13T12:45:27|2024-11-13T13:08:36||||||1389|0:0|||||20|50G|1||2024-11-13T12:45:27|00:00:00|08:15.063|80|53:52.638|gpu-4|ifi_accel|billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1|test-cv
974745.batch|974745.batch||ec35|CANCELLED|2024-11-13T12:45:27|2024-11-13T13:08:37|00:00:00|0.18M|0.13M|6068K|0|1390|0:15|Unknown|6068K|0|00:00:00|20||1||2024-11-13T12:45:27|00:00:00|00:00.028||00:00.004|gpu-4||cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1|batch
974745.extern|974745.extern||ec35|COMPLETED|2024-11-13T12:45:27|2024-11-13T13:08:40|00:00:00|0.01M|0.00M|0|0|1393|0:0|Unknown|0|0|00:00:00|20||1||2024-11-13T12:45:27|00:00:00|00:00.002||00:00:00|gpu-4||billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1|extern
974745.0|974745.0||ec35|CANCELLED|2024-11-13T12:45:28|2024-11-13T13:08:40|00:31:03|6029.92M|0.07M|7029652K|0|1392|0:15|Block|7087640K|0|00:31:03|20||1||2024-11-13T12:45:28|00:00:00|08:15.032||53:52.634|gpu-4||cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1|python
974746|974746|ec313-autotekst|ec313|COMPLETED|2024-11-13T13:08:48|2024-11-13T13:09:11||||||23|0:0|||||8|64G|1||2024-11-13T12:45:35|00:00:00|00:06.736|2|00:10.587|gpu-10|mig|billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|nsk-transcription-job
974746.batch|974746.batch||ec313|COMPLETED|2024-11-13T13:08:48|2024-11-13T13:09:11|00:00:17|0|0.00M|668K|0|23|0:0|Unknown|668K|0|00:00:17|8||1||2024-11-13T13:08:48|00:00:00|00:06.734||00:10.587|gpu-10||cpu=8,gres/gpu=1,mem=64G,node=1|batch
974746.extern|974746.extern||ec313|COMPLETED|2024-11-13T13:08:48|2024-11-13T13:09:11|00:00:00|0.01M|0.00M|0|0|23|0:0|Unknown|0|0|00:00:00|8||1||2024-11-13T13:08:48|00:00:00|00:00.001||00:00:00|gpu-10||billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|extern
974751|974751|ec313-autotekst|ec313|COMPLETED|2024-11-13T13:09:13|2024-11-13T13:09:58||||||45|0:0|||||8|64G|1||2024-11-13T12:46:51|00:00:00|00:12.538|2|00:21.303|gpu-10|mig|billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|uio-transcription-job
974751.batch|974751.batch||ec313|COMPLETED|2024-11-13T13:09:13|2024-11-13T13:09:58|00:00:33|3304.52M|4.36M|321312K|0|45|0:0|Unknown|321312K|0|00:00:33|8||1||2024-11-13T13:09:13|00:00:00|00:12.536||00:21.303|gpu-10||cpu=8,gres/gpu=1,mem=64G,node=1|batch
974751.extern|974751.extern||ec313|COMPLETED|2024-11-13T13:09:13|2024-11-13T13:09:58|00:00:00|0.01M|0.00M|0|0|45|0:0|Unknown|0|0|00:00:00|8||1||2024-11-13T13:09:13|00:00:00|00:00.001||00:00:00|gpu-10||billing=12,cpu=8,gres/gpu=1,mem=64G,node=1|extern
974798|974798|ec-ddddd|ec395|TIMEOUT|2024-11-13T13:03:31|2024-11-13T13:08:58||||||327|0:0|||||1|32G|1||2024-11-13T13:03:08|00:00:00|00:37.071|5|04:30.995|c1-17|normal|billing=8,cpu=1,mem=32G,node=1|cpu_job
974798.batch|974798.batch||ec395|CANCELLED|2024-11-13T13:03:31|2024-11-13T13:08:59|00:05:08|1787.60M|2.13M|5734828K|0|328|0:15|Unknown|5734828K|0|00:05:08|1||1||2024-11-13T13:03:31|00:00:00|00:37.070||04:30.995|c1-17||cpu=1,mem=32G,node=1|batch
974798.extern|974798.extern||ec395|COMPLETED|2024-11-13T13:03:31|2024-11-13T13:08:59|00:00:00|0.01M|0.00M|0|0|328|0:0|Unknown|0|0|00:00:00|1||1||2024-11-13T13:03:31|00:00:00|00:00.001||00:00:00|c1-17||billing=8,cpu=1,mem=32G,node=1|extern
974804|974804|ec-eeeee|ec395|TIMEOUT|2024-11-13T13:05:32|2024-11-13T13:08:58||||||206|0:0|||||1|32G|1||2024-11-13T13:05:06|00:00:00|00:04.084|3|03:05.358|gpu-9|accel|billing=19,cpu=1,gres/gpu:a100=1,gres/gpu=1,mem=32G,node=1|gpu_job
974804.batch|974804.batch||ec395|CANCELLED|2024-11-13T13:05:32|2024-11-13T13:08:59|00:03:09|207.33M|0.08M|3148716K|0|207|0:15|Unknown|3148716K|0|00:03:09|1||1||2024-11-13T13:05:32|00:00:00|00:04.083||03:05.357|gpu-9||cpu=1,gres/gpu:a100=1,gres/gpu=1,mem=32G,node=1|batch
974804.extern|974804.extern||ec395|COMPLETED|2024-11-13T13:05:32|2024-11-13T13:08:59|00:00:00|0.01M|0.00M|0|0|207|0:0|Unknown|0|0|00:00:00|1||1||2024-11-13T13:05:32|00:00:00|00:00.001||00:00:00|gpu-9||billing=19,cpu=1,gres/gpu:a100=1,gres/gpu=1,mem=32G,node=1|extern
974806|974806|ec-ccccc|ec35|CANCELLED by 2101477|2024-11-13T13:06:23|2024-11-13T13:08:30||||||127|0:0|||||20|50G|1||2024-11-13T13:06:23|00:00:00|00:39.637|80|04:03.895|gpu-4|ifi_accel|billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1|test-cv
974806.batch|974806.batch||ec35|CANCELLED|2024-11-13T13:06:23|2024-11-13T13:08:31|00:00:00|0.15M|0.10M|6328K|0|128|0:15|Unknown|6328K|0|00:00:00|20||1||2024-11-13T13:06:23|00:00:00|00:00.028||00:00.005|gpu-4||cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1|batch
974806.extern|974806.extern||ec35|COMPLETED|2024-11-13T13:06:23|2024-11-13T13:08:34|00:00:00|0.01M|0.00M|0|0|131|0:0|Unknown|0|0|00:00:00|20||1||2024-11-13T13:06:23|00:00:00|00:00.001||00:00:00|gpu-4||billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1|extern
974806.0|974806.0||ec35|CANCELLED|2024-11-13T13:06:24|2024-11-13T13:08:33|00:02:21|686.46M|0.06M|2749696K|0|129|0:15|Block|2768716K|0|00:02:21|20||1||2024-11-13T13:06:24|00:00:00|00:39.607||04:03.889|gpu-4||cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1|python
974809|974809|fffff|ec30|FAILED|2024-11-13T13:07:33|2024-11-13T13:07:43||||||10|1:0|||||4|16G|1||2024-11-13T13:07:20|00:00:00|00:00.001|90|00:00:00|c1-19|normal|billing=4,cpu=4,mem=16G,node=1|JUPYTER
974809.extern|974809.extern||ec30|COMPLETED|2024-11-13T13:07:33|2024-11-13T13:07:43|00:00:00|0.01M|0.00M|0|0|10|0:0|Unknown|0|0|00:00:00|4||1||2024-11-13T13:07:33|00:00:00|00:00.001||00:00:00|c1-19||billing=4,cpu=4,mem=16G,node=1|extern
974810|974810|fffff|ec30|FAILED|2024-11-13T13:07:33|2024-11-13T13:07:35||||||2|2:0|||||4|16G|1||2024-11-13T13:07:32|00:00:00|00:00.004|90|00:00.002|c1-28|normal|billing=4,cpu=4,mem=16G,node=1|JUPYTER
974810.extern|974810.extern||ec30|COMPLETED|2024-11-13T13:07:33|2024-11-13T13:07:35|00:00:00|0.01M|0.00M|0|0|2|0:0|Unknown|0|0|00:00:00|4||1||2024-11-13T13:07:33|00:00:00|00:00.001||00:00:00|c1-28||billing=4,cpu=4,mem=16G,node=1|extern
974810.0|974810.0||ec30|FAILED|2024-11-13T13:07:34|2024-11-13T13:07:35|00:00:00|0|0.00M|72K|0|1|2:0|Block|72K|0|00:00:00|4||1||2024-11-13T13:07:34|00:00:00|00:00.002||00:00.002|c1-28||cpu=4,mem=16G,node=1|JUPYTER
974819|974819|ec-ccccc|ec35|CANCELLED by 2101477|2024-11-13T13:09:19|2024-11-13T13:09:32||||||13|0:0|||||20|50G|1||2024-11-13T13:09:16|00:00:00|00:04.790|80|00:05.694|gpu-4|ifi_accel|billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1|test-cv
974819.batch|974819.batch||ec35|CANCELLED|2024-11-13T13:09:19|2024-11-13T13:09:33|00:00:00|0|0.00M|5956K|0|14|0:15|Unknown|5956K|0|00:00:00|20||1||2024-11-13T13:09:19|00:00:00|00:00.027||00:00.005|gpu-4||cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1|batch
974819.extern|974819.extern||ec35|COMPLETED|2024-11-13T13:09:19|2024-11-13T13:09:34|00:00:00|0.01M|0.00M|0|0|15|0:0|Unknown|0|0|00:00:00|20||1||2024-11-13T13:09:19|00:00:00|00:00.001||00:00:00|gpu-4||billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1|extern
974819.0|974819.0||ec35|CANCELLED|2024-11-13T13:09:19|2024-11-13T13:09:34|00:00:05|19.75M|0.00M|54518K|0|15|0:15|Block|79648K|0|00:00:05|20||1||2024-11-13T13:09:19|00:00:00|00:04.761||00:05.688|gpu-4||cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1|python
"#;

    let mut output = Vec::new();
    let mut local = time::now_local();
    // The output below depends on us being in UTC+01:00 and not in dst so mock that.
    local.tm_gmtoff = 3600;
    local.tm_isdst = 0;
    format_jobs(&mut output, sacct_output, &field_names, &local);

    // The golang `sacctd` output for the above input.
    let expected = r#"v=0.1.0,JobID=973821,JobIDRaw=973821,User=ec-aaaaa,Account=ec85,State=COMPLETED,Start=2024-11-13T11:08:00+01:00,End=2024-11-13T13:07:24+01:00,ElapsedRaw=7164,ReqCPUS=6,ReqMem=10000M,ReqNodes=1,Submit=2024-11-13T08:30:40+01:00,SystemCPU=22:53.824,TimelimitRaw=400,UserCPU=11:06:33,NodeList=c1-28,Partition=normal,"AllocTRES=billing=6,cpu=6,mem=10000M,node=1",JobName=command
v=0.1.0,JobID=973821.batch,JobIDRaw=973821.batch,Account=ec85,State=COMPLETED,Start=2024-11-13T11:08:00+01:00,End=2024-11-13T13:07:24+01:00,AveCPU=00:00:03,AveDiskRead=51.54M,AveDiskWrite=0.30M,AveRSS=112488K,ElapsedRaw=7164,MaxRSS=112488K,MinCPU=00:00:03,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T11:08:00+01:00,SystemCPU=00:01.062,UserCPU=00:02.806,NodeList=c1-28,"AllocTRES=cpu=6,mem=10000M,node=1",JobName=batch
v=0.1.0,JobID=973821.extern,JobIDRaw=973821.extern,Account=ec85,State=COMPLETED,Start=2024-11-13T11:08:00+01:00,End=2024-11-13T13:07:24+01:00,AveDiskRead=0.01M,ElapsedRaw=7164,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T11:08:00+01:00,NodeList=c1-28,"AllocTRES=billing=6,cpu=6,mem=10000M,node=1",JobName=extern
v=0.1.0,JobID=973821.0,JobIDRaw=973821.0,Account=ec85,State=COMPLETED,Start=2024-11-13T11:08:09+01:00,End=2024-11-13T13:07:23+01:00,AveCPU=11:29:20,AveDiskRead=10808.05M,AveDiskWrite=3807.56M,AveRSS=10121984K,ElapsedRaw=7154,Layout=Cyclic,MaxRSS=10121984K,MinCPU=11:29:20,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T11:08:09+01:00,SystemCPU=22:52.760,UserCPU=11:06:30,NodeList=c1-28,"AllocTRES=cpu=6,mem=10000M,node=1",JobName=python3.12
v=0.1.0,JobID=973947,JobIDRaw=973947,User=ec-aaaaa,Account=ec85,State=COMPLETED,Start=2024-11-13T11:49:04+01:00,End=2024-11-13T13:10:25+01:00,ElapsedRaw=4881,ReqCPUS=6,ReqMem=10000M,ReqNodes=1,Submit=2024-11-13T09:18:58+01:00,SystemCPU=15:01.681,TimelimitRaw=400,UserCPU=07:29:10,NodeList=c1-17,Partition=normal,"AllocTRES=billing=6,cpu=6,mem=10000M,node=1",JobName=command
v=0.1.0,JobID=973947.batch,JobIDRaw=973947.batch,Account=ec85,State=COMPLETED,Start=2024-11-13T11:49:04+01:00,End=2024-11-13T13:10:25+01:00,AveCPU=00:00:04,AveDiskRead=51.51M,AveDiskWrite=0.28M,AveRSS=111836K,ElapsedRaw=4881,MaxRSS=111836K,MinCPU=00:00:04,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T11:49:04+01:00,SystemCPU=00:01.458,UserCPU=00:02.789,NodeList=c1-17,"AllocTRES=cpu=6,mem=10000M,node=1",JobName=batch
v=0.1.0,JobID=973947.extern,JobIDRaw=973947.extern,Account=ec85,State=COMPLETED,Start=2024-11-13T11:49:04+01:00,End=2024-11-13T13:10:25+01:00,AveDiskRead=0.01M,ElapsedRaw=4881,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T11:49:04+01:00,SystemCPU=00:00.001,NodeList=c1-17,"AllocTRES=billing=6,cpu=6,mem=10000M,node=1",JobName=extern
v=0.1.0,JobID=973947.0,JobIDRaw=973947.0,Account=ec85,State=COMPLETED,Start=2024-11-13T11:49:19+01:00,End=2024-11-13T13:10:25+01:00,AveCPU=07:44:06,AveDiskRead=7708.71M,AveDiskWrite=3091.87M,AveRSS=10124088K,ElapsedRaw=4866,Layout=Cyclic,MaxRSS=10124088K,MinCPU=07:44:06,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T11:49:19+01:00,SystemCPU=15:00.221,UserCPU=07:29:08,NodeList=c1-17,"AllocTRES=cpu=6,mem=10000M,node=1",JobName=python3.12
v=0.1.0,JobID=973980,JobIDRaw=973980,User=ec-aaaaa,Account=ec85,State=COMPLETED,Start=2024-11-13T11:55:35+01:00,End=2024-11-13T13:09:31+01:00,ElapsedRaw=4436,ReqCPUS=6,ReqMem=10000M,ReqNodes=1,Submit=2024-11-13T09:27:02+01:00,SystemCPU=13:30.976,TimelimitRaw=400,UserCPU=06:48:00,NodeList=c1-25,Partition=normal,"AllocTRES=billing=6,cpu=6,mem=10000M,node=1",JobName=command
v=0.1.0,JobID=973980.batch,JobIDRaw=973980.batch,Account=ec85,State=COMPLETED,Start=2024-11-13T11:55:35+01:00,End=2024-11-13T13:09:31+01:00,AveCPU=00:00:04,AveDiskRead=51.51M,AveDiskWrite=0.28M,AveRSS=113872K,ElapsedRaw=4436,MaxRSS=113872K,MinCPU=00:00:04,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T11:55:35+01:00,SystemCPU=00:01.029,UserCPU=00:02.964,NodeList=c1-25,"AllocTRES=cpu=6,mem=10000M,node=1",JobName=batch
v=0.1.0,JobID=973980.extern,JobIDRaw=973980.extern,Account=ec85,State=COMPLETED,Start=2024-11-13T11:55:35+01:00,End=2024-11-13T13:09:31+01:00,AveDiskRead=0.01M,ElapsedRaw=4436,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T11:55:35+01:00,SystemCPU=00:00.001,NodeList=c1-25,"AllocTRES=billing=6,cpu=6,mem=10000M,node=1",JobName=extern
v=0.1.0,JobID=973980.0,JobIDRaw=973980.0,Account=ec85,State=COMPLETED,Start=2024-11-13T11:55:45+01:00,End=2024-11-13T13:09:31+01:00,AveCPU=07:01:25,AveDiskRead=7489.09M,AveDiskWrite=3091.76M,AveRSS=10122528K,ElapsedRaw=4426,Layout=Cyclic,MaxRSS=10122528K,MinCPU=07:01:25,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T11:55:45+01:00,SystemCPU=13:29.945,UserCPU=06:47:57,NodeList=c1-25,"AllocTRES=cpu=6,mem=10000M,node=1",JobName=python3.12
v=0.1.0,JobID=973997,JobIDRaw=973997,User=ec-aaaaa,Account=ec85,State=FAILED,Start=2024-11-13T12:27:39+01:00,End=2024-11-13T13:07:46+01:00,ElapsedRaw=2407,ExitCode=1:0,ReqCPUS=6,ReqMem=10000M,ReqNodes=1,Submit=2024-11-13T09:32:55+01:00,SystemCPU=06:14.540,TimelimitRaw=400,UserCPU=03:37:42,NodeList=c1-11,Partition=normal,"AllocTRES=billing=6,cpu=6,mem=10000M,node=1",JobName=command
v=0.1.0,JobID=973997.batch,JobIDRaw=973997.batch,Account=ec85,State=FAILED,Start=2024-11-13T12:27:39+01:00,End=2024-11-13T13:07:46+01:00,AveCPU=00:00:04,AveDiskRead=51.51M,AveDiskWrite=0.28M,AveRSS=124556K,ElapsedRaw=2407,ExitCode=1:0,MaxRSS=124556K,MinCPU=00:00:04,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T12:27:39+01:00,SystemCPU=00:01.142,UserCPU=00:02.974,NodeList=c1-11,"AllocTRES=cpu=6,mem=10000M,node=1",JobName=batch
v=0.1.0,JobID=973997.extern,JobIDRaw=973997.extern,Account=ec85,State=COMPLETED,Start=2024-11-13T12:27:39+01:00,End=2024-11-13T13:07:46+01:00,AveDiskRead=0.01M,ElapsedRaw=2407,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T12:27:39+01:00,SystemCPU=00:00.001,NodeList=c1-11,"AllocTRES=billing=6,cpu=6,mem=10000M,node=1",JobName=extern
v=0.1.0,JobID=973997.0,JobIDRaw=973997.0,Account=ec85,State=OUT_OF_MEMORY,Start=2024-11-13T12:27:57+01:00,End=2024-11-13T13:07:46+01:00,AveCPU=03:43:52,AveDiskRead=2777.94M,AveDiskWrite=3270.57M,AveRSS=10108844K,ElapsedRaw=2389,ExitCode=0:125,Layout=Cyclic,MaxRSS=10108844K,MinCPU=03:43:52,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T12:27:57+01:00,SystemCPU=06:13.397,UserCPU=03:37:39,NodeList=c1-11,"AllocTRES=cpu=6,mem=10000M,node=1",JobName=python3.12
v=0.1.0,JobID=974001,JobIDRaw=974001,User=ec-aaaaa,Account=ec85,State=FAILED,Start=2024-11-13T12:35:13+01:00,End=2024-11-13T13:06:46+01:00,ElapsedRaw=1893,ExitCode=1:0,ReqCPUS=6,ReqMem=10000M,ReqNodes=1,Submit=2024-11-13T09:33:56+01:00,SystemCPU=04:29.591,TimelimitRaw=400,UserCPU=02:52:03,NodeList=c1-19,Partition=normal,"AllocTRES=billing=6,cpu=6,mem=10000M,node=1",JobName=command
v=0.1.0,JobID=974001.batch,JobIDRaw=974001.batch,Account=ec85,State=FAILED,Start=2024-11-13T12:35:13+01:00,End=2024-11-13T13:06:46+01:00,AveCPU=00:00:03,AveDiskRead=51.51M,AveDiskWrite=0.28M,AveRSS=104300K,ElapsedRaw=1893,ExitCode=1:0,MaxRSS=104300K,MinCPU=00:00:03,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T12:35:13+01:00,SystemCPU=00:01.999,UserCPU=00:02.686,NodeList=c1-19,"AllocTRES=cpu=6,mem=10000M,node=1",JobName=batch
v=0.1.0,JobID=974001.extern,JobIDRaw=974001.extern,Account=ec85,State=COMPLETED,Start=2024-11-13T12:35:13+01:00,End=2024-11-13T13:06:46+01:00,AveDiskRead=0.01M,ElapsedRaw=1893,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T12:35:13+01:00,SystemCPU=00:00.001,NodeList=c1-19,"AllocTRES=billing=6,cpu=6,mem=10000M,node=1",JobName=extern
v=0.1.0,JobID=974001.0,JobIDRaw=974001.0,Account=ec85,State=OUT_OF_MEMORY,Start=2024-11-13T12:35:25+01:00,End=2024-11-13T13:06:46+01:00,AveCPU=02:56:29,AveDiskRead=2336.23M,AveDiskWrite=3300.20M,AveRSS=10119756K,ElapsedRaw=1881,ExitCode=0:125,Layout=Cyclic,MaxRSS=10119756K,MinCPU=02:56:29,ReqCPUS=6,ReqNodes=1,Submit=2024-11-13T12:35:25+01:00,SystemCPU=04:27.590,UserCPU=02:52:01,NodeList=c1-19,"AllocTRES=cpu=6,mem=10000M,node=1",JobName=python3.12
v=0.1.0,JobID=974563,JobIDRaw=974563,User=ec-aaaaa,Account=ec85,State=COMPLETED,Start=2024-11-13T13:10:06+01:00,End=2024-11-13T13:10:28+01:00,ElapsedRaw=22,ReqCPUS=4,ReqMem=10000M,ReqNodes=1,Submit=2024-11-13T11:55:36+01:00,SystemCPU=00:03.329,TimelimitRaw=5,UserCPU=00:09.162,NodeList=c1-19,Partition=normal,"AllocTRES=billing=4,cpu=4,mem=10000M,node=1",JobName=command
v=0.1.0,JobID=974563.batch,JobIDRaw=974563.batch,Account=ec85,State=COMPLETED,Start=2024-11-13T13:10:06+01:00,End=2024-11-13T13:10:28+01:00,AveCPU=00:00:03,AveRSS=348K,ElapsedRaw=22,MaxRSS=348K,MinCPU=00:00:03,ReqCPUS=4,ReqNodes=1,Submit=2024-11-13T13:10:06+01:00,SystemCPU=00:00.945,UserCPU=00:02.379,NodeList=c1-19,"AllocTRES=cpu=4,mem=10000M,node=1",JobName=batch
v=0.1.0,JobID=974563.extern,JobIDRaw=974563.extern,Account=ec85,State=COMPLETED,Start=2024-11-13T13:10:06+01:00,End=2024-11-13T13:10:28+01:00,AveDiskRead=0.01M,ElapsedRaw=22,ReqCPUS=4,ReqNodes=1,Submit=2024-11-13T13:10:06+01:00,SystemCPU=00:00.001,NodeList=c1-19,"AllocTRES=billing=4,cpu=4,mem=10000M,node=1",JobName=extern
v=0.1.0,JobID=974563.0,JobIDRaw=974563.0,Account=ec85,State=COMPLETED,Start=2024-11-13T13:10:15+01:00,End=2024-11-13T13:10:28+01:00,AveCPU=00:00:09,AveRSS=884K,ElapsedRaw=13,Layout=Cyclic,MaxRSS=884K,MinCPU=00:00:09,ReqCPUS=4,ReqNodes=1,Submit=2024-11-13T13:10:15+01:00,SystemCPU=00:02.383,UserCPU=00:06.782,NodeList=c1-19,"AllocTRES=cpu=4,mem=10000M,node=1",JobName=python3.12
v=0.1.0,JobID=974564,JobIDRaw=974564,User=ec-aaaaa,Account=ec85,State=COMPLETED,Start=2024-11-13T13:10:37+01:00,End=2024-11-13T13:11:03+01:00,ElapsedRaw=26,ReqCPUS=4,ReqMem=10000M,ReqNodes=1,Submit=2024-11-13T11:55:45+01:00,SystemCPU=00:03.348,TimelimitRaw=5,UserCPU=00:09.304,NodeList=c1-19,Partition=normal,"AllocTRES=billing=4,cpu=4,mem=10000M,node=1",JobName=command
v=0.1.0,JobID=974564.batch,JobIDRaw=974564.batch,Account=ec85,State=COMPLETED,Start=2024-11-13T13:10:37+01:00,End=2024-11-13T13:11:03+01:00,AveCPU=00:00:03,AveRSS=312K,ElapsedRaw=26,MaxRSS=312K,MinCPU=00:00:03,ReqCPUS=4,ReqNodes=1,Submit=2024-11-13T13:10:37+01:00,SystemCPU=00:00.909,UserCPU=00:02.432,NodeList=c1-19,"AllocTRES=cpu=4,mem=10000M,node=1",JobName=batch
v=0.1.0,JobID=974564.extern,JobIDRaw=974564.extern,Account=ec85,State=COMPLETED,Start=2024-11-13T13:10:37+01:00,End=2024-11-13T13:11:03+01:00,AveDiskRead=0.01M,ElapsedRaw=26,ReqCPUS=4,ReqNodes=1,Submit=2024-11-13T13:10:37+01:00,UserCPU=00:00.001,NodeList=c1-19,"AllocTRES=billing=4,cpu=4,mem=10000M,node=1",JobName=extern
v=0.1.0,JobID=974564.0,JobIDRaw=974564.0,Account=ec85,State=COMPLETED,Start=2024-11-13T13:10:47+01:00,End=2024-11-13T13:11:03+01:00,AveCPU=00:00:09,AveRSS=912K,ElapsedRaw=16,Layout=Cyclic,MaxRSS=912K,MinCPU=00:00:09,ReqCPUS=4,ReqNodes=1,Submit=2024-11-13T13:10:47+01:00,SystemCPU=00:02.438,UserCPU=00:06.871,NodeList=c1-19,"AllocTRES=cpu=4,mem=10000M,node=1",JobName=python3.12
v=0.1.0,JobID=974598,JobIDRaw=974598,User=ec-bbbbb,Account=ec201,State=COMPLETED,Start=2024-11-13T12:17:06+01:00,End=2024-11-13T13:09:47+01:00,ElapsedRaw=3161,ReqCPUS=10,ReqMem=160G,ReqNodes=1,Submit=2024-11-13T12:04:48+01:00,SystemCPU=00:36.549,TimelimitRaw=1440,UserCPU=06:34:28,NodeList=c1-13,Partition=normal,"AllocTRES=billing=40,cpu=10,mem=160G,node=1",JobName=complete_rankings_mixtures
v=0.1.0,JobID=974598.batch,JobIDRaw=974598.batch,Account=ec201,State=COMPLETED,Start=2024-11-13T12:17:06+01:00,End=2024-11-13T13:09:47+01:00,AveCPU=06:35:05,AveDiskRead=23.66M,AveDiskWrite=0.20M,AveRSS=11790764K,ElapsedRaw=3161,MaxRSS=11790764K,MinCPU=06:35:05,ReqCPUS=10,ReqNodes=1,Submit=2024-11-13T12:17:06+01:00,SystemCPU=00:36.548,UserCPU=06:34:28,NodeList=c1-13,"AllocTRES=cpu=10,mem=160G,node=1",JobName=batch
v=0.1.0,JobID=974598.extern,JobIDRaw=974598.extern,Account=ec201,State=COMPLETED,Start=2024-11-13T12:17:06+01:00,End=2024-11-13T13:09:47+01:00,AveDiskRead=0.01M,ElapsedRaw=3161,ReqCPUS=10,ReqNodes=1,Submit=2024-11-13T12:17:06+01:00,NodeList=c1-13,"AllocTRES=billing=40,cpu=10,mem=160G,node=1",JobName=extern
v=0.1.0,JobID=974615,JobIDRaw=974615,User=ec-bbbbb,Account=ec201,State=COMPLETED,Start=2024-11-13T12:53:03+01:00,End=2024-11-13T13:08:22+01:00,ElapsedRaw=919,ReqCPUS=10,ReqMem=160G,ReqNodes=1,Submit=2024-11-13T12:04:48+01:00,SystemCPU=00:19.832,TimelimitRaw=1440,UserCPU=01:55:32,NodeList=c1-20,Partition=normal,"AllocTRES=billing=40,cpu=10,mem=160G,node=1",JobName=complete_rankings_mixtures
v=0.1.0,JobID=974615.batch,JobIDRaw=974615.batch,Account=ec201,State=COMPLETED,Start=2024-11-13T12:53:03+01:00,End=2024-11-13T13:08:22+01:00,AveCPU=01:55:51,AveDiskRead=23.66M,AveDiskWrite=0.23M,AveRSS=6958164K,ElapsedRaw=919,MaxRSS=6958164K,MinCPU=01:55:51,ReqCPUS=10,ReqNodes=1,Submit=2024-11-13T12:53:03+01:00,SystemCPU=00:19.831,UserCPU=01:55:32,NodeList=c1-20,"AllocTRES=cpu=10,mem=160G,node=1",JobName=batch
v=0.1.0,JobID=974615.extern,JobIDRaw=974615.extern,Account=ec201,State=COMPLETED,Start=2024-11-13T12:53:03+01:00,End=2024-11-13T13:08:22+01:00,AveDiskRead=0.01M,ElapsedRaw=919,ReqCPUS=10,ReqNodes=1,Submit=2024-11-13T12:53:03+01:00,SystemCPU=00:00.001,NodeList=c1-20,"AllocTRES=billing=40,cpu=10,mem=160G,node=1",JobName=extern
v=0.1.0,JobID=974620,JobIDRaw=974620,User=ec-bbbbb,Account=ec201,State=COMPLETED,Start=2024-11-13T12:57:58+01:00,End=2024-11-13T13:11:00+01:00,ElapsedRaw=782,ReqCPUS=10,ReqMem=160G,ReqNodes=1,Submit=2024-11-13T12:04:48+01:00,SystemCPU=00:18.078,TimelimitRaw=1440,UserCPU=01:38:09,NodeList=c1-13,Partition=normal,"AllocTRES=billing=40,cpu=10,mem=160G,node=1",JobName=complete_rankings_mixtures
v=0.1.0,JobID=974620.batch,JobIDRaw=974620.batch,Account=ec201,State=COMPLETED,Start=2024-11-13T12:57:58+01:00,End=2024-11-13T13:11:00+01:00,AveCPU=01:38:28,AveDiskRead=23.66M,AveDiskWrite=0.25M,AveRSS=6314188K,ElapsedRaw=782,MaxRSS=6314188K,MinCPU=01:38:28,ReqCPUS=10,ReqNodes=1,Submit=2024-11-13T12:57:58+01:00,SystemCPU=00:18.077,UserCPU=01:38:09,NodeList=c1-13,"AllocTRES=cpu=10,mem=160G,node=1",JobName=batch
v=0.1.0,JobID=974620.extern,JobIDRaw=974620.extern,Account=ec201,State=COMPLETED,Start=2024-11-13T12:57:58+01:00,End=2024-11-13T13:11:00+01:00,AveDiskRead=0.01M,ElapsedRaw=782,ReqCPUS=10,ReqNodes=1,Submit=2024-11-13T12:57:58+01:00,SystemCPU=00:00.001,NodeList=c1-13,"AllocTRES=billing=40,cpu=10,mem=160G,node=1",JobName=extern
v=0.1.0,JobID=974724,JobIDRaw=974724,User=ec313-autotekst,Account=ec313,State=COMPLETED,Start=2024-11-13T13:02:56+01:00,End=2024-11-13T13:06:50+01:00,ElapsedRaw=234,ReqCPUS=8,ReqMem=64G,ReqNodes=1,Submit=2024-11-13T12:37:07+01:00,SystemCPU=00:10.278,TimelimitRaw=22,UserCPU=05:06.252,NodeList=gpu-10,Partition=mig,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=nsk-transcription-job
v=0.1.0,JobID=974724.batch,JobIDRaw=974724.batch,Account=ec313,State=COMPLETED,Start=2024-11-13T13:02:56+01:00,End=2024-11-13T13:06:50+01:00,AveCPU=00:05:16,AveDiskRead=3365.99M,AveDiskWrite=115.41M,AveRSS=4173944K,ElapsedRaw=234,MaxRSS=4173944K,MinCPU=00:05:16,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:02:56+01:00,SystemCPU=00:10.277,UserCPU=05:06.252,NodeList=gpu-10,"AllocTRES=cpu=8,gres/gpu=1,mem=64G,node=1",JobName=batch
v=0.1.0,JobID=974724.extern,JobIDRaw=974724.extern,Account=ec313,State=COMPLETED,Start=2024-11-13T13:02:56+01:00,End=2024-11-13T13:06:50+01:00,AveDiskRead=0.01M,ElapsedRaw=234,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:02:56+01:00,SystemCPU=00:00.001,NodeList=gpu-10,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=extern
v=0.1.0,JobID=974725,JobIDRaw=974725,User=ec313-autotekst,Account=ec313,State=COMPLETED,Start=2024-11-13T13:04:25+01:00,End=2024-11-13T13:08:41+01:00,ElapsedRaw=256,ReqCPUS=8,ReqMem=64G,ReqNodes=1,Submit=2024-11-13T12:37:46+01:00,SystemCPU=00:12.758,TimelimitRaw=28,UserCPU=04:44.632,NodeList=gpu-10,Partition=mig,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=nsk-transcription-job
v=0.1.0,JobID=974725.batch,JobIDRaw=974725.batch,Account=ec313,State=COMPLETED,Start=2024-11-13T13:04:25+01:00,End=2024-11-13T13:08:41+01:00,AveCPU=00:04:57,AveDiskRead=3401.69M,AveDiskWrite=146.66M,AveRSS=4246808K,ElapsedRaw=256,MaxRSS=4246808K,MinCPU=00:04:57,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:04:25+01:00,SystemCPU=00:12.757,UserCPU=04:44.632,NodeList=gpu-10,"AllocTRES=cpu=8,gres/gpu=1,mem=64G,node=1",JobName=batch
v=0.1.0,JobID=974725.extern,JobIDRaw=974725.extern,Account=ec313,State=COMPLETED,Start=2024-11-13T13:04:25+01:00,End=2024-11-13T13:08:41+01:00,AveDiskRead=0.01M,ElapsedRaw=256,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:04:25+01:00,SystemCPU=00:00.001,NodeList=gpu-10,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=extern
v=0.1.0,JobID=974728,JobIDRaw=974728,User=ec313-autotekst,Account=ec313,State=COMPLETED,Start=2024-11-13T13:04:55+01:00,End=2024-11-13T13:07:03+01:00,ElapsedRaw=128,ReqCPUS=8,ReqMem=64G,ReqNodes=1,Submit=2024-11-13T12:39:01+01:00,SystemCPU=00:11.880,TimelimitRaw=7,UserCPU=02:07.245,NodeList=gpu-10,Partition=mig,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=uio-transcription-job
v=0.1.0,JobID=974728.batch,JobIDRaw=974728.batch,Account=ec313,State=COMPLETED,Start=2024-11-13T13:04:55+01:00,End=2024-11-13T13:07:03+01:00,AveCPU=00:02:19,AveDiskRead=3596.31M,AveDiskWrite=63.43M,AveRSS=5212664K,ElapsedRaw=128,MaxRSS=5212664K,MinCPU=00:02:19,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:04:55+01:00,SystemCPU=00:11.878,UserCPU=02:07.245,NodeList=gpu-10,"AllocTRES=cpu=8,gres/gpu=1,mem=64G,node=1",JobName=batch
v=0.1.0,JobID=974728.extern,JobIDRaw=974728.extern,Account=ec313,State=COMPLETED,Start=2024-11-13T13:04:55+01:00,End=2024-11-13T13:07:03+01:00,AveDiskRead=0.01M,ElapsedRaw=128,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:04:55+01:00,SystemCPU=00:00.001,NodeList=gpu-10,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=extern
v=0.1.0,JobID=974729,JobIDRaw=974729,User=ec313-autotekst,Account=ec313,State=COMPLETED,Start=2024-11-13T13:05:22+01:00,End=2024-11-13T13:08:02+01:00,ElapsedRaw=160,ReqCPUS=8,ReqMem=64G,ReqNodes=1,Submit=2024-11-13T12:39:07+01:00,SystemCPU=00:10.466,TimelimitRaw=16,UserCPU=02:56.332,NodeList=gpu-10,Partition=mig,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=nsk-transcription-job
v=0.1.0,JobID=974729.batch,JobIDRaw=974729.batch,Account=ec313,State=COMPLETED,Start=2024-11-13T13:05:22+01:00,End=2024-11-13T13:08:02+01:00,AveCPU=00:03:06,AveDiskRead=3325.18M,AveDiskWrite=82.02M,AveRSS=4110232K,ElapsedRaw=160,MaxRSS=4110232K,MinCPU=00:03:06,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:05:22+01:00,SystemCPU=00:10.464,UserCPU=02:56.332,NodeList=gpu-10,"AllocTRES=cpu=8,gres/gpu=1,mem=64G,node=1",JobName=batch
v=0.1.0,JobID=974729.extern,JobIDRaw=974729.extern,Account=ec313,State=COMPLETED,Start=2024-11-13T13:05:22+01:00,End=2024-11-13T13:08:02+01:00,AveDiskRead=0.01M,ElapsedRaw=160,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:05:22+01:00,SystemCPU=00:00.001,NodeList=gpu-10,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=extern
v=0.1.0,JobID=974730,JobIDRaw=974730,User=ec313-autotekst,Account=ec313,State=COMPLETED,Start=2024-11-13T13:05:55+01:00,End=2024-11-13T13:08:30+01:00,ElapsedRaw=155,ReqCPUS=8,ReqMem=64G,ReqNodes=1,Submit=2024-11-13T12:39:32+01:00,SystemCPU=00:10.839,TimelimitRaw=9,UserCPU=02:40.646,NodeList=gpu-10,Partition=mig,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=uio-transcription-job
v=0.1.0,JobID=974730.batch,JobIDRaw=974730.batch,Account=ec313,State=COMPLETED,Start=2024-11-13T13:05:55+01:00,End=2024-11-13T13:08:30+01:00,AveCPU=00:02:51,AveDiskRead=3617.03M,AveDiskWrite=77.50M,AveRSS=4001088K,ElapsedRaw=155,MaxRSS=4001088K,MinCPU=00:02:51,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:05:55+01:00,SystemCPU=00:10.837,UserCPU=02:40.646,NodeList=gpu-10,"AllocTRES=cpu=8,gres/gpu=1,mem=64G,node=1",JobName=batch
v=0.1.0,JobID=974730.extern,JobIDRaw=974730.extern,Account=ec313,State=COMPLETED,Start=2024-11-13T13:05:55+01:00,End=2024-11-13T13:08:30+01:00,AveDiskRead=0.01M,ElapsedRaw=155,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:05:55+01:00,SystemCPU=00:00.001,NodeList=gpu-10,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=extern
v=0.1.0,JobID=974735,JobIDRaw=974735,User=ec313-autotekst,Account=ec313,State=COMPLETED,Start=2024-11-13T13:07:06+01:00,End=2024-11-13T13:07:46+01:00,ElapsedRaw=40,ReqCPUS=8,ReqMem=64G,ReqNodes=1,Submit=2024-11-13T12:42:53+01:00,SystemCPU=00:10.826,TimelimitRaw=3,UserCPU=00:27.934,NodeList=gpu-10,Partition=mig,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=nsk-transcription-job
v=0.1.0,JobID=974735.batch,JobIDRaw=974735.batch,Account=ec313,State=COMPLETED,Start=2024-11-13T13:07:06+01:00,End=2024-11-13T13:07:46+01:00,AveCPU=00:00:38,AveDiskRead=3245.38M,AveDiskWrite=9.27M,AveRSS=3955768K,ElapsedRaw=40,MaxRSS=3955768K,MinCPU=00:00:38,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:07:06+01:00,SystemCPU=00:10.824,UserCPU=00:27.934,NodeList=gpu-10,"AllocTRES=cpu=8,gres/gpu=1,mem=64G,node=1",JobName=batch
v=0.1.0,JobID=974735.extern,JobIDRaw=974735.extern,Account=ec313,State=COMPLETED,Start=2024-11-13T13:07:06+01:00,End=2024-11-13T13:07:46+01:00,AveDiskRead=0.01M,ElapsedRaw=40,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:07:06+01:00,SystemCPU=00:00.001,NodeList=gpu-10,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=extern
v=0.1.0,JobID=974737,JobIDRaw=974737,User=ec313-autotekst,Account=ec313,State=COMPLETED,Start=2024-11-13T13:07:49+01:00,End=2024-11-13T13:08:18+01:00,ElapsedRaw=29,ReqCPUS=8,ReqMem=64G,ReqNodes=1,Submit=2024-11-13T12:43:17+01:00,SystemCPU=00:10.440,TimelimitRaw=2,UserCPU=00:14.088,NodeList=gpu-10,Partition=mig,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=nsk-transcription-job
v=0.1.0,JobID=974737.batch,JobIDRaw=974737.batch,Account=ec313,State=COMPLETED,Start=2024-11-13T13:07:49+01:00,End=2024-11-13T13:08:18+01:00,AveCPU=00:00:24,AveRSS=3152K,ElapsedRaw=29,MaxRSS=3152K,MinCPU=00:00:24,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:07:49+01:00,SystemCPU=00:10.438,UserCPU=00:14.088,NodeList=gpu-10,"AllocTRES=cpu=8,gres/gpu=1,mem=64G,node=1",JobName=batch
v=0.1.0,JobID=974737.extern,JobIDRaw=974737.extern,Account=ec313,State=COMPLETED,Start=2024-11-13T13:07:49+01:00,End=2024-11-13T13:08:18+01:00,AveDiskRead=0.01M,ElapsedRaw=29,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:07:49+01:00,SystemCPU=00:00.001,NodeList=gpu-10,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=extern
v=0.1.0,JobID=974738,JobIDRaw=974738,User=ec313-autotekst,Account=ec313,State=COMPLETED,Start=2024-11-13T13:08:03+01:00,End=2024-11-13T13:10:31+01:00,ElapsedRaw=148,ReqCPUS=8,ReqMem=64G,ReqNodes=1,Submit=2024-11-13T12:43:25+01:00,SystemCPU=00:08.654,TimelimitRaw=15,UserCPU=02:44.072,NodeList=gpu-10,Partition=mig,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=nsk-transcription-job
v=0.1.0,JobID=974738.batch,JobIDRaw=974738.batch,Account=ec313,State=COMPLETED,Start=2024-11-13T13:08:03+01:00,End=2024-11-13T13:10:31+01:00,AveCPU=00:02:52,AveDiskRead=3318.92M,AveDiskWrite=73.20M,AveRSS=4108448K,ElapsedRaw=148,MaxRSS=4108448K,MinCPU=00:02:52,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:08:03+01:00,SystemCPU=00:08.653,UserCPU=02:44.072,NodeList=gpu-10,"AllocTRES=cpu=8,gres/gpu=1,mem=64G,node=1",JobName=batch
v=0.1.0,JobID=974738.extern,JobIDRaw=974738.extern,Account=ec313,State=COMPLETED,Start=2024-11-13T13:08:03+01:00,End=2024-11-13T13:10:31+01:00,AveDiskRead=0.01M,ElapsedRaw=148,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:08:03+01:00,SystemCPU=00:00.001,NodeList=gpu-10,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=extern
v=0.1.0,JobID=974739,JobIDRaw=974739,User=ec313-autotekst,Account=ec313,State=COMPLETED,Start=2024-11-13T13:08:18+01:00,End=2024-11-13T13:08:47+01:00,ElapsedRaw=29,ReqCPUS=8,ReqMem=64G,ReqNodes=1,Submit=2024-11-13T12:43:27+01:00,SystemCPU=00:10.226,TimelimitRaw=2,UserCPU=00:14.978,NodeList=gpu-10,Partition=mig,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=nsk-transcription-job
v=0.1.0,JobID=974739.batch,JobIDRaw=974739.batch,Account=ec313,State=COMPLETED,Start=2024-11-13T13:08:18+01:00,End=2024-11-13T13:08:47+01:00,AveCPU=00:00:24,AveRSS=1448K,ElapsedRaw=29,MaxRSS=1448K,MinCPU=00:00:24,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:08:18+01:00,SystemCPU=00:10.225,UserCPU=00:14.978,NodeList=gpu-10,"AllocTRES=cpu=8,gres/gpu=1,mem=64G,node=1",JobName=batch
v=0.1.0,JobID=974739.extern,JobIDRaw=974739.extern,Account=ec313,State=COMPLETED,Start=2024-11-13T13:08:18+01:00,End=2024-11-13T13:08:47+01:00,AveDiskRead=0.01M,ElapsedRaw=29,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:08:18+01:00,SystemCPU=00:00.001,NodeList=gpu-10,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=extern
v=0.1.0,JobID=974742,JobIDRaw=974742,User=ec313-autotekst,Account=ec313,State=COMPLETED,Start=2024-11-13T13:08:35+01:00,End=2024-11-13T13:09:00+01:00,ElapsedRaw=25,ReqCPUS=8,ReqMem=64G,ReqNodes=1,Submit=2024-11-13T12:44:21+01:00,SystemCPU=00:06.631,TimelimitRaw=2,UserCPU=00:10.521,NodeList=gpu-10,Partition=mig,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=nsk-transcription-job
v=0.1.0,JobID=974742.batch,JobIDRaw=974742.batch,Account=ec313,State=COMPLETED,Start=2024-11-13T13:08:35+01:00,End=2024-11-13T13:09:00+01:00,AveCPU=00:00:17,AveRSS=1240K,ElapsedRaw=25,MaxRSS=1240K,MinCPU=00:00:17,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:08:35+01:00,SystemCPU=00:06.629,UserCPU=00:10.521,NodeList=gpu-10,"AllocTRES=cpu=8,gres/gpu=1,mem=64G,node=1",JobName=batch
v=0.1.0,JobID=974742.extern,JobIDRaw=974742.extern,Account=ec313,State=COMPLETED,Start=2024-11-13T13:08:35+01:00,End=2024-11-13T13:09:00+01:00,AveDiskRead=0.01M,ElapsedRaw=25,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:08:35+01:00,SystemCPU=00:00.001,NodeList=gpu-10,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=extern
v=0.1.0,JobID=974745,JobIDRaw=974745,User=ec-ccccc,Account=ec35,State=CANCELLED by 2101477,Start=2024-11-13T12:45:27+01:00,End=2024-11-13T13:08:36+01:00,ElapsedRaw=1389,ReqCPUS=20,ReqMem=50G,ReqNodes=1,Submit=2024-11-13T12:45:27+01:00,SystemCPU=08:15.063,TimelimitRaw=80,UserCPU=53:52.638,NodeList=gpu-4,Partition=ifi_accel,"AllocTRES=billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1",JobName=test-cv
v=0.1.0,JobID=974745.batch,JobIDRaw=974745.batch,Account=ec35,State=CANCELLED,Start=2024-11-13T12:45:27+01:00,End=2024-11-13T13:08:37+01:00,AveDiskRead=0.18M,AveDiskWrite=0.13M,AveRSS=6068K,ElapsedRaw=1390,ExitCode=0:15,MaxRSS=6068K,ReqCPUS=20,ReqNodes=1,Submit=2024-11-13T12:45:27+01:00,SystemCPU=00:00.028,UserCPU=00:00.004,NodeList=gpu-4,"AllocTRES=cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1",JobName=batch
v=0.1.0,JobID=974745.extern,JobIDRaw=974745.extern,Account=ec35,State=COMPLETED,Start=2024-11-13T12:45:27+01:00,End=2024-11-13T13:08:40+01:00,AveDiskRead=0.01M,ElapsedRaw=1393,ReqCPUS=20,ReqNodes=1,Submit=2024-11-13T12:45:27+01:00,SystemCPU=00:00.002,NodeList=gpu-4,"AllocTRES=billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1",JobName=extern
v=0.1.0,JobID=974745.0,JobIDRaw=974745.0,Account=ec35,State=CANCELLED,Start=2024-11-13T12:45:28+01:00,End=2024-11-13T13:08:40+01:00,AveCPU=00:31:03,AveDiskRead=6029.92M,AveDiskWrite=0.07M,AveRSS=7029652K,ElapsedRaw=1392,ExitCode=0:15,Layout=Block,MaxRSS=7087640K,MinCPU=00:31:03,ReqCPUS=20,ReqNodes=1,Submit=2024-11-13T12:45:28+01:00,SystemCPU=08:15.032,UserCPU=53:52.634,NodeList=gpu-4,"AllocTRES=cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1",JobName=python
v=0.1.0,JobID=974746,JobIDRaw=974746,User=ec313-autotekst,Account=ec313,State=COMPLETED,Start=2024-11-13T13:08:48+01:00,End=2024-11-13T13:09:11+01:00,ElapsedRaw=23,ReqCPUS=8,ReqMem=64G,ReqNodes=1,Submit=2024-11-13T12:45:35+01:00,SystemCPU=00:06.736,TimelimitRaw=2,UserCPU=00:10.587,NodeList=gpu-10,Partition=mig,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=nsk-transcription-job
v=0.1.0,JobID=974746.batch,JobIDRaw=974746.batch,Account=ec313,State=COMPLETED,Start=2024-11-13T13:08:48+01:00,End=2024-11-13T13:09:11+01:00,AveCPU=00:00:17,AveRSS=668K,ElapsedRaw=23,MaxRSS=668K,MinCPU=00:00:17,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:08:48+01:00,SystemCPU=00:06.734,UserCPU=00:10.587,NodeList=gpu-10,"AllocTRES=cpu=8,gres/gpu=1,mem=64G,node=1",JobName=batch
v=0.1.0,JobID=974746.extern,JobIDRaw=974746.extern,Account=ec313,State=COMPLETED,Start=2024-11-13T13:08:48+01:00,End=2024-11-13T13:09:11+01:00,AveDiskRead=0.01M,ElapsedRaw=23,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:08:48+01:00,SystemCPU=00:00.001,NodeList=gpu-10,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=extern
v=0.1.0,JobID=974751,JobIDRaw=974751,User=ec313-autotekst,Account=ec313,State=COMPLETED,Start=2024-11-13T13:09:13+01:00,End=2024-11-13T13:09:58+01:00,ElapsedRaw=45,ReqCPUS=8,ReqMem=64G,ReqNodes=1,Submit=2024-11-13T12:46:51+01:00,SystemCPU=00:12.538,TimelimitRaw=2,UserCPU=00:21.303,NodeList=gpu-10,Partition=mig,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=uio-transcription-job
v=0.1.0,JobID=974751.batch,JobIDRaw=974751.batch,Account=ec313,State=COMPLETED,Start=2024-11-13T13:09:13+01:00,End=2024-11-13T13:09:58+01:00,AveCPU=00:00:33,AveDiskRead=3304.52M,AveDiskWrite=4.36M,AveRSS=321312K,ElapsedRaw=45,MaxRSS=321312K,MinCPU=00:00:33,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:09:13+01:00,SystemCPU=00:12.536,UserCPU=00:21.303,NodeList=gpu-10,"AllocTRES=cpu=8,gres/gpu=1,mem=64G,node=1",JobName=batch
v=0.1.0,JobID=974751.extern,JobIDRaw=974751.extern,Account=ec313,State=COMPLETED,Start=2024-11-13T13:09:13+01:00,End=2024-11-13T13:09:58+01:00,AveDiskRead=0.01M,ElapsedRaw=45,ReqCPUS=8,ReqNodes=1,Submit=2024-11-13T13:09:13+01:00,SystemCPU=00:00.001,NodeList=gpu-10,"AllocTRES=billing=12,cpu=8,gres/gpu=1,mem=64G,node=1",JobName=extern
v=0.1.0,JobID=974798,JobIDRaw=974798,User=ec-ddddd,Account=ec395,State=TIMEOUT,Start=2024-11-13T13:03:31+01:00,End=2024-11-13T13:08:58+01:00,ElapsedRaw=327,ReqCPUS=1,ReqMem=32G,ReqNodes=1,Submit=2024-11-13T13:03:08+01:00,SystemCPU=00:37.071,TimelimitRaw=5,UserCPU=04:30.995,NodeList=c1-17,Partition=normal,"AllocTRES=billing=8,cpu=1,mem=32G,node=1",JobName=cpu_job
v=0.1.0,JobID=974798.batch,JobIDRaw=974798.batch,Account=ec395,State=CANCELLED,Start=2024-11-13T13:03:31+01:00,End=2024-11-13T13:08:59+01:00,AveCPU=00:05:08,AveDiskRead=1787.60M,AveDiskWrite=2.13M,AveRSS=5734828K,ElapsedRaw=328,ExitCode=0:15,MaxRSS=5734828K,MinCPU=00:05:08,ReqCPUS=1,ReqNodes=1,Submit=2024-11-13T13:03:31+01:00,SystemCPU=00:37.070,UserCPU=04:30.995,NodeList=c1-17,"AllocTRES=cpu=1,mem=32G,node=1",JobName=batch
v=0.1.0,JobID=974798.extern,JobIDRaw=974798.extern,Account=ec395,State=COMPLETED,Start=2024-11-13T13:03:31+01:00,End=2024-11-13T13:08:59+01:00,AveDiskRead=0.01M,ElapsedRaw=328,ReqCPUS=1,ReqNodes=1,Submit=2024-11-13T13:03:31+01:00,SystemCPU=00:00.001,NodeList=c1-17,"AllocTRES=billing=8,cpu=1,mem=32G,node=1",JobName=extern
v=0.1.0,JobID=974804,JobIDRaw=974804,User=ec-eeeee,Account=ec395,State=TIMEOUT,Start=2024-11-13T13:05:32+01:00,End=2024-11-13T13:08:58+01:00,ElapsedRaw=206,ReqCPUS=1,ReqMem=32G,ReqNodes=1,Submit=2024-11-13T13:05:06+01:00,SystemCPU=00:04.084,TimelimitRaw=3,UserCPU=03:05.358,NodeList=gpu-9,Partition=accel,"AllocTRES=billing=19,cpu=1,gres/gpu:a100=1,gres/gpu=1,mem=32G,node=1",JobName=gpu_job
v=0.1.0,JobID=974804.batch,JobIDRaw=974804.batch,Account=ec395,State=CANCELLED,Start=2024-11-13T13:05:32+01:00,End=2024-11-13T13:08:59+01:00,AveCPU=00:03:09,AveDiskRead=207.33M,AveDiskWrite=0.08M,AveRSS=3148716K,ElapsedRaw=207,ExitCode=0:15,MaxRSS=3148716K,MinCPU=00:03:09,ReqCPUS=1,ReqNodes=1,Submit=2024-11-13T13:05:32+01:00,SystemCPU=00:04.083,UserCPU=03:05.357,NodeList=gpu-9,"AllocTRES=cpu=1,gres/gpu:a100=1,gres/gpu=1,mem=32G,node=1",JobName=batch
v=0.1.0,JobID=974804.extern,JobIDRaw=974804.extern,Account=ec395,State=COMPLETED,Start=2024-11-13T13:05:32+01:00,End=2024-11-13T13:08:59+01:00,AveDiskRead=0.01M,ElapsedRaw=207,ReqCPUS=1,ReqNodes=1,Submit=2024-11-13T13:05:32+01:00,SystemCPU=00:00.001,NodeList=gpu-9,"AllocTRES=billing=19,cpu=1,gres/gpu:a100=1,gres/gpu=1,mem=32G,node=1",JobName=extern
v=0.1.0,JobID=974806,JobIDRaw=974806,User=ec-ccccc,Account=ec35,State=CANCELLED by 2101477,Start=2024-11-13T13:06:23+01:00,End=2024-11-13T13:08:30+01:00,ElapsedRaw=127,ReqCPUS=20,ReqMem=50G,ReqNodes=1,Submit=2024-11-13T13:06:23+01:00,SystemCPU=00:39.637,TimelimitRaw=80,UserCPU=04:03.895,NodeList=gpu-4,Partition=ifi_accel,"AllocTRES=billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1",JobName=test-cv
v=0.1.0,JobID=974806.batch,JobIDRaw=974806.batch,Account=ec35,State=CANCELLED,Start=2024-11-13T13:06:23+01:00,End=2024-11-13T13:08:31+01:00,AveDiskRead=0.15M,AveDiskWrite=0.10M,AveRSS=6328K,ElapsedRaw=128,ExitCode=0:15,MaxRSS=6328K,ReqCPUS=20,ReqNodes=1,Submit=2024-11-13T13:06:23+01:00,SystemCPU=00:00.028,UserCPU=00:00.005,NodeList=gpu-4,"AllocTRES=cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1",JobName=batch
v=0.1.0,JobID=974806.extern,JobIDRaw=974806.extern,Account=ec35,State=COMPLETED,Start=2024-11-13T13:06:23+01:00,End=2024-11-13T13:08:34+01:00,AveDiskRead=0.01M,ElapsedRaw=131,ReqCPUS=20,ReqNodes=1,Submit=2024-11-13T13:06:23+01:00,SystemCPU=00:00.001,NodeList=gpu-4,"AllocTRES=billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1",JobName=extern
v=0.1.0,JobID=974806.0,JobIDRaw=974806.0,Account=ec35,State=CANCELLED,Start=2024-11-13T13:06:24+01:00,End=2024-11-13T13:08:33+01:00,AveCPU=00:02:21,AveDiskRead=686.46M,AveDiskWrite=0.06M,AveRSS=2749696K,ElapsedRaw=129,ExitCode=0:15,Layout=Block,MaxRSS=2768716K,MinCPU=00:02:21,ReqCPUS=20,ReqNodes=1,Submit=2024-11-13T13:06:24+01:00,SystemCPU=00:39.607,UserCPU=04:03.889,NodeList=gpu-4,"AllocTRES=cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1",JobName=python
v=0.1.0,JobID=974809,JobIDRaw=974809,User=fffff,Account=ec30,State=FAILED,Start=2024-11-13T13:07:33+01:00,End=2024-11-13T13:07:43+01:00,ElapsedRaw=10,ExitCode=1:0,ReqCPUS=4,ReqMem=16G,ReqNodes=1,Submit=2024-11-13T13:07:20+01:00,SystemCPU=00:00.001,TimelimitRaw=90,NodeList=c1-19,Partition=normal,"AllocTRES=billing=4,cpu=4,mem=16G,node=1",JobName=JUPYTER
v=0.1.0,JobID=974809.extern,JobIDRaw=974809.extern,Account=ec30,State=COMPLETED,Start=2024-11-13T13:07:33+01:00,End=2024-11-13T13:07:43+01:00,AveDiskRead=0.01M,ElapsedRaw=10,ReqCPUS=4,ReqNodes=1,Submit=2024-11-13T13:07:33+01:00,SystemCPU=00:00.001,NodeList=c1-19,"AllocTRES=billing=4,cpu=4,mem=16G,node=1",JobName=extern
v=0.1.0,JobID=974810,JobIDRaw=974810,User=fffff,Account=ec30,State=FAILED,Start=2024-11-13T13:07:33+01:00,End=2024-11-13T13:07:35+01:00,ElapsedRaw=2,ExitCode=2:0,ReqCPUS=4,ReqMem=16G,ReqNodes=1,Submit=2024-11-13T13:07:32+01:00,SystemCPU=00:00.004,TimelimitRaw=90,UserCPU=00:00.002,NodeList=c1-28,Partition=normal,"AllocTRES=billing=4,cpu=4,mem=16G,node=1",JobName=JUPYTER
v=0.1.0,JobID=974810.extern,JobIDRaw=974810.extern,Account=ec30,State=COMPLETED,Start=2024-11-13T13:07:33+01:00,End=2024-11-13T13:07:35+01:00,AveDiskRead=0.01M,ElapsedRaw=2,ReqCPUS=4,ReqNodes=1,Submit=2024-11-13T13:07:33+01:00,SystemCPU=00:00.001,NodeList=c1-28,"AllocTRES=billing=4,cpu=4,mem=16G,node=1",JobName=extern
v=0.1.0,JobID=974810.0,JobIDRaw=974810.0,Account=ec30,State=FAILED,Start=2024-11-13T13:07:34+01:00,End=2024-11-13T13:07:35+01:00,AveRSS=72K,ElapsedRaw=1,ExitCode=2:0,Layout=Block,MaxRSS=72K,ReqCPUS=4,ReqNodes=1,Submit=2024-11-13T13:07:34+01:00,SystemCPU=00:00.002,UserCPU=00:00.002,NodeList=c1-28,"AllocTRES=cpu=4,mem=16G,node=1",JobName=JUPYTER
v=0.1.0,JobID=974819,JobIDRaw=974819,User=ec-ccccc,Account=ec35,State=CANCELLED by 2101477,Start=2024-11-13T13:09:19+01:00,End=2024-11-13T13:09:32+01:00,ElapsedRaw=13,ReqCPUS=20,ReqMem=50G,ReqNodes=1,Submit=2024-11-13T13:09:16+01:00,SystemCPU=00:04.790,TimelimitRaw=80,UserCPU=00:05.694,NodeList=gpu-4,Partition=ifi_accel,"AllocTRES=billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1",JobName=test-cv
v=0.1.0,JobID=974819.batch,JobIDRaw=974819.batch,Account=ec35,State=CANCELLED,Start=2024-11-13T13:09:19+01:00,End=2024-11-13T13:09:33+01:00,AveRSS=5956K,ElapsedRaw=14,ExitCode=0:15,MaxRSS=5956K,ReqCPUS=20,ReqNodes=1,Submit=2024-11-13T13:09:19+01:00,SystemCPU=00:00.027,UserCPU=00:00.005,NodeList=gpu-4,"AllocTRES=cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1",JobName=batch
v=0.1.0,JobID=974819.extern,JobIDRaw=974819.extern,Account=ec35,State=COMPLETED,Start=2024-11-13T13:09:19+01:00,End=2024-11-13T13:09:34+01:00,AveDiskRead=0.01M,ElapsedRaw=15,ReqCPUS=20,ReqNodes=1,Submit=2024-11-13T13:09:19+01:00,SystemCPU=00:00.001,NodeList=gpu-4,"AllocTRES=billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1",JobName=extern
v=0.1.0,JobID=974819.0,JobIDRaw=974819.0,Account=ec35,State=CANCELLED,Start=2024-11-13T13:09:19+01:00,End=2024-11-13T13:09:34+01:00,AveCPU=00:00:05,AveDiskRead=19.75M,AveRSS=54518K,ElapsedRaw=15,ExitCode=0:15,Layout=Block,MaxRSS=79648K,MinCPU=00:00:05,ReqCPUS=20,ReqNodes=1,Submit=2024-11-13T13:09:19+01:00,SystemCPU=00:04.761,UserCPU=00:05.688,NodeList=gpu-4,"AllocTRES=cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1",JobName=python
"#;

    if sacct_output != expected {
        let xs = &output;
        let ys = expected.as_bytes();
        if xs.len() != ys.len() {
            print!("Lengths differ: {} {}\n", xs.len(), ys.len());
            print!("{:?}", output);
            assert!(false);
        }
        for i in 0..xs.len() {
            if xs[i] != ys[i] {
                print!("Failing at {i}: {} {}\n", xs[i], ys[i]);
                print!("{:?}", output);
                assert!(false);
            }
        }
    }
}
