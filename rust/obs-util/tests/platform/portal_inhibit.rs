//! Tier 1 mirror of `test/cmocka/test_inhibit_portal.c` (#1631).
//! The C test includes the original file, so it cannot see this swap.
//! `test_inhibit_sleep_dbus` remains the public OFF/ON check.
//! Callback ownership is a live-count here. C counts `bnum_allocs` because
//! the struct is `bzalloc`; this handle is an `Arc`.

#![cfg(any(target_os = "linux", target_os = "freebsd"))]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use obs_util::platform::unix::portal_inhibit::{
    CALL_FLAGS_NONE, CALL_TIMEOUT_INFINITE, CallResult, Host, INHIBIT_FLAGS, INHIBIT_IFACE,
    PORTAL_NAME, PORTAL_PATH, Portal, REQUEST_IFACE, SESSION_BUS, SIGNAL_FLAGS_NO_MATCH_RULE,
};

// `dyn Fn(u32)` desugars past clippy's type-complexity limit.
type ResponseFn = Rc<dyn Fn(u32)>;

/// The fake bus connection. It counts its drops, as `g_object_unref` on the
/// C's `info->c` would free it.
struct Conn(Rc<Cell<u32>>);

impl Drop for Conn {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

struct Pending {
    done: Box<dyn Fn(CallResult)>,
    cancel: Option<Rc<Cell<bool>>>,
}

struct Inner {
    bus_missing: Cell<bool>,
    portal_missing: Cell<bool>,
    next_id: Cell<u32>,
    inhibits: Cell<u32>,
    closes: Cell<u32>,
    unsubscribes: Cell<u32>,
    flags: Cell<u32>,
    window: RefCell<String>,
    reason: RefCell<String>,
    token: RefCell<String>,
    sub_path: RefCell<String>,
    live: Cell<usize>,
    conns_dropped: Rc<Cell<u32>>,
    logs: RefCell<Vec<(i32, String)>>,
    response: RefCell<Option<(u32, ResponseFn)>>,
    calls: RefCell<Vec<Pending>>,
}

#[derive(Clone)]
struct Transport(Rc<Inner>);

impl Transport {
    fn new() -> Self {
        Self(Rc::new(Inner {
            bus_missing: Cell::new(false),
            portal_missing: Cell::new(false),
            next_id: Cell::new(7),
            inhibits: Cell::new(0),
            closes: Cell::new(0),
            unsubscribes: Cell::new(0),
            flags: Cell::new(0),
            window: RefCell::new(String::new()),
            reason: RefCell::new(String::new()),
            token: RefCell::new(String::new()),
            sub_path: RefCell::new(String::new()),
            live: Cell::new(0),
            conns_dropped: Rc::new(Cell::new(0)),
            logs: RefCell::new(Vec::new()),
            response: RefCell::new(None),
            calls: RefCell::new(Vec::new()),
        }))
    }

    fn complete(&self, error: bool) {
        let call = self.0.calls.borrow_mut().remove(0);
        let cancelled = call.cancel.as_ref().is_some_and(|flag| flag.get());
        let result = if cancelled {
            CallResult::Cancelled
        } else if error {
            CallResult::Failed("portal error".into())
        } else {
            CallResult::Ok
        };
        self.0.live.set(self.0.live.get() - 1);
        (call.done)(result);
    }

    fn respond(&self, code: u32) {
        let response = self
            .0
            .response
            .borrow()
            .as_ref()
            .map(|slot| Rc::clone(&slot.1))
            .expect("response");
        response(code);
    }
}

#[allow(clippy::too_many_arguments)]
impl Host for Transport {
    type Conn = Conn;

    fn bus_get(&self, bus_type: i32) -> Result<Conn, String> {
        assert_eq!(bus_type, SESSION_BUS);
        if self.0.bus_missing.get() {
            Err("no session bus".into())
        } else {
            Ok(Conn(Rc::clone(&self.0.conns_dropped)))
        }
    }

