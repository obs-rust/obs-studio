//! Tier 1 mirrors the encoder assertions in test_context_names.c:469-475.
use obs_output::encoder_set_name::should_rename;

#[test]
fn test_context_names_encoder() {
    assert!(!should_rename(b"Encoder", None));
    assert!(!should_rename(b"Encoder", Some(b"")));
    assert!(!should_rename(b"Encoder", Some(b"Encoder")));
    assert!(should_rename(b"Encoder", Some(b"Other")));
    // No uniqueness transformation: a name already used elsewhere is accepted.
    assert!(should_rename(b"Other", Some(b"Encoder")));
}

#[test]
fn arbitrary_bytes_and_empty_current() {
    assert!(should_rename(b"", Some(b"name")));
    assert!(!should_rename(&[255], Some(&[255])));
    assert!(should_rename(&[255], Some(&[254])));
}
