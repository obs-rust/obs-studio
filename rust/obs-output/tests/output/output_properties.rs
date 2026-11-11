use obs_output::output_properties::{self as core, Host};
use std::cell::RefCell;

#[derive(Default)]
struct Fake(RefCell<Vec<String>>);
impl Host for Fake {
    type Info = (bool, bool);
    type Data = u32;
    type Props = u32;
    fn create(&self) -> u32 {
        self.0.borrow_mut().push("create".into());
        17
    }
    fn defaults(&self, info: Self::Info, data: u32) {
        if info.0 {
            self.0.borrow_mut().push(format!("defaults:{data}"));
        }
    }
    fn has_properties(&self, info: Self::Info) -> bool {
        info.1
    }
    fn properties(&self, _: Self::Info, data: Option<u32>) -> u32 {
        self.0.borrow_mut().push(format!("properties:{data:?}"));
        5
    }
    fn apply(&self, props: &u32, data: u32) {
        self.0.borrow_mut().push(format!("apply:{props}:{data}"));
    }
    fn release(&self, data: u32) {
        self.0.borrow_mut().push(format!("release:{data}"));
    }
    fn null_output(&self) {
        self.0.borrow_mut().push("null".into());
    }
}
#[test]
fn test_output_lookup_by_id_defaults_and_properties() {
    let host = Fake::default();
    assert_eq!(core::defaults(&host, None), None);
    assert_eq!(core::type_properties(&host, None), None);
    assert_eq!(core::type_properties(&host, Some((true, false))), None);
    assert!(host.0.borrow().is_empty());
    assert_eq!(core::defaults(&host, Some((false, false))), Some(17));
    assert_eq!(*host.0.borrow(), ["create"]);
    host.0.borrow_mut().clear();
    assert_eq!(core::defaults(&host, Some((true, true))), Some(17));
    assert_eq!(*host.0.borrow(), ["create", "defaults:17"]);
    host.0.borrow_mut().clear();
    assert_eq!(core::type_properties(&host, Some((true, true))), Some(5));
    assert_eq!(
        *host.0.borrow(),
        [
            "create",
            "defaults:17",
            "properties:None",
            "apply:5:17",
            "release:17"
        ]
    );
}
#[test]
fn test_output_properties_callbacks_instance_and_null_guards() {
    let host = Fake::default();
    assert_eq!(
        core::properties(&host, Some(((true, true), 9, 43))),
        Some(5)
    );
    assert_eq!(*host.0.borrow(), ["properties:Some(9)", "apply:5:43"]);
    host.0.borrow_mut().clear();
    assert_eq!(core::properties(&host, Some(((true, false), 9, 43))), None);
    assert!(host.0.borrow().is_empty());
    assert_eq!(core::properties(&host, None), None);
    assert_eq!(*host.0.borrow(), ["null"]);
}