    fn unique_name(&self, _: &Conn) -> String {
        ":1.42".into()
    }

    fn properties_get(&self, _: &Conn, interface: &str, property: &str) -> bool {
        assert_eq!(interface, INHIBIT_IFACE);
        assert_eq!(property, "version");
        !self.0.portal_missing.get()
    }

    fn request_id(&self) -> u32 {
        let id = self.0.next_id.get();
        self.0.next_id.set(id.wrapping_add(1));
        id
    }

    fn subscribe(
        &self,
        _: &Conn,
        sender: &str,
        interface: &str,
        member: &str,
        path: &str,
        flags: u32,
        on_response: Box<dyn Fn(u32)>,
    ) -> u32 {
        assert_eq!(sender, PORTAL_NAME);
        assert_eq!(interface, REQUEST_IFACE);
        assert_eq!(member, "Response");
        assert_eq!(flags, SIGNAL_FLAGS_NO_MATCH_RULE);
        assert!(self.0.response.borrow().is_none());
        *self.0.sub_path.borrow_mut() = path.to_string();
        self.0.live.set(self.0.live.get() + 1);
        self.0.response.replace(Some((17, Rc::from(on_response))));
        17
    }

    fn unsubscribe(&self, _: &Conn, id: u32) {
        assert_eq!(id, 17);
        self.0.unsubscribes.set(self.0.unsubscribes.get() + 1);
        let response = self.0.response.borrow_mut().take();
        assert!(response.is_some());
        self.0.live.set(self.0.live.get() - 1);
    }

    fn inhibit(
        &self,
        _: &Conn,
        destination: &str,
        path: &str,
        interface: &str,
        method: &str,
        call_flags: i32,
        timeout: i32,
        window: &str,
        inhibit_flags: u32,
        token: &str,
        reason: &str,
        on_done: Box<dyn Fn(CallResult)>,
    ) {
        assert_eq!(destination, PORTAL_NAME);
        assert_eq!(path, PORTAL_PATH);
        assert_eq!(interface, INHIBIT_IFACE);
        assert_eq!(method, "Inhibit");
        assert_eq!(call_flags, CALL_FLAGS_NONE);
        assert_eq!(timeout, CALL_TIMEOUT_INFINITE);
        self.0.inhibits.set(self.0.inhibits.get() + 1);
        self.0.flags.set(inhibit_flags);
        *self.0.window.borrow_mut() = window.to_string();
        *self.0.reason.borrow_mut() = reason.to_string();
        *self.0.token.borrow_mut() = token.to_string();
        let cancel = Rc::new(Cell::new(false));
        self.0.live.set(self.0.live.get() + 1);
        self.0.calls.borrow_mut().push(Pending {
            done: on_done,
            cancel: Some(cancel),
        });
    }

    fn close(
        &self,
        _: &Conn,
        destination: &str,
        path: &str,
        interface: &str,
        method: &str,
        call_flags: i32,
        timeout: i32,
        on_done: Box<dyn Fn(CallResult)>,
    ) {
        assert_eq!(destination, PORTAL_NAME);
        assert_eq!(path, self.0.sub_path.borrow().as_str());
        assert_eq!(interface, REQUEST_IFACE);
        assert_eq!(method, "Close");
        assert_eq!(call_flags, CALL_FLAGS_NONE);
        assert_eq!(timeout, CALL_TIMEOUT_INFINITE);
        self.0.closes.set(self.0.closes.get() + 1);
        self.0.live.set(self.0.live.get() + 1);
        self.0.calls.borrow_mut().push(Pending {
            done: on_done,
            cancel: None,
        });
    }

    fn cancel(&self) {
        let calls = self.0.calls.borrow();
        let pending = calls.iter().rev().find(|call| call.cancel.is_some());
        pending
            .and_then(|call| call.cancel.clone())
            .expect("cancellable")
            .set(true);
    }

