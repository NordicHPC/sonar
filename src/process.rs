// Run "ps" and return a vector of structures with all the information we need.

use crate::command;
use crate::jobs;
use crate::util;

#[derive(PartialEq)]
pub struct Process {
    pub pid: usize,
    pub user: String,
    pub cpu_pct: f64,
    pub mem_pct: f64,
    pub mem_size_kib: usize,
    pub command: String,
    pub session: String,         // "" if !jobs.need_process_tree()
}

pub fn get_process_information(jobs: &mut dyn jobs::JobManager) -> Vec<Process> {
    let need_process_tree = jobs.need_process_tree();
    if let Some(out) = command::safe_command(
        if need_process_tree {
            PS_COMMAND_COMPLETE
        } else {
            PS_COMMAND_FILTERED
        },
        TIMEOUT_SECONDS,
    ) {
        parse_ps_output(&out, need_process_tree)
    } else {
        vec![]
    }
}

const TIMEOUT_SECONDS: u64 = 2; // for `ps`

const PS_COMMAND_FILTERED: &str =
    "ps -e --no-header -o pid,user:22,pcpu,pmem,size,comm | grep -v ' 0.0  0.0 '";

const PS_COMMAND_COMPLETE: &str = "ps -e --no-header -o pid,user:22,pcpu,pmem,size,sess,comm";

fn parse_ps_output(raw_text: &str, complete_output: bool) -> Vec<Process> {
    raw_text
        .lines()
        .map(|line| {
            let (start_indices, parts) = util::chunks(line);
            Process {
                pid: parts[0].parse::<usize>().unwrap(),
                user: parts[1].to_string(),
                cpu_pct: parts[2].parse::<f64>().unwrap(),
                mem_pct: parts[3].parse::<f64>().unwrap(),
                mem_size_kib: parts[4].parse::<usize>().unwrap(),
                session: if complete_output { parts[5].to_string() } else { "".to_string() },
                // this is done because command can have spaces
                command: line[start_indices[if complete_output { 6 } else { 5 }]..].to_string(),
            }
        })
        .collect::<Vec<Process>>()
}

#[cfg(test)]
pub fn parsed_partial_test_output() -> Vec<Process> {
    let text = "   2022 bob                            10.0 20.0 553348 slack
  42178 bob                            10.0 15.0 353348 chromium
  42178 bob                            10.0 15.0  5536 chromium
  42189 alice                          10.0  5.0  5528 slack
  42191 bob                            10.0  5.0  5552 someapp
  42213 alice                          10.0  5.0 348904 some app
  42213 alice                          10.0  5.0 135364 some app";

    parse_ps_output(text, false)
}

#[test]
fn test_parse_ps_output() {
    macro_rules! proc(
	{ $a:expr, $b:expr, $c:expr, $d:expr, $e: expr, $f:expr } => {
	    Process { pid: $a,
		      user: $b.to_string(),
		      cpu_pct: $c,
		      mem_pct: $d,
		      mem_size_kib: $e,
		      command: $f.to_string()
	    }
	});

    assert!(parsed_partial_test_output().into_iter().eq(vec![
        proc! {  2022, "bob",   10.0, 20.0, 553348, "slack" },
        proc! { 42178, "bob",   10.0, 15.0, 353348, "chromium" },
        proc! { 42178, "bob",   10.0, 15.0,   5536, "chromium" },
        proc! { 42189, "alice", 10.0,  5.0,   5528, "slack" },
        proc! { 42191, "bob",   10.0,  5.0,   5552, "someapp" },
        proc! { 42213, "alice", 10.0,  5.0, 348904, "some app" },
        proc! { 42213, "alice", 10.0,  5.0, 135364, "some app" }
    ]))
}
