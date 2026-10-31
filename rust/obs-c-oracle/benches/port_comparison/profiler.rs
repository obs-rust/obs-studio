//! `libobs/util/profiler.c` shim against the C oracle (#3213): one root call
//! with nested `profile_start`/`profile_end` frames, depth 1, 4 and 8, as the
//! graphics and audio threads run them each frame. "disabled" is the same
//! call after `profiler_stop`, once the thread has seen the profiler off.

use core::ffi::c_char;

use criterion::Criterion;
use obs_c_oracle::profiler as c_prof;
use obs_util::ffi::profiler as rs_prof;

use crate::pair;

const NAMES: [&core::ffi::CStr; 8] = [c"root", c"f1", c"f2", c"f3", c"f4", c"f5", c"f6", c"f7"];

struct Side {
    start: unsafe extern "C" fn(*const c_char),
    end: unsafe extern "C" fn(*const c_char),
}

fn nested(side: &Side, depth: usize) -> usize {
    for name in &NAMES[..depth] {
        // SAFETY: static NUL-terminated names, which the profiler keeps.
        unsafe { (side.start)(name.as_ptr()) };
    }
    for name in NAMES[..depth].iter().rev() {
        // SAFETY: as above, ended in reverse order.
        unsafe { (side.end)(name.as_ptr()) };
    }
    depth
}

unsafe extern "C" fn rs_start(name: *const c_char) {
    // SAFETY: forwarded.
    unsafe { rs_prof::profile_start(name) }
}
unsafe extern "C" fn rs_end(name: *const c_char) {
    // SAFETY: forwarded.
    unsafe { rs_prof::profile_end(name) }
}

pub fn benchmarks(criterion: &mut Criterion) {
    let rust = Side {
        start: rs_start,
        end: rs_end,
    };
    let c = Side {
        start: c_prof::oracle_profile_start,
        end: c_prof::oracle_profile_end,
    };
    // SAFETY: no preconditions; each profiler has its own globals.
    unsafe {
        rs_prof::profiler_start();
        c_prof::oracle_profiler_start();
    }
    for depth in [1, 4, 8] {
        let name = format!("profiler/nested/{depth}");
        pair(criterion, &name, || nested(&rust, depth), || nested(&c, depth));
    }
    // SAFETY: as above. One call each lets the thread see the profiler off.
    unsafe {
        rs_prof::profiler_stop();
        c_prof::oracle_profiler_stop();
    }
    nested(&rust, 1);
    nested(&c, 1);
    for depth in [1, 8] {
        let name = format!("profiler/disabled/{depth}");
        pair(criterion, &name, || nested(&rust, depth), || nested(&c, depth));
    }
    // SAFETY: as above; recorded roots are dropped.
    unsafe {
        rs_prof::profiler_free();
        c_prof::oracle_profiler_free();
        rs_prof::profile_reenable_thread();
        c_prof::oracle_profile_reenable_thread();
    }
}
