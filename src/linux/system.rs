use crate::command;
use crate::gpu;
use crate::gpu::realgpu;
use crate::hostname;
use crate::jobsapi;
use crate::linux::procfs;
use crate::linux::slurm;
use crate::systemapi;
use crate::time;
use crate::users;
use crate::util;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs;
use std::io;
#[cfg(debug_assertions)]
use std::io::{Read, Write};
use std::os::linux::fs::MetadataExt;
use std::path;
#[cfg(debug_assertions)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use signal_hook::consts::signal;
use signal_hook::flag;

// 3 minutes ought to be enough for anyone.
const SACCT_TIMEOUT_S: u64 = 180;

// scontrol is also pretty quick
const SCONTROL_TIMEOUT_S: u64 = 30;

// sinfo will normally be very quick
const SINFO_TIMEOUT_S: u64 = 10;

pub struct Builder {
    jm: Option<Box<dyn jobsapi::JobManager>>,
    cluster: String,
    node_domain: Option<Vec<String>>,
    sacct: String,
    scontrol: String,
    sinfo: String,
    topo_svg: Option<String>,
    topo_text: Option<String>,
}

impl Builder {
    pub fn new() -> Builder {
        Builder {
            jm: None,
            cluster: "".to_string(),
            node_domain: None,
            sacct: "sacct".to_string(),
            scontrol: "scontrol".to_string(),
            sinfo: "sinfo".to_string(),
            topo_svg: None,
            topo_text: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_node_domain(self, domain: &[String]) -> Builder {
        Builder {
            node_domain: Some(domain.iter().map(|x| x.clone()).collect::<Vec<String>>()),
            ..self
        }
    }

    #[allow(dead_code)]
    pub fn with_cluster(self, cluster: &str) -> Builder {
        Builder {
            cluster: cluster.to_string(),
            ..self
        }
    }

    pub fn with_jobmanager(self, jm: Box<dyn jobsapi::JobManager>) -> Builder {
        Builder {
            jm: Some(jm),
            ..self
        }
    }

    #[allow(dead_code)]
    pub fn with_sacct_cmd(self, cmd: &str) -> Builder {
        Builder {
            sacct: cmd.to_string(),
            ..self
        }
    }

    #[allow(dead_code)]
    pub fn with_sinfo_cmd(self, cmd: &str) -> Builder {
        Builder {
            sinfo: cmd.to_string(),
            ..self
        }
    }

    #[allow(dead_code)]
    pub fn with_scontrol_cmd(self, cmd: &str) -> Builder {
        Builder {
            scontrol: cmd.to_string(),
            ..self
        }
    }

    #[allow(dead_code)]
    pub fn with_topo_svg_cmd(self, cmd: &str) -> Builder {
        Builder {
            topo_svg: Some(cmd.to_string()),
            ..self
        }
    }

    #[allow(dead_code)]
    pub fn with_topo_text_cmd(self, cmd: &str) -> Builder {
        Builder {
            topo_text: Some(cmd.to_string()),
            ..self
        }
    }

    pub fn freeze(self) -> Result<System, String> {
        let fs = RealProcFS {};
        let boot_time = procfs::get_boot_time_in_secs_since_epoch(&fs)?;
        let hostname = hostname::get();
        Ok(System {
            hostname: hostname.clone(),
            node_domain: self.node_domain,
            cluster: self.cluster,
            jm: if let Some(x) = self.jm {
                x
            } else {
                Box::new(jobsapi::NoJobManager::new())
            },
            fs,
            gpus: realgpu::RealGpu::new(hostname, boot_time),
            timestamp: RefCell::new(time::now_iso8601()),
            now: Cell::new(time::unix_now()),
            boot_time,
            interrupted: Arc::new(AtomicBool::new(false)),
            cpu_info: RefCell::new(None),
            sacct: self.sacct,
            scontrol: self.scontrol,
            sinfo: self.sinfo,
            topo_svg: self.topo_svg,
            topo_text: self.topo_text,
        })
    }
}

// The entire suffix of hostname must match a prefix of domain, and in that case we attach the rest
// of domain, otherwise we attach the entire domain to hostname.
#[allow(dead_code)]
fn expand_domain(hostname: String, domain: &[String]) -> String {
    let mut full = hostname
        .split('.')
        .map(|x| x.to_string())
        .collect::<Vec<String>>();
    let mut f = 1;
    let mut d = 0;
    let mut matched = true;
    while f < full.len() && d < domain.len() && matched {
        if full[f] != domain[d] {
            matched = false;
            break;
        }
        f += 1;
        d += 1;
    }
    if matched && f == full.len() {
        for de in domain[d..].iter() {
            full.push(de.to_string())
        }
    } else {
        for de in domain {
            full.push(de.to_string());
        }
    }
    full.join(".")
}

#[test]
fn test_expand_domain() {
    assert!(expand_domain("a".to_string(), &[]) == "a");
    assert!(expand_domain("a.b.c".to_string(), &[]) == "a.b.c");
    assert!(expand_domain("a.b".to_string(), &["c".to_string()]) == "a.b.c");
    assert!(expand_domain("a.b".to_string(), &["b".to_string(), "c".to_string()]) == "a.b.c");
    assert!(expand_domain("a.b.c".to_string(), &["b".to_string(), "c".to_string()]) == "a.b.c");
    assert!(
        expand_domain("a.b.c.d".to_string(), &["b".to_string(), "c".to_string()]) == "a.b.c.d.b.c"
    );
    assert!(
        expand_domain(
            "a.b".to_string(),
            &["c".to_string(), "d".to_string(), "e".to_string()]
        ) == "a.b.c.d.e"
    );
}

#[cfg(target_arch = "x86_64")]
const ARCHITECTURE: &str = "x86_64";

#[cfg(target_arch = "aarch64")]
const ARCHITECTURE: &'static str = "aarch64";

// Otherwise, `ARCHITECTURE` will be undefined and we'll get a compile error; rustc is good about
// notifying the user that there are ifdef'd cases that are inactive.

pub struct System {
    hostname: String,
    node_domain: Option<Vec<String>>,
    cluster: String,
    fs: RealProcFS,
    gpus: realgpu::RealGpu,
    jm: Box<dyn jobsapi::JobManager>,
    timestamp: RefCell<String>,
    now: Cell<u64>,
    boot_time: u64,
    interrupted: Arc<AtomicBool>,
    cpu_info: RefCell<Option<systemapi::CpuInfo>>,
    sacct: String,
    sinfo: String,
    scontrol: String,
    topo_svg: Option<String>,
    topo_text: Option<String>,
}

impl System {
    // We want the time to be stable (ie unchanging if we read it multiple times), but that also
    // means that when it must move we must move it specifically.  This is not yet part of the
    // SystemAPI because that's not been necessary.
    #[cfg(feature = "daemon")]
    pub fn update_time(&self) {
        self.timestamp.replace(time::now_iso8601());
        self.now.set(time::unix_now());
    }
}

impl systemapi::SystemAPI for System {
    fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn get_timestamp(&self) -> String {
        self.timestamp.borrow().clone()
    }

    fn get_cluster(&self) -> String {
        self.cluster.clone()
    }

    fn get_node_domain(&self) -> &Option<Vec<String>> {
        &self.node_domain
    }

    fn get_hostname(&self) -> String {
        self.hostname.clone()
    }

    fn get_os_name(&self) -> String {
        unsafe {
            let mut utsname: libc::utsname = std::mem::zeroed();
            if libc::uname(&mut utsname) != 0 {
                return "".to_string();
            }
            util::cstrdup(&utsname.sysname)
        }
    }

    fn get_os_release(&self) -> String {
        unsafe {
            let mut utsname: libc::utsname = std::mem::zeroed();
            if libc::uname(&mut utsname) != 0 {
                return "".to_string();
            }
            util::cstrdup(&utsname.release)
        }
    }

    fn get_architecture(&self) -> String {
        ARCHITECTURE.to_string()
    }

    fn get_gpus(&self) -> &dyn gpu::GpuAPI {
        &self.gpus
    }

    fn get_jobs(&self) -> &dyn jobsapi::JobManager {
        &*self.jm
    }

    fn get_pid(&self) -> u32 {
        std::process::id()
    }

    fn get_boot_time_in_secs_since_epoch(&self) -> u64 {
        self.boot_time
    }

    fn get_clock_ticks_per_sec(&self) -> usize {
        // We're assuming this never changes while the system is running.
        unsafe { libc::sysconf(libc::_SC_CLK_TCK) as usize }
    }

    fn get_page_size_in_kib(&self) -> usize {
        // We're assuming this never changes while the system is running.
        (unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }) / 1024
    }

    fn get_now_in_secs_since_epoch(&self) -> u64 {
        self.now.get()
    }

    fn get_cpu_info(&self) -> Result<systemapi::CpuInfo, String> {
        if self.cpu_info.borrow().is_some() {
            Ok(self.cpu_info.borrow().clone().unwrap())
        } else {
            let info = procfs::get_cpu_info(&self.fs)?;
            self.cpu_info.replace(Some(info.clone()));
            Ok(info)
        }
    }

    fn get_memory_in_kib(&self) -> Result<systemapi::Memory, String> {
        procfs::get_memory_in_kib(&self.fs)
    }

    fn get_numa_distances(&self) -> Result<Vec<Vec<u32>>, String> {
        get_numa_distances(self)
    }

    fn compute_node_information(&self) -> Result<(u64, Vec<u64>), String> {
        procfs::compute_node_information(self, &self.fs)
    }

    fn compute_loadavg(&self) -> Result<(f64, f64, f64, u64, u64), String> {
        procfs::compute_loadavg(&self.fs)
    }

    fn compute_process_information(&self) -> Result<HashMap<usize, systemapi::Process>, String> {
        procfs::compute_process_information(self, &self.fs)
    }

    fn compute_cpu_utilization(
        &self,
        processes: &HashMap<usize, systemapi::Process>,
        wait_time_ms: usize,
    ) -> Result<Vec<(usize, f64)>, String> {
        procfs::compute_cpu_utilization(self, &self.fs, processes, wait_time_ms)
    }

    fn compute_slurm_job_id(&self, pid: usize) -> Option<usize> {
        slurm::get_job_id(&self.fs, pid)
    }

    fn compute_node_topo_svg(&self) -> Result<Option<String>, String> {
        if let Some(ref cmd) = self.topo_svg {
            Ok(run_command_unsafely(cmd))
        } else {
            Ok(None)
        }
    }

    fn compute_node_topo_text(&self) -> Result<Option<String>, String> {
        if let Some(ref cmd) = self.topo_text {
            Ok(run_command_unsafely(cmd))
        } else {
            Ok(None)
        }
    }

    fn compute_user_by_uid(&self, uid: u32) -> Option<String> {
        users::lookup_user_by_uid(uid).map(|u| u.to_string_lossy().to_string())
    }

    fn create_lock_file(&self, p: &path::PathBuf) -> io::Result<fs::File> {
        fs::File::options().write(true).create_new(true).open(p)
    }

    fn remove_lock_file(&self, p: path::PathBuf) -> io::Result<()> {
        fs::remove_file(p)
    }

    fn read_node_file_to_string(&self, f: &str) -> io::Result<String> {
        let filename = format!("/sys/devices/system/node/{f}");
        fs::read_to_string(path::Path::new(&filename))
    }

    fn run_sacct(
        &self,
        job_states: &[&str],
        field_names: &[&str],
        from: &str,
        to: &str,
    ) -> Result<String, String> {
        #[cfg(debug_assertions)]
        if let Ok(filename) = std::env::var("SONARTEST_MOCK_SACCT") {
            return Ok(mock_input(filename));
        }
        if self.sacct == "" {
            return Ok("".to_string());
        }
        runit(
            &self.sacct,
            &[
                "-aP",
                "-s",
                &job_states.join(","),
                "--noheader",
                "-o",
                &field_names.join(","),
                "-S",
                from,
                "-E",
                to,
            ],
            SACCT_TIMEOUT_S,
        )
    }

    fn run_scontrol(&self) -> Result<String, String> {
        #[cfg(debug_assertions)]
        if let Ok(filename) = std::env::var("SONARTEST_MOCK_SCONTROL") {
            return Ok(mock_input(filename));
        }
        if self.scontrol == "" {
            return Ok("".to_string());
        }
        runit(&self.scontrol, &["-o", "show", "job"], SCONTROL_TIMEOUT_S)
    }

    // Whether we try to run sinfo or run some code to look for the program in the path probably
    // does not matter, and the former is easier.  We could/should cache this value in case the
    // client wants it repeatedly.

    fn compute_cluster_kind(&self) -> Option<systemapi::ClusterKind> {
        #[cfg(debug_assertions)]
        if std::env::var("SONARTEST_MOCK_PARTITIONS").is_ok()
            || std::env::var("SONARTEST_MOCK_NODES").is_ok()
        {
            return Some(systemapi::ClusterKind::Slurm);
        }
        if self.sinfo == "" {
            return None;
        }
        match runit(&self.sinfo, &["--usage"], SINFO_TIMEOUT_S) {
            Ok(_) => Some(systemapi::ClusterKind::Slurm),
            Err(_) => None,
        }
    }

    // Basically the `cluster` operations wrap `sinfo`:
    //
    //  - Run sinfo to list partitions.
    //  - Run sinfo to get a list of nodes broken down by partition and their state.
    //
    // The same could have been had in a different form by:
    //
    //  scontrol -o show nodes
    //  scontrol -o show partitions
    //
    // Anyway, we emit a list of partitions with their nodes and a list of nodes with their states.

    fn compute_cluster_partitions(&self) -> Result<Vec<(String, String)>, String> {
        let mut input = None;
        #[cfg(debug_assertions)]
        if let Ok(filename) = std::env::var("SONARTEST_MOCK_PARTITIONS") {
            input = Some(mock_input(filename));
        }
        if input.is_none() {
            if self.sinfo == "" {
                return Ok(vec![]);
            }
            input = Some(runit(
                &self.sinfo,
                &["-h", "-a", "-O", "Partition:|,NodeList:|"],
                SINFO_TIMEOUT_S,
            )?);
        }
        twofields(input.unwrap())
    }

    fn compute_cluster_nodes(&self) -> Result<Vec<(String, String)>, String> {
        let mut input = None;
        #[cfg(debug_assertions)]
        if let Ok(filename) = std::env::var("SONARTEST_MOCK_NODES") {
            input = Some(mock_input(filename));
        }
        if input.is_none() {
            if self.sinfo == "" {
                return Ok(vec![]);
            }
            input = Some(runit(
                &self.sinfo,
                &["-h", "-a", "-e", "-O", "NodeList:|,StateComplete:|"],
                SINFO_TIMEOUT_S,
            )?);
        }
        twofields(input.unwrap())
    }

    // Assuming no bugs, the interesting interrupt signals are SIGHUP, SIGTERM, SIGINT, and SIGQUIT.
    // Of these, only SIGHUP and SIGTERM are really interesting because they are sent by the OS or
    // by job control (and will often be followed by SIGKILL if not honored within some reasonable
    // time); INT/QUIT are sent by a user in response to keyboard action and more typical during
    // development/debugging.

    fn handle_interruptions(&self) {
        let _ = flag::register(signal::SIGTERM, Arc::clone(&self.interrupted));
        let _ = flag::register(signal::SIGHUP, Arc::clone(&self.interrupted));
    }

    #[cfg(not(debug_assertions))]
    fn is_interrupted(&self) -> bool {
        self.interrupted.load(Ordering::Relaxed)
    }

    #[cfg(debug_assertions)]
    fn is_interrupted(&self) -> bool {
        if std::env::var("SONARTEST_WAIT_INTERRUPT").is_ok() {
            std::thread::sleep(std::time::Duration::new(10, 0));
        }
        let flag = self.interrupted.load(Ordering::Relaxed);
        if flag {
            // Test cases depend on this exact output.
            log::debug!("Interrupt flag was set!")
        }
        flag
    }
}

#[cfg(debug_assertions)]
fn mock_input(filename: String) -> String {
    match fs::File::open(&filename) {
        Ok(mut f) => {
            let mut buf = String::new();
            match f.read_to_string(&mut buf) {
                Ok(_) => buf,
                Err(e) => {
                    panic!("Could not read sacct input file {filename}: {e}");
                }
            }
        }
        Err(e) => {
            panic!("Could not open sacct input filename {filename}: {e}")
        }
    }
}

// This is atomic only to allow it to be a global variable w/o using unsafe, there's no threading
// here, and Relaxed is sufficient.
#[cfg(debug_assertions)]
static FILE_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn runit(cmd: &str, args: &[&str], timeout: u64) -> Result<String, String> {
    match command::safe_command(cmd, args, timeout) {
        Ok((subcommand_output, _)) => {
            #[cfg(debug_assertions)]
            if let Ok(ref filename) = std::env::var("SONARTEST_SUBCOMMAND_OUTPUT") {
                eprintln!("{cmd} {:?}", args);
                let filename = match FILE_COUNTER.fetch_add(1, Ordering::Relaxed) {
                    0 => filename.to_string(),
                    n => format!("{filename}.{n}"),
                };
                match fs::File::create(&filename) {
                    Ok(mut f) => match f.write_all(subcommand_output.as_bytes()) {
                        Ok(()) => {}
                        Err(e) => {
                            panic!("Could not write subcommand output file {filename}: {e}");
                        }
                    },
                    Err(e) => {
                        panic!("Could not open subcommand output file {filename}: {e}");
                    }
                }
            }
            Ok(subcommand_output)
        }
        Err(command::CmdError::CouldNotStart(e)) => Err(e),
        Err(command::CmdError::Failed(e)) => Err(e),
        Err(command::CmdError::Hung(e)) => Err(e),
        Err(command::CmdError::InternalError(e)) => Err(e),
    }
}

// "Unsafely" because technically both the verb and args can contain spaces, but there's no way to
// express that.
fn run_command_unsafely(cmd: &str) -> Option<String> {
    let mut tokens = cmd.split_ascii_whitespace();
    match tokens.next() {
        Some(verb) => {
            let args = tokens.collect::<Vec<&str>>();
            match command::safe_command(verb, &args, 5) {
                Ok((s, _)) => Some(s),
                Err(_) => None,
            }
        }
        None => None,
    }
}

fn twofields(text: String) -> Result<Vec<(String, String)>, String> {
    let mut v = vec![];
    for l in text.lines() {
        let mut fields = l.split('|');
        let a = fields.next().ok_or("Bad sinfo output")?;
        let b = fields.next().ok_or("Bad sinfo output")?;
        v.push((a.to_string(), b.to_string()));
    }
    Ok(v)
}

struct RealProcFS {}

impl procfs::ProcfsAPI for RealProcFS {
    fn read_to_string(&self, path: &str) -> Result<String, String> {
        let filename = format!("/proc/{path}");
        match fs::read_to_string(path::Path::new(&filename)) {
            Ok(s) => Ok(s),
            Err(_) => Err(format!("Unable to read {filename}")),
        }
    }

