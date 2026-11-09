//! Tier 1 mirrors the count assertions in the unchanged C fixtures.
use obs_source::filter_count::count;

#[test]
fn test_filter_skip_removed_empty_count() {
    assert_eq!(count(Some(0), |_| panic!("valid source logged")), 0);
}

#[test]
fn test_shutdown_leaked_scenes_populated_count() {
    assert_eq!(count(Some(1), |_| panic!("valid source logged")), 1);
}

#[test]
fn size_t_edges_are_copied_without_mutation() {
    for value in [0, 1, 2, u32::MAX as usize, usize::MAX] {
        assert_eq!(count(Some(value), |_| panic!("valid source logged")), value);
    }
}

#[test]
fn null_source_logs_once_and_returns_zero() {
    let mut logs = Vec::new();
    assert_eq!(count(None, |name| logs.push(name)), 0);
    assert_eq!(logs, [c"obs_source_filter_count"]);
}
