//! Tier 1 mirrors test_encoder_defaults_dispatch.c without FFI or unsafe code.
use obs_output::encoder_defaults::{Host, query};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Default)]
struct Settings {
    winner: Option<i64>,
    legacy: bool,
    caller_only: Option<i64>,
    existing: Option<i64>,
}
struct Mock {
    mask: u8,
    type_data: Option<usize>,
    calls: Cell<usize>,
    callback_settings: RefCell<Option<Rc<RefCell<Settings>>>>,
}
impl Host for Mock {
    type Settings = Rc<RefCell<Settings>>;
    type TypeData = Option<usize>;
    fn create_settings(&self) -> Self::Settings {
        Rc::new(RefCell::new(Settings::default()))
    }
    fn has_legacy(&self) -> bool { self.mask & 1 != 0 }
    fn has_modern(&self) -> bool { self.mask & 2 != 0 }
    fn legacy(&self, settings: &Self::Settings) {
        assert_eq!(self.calls.replace(self.calls.get() + 1), 0);
        *self.callback_settings.borrow_mut() = Some(Rc::clone(settings));
        settings.borrow_mut().winner = Some(11);
        settings.borrow_mut().legacy = true;
    }
    fn type_data(&self) -> Self::TypeData { self.type_data }
    fn modern(&self, settings: &Self::Settings, type_data: Self::TypeData) {
        assert_eq!(self.calls.replace(self.calls.get() + 1), usize::from(self.has_legacy()));
        if self.has_legacy() {
            assert!(Rc::ptr_eq(settings, self.callback_settings.borrow().as_ref().unwrap()));
            assert_eq!(settings.borrow().winner, Some(11));
            assert!(settings.borrow().legacy);
        } else {
            assert_eq!(settings.borrow().winner, None);
        }
        // Both NULL and non-NULL opaque type-data identities are accepted unchanged.
        assert_eq!(type_data, self.type_data);
        *self.callback_settings.borrow_mut() = Some(Rc::clone(settings));
        settings.borrow_mut().winner = Some(22);
    }
}
fn query_checked(host: &Mock) -> Rc<RefCell<Settings>> {
    host.calls.set(0);
    host.callback_settings.borrow_mut().take();
    let settings = query(Some(host), || panic!("unexpected diagnostic")).unwrap();
    assert_eq!(host.calls.get(), usize::from(host.has_legacy()) + usize::from(host.has_modern()));
    if host.mask != 0 {
        assert!(Rc::ptr_eq(&settings, host.callback_settings.borrow().as_ref().unwrap()));
        assert_eq!(settings.borrow().winner, Some(if host.has_modern() {22} else {11}));
        // A winner is a default, never a user value; user fields remain empty.
    } else {
        assert_eq!(settings.borrow().winner, None);
        assert!(!settings.borrow().legacy);
    }
    assert_eq!(settings.borrow().caller_only, None);
    assert_eq!(settings.borrow().existing, None);
    settings
}
fn fresh_owned(mask: u8, type_data: Option<usize>, instance: bool) {
    let host = Mock { mask, type_data, calls: Cell::new(0), callback_settings: RefCell::new(None) };
    let context = Rc::new(RefCell::new(Settings { existing: Some(123), ..Default::default() }));
    let first = query_checked(&host);
    first.borrow_mut().caller_only = Some(99);
    first.borrow_mut().winner = Some(77);
    let second = query_checked(&host);
    assert!(!Rc::ptr_eq(&first, &second));
    if instance {
        assert!(!Rc::ptr_eq(&first, &context));
        assert!(!Rc::ptr_eq(&second, &context));
        assert_eq!(context.borrow().existing, Some(123));
        assert_eq!(context.borrow().winner, None);
    }
    drop(first);
    second.borrow_mut().caller_only = Some(42);
    let extra_reference = Rc::clone(&second);
    drop(extra_reference);
    assert_eq!(second.borrow().caller_only, Some(42));
    drop(second);
    drop(query_checked(&host));
}
#[test]
fn by_id() {
    assert!(query::<Mock>(None, || {}).is_none());
    for mask in 0..4 { fresh_owned(mask, Some(73), false); }
}
#[test]
fn by_instance() {
    for mask in 0..4 {
        fresh_owned(mask, Some(73), true);
        fresh_owned(mask, None, true);
    }
}
#[test]
fn null_instance() {
    let logs = Cell::new(0);
    assert!(query::<Mock>(None, || logs.set(logs.get() + 1)).is_none());
    assert_eq!(logs.get(), 1);
}
