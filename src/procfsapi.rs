pub trait ProcfsAPI {
    // Open /proc/<path> (which can have multiple path elements, eg, {PID}/filename), read it, and
    // return its entire contents as a string.  Return a sensible error message if the file can't
    // be opened or read.
    fn read_to_string(&self, path: &str) -> Result<String, String>;

    // Return (pid,uid) for every file /proc/{PID}.  Return a sensible error message in case
    // something goes really, really wrong, but otherwise try to make the best of it.
    fn read_proc_pids(&self) -> Result<Vec<(usize, u32)>, String>;
}
