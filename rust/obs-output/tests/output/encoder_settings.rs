//! Tier 1: settings identity and ownership used by scene crop/load/duplication.
use obs_output::encoder_settings::get;

#[test]
fn encoder_settings_identity_and_one_owned_reference() {
    for settings in [0, 1, usize::MAX] {
        let mut references = 7;
        let encoder = Some(settings);
        assert_eq!(
            get(
                encoder,
                |value| {
                    assert_eq!(value, settings);
                    references += 1;
                },
                |_| panic!("valid encoder")
            ),
            settings
        );
        assert_eq!(references, 8);
        assert_eq!(encoder, Some(settings));
    }
}

#[test]
fn null_encoder_logs_without_adding_reference() {
    let mut logs = 0;
    assert_eq!(
        get::<usize>(
            None,
            |_| panic!("null encoder"),
            |name| {
                assert_eq!(name, c"obs_encoder_get_settings");
                logs += 1;
            }
        ),
        0
    );
    assert_eq!(logs, 1);
}
