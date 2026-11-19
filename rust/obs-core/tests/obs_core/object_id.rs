//! Tier 1 mirrors every case in unchanged test_object_id.c.
use obs_core::object_id::{Kind, query};

#[test]
fn null_object() {
    assert_eq!(query::<()>(None, |_| panic!("NULL projection")), None);
}

#[test]
fn unknown_kinds() {
    for kind in [0, 5, 6, i32::MAX, -1, i32::MIN] {
        let before = kind;
        for _ in 0..2 {
            assert_eq!(
                query::<()>(Some(kind), |_| panic!("unsupported projection")),
                None
            );
        }
        assert_eq!(kind, before);
    }
}

fn stored_id(kind: i32, expected: Kind) {
    for id in [Some("concrete_id"), Some(""), None] {
        let object = (kind, id, "display name", "original_encoder_id");
        let before = object;
        for _ in 0..2 {
            let actual = query(Some(object.0), |selected| {
                assert_eq!(selected, expected);
                object.1
            })
            .expect("supported kind");
            match (actual, id) {
                (Some(a), Some(b)) => assert!(std::ptr::eq(a, b)),
                (None, None) => (),
                _ => panic!("stored NULL changed"),
            }
        }
        assert_eq!(object, before);
    }
}

#[test]
fn source_id() {
    stored_id(1, Kind::Source);
}

#[test]
fn output_id() {
    stored_id(2, Kind::Output);
}

#[test]
fn encoder_id() {
    stored_id(3, Kind::Encoder);
}

#[test]
fn service_id() {
    stored_id(4, Kind::Service);
}
