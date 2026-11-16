use obs_encoder::last_error::{duplicate, replace};
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn test_encoder_error_ownership() {
    let mut slot = None;
    assert_eq!(duplicate(slot.as_ref(), Vec::<u8>::clone), None);
    let mut input = vec![b'a', 0xff, b'b'];
    replace(&mut slot, Some(&input), <[u8]>::to_vec);
    input[0] = b'z';
    let mut first = duplicate(slot.as_ref(), Vec::clone).unwrap();
    let second = duplicate(slot.as_ref(), Vec::clone).unwrap();
    assert_ne!(slot.as_ref().unwrap().as_ptr(), input.as_ptr());
    assert_ne!(first.as_ptr(), slot.as_ref().unwrap().as_ptr());
    assert_ne!(first.as_ptr(), second.as_ptr());
    first[0] = b'x';
    assert_eq!(second, [b'a', 0xff, b'b']);
    assert_eq!(slot.as_ref().unwrap(), &second);
    replace(&mut slot, Some(b"replacement"), <[u8]>::to_vec);
    assert_eq!(slot.as_deref(), Some(b"replacement".as_slice()));
    assert_eq!(second, [b'a', 0xff, b'b']);
    replace(&mut slot, Some(b""), <[u8]>::to_vec);
    assert_eq!(duplicate(slot.as_ref(), Vec::clone), Some(vec![]));
    replace(&mut slot, None, <[u8]>::to_vec);
    assert_eq!(duplicate(slot.as_ref(), Vec::clone), None);
}

#[test]
fn frees_before_copy_and_clear_does_not_allocate() {
    struct Owned(Rc<Cell<usize>>);
    impl Drop for Owned {
        fn drop(&mut self) {
            self.0.set(self.0.get() - 1);
        }
    }
    let live = Rc::new(Cell::new(1));
    let mut slot = Some(Owned(live.clone()));
    replace(&mut slot, Some(b""), |_| {
        assert_eq!(live.get(), 0);
        live.set(1);
        Owned(live.clone())
    });
    replace(&mut slot, None, |_| panic!("clear allocated"));
    assert_eq!(live.get(), 0);
}
