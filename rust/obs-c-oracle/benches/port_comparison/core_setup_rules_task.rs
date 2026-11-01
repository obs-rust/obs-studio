//! obs-core: the task queue's route (`libobs/obs-core-setup-rules-task.c.h`),
//! as `obs_queue_task()` walks it for every queued task: a graphics task
//! from another thread, an audio task on its own thread, a destroy task that
//! waits, and a UI task with a handler. Plain values: no allocation.

use obs_c_oracle::core_setup_rules::oracle_obs_core_setup_rules_task_step;
use obs_core::ffi::core_setup_rules::obs_core_setup_rules_task_step;

use criterion::Criterion;

/// `OBS_CORE_SETUP_RULES_TASK_TYPE`.
const TASK_TYPE: u32 = 0;

/// Each task's type and the handler, thread and wait answers it gets.
const TASKS: [(i32, [bool; 3]); 4] = [
    (1, [false, false, false]),
    (2, [false, true, false]),
    (3, [false, false, true]),
    (0, [true, false, false]),
];

/// `obs_queue_task()`'s walk: each step that is asked, in its order.
fn walk(task_step: impl Fn(u32, i32, bool) -> u32) -> u32 {
    let mut routes = 0;
    for (task_type, values) in TASKS {
        let mut step = task_step(TASK_TYPE, task_type, false);
        for (at, value) in (1..=3).zip(values) {
            if step == at {
                step = task_step(step, task_type, value);
            }
        }
        routes += step;
    }
    routes
}

pub fn benchmarks(criterion: &mut Criterion) {
    super::pair(
        criterion,
        "core_setup_rules_task/task_step",
        || walk(|s, t, v| obs_core_setup_rules_task_step(s, t, v)),
        // SAFETY: plain values; only the four task types, which the C and
        // the port route alike.
        || walk(|s, t, v| unsafe { oracle_obs_core_setup_rules_task_step(s, t, v) }),
    );
}
