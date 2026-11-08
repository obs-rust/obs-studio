use obs_source::state_getters::{enabled, flags, last_obs_version};

#[test]
fn mirrors_c_null_defaults() {
    let mut logs = Vec::new();
    assert_eq!(flags(None, |f, p| logs.push((f, p))), 0);
    assert!(!enabled(None, |f, p| logs.push((f, p))));
    assert_eq!(last_obs_version(None, |f, p| logs.push((f, p))), 0);
    assert_eq!(
        logs,
        [
            (c"obs_source_get_flags", c"source"),
            (c"obs_source_enabled", c"source"),
            (c"obs_source_get_last_obs_version", c"source")
        ]
    );
}

#[test]
fn mirrors_c_independent_fields() {
    let edges = [0, 1, 0x8000_0000, 0x7fff_ffff, u32::MAX, 0xa5a5_5a5a];
    for f in edges {
        for v in edges {
            for e in [false, true] {
                for _ in 0..2 {
                    assert_eq!(flags(Some(f), |_, _| panic!("unexpected log")), f);
                    assert_eq!(enabled(Some(e), |_, _| panic!("unexpected log")), e);
                    assert_eq!(
                        last_obs_version(Some(v), |_, _| panic!("unexpected log")),
                        v
                    );
                }
            }
        }
    }
}
