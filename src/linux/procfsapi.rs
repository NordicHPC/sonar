pub trait ProcfsAPI {
    // Open /proc/<path> (which can have multiple path elements, eg, {PID}/filename), read it, and
    // return its entire contents as a string.  Return a sensible error message if the file can't
    // be opened or read.
    fn read_to_string(&self, path: &str) -> Result<String, String>;

    // Return (name,owner-uid) for every file /proc/<path>/{name} where path can be empty.  Return a
    // sensible error message in case something goes really, really wrong, but otherwise try to make
    // the best of it.
    fn read_numeric_file_names(&self, path: &str) -> Result<Vec<(usize, u32)>, String>;
}
