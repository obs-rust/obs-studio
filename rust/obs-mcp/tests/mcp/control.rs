//! Control API domains (#4446); no additional test executable.
use obs_mcp::tools::control::counter_delta;
#[test]
fn counters_are_relative_to_baseline_and_survive_recreation() {
    assert_eq!(counter_delta(20, 5), 15);
    assert_eq!(counter_delta(5, 5), 0);
    assert_eq!(counter_delta(3, 20), 3);
}
