use obs_source::async_flag_getters::{decoupled, unbuffered};

#[test]
fn mirrors_c_null_defaults() {
    let mut logs = Vec::new();
    assert!(!unbuffered(None, |f, p| logs.push((f, p))));
    assert!(!decoupled(None, |f, p| logs.push((f, p))));
    assert_eq!(
        logs,
        [
            (c"obs_source_async_unbuffered", c"source"),
            (c"obs_source_async_decoupled", c"source"),
        ]
    );
}

#[test]
fn mirrors_c_independent_fields() {
    for u in [false, true] {
        for d in [false, true] {
            assert_eq!(unbuffered(Some(u), |_, _| panic!("unexpected log")), u);
            assert_eq!(decoupled(Some(d), |_, _| panic!("unexpected log")), d);
        }
    }
}
