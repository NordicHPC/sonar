// The PidMap is used for rolled-up pid synthesis.  When a sample is processed, a new rolled-up
// process - in the form of a (job-id, parent-pid, command-name) triple - is entered into the map
// and holds the synthesized pid of that process.  When the same triple is encountered in subsequent
// samples it gets the same pid.  If a sample does not have any data for the rolled-up process the
// synthesized pid may be garbage collected.
//
// A synthesized pid is taken from outside the system pid range.  Synthesized pids come from a large
// range but can still be reused.
//
// Semantically it is desirable that a synthesized pid is not reused within the same job.  It's
// impossible to guarantee that, though it is very, very likely to work out if we manage to reuse
// pids in more or less LRU order.  With a large enough PID space, simply cycling through the space
// and always picking the next available one is likely to be a good approximation to that.
//
// The PidMap has two parts: a map of (job,parent,command) -> pid and a data structure for picking
// free PIDs.
//
// The free PIDs can be represented either as a list of PIDs that are in use (which must be skipped
// to find the next free one), or as a list of PIDs that are free (from which we can pick a free
// one), along with a cursor pointing into that set (to implement some quasi-LRU scheme).  Here we
// maintain a list of PIDs that are free as a stack of ranges of free PIDs where the deeper entries
// have higher PID values; the "current" element is not on the stack but in the pidmap itself.  This
// is coupled with a simple garbage collector that will rebuild that stack from the PID map.
//
// A rolled-up process in the map is born dirty.  Whenever it is encountered during subsequent
// sampling it is also marked dirty.  At the end of processing a full set of samples, the map is
// scanned and clean elements are removed, following which all the dirty elements that remain become
// clean.  The garbage collector creates a sorted list of the active pids and then builds a set of
// ranges of available pids from that, and pushes those ranges onto the stack in reverse order.
//
// The set of active synthesized pids is expected to be small - at most a few thousand elements even
// on very large and busy nodes, and typically much less than that.

use crate::systemapi;
use crate::types::{JobID, Pid};
use std::collections::HashMap;

// These parameters are sensible for a "large enough" pid range, but can be set to other, more
// aggressive, values during testing with the SONARTEST_ROLLUP_PIDS env var.  That var should have
// the form p,r where p is the number of available pids and r is the minimum pid range size to keep
// after garbage collection.  Both are optional.

// The upper limit on the pid range, exclusive.
const PID_LIMIT: u64 = std::u64::MAX;

// Ranges with fewer than this many elements are not retained in the free pool, to keep its size
// manageable.
const MIN_RANGE_SIZE: u64 = 100;

pub struct PidMap {
    map: HashMap<ProcessKey, ProcessValue>,
    min_range_size: u64,       // dynamic MIN_RANGE_SIZE
    before_first: u64,         // sentinel, max system pid
    after_last: u64,           // sentinel, at most u64::MAX
    fresh_pid: u64,            // current range min (changes as we allocate pids)
    curr_max: u64,             // current range max
    pid_pool: Vec<(u64, u64)>, // (min, max) of a range, but the max is never u64::MAX; sorted descending.
    dirty: bool,               // value meaning dirty
    verbose: bool,             // true iff SONARTEST_ROLLUP_PIDS is set
}

#[derive(Eq, Hash, PartialEq)]
struct ProcessKey {
    job_id: JobID,
    ppid: Pid,
    command: String,
}

struct ProcessValue {
    pid: Pid,
    dirty: bool,
}

impl PidMap {
    pub fn new(system: &dyn systemapi::SystemAPI) -> PidMap {
        #[allow(unused_mut)]
        let mut pid_limit = PID_LIMIT;
        #[allow(unused_mut)]
        let mut min_range_size = MIN_RANGE_SIZE;
        let mut verbose = false;
        #[allow(unused_variables)]
        if let Ok(s) = std::env::var("SONARTEST_ROLLUP_PIDS") {
            verbose = true;
            #[cfg(debug_assertions)]
            {
                // See documentation above.
                let mut xs = s.split(",").map(|v| v.parse::<u64>());
                match xs.next() {
                    Some(Ok(v)) => pid_limit = system.get_pid_max() + 1 + v,
                    Some(_) | None => {}
                }
                match xs.next() {
                    Some(Ok(v)) => min_range_size = v,
                    Some(_) | None => {}
                }
            }
        }
        PidMap {
            map: HashMap::new(),
            min_range_size: min_range_size,
            before_first: system.get_pid_max(),
            after_last: pid_limit,
            fresh_pid: system.get_pid_max() + 1,
            curr_max: pid_limit - 1,
            pid_pool: vec![],
            dirty: true,
            verbose: verbose,
        }
    }

