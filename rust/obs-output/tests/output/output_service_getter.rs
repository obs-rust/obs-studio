//! Tier 1 mirrors test_output_service_getter.c through the safe API.
use obs_output::output_service_getter::service;

#[test]
fn borrowed_service() {
    for value in [None, Some(7_u32), Some(9_u32)] {
        for _ in 0..2 {
            assert_eq!(
                service(Some(value), |_| panic!("valid output must not log")),
                value
            );
        }
    }
}

#[test]
fn null_output_logs_getter_name() {
    let mut logs = Vec::new();
    assert_eq!(service::<Option<u32>>(None, |name| logs.push(name)), None);
    assert_eq!(logs, [c"obs_output_get_service"]);
}
