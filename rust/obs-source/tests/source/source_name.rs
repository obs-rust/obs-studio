//! Tier 1: getter assertions from test_context_names.c; naming itself is not this unit.
use obs_source::source_name::name;

#[test]
fn test_context_names_stored_names() {
    let long = vec![b'L'; 64 * 1024];
    let names: &[&[u8]] = &[
        b"A",
        b"A 2",
        b"a",
        b"A ",
        b" A",
        b"%s %d %n 2",
        b"\xff\xfe\x80 2",
        b"caf\xc3\xa9",
        b"cafe\xcc\x81",
        b"caf\xc3\xa9 2",
        b"\t\n",
        b"__unnamed0000",
        b"__unnamed0001",
        b"__unnamed0002 2",
        b"",
        &long,
    ];
    for &stored in names {
        for _ in 0..2 {
            let got = name(true, || Some(stored), |_| panic!("valid source logged"));
            assert_eq!(got, Some(stored));
            assert!(core::ptr::eq(got.unwrap(), stored));
        }
    }
}

#[test]
fn test_context_names_private_and_rename() {
    for stored in [
        None,
        Some(&b"A"[..]),
        Some(&b"A"[..]),
        None,
        Some(&b"__unnamed0001"[..]),
        Some(&b"__unnamed0002"[..]),
    ] {
        assert_eq!(
            name(true, || stored, |_| panic!("valid source logged")),
            stored
        );
    }
}

#[test]
fn null_source_logs_without_reading_fields() {
    let mut logs = Vec::new();
    let got: Option<&[u8]> = name(
        false,
        || panic!("NULL field access"),
        |label| logs.push(label),
    );
    assert_eq!(got, None);
    assert_eq!(logs, [c"obs_source_get_name"]);
}