    /// Assign_pid() will get the synthesized pid for the rolled-up process described by the (job,
    /// parent, command) triple.
    pub fn assign_pid(&mut self, job_id: JobID, ppid: Pid, command: &str) -> Pid {
        let key = ProcessKey {
            job_id: job_id,
            ppid: ppid,
            command: command.to_string(),
        };
        let mut advance = false;
        let mut fresh_pid = 0 as Pid;
        self.map
            .entry(key)
            .and_modify(|e| {
                // Note this case should only be hit on *subsequent* samples.
                if self.verbose {
                    log::debug!(
                        "PID synthesis: Old process: {} {} {} {}",
                        job_id,
                        ppid,
                        command,
                        e.pid
                    );
                }
                fresh_pid = e.pid;
                e.dirty = self.dirty;
            })
            .or_insert_with(|| {
                if self.verbose {
                    log::debug!(
                        "PID synthesis: New process: {} {} {} {}",
                        job_id,
                        ppid,
                        command,
                        self.fresh_pid
                    );
                }
                advance = true;
                fresh_pid = self.fresh_pid as Pid;
                ProcessValue {
                    pid: fresh_pid,
                    dirty: self.dirty,
                }
            });
        if advance {
            self.advance();
        }
        fresh_pid
    }

    /// Assignments_compete() will clean up the pidmap once all rolled-up jobs for a sample have
    /// been processed.  It is not safe to call this until after all samples have been processed as
    /// it may recycle some pids.
    pub fn assignments_complete(&mut self) {
        // For now, always purge all clean elements.  This costs a little extra - we could have
        // decided not to purge every time this function is called - but keeps things predictable.
        // Sweeping always happens on demand, not here.

        self.map.retain(|_, v| v.dirty == self.dirty);
        self.dirty = !self.dirty;

        if self.verbose {
            log::debug!("PID GC: Dirty after purge: {}", self.map.len());
        }
    }

    fn avail(&self) -> u64 {
        self.pid_pool
            .iter()
            .map(|v| v.1 - v.0 + 1)
            .fold(0, |a, b| a + b)
            + (self.curr_max - self.fresh_pid + 1)
    }

    fn advance(&mut self) {
        self.fresh_pid += 1;
        if self.fresh_pid > self.curr_max {
            match self.pid_pool.pop() {
                Some((low, high)) => {
                    (self.fresh_pid, self.curr_max) = (low, high);
                }
                None => {
                    self.sweep();
                }
            }
        }
    }

    // The sweeper will set up new fresh_pid/curr_max values.  It does not return if it can't
    // allocate at least one pid.

    fn sweep(&mut self) {
        let target = self.fresh_pid;

        if self.verbose {
            log::debug!("PID GC: Target = {target}");
        }

        self.fresh_pid = 0;
        self.curr_max = 0;
        self.pid_pool.clear();
        let mut xs = self
            .map
            .values()
            .map(|v| v.pid as u64)
            .collect::<Vec<u64>>();
        xs.push(self.before_first);
        xs.push(self.after_last);
        xs.sort();
        let mut i = xs.len() - 1;
        while i > 0 {
            // Note we may have high < low now.
            let high = xs[i] - 1;
            let low = xs[i - 1] + 1;
            if high >= low && high - low + 1 >= self.min_range_size {
                if self.verbose {
                    log::debug!("PID GC: Recover {low}..{high}");
                }
                self.pid_pool.push((low, high));
            }
            i -= 1;
        }
        if self.pid_pool.is_empty() {
            panic!("PID GC: Empty PID pool");
        }

        if self.verbose {
            log::debug!("PID GC: Total available after collection {}", self.avail());
        }

        // Now, target points to the next pid to use, so we must pop the pool until we we find a
        // range that covers that value or one that is higher.  If there are no such ranges then we
        // retain all ranges and start at the low one.  This ensures that we cycle through available
        // pids and get a quasi-LRU order for a large enough PID space.

        if target > self.pid_pool[0].1 {
            if self.verbose {
                log::debug!("PID GC: Wrapped around");
            }
            (self.fresh_pid, self.curr_max) = self.pid_pool.pop().unwrap();
        } else {
            loop {
                (self.fresh_pid, self.curr_max) = self.pid_pool.pop().unwrap();
                if self.curr_max >= target {
                    if self.verbose {
                        log::debug!("PID GC: Finding {} {}", self.fresh_pid, self.curr_max);
                    }
                    self.fresh_pid = target;
                    break;
                }
                if self.verbose {
                    log::debug!(
                        "PID GC: Discarding {} {} avail = {}",
                        self.fresh_pid,
                        self.curr_max,
                        self.avail()
                    );
                }
            }
        }

        if self.verbose {
            log::debug!("PID GC: Actual available after collection {}", self.avail());
        }
    }
}
