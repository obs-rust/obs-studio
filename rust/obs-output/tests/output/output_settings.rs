//! Tier 1 mirrors the settings assertions in test_encoder_output_getters_characterization.c.
use obs_output::output_settings::settings;

#[test]
fn test_encoded_output_getters_settings() {
    let value = Some("default.mkv");
    let mut retained = Vec::new();
    assert_eq!(
        settings(
            Some(value),
            |v| retained.push(v),
            |_| panic!("unexpected log")
        ),
        value
    );
    assert_eq!(retained, [value]);
}

#[test]
fn test_output_null_guards_settings() {
    let mut logs = Vec::new();
    assert_eq!(
        settings::<Option<u32>>(
            None,
            |_| panic!("unexpected addref"),
            |name| logs.push(name)
        ),
        None
    );
    assert_eq!(logs, [c"obs_output_get_settings"]);
}

#[test]
fn valid_output_with_null_settings_still_calls_addref() {
    let mut retained = Vec::new();
    assert_eq!(
        settings(
            Some(None::<u32>),
            |v| retained.push(v),
            |_| panic!("unexpected log")
        ),
        None
    );
    assert_eq!(retained, [None]);
}
