//! Tier 3: session D-Bus inhibitor state and GVariant calls against the
//! original C file. The C oracle's GIO boundary is deterministic; both
//! sides see the same service owners, failures and reply cookie.
//! The sole excluded input is a NULL reason: C forwards it to GLib's string
//! constructor, which rejects it. The Rust C shim uses an empty string.

use std::cell::RefCell;
use std::ffi::CStr;
use std::rc::Rc;
use std::sync::Mutex;

use obs_c_oracle::dbus_inhibit as c;
use obs_util::platform::unix::dbus_inhibit::{DbusInfo, Host, Service};
use proptest::prelude::*;

static ORACLE: Mutex<()> = Mutex::new(());

#[derive(Clone)]
struct Fake {
    mask: u32,
    fail_connection: bool,
    fail_call: bool,
    cookie: u32,
    trace: Rc<RefCell<String>>,
}

impl Fake {
    fn record(&self, line: &str) {
        self.trace.borrow_mut().push_str(line);
        self.trace.borrow_mut().push('\n');
    }
}

fn index(service: Service) -> u32 {
    match service {
        Service::ScreenSaver => 0,
        Service::PowerManagement => 1,
        Service::MateSession => 2,
        Service::GnomeSession => 3,
    }
}

impl Host for Fake {
    type Connection = ();

    fn connect(&self) -> Result<(), String> {
        self.record("connect 2");
        if self.fail_connection {
            Err("no session bus".into())
        } else {
            Ok(())
        }
    }

    fn has_owner(&self, _: &(), service: Service) -> bool {
        self.record(&format!("owner {} 1", service.name()));
        self.mask & (1 << index(service)) != 0
    }

    fn inhibit(&self, _: &(), service: Service, reason: &CStr) -> Result<u32, String> {
        let reason = reason.to_string_lossy();
        if matches!(service, Service::MateSession | Service::GnomeSession) {
            self.record(&format!("inhibit {} libobs {reason} 0 12", service.name()));
        } else {
            self.record(&format!("inhibit {} libobs {reason}", service.name()));
        }
        if self.fail_call {
            Err("call failed".into())
        } else {
            Ok(self.cookie)
        }
    }

    fn uninhibit(&self, _: &(), service: Service, cookie: u32) -> Result<(), String> {
        self.record(&format!(
            "uninhibit {} {} {cookie}",
            service.name(),
            service.uninhibit_method().to_string_lossy()
        ));
        if self.fail_call {
            Err("call failed".into())
        } else {
            Ok(())
        }
    }

    fn log(&self, level: i32, message: &str) {
        self.record(&format!("log {level} {message}"));
    }
}

fn compare(mask: u32, fail_connection: bool, fail_call: bool, cookie: u32, steps: &[bool]) {
    let _lock = ORACLE.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: the oracle owns its configuration, returned handle and trace.
    let c_trace = unsafe {
        c::oracle_dbus_stub_config(mask, fail_connection, fail_call, cookie);
        let info = c::oracle_dbus_sleep_info_create();
        if !info.is_null() {
            for &active in steps {
                c::oracle_dbus_inhibit_sleep(info, c"OBS Video/audio".as_ptr(), active);
            }
        }
        c::oracle_dbus_sleep_info_destroy(info);
        let mut buf = vec![0i8; 8192];
        let len = c::oracle_dbus_stub_take_trace(buf.as_mut_ptr(), buf.len());
        assert!(len < buf.len(), "oracle trace truncated");
        CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
    };

    let trace = Rc::new(RefCell::new(String::new()));
    let host = Fake {
        mask,
        fail_connection,
        fail_call,
        cookie,
        trace: Rc::clone(&trace),
    };
    if let Some(mut info) = DbusInfo::create(host) {
        for &active in steps {
            info.set_active(c"OBS Video/audio", active);
        }
    }
    let rust_trace = trace.borrow().clone();
    assert_eq!(
        rust_trace, c_trace,
        "mask={mask} fail_connection={fail_connection} fail_call={fail_call} cookie={cookie} steps={steps:?}"
    );
}

#[test]
fn service_selection_and_cookie_lifecycle_match_c() {
    for mask in 0..16 {
        compare(
            mask,
            false,
            false,
            4242,
            &[true, true, false, false, true, false],
        );
        compare(mask, false, true, 4242, &[true, true, false]);
        compare(mask, false, false, 0, &[true, true, false]);
    }
    compare(15, true, false, 4242, &[true]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(128)))]

    #[test]
    fn random_state_transitions_match_c(
        mask in 0u32..16,
        fail_connection in any::<bool>(),
        fail_call in any::<bool>(),
        cookie in 0u32..10000,
        steps in prop::collection::vec(any::<bool>(), 0..16),
    ) {
        compare(mask, fail_connection, fail_call, cookie, &steps);
    }
}
