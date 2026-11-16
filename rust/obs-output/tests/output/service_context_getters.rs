use obs_output::service_context_getters::{name, settings};

#[test]
fn test_identity_borrows_name() {
    let value = Some("full");
    assert_eq!(name(&value), value);
    assert_eq!(name(&None::<&str>), None);
    assert_eq!(name(&Some("")), Some(""));
    assert_eq!(value, Some("full"));
}

#[test]
fn test_update_and_settings_borrows_current_settings() {
    let mut value = Some(17);
    assert_eq!(settings(&value), Some(17));
    assert_eq!(value, Some(17));
    value = Some(29);
    assert_eq!(settings(&value), Some(29));
    assert_eq!(settings(&None::<usize>), None);
    assert_eq!(value, Some(29));
}
