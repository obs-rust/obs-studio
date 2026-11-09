//! Tier 1 mirrors the zero/nonzero setter cases in test_output_reconnect_route.
use obs_output::output_reconnect_settings::set;

#[test]
fn test_output_reconnect_route_settings() {
    for (count, seconds) in [(0, 60), (20, 60), (5, 60), (0, 0), (5, 1)] {
        let (mut max, mut sec) = (-99, -98);
        set(Some((&mut max, &mut sec)), count, seconds, || {
            panic!("valid output logged as null")
        });
        assert_eq!((max, sec), (count, seconds));
    }
}

#[test]
fn signed_edges_and_replacement() {
    let (mut max, mut sec) = (0, 0);
    for count in [i32::MIN, -1, 0, 1, i32::MAX] {
        for seconds in [i32::MIN, -1, 0, 1, i32::MAX] {
            set(Some((&mut max, &mut sec)), count, seconds, || panic!());
            assert_eq!((max, sec), (count, seconds));
        }
    }
}

#[test]
fn absent_output_logs_once() {
    let mut logs = 0;
    set(None, i32::MIN, i32::MAX, || logs += 1);
    assert_eq!(logs, 1);
}
