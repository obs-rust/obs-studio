use obs_output::output_last_error::replace;

#[test]
fn encoder_output_getters_characterization_set_and_clear() {
    let mut message = None;
    assert_eq!(message, None);
    replace(&mut message, Some(b"boom"), <[u8]>::to_vec);
    assert_eq!(message.as_deref(), Some(b"boom".as_slice()));
    replace(&mut message, None, <[u8]>::to_vec);
    assert_eq!(message, None);
}

#[test]
fn replacement_empty_and_arbitrary_bytes() {
    let mut message = Some(b"old".to_vec());
    for bytes in [b"replacement".as_slice(), b"", &[0xff, 0x80]] {
        replace(&mut message, Some(bytes), <[u8]>::to_vec);
        assert_eq!(message.as_deref(), Some(bytes));
    }
    replace(&mut message, None, |_| panic!("clearing must not allocate"));
    assert_eq!(message, None);
}

#[test]
fn old_message_is_dropped_before_duplication() {
    use std::cell::Cell;
    struct Message<'a>(&'a Cell<usize>);
    impl Drop for Message<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Cell::new(0);
    let mut message = Some(Message(&drops));
    replace(&mut message, Some(b"new"), |_| {
        assert_eq!(drops.get(), 1);
        Message(&drops)
    });
    replace(&mut message, None, |_| panic!("clearing must not allocate"));
    assert_eq!(drops.get(), 2);
}
