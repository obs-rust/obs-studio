//! `libobs/util/task.c` shim against the C oracle (#3216). The fixture is
//! the shape a burst of deferred work takes: a caller queues 256 small tasks
//! back to back, then waits for the queue to drain. Each task bumps a
//! counter, so the benchmark compares how a queue hands work to its worker,
//! not what the work does.

use core::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

use criterion::Criterion;
use obs_c_oracle::task as c_task;
use obs_util::ffi::task as rs_task;

use crate::pair;

const BURST: usize = 256;

unsafe extern "C" fn bump(param: *mut c_void) {
    // SAFETY: `param` is the `AtomicUsize` the benchmark keeps alive until
    // the queue has drained.
    unsafe { &*param.cast::<AtomicUsize>() }.fetch_add(1, Ordering::Relaxed);
}

pub fn benchmarks(criterion: &mut Criterion) {
    // SAFETY: each queue is created here and destroyed at the end.
    let rust = unsafe { rs_task::os_task_queue_create() };
    let c = unsafe { c_task::oracle_os_task_queue_create() };
    assert!(!rust.is_null() && !c.is_null(), "task queue creation failed");
    let count = AtomicUsize::new(0);
    let param = core::ptr::from_ref(&count).cast_mut().cast::<c_void>();

    pair(
        criterion,
        "task_queue/burst_256_then_wait",
        || {
            count.store(0, Ordering::Relaxed);
            for _ in 0..BURST {
                // SAFETY: `rust` is live and `count` outlives the wait below.
                unsafe { rs_task::os_task_queue_queue_task(rust, Some(bump), param) };
            }
            // SAFETY: as above.
            unsafe { rs_task::os_task_queue_wait(rust) };
            count.load(Ordering::Relaxed)
        },
        || {
            count.store(0, Ordering::Relaxed);
            for _ in 0..BURST {
                // SAFETY: `c` is live and `count` outlives the wait below.
                unsafe { c_task::oracle_os_task_queue_queue_task(c, Some(bump), param) };
            }
            // SAFETY: as above.
            unsafe { c_task::oracle_os_task_queue_wait(c) };
            count.load(Ordering::Relaxed)
        },
    );

    // SAFETY: both queues are drained and no longer used.
    unsafe {
        rs_task::os_task_queue_destroy(rust);
        c_task::oracle_os_task_queue_destroy(c);
    }
}
