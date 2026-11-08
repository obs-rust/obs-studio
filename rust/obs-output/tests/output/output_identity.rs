use obs_output::output_identity::{id, type_data};

#[test]
fn mirrors_c_null_defaults_and_logs() {
    let mut logs = Vec::new();
    assert_eq!(id::<Option<&str>>(None, |f| logs.push(f)), None);
    assert_eq!(type_data::<Option<&i32>>(None, |f| logs.push(f)), None);
    assert_eq!(logs, [c"obs_output_get_id", c"obs_output_get_type_data"]);
}

#[test]
fn mirrors_c_exact_independent_metadata() {
    let marker = 17;
    let strings: [Option<&[u8]>; 4] = [None, Some(b""), Some(b"current-id"), Some(b"\xffx")];
    for text in strings {
        for data in [None, Some(&marker)] {
            assert_eq!(id(Some(text), |_| panic!("unexpected log")), text);
            assert_eq!(type_data(Some(data), |_| panic!("unexpected log")), data);
        }
    }
    assert_eq!(marker, 17);
}
