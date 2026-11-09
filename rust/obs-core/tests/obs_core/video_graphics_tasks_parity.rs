//! Tier 3: unchanged C drain vs Rust FFI over real deque/mutex adapters.
//! Exact trace/state equality; no excluded inputs or intentional differences.
use obs_c_oracle as _;
use obs_core::ffi::video_graphics_tasks::obs_graphics_tasks_execute;
use proptest::prelude::*;
use std::ffi::c_void;

unsafe extern "C" {
    fn oracle_graphics_tasks_run(
        drain: Option<unsafe extern "C" fn(*mut c_void)>,
        flags: *const u8,
        count: usize,
        trace: *mut u32,
        state: *mut usize,
    );
}

fn run(flags: &[u8], rust: bool) -> (Vec<u32>, [usize; 4]) {
    assert!(flags.len() <= 32);
    let mut trace = [0; 96];
    let mut state = [0; 4];
    // SAFETY: bounded input and output buffers satisfy the fixture contract;
    // its initialized video and callback data live throughout the drain.
    unsafe {
        oracle_graphics_tasks_run(
            rust.then_some(obs_graphics_tasks_execute),
            flags.as_ptr(),
            flags.len(),
            trace.as_mut_ptr(),
            state.as_mut_ptr(),
        );
    }
    (trace[..state[0]].to_vec(), state)
}

fn check(flags: &[u8]) {
    let c = run(flags, false);
    let rust = run(flags, true);
    assert_eq!(rust, c);
    assert_eq!(rust.1[1..], [0, 1, 1]);
    let mut expected = Vec::new();
    for (id, flag) in flags.iter().enumerate() {
        expected.push(id as u32);
        if flag & 1 != 0 {
            expected.push(id as u32 + 1000);
        }
    }
    for (id, flag) in flags.iter().enumerate() {
        if flag & 2 != 0 {
            expected.push(id as u32 + 2000);
        }
    }
    assert_eq!(rust.0, expected);
}

#[test]
fn original_c_trace_and_mutex_edges() {
    for flags in [&[][..], &[0][..], &[3, 0][..], &[3; 32][..], &[2; 32][..]] {
        check(flags);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn exact_original_c_parity(flags in prop::collection::vec(any::<u8>(), 0..=32)) {
        check(&flags);
    }
}
