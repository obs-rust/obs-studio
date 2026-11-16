//! Mirrors the direct constructor calls in test_encoder_output_getters_characterization.c.
use obs_output::encoder_properties::{Host, construct};
use std::cell::RefCell;

struct Mock {
    old: bool,
    new: bool,
    null: bool,
    events: RefCell<Vec<&'static str>>,
}
impl Host for Mock {
    type Settings = usize;
    type Properties = usize;
    fn null() -> usize {
        0
    }
    fn has_properties(&self) -> bool {
        self.old
    }
    fn has_properties2(&self) -> bool {
        self.new
    }
    fn create_settings(&self) -> usize {
        self.events.borrow_mut().push("create");
        11
    }
    fn defaults(&self, s: usize) {
        assert_eq!(s, 11);
        self.events.borrow_mut().push("defaults");
    }
    fn defaults2(&self, s: usize) {
        assert_eq!(s, 11);
        self.events.borrow_mut().push("defaults2");
    }
    fn settings(&self) -> usize {
        22
    }
    fn properties(&self, instance: bool) -> usize {
        self.events
            .borrow_mut()
            .push(if instance { "old-instance" } else { "old-null" });
        if self.null { 0 } else { 1 }
    }
    fn properties2(&self, instance: bool) -> usize {
        self.events
            .borrow_mut()
            .push(if instance { "new-instance" } else { "new-null" });
        if self.null { 0 } else { 2 }
    }
    fn apply(&self, p: &usize, s: usize) {
        assert_eq!(
            *p,
            if self.null {
                0
            } else if self.new {
                2
            } else {
                1
            }
        );
        self.events.borrow_mut().push(if s == 11 {
            "apply-defaults"
        } else {
            assert_eq!(s, 22);
            "apply-settings"
        });
    }
    fn release(&self, s: usize) {
        assert_eq!(s, 11);
        self.events.borrow_mut().push("release");
    }
}

#[test]
fn test_encoder_properties_by_id_and_video_encoder_getters() {
    assert_eq!(construct::<Mock>(None, false), 0);
    assert_eq!(construct::<Mock>(None, true), 0);
    for old in [false, true] {
        for new in [false, true] {
            for null in [false, true] {
                for instance in [false, true] {
                    let h = Mock {
                        old,
                        new,
                        null,
                        events: RefCell::new(Vec::new()),
                    };
                    let p = construct(Some(&h), instance);
                    if !old && !new {
                        assert_eq!(p, 0);
                        assert!(h.events.borrow().is_empty());
                        continue;
                    }
                    assert_eq!(
                        p,
                        if null {
                            0
                        } else if new {
                            2
                        } else {
                            1
                        }
                    );
                    let mut expected = Vec::new();
                    if !instance {
                        expected.extend(["create", "defaults", "defaults2"]);
                    }
                    expected.push(match (new, instance) {
                        (true, true) => "new-instance",
                        (true, false) => "new-null",
                        (false, true) => "old-instance",
                        _ => "old-null",
                    });
                    expected.push(if instance {
                        "apply-settings"
                    } else {
                        "apply-defaults"
                    });
                    if !instance {
                        expected.push("release");
                    }
                    assert_eq!(*h.events.borrow(), expected);
                }
            }
        }
    }
}
