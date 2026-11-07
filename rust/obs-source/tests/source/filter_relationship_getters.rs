use obs_source::filter_relationship_getters::{parent, target};

#[test]
fn mirrors_c_null_defaults() {
    let mut logs = Vec::new();
    assert_eq!(parent::<usize>(None, |f, p| logs.push((f, p))), 0);
    assert_eq!(target::<usize>(None, |f, p| logs.push((f, p))), 0);
    assert_eq!(logs, [(c"obs_filter_get_parent", c"filter"), (c"obs_filter_get_target", c"filter")]);
}

#[test]
fn mirrors_c_independent_borrowed_fields() {
    // Zero is a NULL relationship on a non-NULL filter; it must not log.
    // Identity 1 represents self, 2 and 3 distinct borrowed sources.
    for p in 0usize..4 {
        for t in 0usize..4 {
            assert_eq!(parent(Some(p), |_, _| panic!("unexpected log")), p);
            assert_eq!(target(Some(t), |_, _| panic!("unexpected log")), t);
        }
    }
}

#[test]
fn preserves_opaque_identity() {
    for value in [0usize, 1, 17, usize::MAX / 2, usize::MAX] {
        assert_eq!(parent(Some(value), |_, _| panic!("unexpected log")), value);
        assert_eq!(target(Some(value), |_, _| panic!("unexpected log")), value);
    }
}
