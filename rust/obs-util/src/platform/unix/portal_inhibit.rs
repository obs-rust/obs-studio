//! Safe core of `libobs/util/platform-nix-portal.c.h` (#1631).
//!
//! GIO stays behind [`Host`]. The session bus, the Inhibit method, the
//! Response signal and Close are asynchronous in C; the host stores the
//! callbacks and the tests (or the C ABI shim) run them.
//!
//! The C file only makes `refs` and `destroying` atomic. Every other field
//! is read and written with no lock. This core keeps those fields behind one
//! mutex and drops it before `cancel` or any host call that can run a
//! callback on the same stack. Real GLib can invoke the async callback from
//! inside `g_cancellable_cancel`.

use std::sync::{Arc, Mutex};

use crate::base::{LOG_DEBUG, LOG_ERROR, LOG_WARNING};

/// `G_BUS_TYPE_SESSION`.
pub const SESSION_BUS: i32 = 2;
/// Sleep and idle, the flags `platform-nix-portal.c` always sends.
pub const INHIBIT_FLAGS: u32 = 0xC;
pub const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
pub const INHIBIT_IFACE: &str = "org.freedesktop.portal.Inhibit";
pub const REQUEST_IFACE: &str = "org.freedesktop.portal.Request";
pub const PROPERTIES_IFACE: &str = "org.freedesktop.DBus.Properties";
pub const CALL_FLAGS_NONE: i32 = 0;
pub const CALL_TIMEOUT_INFINITE: i32 = -1;
/// `G_DBUS_SIGNAL_FLAGS_NO_MATCH_RULE`.
pub const SIGNAL_FLAGS_NO_MATCH_RULE: u32 = 1;

/// How `g_dbus_connection_call_finish` ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallResult {
    /// A reply arrived. The Response signal is separate.
    Ok,
    /// A D-Bus error other than cancellation. The string is `GError.message`.
    Failed(String),
    /// `G_IO_ERROR_CANCELLED`.
    Cancelled,
}

/// What the portal client asks the desktop bus to do.
pub trait Host {
    type Conn;

    fn bus_get(&self, bus_type: i32) -> Result<Self::Conn, String>;
    fn unique_name(&self, conn: &Self::Conn) -> String;
    fn properties_get(&self, conn: &Self::Conn, interface: &str, property: &str) -> bool;
    fn request_id(&self) -> u32;
    #[allow(clippy::too_many_arguments)]
    fn subscribe(
        &self,
        conn: &Self::Conn,
        sender: &str,
        interface: &str,
        member: &str,
        path: &str,
        flags: u32,
        on_response: Box<dyn Fn(u32)>,
    ) -> u32;
    fn unsubscribe(&self, conn: &Self::Conn, id: u32);
    #[allow(clippy::too_many_arguments)]
    fn inhibit(
        &self,
        conn: &Self::Conn,
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
    );
    #[allow(clippy::too_many_arguments)]
    fn close(
        &self,
        conn: &Self::Conn,
        destination: &str,
        path: &str,
        interface: &str,
        method: &str,
        call_flags: i32,
        timeout: i32,
        on_done: Box<dyn Fn(CallResult)>,
    );
    fn cancel(&self);
    fn log(&self, level: i32, message: &str);
}

struct State {
    destroying: bool,
    active: bool,
    sender_name: String,
    request_path: Option<String>,
    signal_id: u32,
    has_cancellable: bool,
}

struct Shared<H: Host> {
    host: H,
    /// `info->c`. [`Portal::destroy`] takes it out, as C clears it, while
    /// the callbacks may still hold this struct until GIO frees them (#3444).
    /// A call clones the `Arc` and drops the lock before it reaches GIO.
    conn: Mutex<Option<Arc<H::Conn>>>,
    state: Mutex<State>,
}

impl<H: Host> Shared<H> {
    /// The connection, or `None` after [`Portal::destroy`].
    fn conn(&self) -> Option<Arc<H::Conn>> {
        self.conn.lock().expect("portal conn").clone()
    }
}

