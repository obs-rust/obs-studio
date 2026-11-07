use obs_output::encoder_metadata::{caps, codec, id, kind, type_data};

#[test]
fn mirrors_c_null_defaults_and_logs() {
    let mut logs = Vec::new();
    assert_eq!(codec::<Option<&str>>(None, |f, p| logs.push((f, p))), None);
    assert_eq!(kind(None, |f, p| logs.push((f, p))), 0);
    assert_eq!(id::<Option<&str>>(None, |f, p| logs.push((f, p))), None);
    assert_eq!(
        type_data::<Option<&i32>>(None, |f, p| logs.push((f, p))),
        None
    );
    assert_eq!(caps(None, |f, p| logs.push((f, p))), 0);
    assert_eq!(
        logs,
        [
            (c"obs_encoder_get_codec", c"encoder"),
            (c"obs_encoder_get_type", c"encoder"),
            (c"obs_encoder_get_id", c"encoder"),
            (c"obs_encoder_get_type_data", c"encoder"),
            (c"obs_encoder_get_caps", c"encoder"),
        ]
    );
}

#[test]
fn mirrors_c_exact_independent_metadata() {
    let original_data = 29;
    for text in [None, Some(""), Some("codec")] {
        for cap in [0, 1, 0x8000_0000, u32::MAX] {
            for ty in [0, 1] {
                let original_id = text.map(|_| "original-id");
                let data = text.map(|_| &original_data);
                assert_eq!(codec(Some(text), |_, _| panic!("unexpected log")), text);
                assert_eq!(kind(Some(ty), |_, _| panic!("unexpected log")), ty);
                assert_eq!(
                    id(Some(original_id), |_, _| panic!("unexpected log")),
                    original_id
                );
                assert_eq!(type_data(Some(data), |_, _| panic!("unexpected log")), data);
                assert_eq!(caps(Some(cap), |_, _| panic!("unexpected log")), cap);
            }
        }
    }
}
