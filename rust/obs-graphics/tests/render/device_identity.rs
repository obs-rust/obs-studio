//! Tier 1 mirrors test_graphics_device_identity.c without FFI or unsafe code.
use obs_graphics::device_identity::{Entry, Host, identity};
use std::cell::RefCell;

#[derive(Default)]
struct Fixture<'a> {
    context: bool,
    callbacks: bool,
    device: usize,
    last_device: Option<usize>,
    values: [Option<&'a RefCell<String>>; 2],
    calls: [usize; 2],
    logs: Vec<(i32, String)>,
}
impl<'a> Host for Fixture<'a> {
    type Value = Option<&'a RefCell<String>>;
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, entry: Entry) {
        self.logs.push((
            400,
            format!("{}: called while not in a graphics context", entry.name()),
        ));
    }
    fn dispatch(&mut self, entry: Entry) -> Self::Value {
        assert!(self.context && self.callbacks);
        let index = usize::from(entry == Entry::Name);
        self.calls[index] += 1;
        if entry == Entry::Object {
            self.last_device = Some(self.device);
        }
        self.values[index]
    }
}
fn same(actual: Option<&RefCell<String>>, expected: Option<&RefCell<String>>) {
    match (actual, expected) {
        (None, None) => (),
        (Some(a), Some(b)) => assert!(std::ptr::eq(a, b)),
        _ => panic!("borrowed identity differs"),
    }
}
fn diagnostic(host: &Fixture<'_>, entry: Entry, count: usize) {
    let function = match entry {
        Entry::Object => "gs_get_device_obj",
        Entry::Name => "gs_get_device_name",
    };
    assert_eq!(entry.name(), function);
    assert_eq!(host.logs.len(), count);
    assert_eq!(
        host.logs.last(),
        Some(&(
            400,
            format!("{function}: called while not in a graphics context")
        ))
    );
}
#[test]
fn test_graphics_device_identity_no_context() {
    let mut host = Fixture::default();
    for (i, callbacks) in [true, false].into_iter().enumerate() {
        host.callbacks = callbacks;
        same(identity(&mut host, Entry::Object), None);
        diagnostic(&host, Entry::Object, 2 * i + 1);
        same(identity(&mut host, Entry::Name), None);
        diagnostic(&host, Entry::Name, 2 * i + 2);
        assert_eq!(host.calls, [0, 0]);
        assert_eq!(host.last_device, None);
    }
}
#[test]
fn test_graphics_device_identity_borrowed_object() {
    let first = RefCell::new(String::new());
    let second = RefCell::new(String::new());
    let mut host = Fixture {
        context: true,
        callbacks: true,
        device: 1,
        ..Fixture::default()
    };
    for (i, value) in [None, Some(&first), Some(&second), Some(&first)]
        .into_iter()
        .enumerate()
    {
        host.values[0] = value;
        same(identity(&mut host, Entry::Object), value);
        assert_eq!(host.calls, [i + 1, 0]);
        assert_eq!(host.last_device, Some(1));
        assert!(host.logs.is_empty());
    }
    for device in [2, 0] {
        host.device = device;
        same(identity(&mut host, Entry::Object), Some(&first));
        assert_eq!(host.last_device, Some(device));
    }
    assert_eq!(host.calls, [6, 0]);
    assert!(host.logs.is_empty());
}
#[test]
fn test_graphics_device_identity_borrowed_name() {
    let empty = RefCell::new(String::new());
    let first = RefCell::new("recording backend".into());
    let same_bytes = RefCell::new("recording backend".into());
    let mut host = Fixture {
        context: true,
        callbacks: true,
        ..Fixture::default()
    };
    for (i, value) in [
        None,
        Some(&empty),
        Some(&first),
        Some(&same_bytes),
        Some(&first),
    ]
    .into_iter()
    .enumerate()
    {
        host.values[1] = value;
        same(identity(&mut host, Entry::Name), value);
        assert_eq!(host.calls, [0, i + 1]);
        assert!(host.logs.is_empty());
    }
    let borrowed = identity(&mut host, Entry::Name).unwrap();
    first.borrow_mut().replace_range(..1, "R");
    assert_eq!(*borrowed.borrow(), "Recording backend");
    assert_eq!(*same_bytes.borrow(), "recording backend");
    host.device = 0;
    same(identity(&mut host, Entry::Name), Some(&first));
    assert_eq!(host.calls, [0, 7]);
    assert_eq!(host.last_device, None);
    assert!(host.logs.is_empty());
}
#[test]
fn test_graphics_device_identity_context_leave_reenter() {
    let object = RefCell::new(String::new());
    let name = RefCell::new("backend".into());
    let mut host = Fixture {
        context: true,
        callbacks: true,
        values: [Some(&object), Some(&name)],
        ..Fixture::default()
    };
    same(identity(&mut host, Entry::Object), Some(&object));
    same(identity(&mut host, Entry::Name), Some(&name));
    host.context = false;
    assert!(!host.has_context());
    same(identity(&mut host, Entry::Object), None);
    diagnostic(&host, Entry::Object, 1);
    same(identity(&mut host, Entry::Name), None);
    diagnostic(&host, Entry::Name, 2);
    assert_eq!(host.calls, [1, 1]);
    host.context = true;
    same(identity(&mut host, Entry::Object), Some(&object));
    same(identity(&mut host, Entry::Name), Some(&name));
    assert_eq!(host.calls, [2, 2]);
    assert_eq!(host.logs.len(), 2);
}
