//! Tier 1: settings identity and ownership used by scene crop/load/duplication.
use obs_source::settings_getter::get;

#[test]
fn scene_settings_identity_and_one_owned_reference() {
    for settings in [0, 1, usize::MAX] {
        let mut references = 7;
        let source = Some(settings);
        assert_eq!(get(source, |value| { assert_eq!(value, settings); references += 1; }, |_| panic!("valid source")), settings);
        assert_eq!(references, 8);
        assert_eq!(source, Some(settings));
    }
}

#[test]
fn null_source_logs_without_adding_reference() {
    let mut logs = 0;
    assert_eq!(get::<usize>(None, |_| panic!("null source"), |name| {
        assert_eq!(name, c"obs_source_get_settings");
        logs += 1;
    }), 0);
    assert_eq!(logs, 1);
}
