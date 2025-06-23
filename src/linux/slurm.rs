use crate::linux::procfsapi;

// This returns Some(n) where n > 0 if we could parse the job ID, Some(0) if the overall pattern
// matched but the ID was not parseable, or None otherwise.  Thus None is a signal to fall back to
// other (non-Slurm) mechanisms.

pub fn get_job_id(fs: &dyn procfsapi::ProcfsAPI, pid: usize) -> Option<usize> {
    match fs.read_to_string(&format!("{pid}/cgroup")) {
        Ok(text) => {
            // We want \1 of the first line that matches "/job_(.*?)/"
            //
            // The reason is that there can be several lines in that file that look roughly like
            // this, with different contents (except for the job info) but with the pattern the
            // same:
            //
            //    10:devices:/slurm/uid_2101171/job_280678/step_interactive/task_0
            //
            // It could be that we should match all of `/slurm.*/job_(\d+)/` to be on the safe side,
            // everything I've seen fits that, but not all use `/slurm/`, as seen in the test cases
            // below.

            for l in text.split('\n') {
                if let Some(x) = l.find("/job_") {
                    if let Some(y) = l[x + 5..].find('/') {
                        // Pattern found, so commit
                        return match l[x + 5..x + 5 + y].trim().parse::<usize>() {
                            Ok(n) => Some(n),
                            _ => Some(0),
                        };
                    }
                }
            }
            // Readable lines exhausted
            None
        }
        Err(_) => None,
    }
}
