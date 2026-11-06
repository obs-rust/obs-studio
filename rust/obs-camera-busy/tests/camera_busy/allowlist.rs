//! The "Always allow" list in OBS's user config (#3724).

use obs_camera_busy::Allowlist;

#[test]
fn parses_and_writes_the_config_string() {
    let list = Allowlist::parse("zoom; cheese;;  ;zoom");
    assert!(list.contains("zoom"));
    assert!(list.contains("cheese"));
    assert!(!list.contains(""));
    assert!(!list.contains("Zoom"));
    assert_eq!(list.to_config(), "cheese;zoom");
    assert_eq!(Allowlist::parse(&list.to_config()), list);
}

#[test]
fn empty_config_is_an_empty_list() {
    assert_eq!(Allowlist::parse("").to_config(), "");
    assert_eq!(Allowlist::parse(""), Allowlist::default());
}

#[test]
fn allow_adds_once_and_refuses_bad_names() {
    let mut list = Allowlist::default();
    assert!(list.allow("zoom"));
    assert!(!list.allow("zoom"));
    assert!(!list.allow(""));
    assert!(!list.allow("?"));
    assert!(!list.allow("a;b"));
    assert_eq!(list.to_config(), "zoom");
}
