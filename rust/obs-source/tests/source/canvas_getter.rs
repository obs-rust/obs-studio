//! Tier 1 mirrors test_source_canvas_getter.c without FFI or raw pointers.
use obs_source::canvas_getter::get;
use std::rc::{Rc, Weak};

fn query(weak: &Option<Weak<u32>>) -> Option<Rc<u32>> {
    let mut calls = 0;
    let result = get(weak.clone(), |weak| {
        calls += 1;
        weak.and_then(|weak| weak.upgrade())
    });
    assert_eq!(calls, 1);
    result
}

fn expect_owned_canvas(weak: &Option<Weak<u32>>, expected: &Rc<u32>) {
    let stored = weak.as_ref().unwrap();
    let refs = stored.strong_count();
    let weak_refs = stored.weak_count();
    let first = query(weak).unwrap();
    let second = query(weak).unwrap();
    assert!(Rc::ptr_eq(&first, expected));
    assert!(Rc::ptr_eq(&second, expected));
    assert_eq!(stored.strong_count(), refs + 2);
    assert_eq!(stored.weak_count(), weak_refs);
    assert!(Weak::ptr_eq(stored, weak.as_ref().unwrap()));
    drop(first);
    assert_eq!(stored.strong_count(), refs + 1);
    drop(second);
    assert_eq!(stored.strong_count(), refs);
}

#[test]
fn test_source_canvas_getter_main_canvas() {
    let canvas = Rc::new(1);
    let weak = Some(Rc::downgrade(&canvas));
    expect_owned_canvas(&weak, &canvas);
}

#[test]
fn test_source_canvas_getter_private_canvas() {
    let canvas = Rc::new(2);
    let weak = Some(Rc::downgrade(&canvas));
    expect_owned_canvas(&weak, &canvas);
    let owned = query(&weak).unwrap();
    assert!(Rc::ptr_eq(&owned, &canvas));
    drop(canvas);
    expect_owned_canvas(&weak, &owned);
}

#[test]
fn test_source_canvas_getter_moved_source() {
    let from = Rc::new(3);
    let to = Rc::new(4);
    let mut weak = Some(Rc::downgrade(&from));
    expect_owned_canvas(&weak, &from);
    let old_owner = query(&weak).unwrap();
    assert!(Rc::ptr_eq(&old_owner, &from));
    weak = Some(Rc::downgrade(&to));
    expect_owned_canvas(&weak, &to);
    drop(from);
    let old_ref = old_owner.clone();
    assert!(Rc::ptr_eq(&old_ref, &old_owner));
    drop(old_ref);
    drop(old_owner);
    weak = Some(Rc::downgrade(&to));
    expect_owned_canvas(&weak, &to);
}

#[test]
fn test_source_canvas_getter_null_weak_canvas() {
    let weak = None;
    assert!(query(&weak).is_none());
    assert!(weak.is_none());
}

#[test]
fn test_source_canvas_getter_expired_weak_canvas() {
    let canvas = Rc::new(5);
    let weak = Some(Rc::downgrade(&canvas));
    drop(canvas);
    let stored = weak.as_ref().unwrap();
    let weak_refs = stored.weak_count();
    assert_eq!(stored.strong_count(), 0);
    assert!(query(&weak).is_none());
    assert!(query(&weak).is_none());
    assert_eq!(stored.strong_count(), 0);
    assert_eq!(stored.weak_count(), weak_refs);
}