    fn read_numeric_file_names(&self, path: &str) -> Result<Vec<(usize, u32)>, String> {
        let mut pids = vec![];
        let dir = if path == "" {
            "/proc".to_string()
        } else {
            format!("/proc/{path}/")
        };
        if let Ok(dir) = fs::read_dir(&dir) {
            for dirent in dir.flatten() {
                if let Ok(meta) = dirent.metadata() {
                    let owner = meta.st_uid();
                    if let Some(name) = dirent.path().file_name() {
                        if let Ok(name) = name.to_string_lossy().parse::<usize>() {
                            pids.push((name, owner));
                        }
                    }
                }
            }
        } else {
            return Err("Could not open {dir}".to_string());
        };
        Ok(pids)
    }
}

pub fn get_numa_distances(system: &dyn systemapi::SystemAPI) -> Result<Vec<Vec<u32>>, String> {
    let mut m = vec![];
    // The number of NUMA nodes is no greater than the number of sockets but it is frequently
    // smaller.  The rule here is that we enumerate the files and we will expect a dense range
    // (even though numactl currently handles a sparse range), but if there's any kind of hiccup
    // we fall back to returning an empty matrix, it is only if we can't parse a file that we
    // could read that we return an error.
    for n in 0..system.get_cpu_info()?.sockets {
        match system.read_node_file_to_string(&format!("node{n}/distance")) {
            Ok(s) => {
                let mut r = vec![];
                for v in s.trim().split(" ") {
                    match v.parse::<u32>() {
                        Ok(n) => r.push(n),
                        Err(_) => {
                            return Err(format!("Unable to parse {s}"));
                        }
                    }
                }
                m.push(r);
            }
            Err(_) => break,
        }
    }
    // Check that we have a square matrix.
    if m.len() > 0 {
        let n = m[0].len();
        if m.len() != n {
            return Ok(vec![]);
        }
        for i in 1..m.len() {
            if m[i].len() != n {
                return Ok(vec![]);
            }
        }
    }
    Ok(m)
}