    fn log(&self, level: i32, message: &str) {
        self.0.logs.borrow_mut().push((level, message.to_string()));
    }
}

#[test]
fn test_discovery() {
    let transport = Transport::new();
    Portal::<Transport>::destroy(None);
    transport.0.bus_missing.set(true);
    assert!(Portal::create(transport.clone()).is_none());
    transport.0.bus_missing.set(false);
    transport.0.portal_missing.set(true);
    assert!(Portal::create(transport.clone()).is_none());
    transport.0.portal_missing.set(false);
    let info = Portal::create(transport.clone()).expect("portal");
    assert!(!info.is_active());
    info.set_active("unused", false);
    assert!(transport.0.calls.borrow().is_empty());
    Portal::destroy(Some(info));
    assert_eq!(transport.0.live.get(), 0);
}

#[test]
fn test_request_and_success() {
    let transport = Transport::new();
    let info = Portal::create(transport.clone()).expect("portal");
    info.set_active("recording ☃", true);
    assert!(info.is_active());
    assert_eq!(transport.0.window.borrow().as_str(), "");
    assert_eq!(transport.0.flags.get(), INHIBIT_FLAGS);
    assert_eq!(transport.0.reason.borrow().as_str(), "recording ☃");
    let token = transport.0.token.borrow().clone();
    assert!(token.starts_with("obs_inhibit_portal"));
    assert_eq!(
        transport.0.sub_path.borrow().as_str(),
        format!("/org/freedesktop/portal/desktop/request/1_42/{token}")
    );
    info.set_active("ignored replacement reason", true);
    assert_eq!(transport.0.inhibits.get(), 1);
    transport.complete(false);
    assert!(!info.has_cancellable());
    transport.respond(0);
    assert!(info.is_active());
    assert_eq!(transport.0.unsubscribes.get(), 1);
    info.set_active("unused", false);
    assert!(!info.is_active());
    assert!(info.request_path().is_none());
    assert_eq!(transport.0.closes.get(), 1);
    info.set_active("unused", false);
    assert_eq!(transport.0.closes.get(), 1);
    transport.complete(false);
    Portal::destroy(Some(info));
    assert_eq!(transport.0.live.get(), 0);
}

#[test]
fn test_rejected_responses() {
    for code in [1, 2, 99] {
        let transport = Transport::new();
        let info = Portal::create(transport.clone()).expect("portal");
        info.set_active("", true);
        transport.complete(false);
        transport.respond(code);
        assert!(!info.is_active());
        assert!(info.request_path().is_none());
        assert!(!info.subscribed());
        info.set_active("unused", false);
        assert_eq!(transport.0.closes.get(), 0);
        info.set_active("retry", true);
        transport.complete(false);
        transport.respond(0);
        Portal::destroy(Some(info));
        assert_eq!(transport.0.live.get(), 0);
    }
}

#[test]
fn test_method_error() {
    let transport = Transport::new();
    let info = Portal::create(transport.clone()).expect("portal");
    info.set_active("recording", true);
    transport.complete(true);
    assert!(!info.is_active());
    assert!(info.request_path().is_none());
    assert!(!info.has_cancellable());
    assert_eq!(transport.0.unsubscribes.get(), 1);
    info.set_active("retry", true);
    assert_eq!(transport.0.inhibits.get(), 2);
    transport.complete(false);
    transport.respond(0);
    Portal::destroy(Some(info));
}

#[test]
fn test_cancel_pending() {
    let transport = Transport::new();
    let info = Portal::create(transport.clone()).expect("portal");
    info.set_active("recording", true);
    info.set_active("unused", false);
    assert!(transport.0.calls.borrow()[0].cancel.as_ref().unwrap().get());
    assert!(!info.is_active());
    assert!(!info.has_cancellable());
    assert!(info.request_path().is_none());
    assert_eq!(transport.0.closes.get(), 0);
    transport.complete(true);
    assert_eq!(transport.0.unsubscribes.get(), 1);
    Portal::destroy(Some(info));
}

#[test]
fn test_destroy_pending() {
    let transport = Transport::new();
    let info = Portal::create(transport.clone()).expect("portal");
    info.set_active("recording", true);
    let before = transport.0.live.get();
    Portal::destroy(Some(info));
    assert!(transport.0.calls.borrow()[0].cancel.as_ref().unwrap().get());
    assert_eq!(transport.0.unsubscribes.get(), 1);
    assert_eq!(transport.0.closes.get(), 0);
    assert!(transport.0.live.get() > 0);
    assert!(transport.0.live.get() < before);
    transport.complete(true);
    assert_eq!(transport.0.live.get(), 0);
}

/// `portal_inhibit_info_destroy` drops the bus connection
/// (`g_clear_object(&info->c)`) even while a pending call still holds the
/// info, so nothing keeps the session bus alive after it (#3444).
#[test]
fn test_destroy_pending_drops_the_connection() {
    let transport = Transport::new();
    let info = Portal::create(transport.clone()).expect("portal");
    info.set_active("recording", true);
    assert_eq!(transport.0.conns_dropped.get(), 0);
    Portal::destroy(Some(info));
    assert!(
        transport.0.live.get() > 0,
        "the Inhibit call still holds the info"
    );
    assert_eq!(transport.0.conns_dropped.get(), 1);
    transport.complete(true);
    assert_eq!(transport.0.live.get(), 0);
    assert_eq!(transport.0.conns_dropped.get(), 1);
}

#[test]
fn test_destroy_pending_close() {
    let transport = Transport::new();
    let info = Portal::create(transport.clone()).expect("portal");
    info.set_active("recording", true);
    transport.complete(false);
    transport.respond(0);
    info.set_active("unused", false);
    Portal::destroy(Some(info));
    assert!(transport.0.live.get() > 0);
    transport.complete(false);
    assert_eq!(transport.0.live.get(), 0);
}

#[test]
fn test_response_before_reply() {
    for code in [0, 1, 2] {
        let transport = Transport::new();
        let info = Portal::create(transport.clone()).expect("portal");
        info.set_active("recording", true);
        transport.respond(code);
        assert_eq!(info.is_active(), code == 0);
        assert!(!info.subscribed());
        assert!(info.has_cancellable());
        transport.complete(false);
        assert!(!info.has_cancellable());
        assert_eq!(info.is_active(), code == 0);
        Portal::destroy(Some(info));
        assert_eq!(transport.0.live.get(), 0);
    }
}

#[test]
fn test_destroy_active() {
    let transport = Transport::new();
    let info = Portal::create(transport.clone()).expect("portal");
    info.set_active("recording", true);
    transport.complete(false);
    transport.respond(0);
    Portal::destroy(Some(info));
    assert_eq!(transport.0.closes.get(), 0);
    assert_eq!(transport.0.live.get(), 0);
}

#[test]
fn test_discovery_logs_the_c_messages() {
    let transport = Transport::new();
    transport.0.bus_missing.set(true);
    assert!(Portal::create(transport.clone()).is_none());
    assert_eq!(
        transport.0.logs.borrow().as_slice(),
        &[(
            100,
            "Could not create dbus connection: no session bus".into()
        )]
    );
    transport.0.logs.borrow_mut().clear();
    transport.0.bus_missing.set(false);
    transport.0.portal_missing.set(true);
    assert!(Portal::create(transport.clone()).is_none());
    assert!(transport.0.logs.borrow().is_empty());
    transport.0.portal_missing.set(false);
    let info = Portal::create(transport.clone()).expect("portal");
    assert_eq!(
        transport.0.logs.borrow().as_slice(),
        &[(400, "Found portal inhibitor".into())]
    );
    Portal::destroy(Some(info));
}
