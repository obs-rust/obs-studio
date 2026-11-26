//! macOS side of the [platform boundary](super): the time-constraint policy
//! for the audio thread (obs-rust/obs-studio#5771), as Core Audio's own I/O
//! threads use, the counterpart of the MMCSS "Audio" class on Windows.
//!
//! The values come from [`crate::thread_priority::time_constraint`]: one
//! audio tick as the period, a quarter of it as computation, half of it as
//! constraint, preemptible. macOS needs no guard like Linux's `SIGXCPU`
//! handler: the scheduler itself demotes a time-constraint thread that uses
//! much more than its computation.

use core::ffi::c_int;

use crate::thread_priority::{ns_to_mach_abs, time_constraint};

const LOG_WARNING: i32 = 200;
const LOG_INFO: i32 = 300;

/// `THREAD_TIME_CONSTRAINT_POLICY` (`mach/thread_policy.h`).
const THREAD_TIME_CONSTRAINT_POLICY: c_int = 2;
/// Its size in `integer_t` units.
const THREAD_TIME_CONSTRAINT_POLICY_COUNT: u32 = 4;
const KERN_SUCCESS: c_int = 0;

#[repr(C)]
struct ThreadTimeConstraintPolicy {
    period: u32,
    computation: u32,
    constraint: u32,
    preemptible: u32,
}

#[repr(C)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

unsafe extern "C" {
    fn mach_thread_self() -> u32;
    fn mach_port_deallocate(task: u32, name: u32) -> c_int;
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> c_int;
    fn thread_policy_set(
        thread: u32,
        flavor: c_int,
        policy_info: *mut ThreadTimeConstraintPolicy,
        count: u32,
    ) -> c_int;
    static mach_task_self_: u32;
}

/// The time-constraint policy for the audio thread's lifetime; the policy
/// ends with the thread.
pub(crate) struct AudioThreadClass {
    line: Option<(i32, String)>,
}

impl AudioThreadClass {
    pub(crate) fn enter(samples_per_sec: u32) -> Self {
        let values = time_constraint(samples_per_sec);
        let mut timebase = MachTimebaseInfo { numer: 0, denom: 0 };
        // SAFETY: `timebase` is a live out-parameter.
        let timebase_ok = unsafe { mach_timebase_info(&mut timebase) } == KERN_SUCCESS;
        let line = if !timebase_ok {
            (
                LOG_WARNING,
                "audio thread: default priority (mach_timebase_info failed)".to_string(),
            )
        } else {
            let abs = |ns| ns_to_mach_abs(ns, timebase.numer, timebase.denom);
            let mut policy = ThreadTimeConstraintPolicy {
                period: abs(values.period_ns),
                computation: abs(values.computation_ns),
                constraint: abs(values.constraint_ns),
                preemptible: 1,
            };
            // SAFETY: the calling thread's port, a live policy struct and its
            // documented count; the port reference is released after the call.
            let result = unsafe {
                let thread = mach_thread_self();
                let result = thread_policy_set(
                    thread,
                    THREAD_TIME_CONSTRAINT_POLICY,
                    &mut policy,
                    THREAD_TIME_CONSTRAINT_POLICY_COUNT,
                );
                mach_port_deallocate(mach_task_self_, thread);
                result
            };
            if result == KERN_SUCCESS {
                (
                    LOG_INFO,
                    format!(
                        "audio thread: time-constraint policy (period {:.3} ms, \
                         computation {:.3} ms, constraint {:.3} ms)",
                        values.period_ns as f64 / 1e6,
                        values.computation_ns as f64 / 1e6,
                        values.constraint_ns as f64 / 1e6,
                    ),
                )
            } else {
                (
                    LOG_WARNING,
                    format!("audio thread: default priority (thread_policy_set: {result})"),
                )
            }
        };
        Self { line: Some(line) }
    }

    /// The startup line, once.
    pub(crate) fn notice(&mut self) -> Option<(i32, String)> {
        self.line.take()
    }
}
