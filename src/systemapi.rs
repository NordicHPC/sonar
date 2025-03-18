use crate::gpuapi;
use crate::jobsapi;
use crate::procfsapi;

use std::fs;
use std::io;
use std::path;

pub trait SystemAPI {
    // The `get_` methods always return the same values for every call.
    fn get_version(&self) -> String;
    fn get_timestamp(&self) -> String;
    fn get_cluster(&self) -> String;
    fn get_hostname(&self) -> String;
    fn get_os_name(&self) -> String;
    fn get_os_release(&self) -> String;
    fn get_architecture(&self) -> String;
    fn get_clock_ticks_per_sec(&self) -> usize;
    fn get_page_size_in_kib(&self) -> usize;
    fn get_now_in_secs_since_epoch(&self) -> u64;
    fn get_pid(&self) -> u32;
    fn get_boot_time(&self) -> u64;
    fn get_procfs(&self) -> &dyn procfsapi::ProcfsAPI;
    fn get_gpus(&self) -> &dyn gpuapi::GpuAPI;
    fn get_jobs(&self) -> &dyn jobsapi::JobManager;

    // Try to figure out the user's name from system tables, this may be an expensive operation.
    // There's a tiny risk that the answer could change between two calls (if a user were added
    // and/or removed).
    fn user_by_uid(&self, uid: u32) -> Option<String>;

    // Run sacct and return its output.  The arguments are passed on to sacct: `job_states` to `-s`,
    // `field_names` to `-o`, `from` to `-S` and `to` to `-E`.  This is only defined for state and
    // field names that exist, and for properly slurm-formatted dates.
    fn run_sacct(
        &self,
        job_states: &[&str],
        field_names: &[&str],
        from: &str,
        to: &str,
    ) -> Result<String, String>;

    // `create_lock_file` creates it atomically if it does not exist, returning Ok if so; if it does
    // exist, returns Err(io::ErrorKind::AlreadyExists), otherwise some other Err.
    // `remove_lock_file` unconditionally tries to remove the file, returning some Err if it fails.
    fn create_lock_file(&self, p: &path::PathBuf) -> io::Result<fs::File>;
    fn remove_lock_file(&self, p: path::PathBuf) -> io::Result<()>;

    // `handle_interruptions` enables interrupt checking; `is_interrupted` returns true if an
    // interrupt has been received.
    fn handle_interruptions(&self);
    fn is_interrupted(&self) -> bool;
}
