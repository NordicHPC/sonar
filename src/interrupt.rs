#[cfg(debug_assertions)]
use crate::log;

use std::sync::atomic::{AtomicBool, Ordering};

// Signal handling logic.
//
// Assuming no bugs, the interesting interrupt signals are SIGHUP, SIGTERM, SIGINT, and SIGQUIT.  Of
// these, only SIGHUP and SIGTERM are really interesting because they are sent by the OS or by job
// control (and will often be followed by SIGKILL if not honored within some reasonable time);
// INT/QUIT are sent by a user in response to keyboard action and more typical during
// development/debugging.
//
// Call handle_interruptions() to establish handlers, then is_interrupted() to check whether signals
// have been received.

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn sonar_signal_handler(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::Relaxed);
}

pub fn handle_interruptions() {
    unsafe {
        let nomask: libc::sigset_t = std::mem::zeroed();
        let action = libc::sigaction {
            sa_sigaction: sonar_signal_handler as usize,
            sa_mask: nomask,
            sa_flags: 0,
            sa_restorer: None,
        };
        libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut());
        libc::sigaction(libc::SIGHUP, &action, std::ptr::null_mut());
    }
}

#[cfg(debug_assertions)]
pub fn is_interrupted() -> bool {
    if std::env::var("SONARTEST_WAIT_INTERRUPT").is_ok() {
        std::thread::sleep(std::time::Duration::new(10, 0));
    }
    let flag = INTERRUPTED.load(Ordering::Relaxed);
    if flag {
        // Test cases depend on this exact output.
        log::info("Interrupt flag was set!")
    }
    flag
}

#[cfg(not(debug_assertions))]
pub fn is_interrupted() -> bool {
    INTERRUPTED.load(Ordering::Relaxed)
}
