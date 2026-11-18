//! Tier 3 compares original C with the actual Rust FFI over bounded operation
//! sequences. The test-owned C core and recorded UI queue are thread-local;
//! unchanged Tier 2 covers the complete public queue and real logger.
use obs_core::ui_task_handler;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

unsafe extern "C" {
    fn oracle_ui_task_handler_layout(out: *mut usize);
    fn oracle_ui_task_handler_reset(rust: bool, marked: bool);
    fn oracle_ui_task_handler_step(op: u32, data: usize, wait: bool);
    fn oracle_ui_task_handler_snapshot(out: *mut usize) -> usize;
}
fn run(rust: bool, marked: bool, steps: &[(u32, u8, bool)]) -> Vec<Vec<usize>> {
    // SAFETY: this thread owns the oracle core and queue. At most 128 steps
    // produce 256 dispatches, each four size_t values plus seven header values.
    // Callbacks ignore their data (arbitrary byte-sized opaque tokens).
    unsafe {
        oracle_ui_task_handler_reset(rust, marked);
        let mut snapshots = Vec::new();
        for &(op, data, wait) in steps {
            oracle_ui_task_handler_step(op, usize::from(data), wait);
            let mut state = vec![0; 7 + 256 * 4];
            let len = oracle_ui_task_handler_snapshot(state.as_mut_ptr());
            state.truncate(len);
            snapshots.push(state);
        }
        snapshots
    }
}
fn compare(steps: &[(u32, u8, bool)]) {
    let marked = ui_task_handler::is_ui_thread();
    let expected = run(false, marked, steps);
    let actual = run(true, marked, steps);
    assert_eq!(actual, expected, "steps: {steps:?}");
}
#[test]
fn test_ui_task_handler_characterization_and_randomized_parity() {
    // Fresh TLS is needed to compare the initially unmarked case as well as
    // the sticky marker after a completed bootstrap. One worker thread only.
    std::thread::spawn(|| {
        compare(&[
            (5, 0, false),
            (4, 0, false),
            (4, 1, false),
            (2, 0, false),
            (3, 7, false),
            (4, 2, false),
            (4, 3, false),
            (0, 0, false),
            (3, 7, false),
            (3, 7, true),
            (1, 0, false),
            (4, 4, false),
        ]);
        compare(&[(0, 0, false), (0, 0, false), (1, 0, false), (2, 0, false)]);
        let mut runner = TestRunner::new(Config {
            cases: obs_c_oracle::parity::cases(256),
            failure_persistence: None,
            ..Config::default()
        });
        runner
            .run(
                &prop::collection::vec((0u32..6, any::<u8>(), any::<bool>()), 1..129),
                |steps| {
                    compare(&steps);
                    Ok(())
                },
            )
            .unwrap();
    })
    .join()
    .unwrap();
}

#[test]
fn real_header_callback_and_bool_layout() {
    use obs_core::ffi::ui_task_handler::{Handler, Task};
    let mut layout = [0; 6];
    // SAFETY: the oracle writes exactly six size_t entries into this array.
    unsafe { oracle_ui_task_handler_layout(layout.as_mut_ptr()) };
    assert_eq!(layout, [size_of::<Handler>(), align_of::<Handler>(),
        size_of::<Task>(), align_of::<Task>(), size_of::<bool>(), align_of::<bool>()]);
}
