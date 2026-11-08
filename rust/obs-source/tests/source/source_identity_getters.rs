//! Tier 1 mirrors test_source_identity_getters.c without FFI or unsafe code.
use obs_source::source_identity_getters::{Identity, unversioned_id, uuid};

#[test]
fn borrowed_identity() {
    let values: [Option<&[u8]>; 5] = [
        None,
        Some(b""),
        Some(b"01234567-89ab-cdef-0123-456789abcdef"),
        Some(b"source_v2"),
        Some(b"\xff"),
    ];
    for v in values {
        for a in values {
            let fields = Some(Identity {
                uuid: v,
                unversioned_id: a,
            });
            for _ in 0..2 {
                assert_eq!(uuid(fields, |_| panic!("valid source must not log")), v);
                assert_eq!(
                    unversioned_id(fields, |_| panic!("valid source must not log")),
                    a
                );
            }
        }
    }
}

#[test]
fn null_source_logs_each_getter_name() {
    let mut logs = Vec::new();
    assert_eq!(uuid::<Option<&[u8]>>(None, |name| logs.push(name)), None);
    assert_eq!(
        unversioned_id::<Option<&[u8]>>(None, |name| logs.push(name)),
        None
    );
    assert_eq!(
        logs,
        [c"obs_source_get_uuid", c"obs_source_get_unversioned_id"]
    );
}
