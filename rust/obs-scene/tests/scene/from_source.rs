//! Mirrors test_scene_from_source.c through the safe core, including borrowing.
use obs_scene::from_source::from_source;
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn null_source() {
    for expected in [b"scene".as_slice(), b"group"] {
        let value: Option<&u32> = from_source(None, expected, || panic!("NULL context read"));
        assert!(value.is_none());
    }
}

#[test]
fn non_scene_sources() {
    for id in [
        b"scene-extra".as_slice(),
        b"group-extra",
        b"Scene",
        b"Group",
        b"",
        b"\xff",
    ] {
        let owner = Rc::new(17_u32);
        let weak = Rc::downgrade(&owner);
        for _ in 0..3 {
            for expected in [b"scene".as_slice(), b"group"] {
                let value: Option<&u32> =
                    from_source(Some(id), expected, || panic!("rejected context read"));
                assert!(value.is_none());
                assert!(weak.upgrade().is_some());
                assert_eq!(Rc::strong_count(&owner), 1);
            }
        }
        drop(owner);
        assert!(weak.upgrade().is_none());
    }
}

fn borrowed_source(id: &[u8], other: &[u8]) {
    let owner = Rc::new(31_u32);
    let weak = Rc::downgrade(&owner);
    let reads = Cell::new(0);
    for _ in 0..3 {
        let value = from_source(Some(id), id, || {
            reads.set(reads.get() + 1);
            Some(owner.as_ref())
        });
        assert!(std::ptr::eq(value.unwrap(), owner.as_ref()));
        let rejected: Option<&u32> =
            from_source(Some(id), other, || panic!("cross-kind context read"));
        assert!(rejected.is_none());
        assert!(weak.upgrade().is_some());
        assert_eq!(Rc::strong_count(&owner), 1);
    }
    assert_eq!(reads.get(), 3);
    drop(owner);
    assert!(weak.upgrade().is_none());
    let null_context: Option<&u32> = from_source(Some(id), id, || None);
    assert!(null_context.is_none());
}

#[test]
fn scene_source() {
    borrowed_source(b"scene", b"group");
}

#[test]
fn group_source() {
    borrowed_source(b"group", b"scene");
}
