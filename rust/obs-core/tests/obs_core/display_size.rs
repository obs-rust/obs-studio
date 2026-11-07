//! Tier 1 mirrors test_display_size.c, including aliased stores and lock order.
use obs_core::display_size::{SizeAccess, size};

#[derive(Default)]
struct Access {
    present: bool,
    cx: u32,
    cy: u32,
    width: u32,
    height: u32,
    shared_output: bool,
    width_is_cy: bool,
    locked: bool,
    events: Vec<&'static str>,
    unrelated: [u32; 4],
}
impl SizeAccess for Access {
    fn set_width(&mut self, value: u32) {
        self.events.push("write width");
        if self.width_is_cy {
            self.cy = value;
        } else {
            self.width = value;
        }
    }
    fn set_height(&mut self, value: u32) {
        self.events.push("write height");
        if self.shared_output {
            self.width = value;
        } else {
            self.height = value;
        }
    }
    fn has_display(&self) -> bool {
        self.present
    }
    fn lock(&mut self) {
        assert!(!self.locked);
        self.locked = true;
        self.events.push("lock");
    }
    fn width(&self) -> u32 {
        assert!(self.locked);
        self.cx
    }
    fn height(&self) -> u32 {
        assert!(self.locked);
        self.cy
    }
    fn unlock(&mut self) {
        assert!(self.locked);
        self.locked = false;
        self.events.push("unlock");
    }
}

#[test]
fn test_values() {
    let mut a = Access {
        width: 123,
        height: 456,
        ..Default::default()
    };
    size(&mut a);
    assert_eq!((a.width, a.height), (0, 0));
    assert_eq!(a.events, ["write width", "write height"]);
    for value in [0, 1, u32::MAX, 0x8000_0000, 1920] {
        a = Access {
            present: true,
            cx: value,
            cy: !value,
            unrelated: [1, 99, 77, 0x89ab_cdef],
            ..Default::default()
        };
        size(&mut a);
        assert_eq!((a.width, a.height), (value, !value));
        assert_eq!((a.cx, a.cy), (value, !value));
        a.shared_output = true;
        size(&mut a);
        assert_eq!(a.width, !value);
        assert_eq!(a.unrelated, [1, 99, 77, 0x89ab_cdef]);
    }
    a = Access {
        present: true,
        cx: 123,
        cy: 456,
        width_is_cy: true,
        ..Default::default()
    };
    size(&mut a);
    assert_eq!((a.cy, a.height), (123, 123));
}

#[test]
fn test_lock() {
    let mut a = Access {
        present: true,
        cx: u32::MAX,
        cy: 0x8000_0001,
        ..Default::default()
    };
    size(&mut a);
    assert_eq!((a.width, a.height), (u32::MAX, 0x8000_0001));
    assert_eq!(
        a.events,
        [
            "write width",
            "write height",
            "lock",
            "write width",
            "write height",
            "unlock"
        ]
    );
    assert!(!a.locked);
}