/// `struct portal_inhibit_info`.
///
/// Callbacks hold their own [`Arc`] clone, the same way the C callbacks hold
/// a `gatomicrefcount`. Destroying the owner's handle does not wait for them.
pub struct Portal<H: Host> {
    shared: Arc<Shared<H>>,
}

impl<H: Host + 'static> Portal<H> {
    /// `portal_inhibit_info_create`.
    pub fn create(host: H) -> Option<Self> {
        let conn = match host.bus_get(SESSION_BUS) {
            Ok(conn) => conn,
            Err(error) => {
                host.log(
                    LOG_ERROR,
                    &format!("Could not create dbus connection: {error}"),
                );
                return None;
            }
        };
        let sender_name = sender_name(&host.unique_name(&conn));
        if !host.properties_get(&conn, INHIBIT_IFACE, "version") {
            return None;
        }
        host.log(LOG_DEBUG, "Found portal inhibitor");
        Some(Self {
            shared: Arc::new(Shared {
                host,
                conn: Mutex::new(Some(Arc::new(conn))),
                state: Mutex::new(State {
                    destroying: false,
                    active: false,
                    sender_name,
                    request_path: None,
                    signal_id: 0,
                    has_cancellable: false,
                }),
            }),
        })
    }

    /// `portal_inhibit_info_destroy`. `None` is the C NULL handle.
    ///
    /// An active inhibit is not closed. C only sends `Request.Close` from
    /// `portal_inhibit(..., false)`.
    pub fn destroy(portal: Option<Self>) {
        let Some(Portal { shared }) = portal else {
            return;
        };
        let (cancel, signal_id) = {
            let mut state = shared.state.lock().expect("portal state");
            state.destroying = true;
            let cancel = state.has_cancellable;
            state.has_cancellable = false;
            let signal_id = state.signal_id;
            state.signal_id = 0;
            state.request_path = None;
            state.active = false;
            (cancel, signal_id)
        };
        if cancel {
            shared.host.cancel();
        }
        let conn = shared.conn.lock().expect("portal conn").take();
        if signal_id > 0
            && let Some(conn) = conn.as_deref()
        {
            shared.host.unsubscribe(conn, signal_id);
        }
        // `g_clear_object(&info->c)`: a pending call or the subscription
        // may keep `shared` alive, but not the bus connection.
        drop(conn);
    }

    pub fn is_active(&self) -> bool {
        self.shared.state.lock().expect("portal state").active
    }

    pub fn request_path(&self) -> Option<String> {
        self.shared
            .state
            .lock()
            .expect("portal state")
            .request_path
            .clone()
    }

    pub fn subscribed(&self) -> bool {
        self.shared.state.lock().expect("portal state").signal_id > 0
    }

    pub fn has_cancellable(&self) -> bool {
        self.shared
            .state
            .lock()
            .expect("portal state")
            .has_cancellable
    }

    /// `portal_inhibit`.
    pub fn set_active(&self, reason: &str, active: bool) {
        enum Next {
            Inhibit { sender: String },
            Cancel,
            Close { path: String },
        }
        let next = {
            let mut state = self.shared.state.lock().expect("portal state");
            if state.destroying || active == state.active {
                return;
            }
            if active {
                Next::Inhibit {
                    sender: state.sender_name.clone(),
                }
            } else if state.has_cancellable {
                state.has_cancellable = false;
                state.active = false;
                state.request_path = None;
                Next::Cancel
            } else {
                let Some(path) = state.request_path.take() else {
                    state.active = false;
                    return;
                };
                state.active = false;
                Next::Close { path }
            }
        };
        match next {
            Next::Inhibit { sender } => self.do_inhibit(reason, &sender),
            Next::Cancel => self.shared.host.cancel(),
            Next::Close { path } => self.do_close(&path),
        }
    }

    fn do_inhibit(&self, reason: &str, sender: &str) {
        let Some(conn) = self.shared.conn() else {
            return;
        };
        let id = self.shared.host.request_id();
        let token = format!("obs_inhibit_portal{id}");
        let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
        {
            let mut state = self.shared.state.lock().expect("portal state");
            if state.destroying {
                return;
            }
            state.active = true;
            state.request_path = Some(path.clone());
            state.has_cancellable = true;
        }
        let shared = Arc::clone(&self.shared);
        let signal_id = self.shared.host.subscribe(
            &conn,
            PORTAL_NAME,
            REQUEST_IFACE,
            "Response",
            &path,
            SIGNAL_FLAGS_NO_MATCH_RULE,
            Box::new(move |code| on_response(&shared, code)),
        );
        {
            let mut state = self.shared.state.lock().expect("portal state");
            if state.destroying {
                drop(state);
                self.shared.host.unsubscribe(&conn, signal_id);
                return;
            }
            state.signal_id = signal_id;
        }
        let shared = Arc::clone(&self.shared);
        self.shared.host.inhibit(
            &conn,
            PORTAL_NAME,
            PORTAL_PATH,
            INHIBIT_IFACE,
            "Inhibit",
            CALL_FLAGS_NONE,
            CALL_TIMEOUT_INFINITE,
            "",
            INHIBIT_FLAGS,
            &token,
            reason,
            Box::new(move |result| on_inhibited(&shared, result)),
        );
    }

    fn do_close(&self, path: &str) {
        let Some(conn) = self.shared.conn() else {
            return;
        };
        let shared = Arc::clone(&self.shared);
        self.shared.host.close(
            &conn,
            PORTAL_NAME,
            path,
            REQUEST_IFACE,
            "Close",
            CALL_FLAGS_NONE,
            CALL_TIMEOUT_INFINITE,
            Box::new(move |result| on_uninhibited(&shared, result)),
        );
    }
}

