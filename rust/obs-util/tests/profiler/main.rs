//! obs-util integration tests: profiler (one module per former tests/*.rs file, #968).

use std::sync::Mutex;

/// One lock for every test that touches the process-global profiler.
pub(crate) static PROFILER_LOCK: Mutex<()> = Mutex::new(());

mod profiler;
mod profiler_io;
mod profiler_io_parity;
mod profiler_parity;
mod profiler_snapshot;
mod profiler_snapshot_layout;
mod profiler_snapshot_parity;

// Crash probes re-exec this binary before forking, isolating parallel pipe tests.
mod platform_oracle;

/// profiler.rs and profiler_parity.rs both reset the one process-global
/// profiler. As separate binaries (before #968) each ran in its own process;
/// in this shared binary libtest runs them on parallel threads, so they must
/// hold the same lock or each sees the other's roots (Windows CI on #978).
#[test]
fn profiler_tests_share_one_lock() {
    assert!(
        std::ptr::eq(&profiler::TEST_LOCK, &profiler_parity::PARITY_LOCK),
        "profiler.rs and profiler_parity.rs must serialize on one lock"
    );
}