/// D-Bus unique names look like `:1.42`. C copies from the second byte and
/// turns each `.` into `_`, which is the object-path segment.
fn sender_name(unique: &str) -> String {
    unique.get(1..).unwrap_or("").replace('.', "_")
}

fn on_response<H: Host>(shared: &Shared<H>, code: u32) {
    let signal_id = {
        let mut state = shared.state.lock().expect("portal state");
        if state.destroying {
            return;
        }
        if code != 0 {
            state.request_path = None;
            state.active = false;
        }
        let signal_id = state.signal_id;
        state.signal_id = 0;
        signal_id
    };
    if code == 1 {
        shared.host.log(LOG_WARNING, "Inhibit denied by user");
    }
    if signal_id > 0
        && let Some(conn) = shared.conn()
    {
        shared.host.unsubscribe(&conn, signal_id);
    }
}

fn on_inhibited<H: Host>(shared: &Shared<H>, result: CallResult) {
    let follow_up = {
        let mut state = shared.state.lock().expect("portal state");
        if state.destroying {
            return;
        }
        let follow_up = match &result {
            CallResult::Ok => None,
            CallResult::Cancelled => {
                let signal_id = state.signal_id;
                state.signal_id = 0;
                state.request_path = None;
                state.active = false;
                Some((signal_id, None))
            }
            CallResult::Failed(message) => {
                let signal_id = state.signal_id;
                state.signal_id = 0;
                state.request_path = None;
                state.active = false;
                Some((signal_id, Some(message.clone())))
            }
        };
        state.has_cancellable = false;
        follow_up
    };
    let Some((signal_id, message)) = follow_up else {
        return;
    };
    if let Some(message) = message {
        shared
            .host
            .log(LOG_ERROR, &format!("Failed to inhibit: {message}"));
    }
    if signal_id > 0
        && let Some(conn) = shared.conn()
    {
        shared.host.unsubscribe(&conn, signal_id);
    }
}

fn on_uninhibited<H: Host>(shared: &Shared<H>, result: CallResult) {
    if let CallResult::Failed(message) = result {
        shared
            .host
            .log(LOG_WARNING, &format!("Error uninhibiting: {message}"));
    }
}
