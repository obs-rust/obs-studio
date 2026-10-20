//! Tier 1 for `libobs/util/platform-nix-inhibit.c` (#1500): the safe core
//! against a host that records every call it makes. The first cases port
//! test/cmocka/test_inhibit_sleep.c and test_inhibit_sleep_dbus.c 1:1 (same
//! names); the rest reach what a C test cannot: the reset thread without a
//! 30 s wait, a failed spawn, a failed thread start, the Flatpak branch, and
//! a build without GIO.

#![cfg(any(target_os = "linux", target_os = "freebsd"))]

use std::ffi::{CStr, CString};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use obs_c_oracle as _;
use obs_util::platform::Event;
use obs_util::platform::unix::nix_inhibit::{
    self, Host, Inhibit, RESET_INTERVAL_MS, SCREENSAVER_ARGS, SCREENSAVER_PROGRAM,
};

use crate::common::ScratchDir;

const REASON: &CStr = c"OBS Video/audio";

/// What the desktop has: a portal, a session D-Bus inhibitor, a Flatpak.
#[derive(Clone, Copy, Default)]
struct Desk {
    portal: bool,
    dbus: bool,
    flatpak: bool,
}

/// Records every call. `GIO` is the C's `GIO_FOUND`.
struct TestHost<const GIO: bool> {
    desk: Desk,
    calls: Mutex<Vec<String>>,
    next_id: Mutex<u32>,
    /// Waits that time out (one reset each) before a thread parks.
    timeouts: Mutex<usize>,
    /// Fallback threads blocked in their wait for `stop`.
    parked: (Mutex<usize>, Condvar),
    spawn_result: Result<(), i32>,
    thread_fails: bool,
}

type Gio = TestHost<true>;
type NoGio = TestHost<false>;

impl<const GIO: bool> TestHost<GIO> {
    fn new(desk: Desk) -> Arc<Self> {
        Self::with(desk, Ok(()), false)
    }

    fn with(desk: Desk, spawn_result: Result<(), i32>, thread_fails: bool) -> Arc<Self> {
        Arc::new(Self {
            desk,
            calls: Mutex::new(Vec::new()),
            next_id: Mutex::new(0),
            timeouts: Mutex::new(0),
            parked: (Mutex::new(0), Condvar::new()),
            spawn_result,
            thread_fails,
        })
    }

    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }

    /// The calls since the last `take`.
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.calls.lock().unwrap())
    }

    fn id(&self) -> u32 {
        let mut id = self.next_id.lock().unwrap();
        *id += 1;
        *id
    }

    fn set_timeouts(&self, n: usize) {
        *self.timeouts.lock().unwrap() = n;
    }

    /// Blocks until `n` fallback threads are parked in their wait. Fails
    /// after 10 s rather than hanging.
    fn await_parked(&self, n: usize) {
        let (lock, cond) = &self.parked;
        let (guard, timeout) = cond
            .wait_timeout_while(lock.lock().unwrap(), Duration::from_secs(10), |p| *p != n)
            .unwrap();
        assert!(
            !timeout.timed_out(),
            "expected {n} parked threads, have {}",
            *guard
        );
    }
}

fn reason_text(reason: Option<&CStr>) -> String {
    reason.map_or_else(|| "NULL".into(), |r| format!("{r:?}"))
}

impl<const GIO: bool> Host for TestHost<GIO> {
    const GIO: bool = GIO;
    type Portal = u32;
    type Dbus = u32;

    fn portal_create(&self) -> Option<u32> {
        self.record("portal_create".into());
        self.desk.portal.then(|| self.id())
    }
    fn dbus_create(&self) -> Option<u32> {
        self.record("dbus_create".into());
        self.desk.dbus.then(|| self.id())
    }
    fn portal_inhibit(&self, portal: &mut u32, reason: Option<&CStr>, active: bool) {
        self.record(format!(
            "portal_inhibit {portal} {} {active}",
            reason_text(reason)
        ));
    }
    fn dbus_inhibit(&self, dbus: &mut u32, reason: Option<&CStr>, active: bool) {
        self.record(format!(
            "dbus_inhibit {dbus} {} {active}",
            reason_text(reason)
        ));
    }
    fn portal_destroy(&self, portal: u32) {
        self.record(format!("portal_destroy {portal}"));
    }
    fn dbus_destroy(&self, dbus: u32) {
        self.record(format!("dbus_destroy {dbus}"));
    }
    fn log(&self, level: i32, message: &str) {
        self.record(format!("log {level} {message}"));
    }
    fn in_flatpak(&self) -> bool {
        self.record("flatpak".into());
        self.desk.flatpak
    }
    fn spawn(&self, program: &str, args: &[&str]) -> Result<(), i32> {
        self.record(format!("spawn {program} {}", args.join(" ")));
        self.spawn_result
    }
    fn wait_for_stop(&self, stop: &Event, milliseconds: u64) -> bool {
        {
            let mut timeouts = self.timeouts.lock().unwrap();
            if *timeouts > 0 {
                *timeouts -= 1;
                self.record(format!("wait {milliseconds} timeout"));
                return false;
            }
        }
        self.record(format!("wait {milliseconds}"));
        let (lock, cond) = &self.parked;
        *lock.lock().unwrap() += 1;
        cond.notify_all();
        let stopped = stop.timed_wait(milliseconds);
        *lock.lock().unwrap() -= 1;
        cond.notify_all();
        stopped
    }
    fn spawn_thread(
        &self,
        body: Box<dyn FnOnce() + Send>,
    ) -> std::io::Result<std::thread::JoinHandle<()>> {
        if self.thread_fails {
            return Err(std::io::Error::other("no threads"));
        }
        std::thread::Builder::new().spawn(body)
    }
}

/// No desktop inhibitor: what test_inhibit_sleep.c gets with no session bus.
const NO_BUS: Desk = Desk {
    portal: false,
    dbus: false,
    flatpak: false,
};
const PORTAL: Desk = Desk {
    portal: true,
    dbus: false,
    flatpak: false,
};
const SCREENSAVER: Desk = Desk {
    portal: false,
    dbus: true,
    flatpak: false,
};
const BOTH: Desk = Desk {
    portal: true,
    dbus: true,
    flatpak: false,
};

fn strings(calls: &[&str]) -> Vec<String> {
    calls.iter().map(|s| s.to_string()).collect()
}

fn create<const G: bool>(host: &Arc<TestHost<G>>, reason: Option<&CStr>) -> Inhibit<TestHost<G>> {
    Inhibit::create(Arc::clone(host), reason).expect("an inhibitor")
}

// --- test/cmocka/test_inhibit_sleep.c ---------------------------------------

#[test]
fn null_info_is_rejected() {
    assert!(!nix_inhibit::set_active::<Gio>(None, true));
    assert!(!nix_inhibit::set_active::<Gio>(None, false));
    nix_inhibit::destroy::<Gio>(None);
}

#[test]
fn no_bus_falls_back_to_xdg_screensaver() {
    let host = Gio::new(NO_BUS);
    let mut info = create(&host, Some(REASON));
    assert_eq!(
        host.take(),
        strings(&["portal_create", "flatpak", "dbus_create"])
    );

    assert!(!info.set_active(false));
    assert!(info.set_active(true));
    host.await_parked(1);
    assert!(!info.set_active(true));
    assert!(info.set_active(false));
    assert!(!info.set_active(false));
    info.destroy();

    // The thread waits the full interval before its first reset.
    assert_eq!(host.take(), vec![format!("wait {RESET_INTERVAL_MS}")]);
}

#[test]
fn repeated_activate_deactivate() {
    let host = Gio::new(NO_BUS);
    let mut info = create(&host, Some(REASON));
    host.take();
    for _ in 0..50 {
        assert!(info.set_active(true));
        assert!(!info.set_active(true));
        assert!(info.set_active(false));
        assert!(!info.set_active(false));
    }
    info.destroy();
    assert!(host.take().iter().all(|c| c.starts_with("wait ")));
}

#[test]
fn destroy_while_active_joins() {
    let host = Gio::new(NO_BUS);
    let mut info = create(&host, Some(REASON));
    assert!(info.set_active(true));
    host.await_parked(1);
    info.destroy();
    host.await_parked(0);
    assert!(!host.take().iter().any(|c| c.starts_with("spawn")));
}

#[test]
fn destroy_inactive() {
    let host = Gio::new(NO_BUS);
    for _ in 0..10 {
        create(&host, Some(REASON)).destroy();
    }
    assert!(!host.take().iter().any(|c| c.starts_with("wait")));
}

#[test]
fn null_and_odd_reasons() {
    let long = CString::new(vec![b'r'; 4096]).unwrap();
    let reasons: [Option<&CStr>; 5] = [
        None,
        Some(c""),
        Some(c"Aufnahme l\xC3\xA4uft \xE2\x80\x93 \xE2\x9C\x93"),
        Some(c"line\nbreak\ttab"),
        Some(&long),
    ];
    let host = Gio::new(NO_BUS);
    for reason in reasons {
        let mut info = create(&host, reason);
        assert_eq!(info.reason(), reason);
        assert!(info.set_active(true));
        assert!(info.set_active(false));
        info.destroy();
    }
}

#[test]
fn reason_is_copied() {
    let host = Gio::new(NO_BUS);
    let reason = CString::new("OBS Video/audio").unwrap();
    let mut info = create(&host, Some(&reason));
    drop(reason);
    assert_eq!(info.reason(), Some(REASON));
    assert!(info.set_active(true));
    info.destroy();
}

#[test]
fn two_inhibitors_are_independent() {
    let host = Gio::new(NO_BUS);
    let mut a = create(&host, Some(c"a"));
    let mut b = create(&host, Some(c"b"));
    assert!(a.set_active(true));
    assert!(b.set_active(true));
    host.await_parked(2);
    assert!(a.set_active(false));
    assert!(!b.set_active(true));
    assert!(!a.set_active(false));
    a.destroy();
    assert!(b.is_active());
    assert!(b.set_active(false));
    b.destroy();
}

// --- test/cmocka/test_inhibit_sleep_dbus.c -----------------------------------

#[test]
fn test_portal_inhibit_and_release() {
    let host = Gio::new(PORTAL);
    let mut info = create(&host, Some(REASON));
    assert!(info.set_active(true));
    assert!(info.set_active(false));
    info.destroy();
    assert_eq!(
        host.take(),
        strings(&[
            "portal_create",
            "portal_inhibit 1 \"OBS Video/audio\" true",
            "portal_inhibit 1 \"OBS Video/audio\" false",
            "portal_destroy 1",
        ])
    );
}

#[test]
fn test_portal_destroy_while_inhibit_pending() {
    let host = Gio::new(PORTAL);
    let mut info = create(&host, Some(REASON));
    assert!(info.set_active(true));
    info.destroy();
    assert_eq!(
        host.take(),
        strings(&[
            "portal_create",
            "portal_inhibit 1 \"OBS Video/audio\" true",
            "portal_inhibit 1 \"OBS Video/audio\" false",
            "portal_destroy 1",
        ])
    );
}

#[test]
fn test_screensaver_inhibit_and_release() {
    let host = Gio::new(SCREENSAVER);
    let mut info = create(&host, Some(REASON));
    assert!(info.set_active(true));
    assert!(info.set_active(false));
    info.destroy();
    assert_eq!(
        host.take(),
        strings(&[
            "portal_create",
            "flatpak",
            "dbus_create",
            "dbus_inhibit 1 \"OBS Video/audio\" true",
            "dbus_inhibit 1 \"OBS Video/audio\" false",
            "dbus_destroy 1",
        ])
    );
}

#[test]
fn test_portal_preferred_over_screensaver() {
    let host = Gio::new(BOTH);
    let mut info = create(&host, Some(REASON));
    assert!(info.set_active(true));
    assert!(info.set_active(false));
    info.destroy();
    let calls = host.take();
    assert_eq!(calls.first().map(String::as_str), Some("portal_create"));
    assert!(
        !calls
            .iter()
            .any(|c| c.starts_with("dbus") || c == "flatpak")
    );
}

#[test]
fn test_portal_repeated_set_active() {
    let host = Gio::new(PORTAL);
    let mut info = create(&host, Some(REASON));
    assert!(!info.set_active(false));
    for _ in 0..2 {
        assert!(info.set_active(true));
        assert!(!info.set_active(true));
        assert!(info.set_active(false));
        assert!(!info.set_active(false));
    }
    info.destroy();
    let calls = host.take();
    let inhibits = |active: bool| {
        calls
            .iter()
            .filter(|c| c.starts_with("portal_inhibit") && c.ends_with(&format!(" {active}")))
            .count()
    };
    assert_eq!((inhibits(true), inhibits(false)), (2, 2));
}

#[test]
fn test_portal_destroy_while_active() {
    let host = Gio::new(PORTAL);
    let mut info = create(&host, Some(REASON));
    assert!(info.set_active(true));
    host.take();
    info.destroy();
    assert_eq!(
        host.take(),
        strings(&[
            "portal_inhibit 1 \"OBS Video/audio\" false",
            "portal_destroy 1"
        ])
    );
}

#[test]
fn test_screensaver_repeated_set_active() {
    let host = Gio::new(SCREENSAVER);
    let mut info = create(&host, Some(REASON));
    assert!(!info.set_active(false));
    for _ in 0..5 {
        assert!(info.set_active(true));
        assert!(!info.set_active(true));
        assert!(info.set_active(false));
        assert!(!info.set_active(false));
    }
    info.destroy();
    let calls = host.take();
    assert_eq!(calls.iter().filter(|c| c.ends_with(" true")).count(), 5);
    assert_eq!(calls.iter().filter(|c| c.ends_with(" false")).count(), 5);
    assert!(!calls.iter().any(|c| c.starts_with("portal_inhibit")));
}

#[test]
fn test_screensaver_destroy_while_active() {
    let host = Gio::new(SCREENSAVER);
    let mut info = create(&host, Some(REASON));
    assert!(info.set_active(true));
    host.take();
    info.destroy();
    assert_eq!(
        host.take(),
        strings(&["dbus_inhibit 1 \"OBS Video/audio\" false", "dbus_destroy 1"])
    );
}

#[test]
fn test_screensaver_reason_is_copied() {
    let long = CString::new(vec![b'r'; 1024]).unwrap();
    let reasons: [&CStr; 4] = [
        REASON,
        c"",
        c"Aufnahme l\xC3\xA4uft \xE2\x80\x93 \xE2\x9C\x93",
        &long,
    ];
    for reason in reasons {
        let host = Gio::new(SCREENSAVER);
        let copy = reason.to_owned();
        let mut info = create(&host, Some(&copy));
        drop(copy);
        host.take();
        assert!(info.set_active(true));
        assert_eq!(
            host.take(),
            vec![format!("dbus_inhibit 1 {} true", reason_text(Some(reason)))]
        );
        info.destroy();
    }
}

#[test]
fn test_empty_bus_falls_back() {
    let host = Gio::new(NO_BUS);
    let mut info = create(&host, Some(REASON));
    assert!(info.set_active(true));
    assert!(!info.set_active(true));
    assert!(info.set_active(false));
    info.destroy();
    assert!(
        !host
            .take()
            .iter()
            .any(|c| c.contains("_inhibit") || c.contains("_destroy"))
    );
}

// --- Rust-only cases ----------------------------------------------------------

/// Without GIO the C has no portal or D-Bus branch: the desktop is never
/// asked, even when it has both.
#[test]
fn without_gio_only_the_fallback_is_used() {
    let host = NoGio::new(BOTH);
    let mut info = create(&host, Some(REASON));
    assert!(info.set_active(true));
    host.await_parked(1);
    assert!(info.set_active(false));
    info.destroy();
    assert_eq!(host.take(), vec![format!("wait {RESET_INTERVAL_MS}")]);
}

/// In a Flatpak with no portal nothing else may inhibit: create returns
/// `None` and never looks for the session D-Bus inhibitors.
#[test]
fn flatpak_without_portal_gives_no_inhibitor() {
    let host = Gio::new(Desk {
        portal: false,
        dbus: true,
        flatpak: true,
    });
    assert!(Inhibit::create(Arc::clone(&host), Some(REASON)).is_none());
    assert_eq!(host.take(), strings(&["portal_create", "flatpak"]));

    // With a portal the Flatpak check is not made.
    let host = Gio::new(Desk {
        portal: true,
        dbus: true,
        flatpak: true,
    });
    create(&host, Some(REASON)).destroy();
    assert_eq!(host.take(), strings(&["portal_create", "portal_destroy 1"]));
}

/// Each timed-out wait runs one `xdg-screensaver reset`, then the thread
/// waits again.
#[test]
fn reset_runs_after_each_timeout() {
    let host = Gio::new(NO_BUS);
    let mut info = create(&host, Some(REASON));
    host.take();
    host.set_timeouts(3);
    assert!(info.set_active(true));
    host.await_parked(1);
    assert!(info.set_active(false));
    info.destroy();

    let wait = format!("wait {RESET_INTERVAL_MS}");
    let timeout = format!("{wait} timeout");
    let spawn = format!("spawn {SCREENSAVER_PROGRAM} {}", SCREENSAVER_ARGS.join(" "));
    let mut expected = Vec::new();
    for _ in 0..3 {
        expected.push(timeout.clone());
        expected.push(spawn.clone());
    }
    expected.push(wait);
    assert_eq!(host.take(), expected);
    assert_eq!(spawn, "spawn xdg-screensaver reset");
}

/// A reset that cannot start is logged with its errno, and the thread
/// keeps going.
#[test]
fn failed_reset_is_logged() {
    let host = Gio::with(NO_BUS, Err(2), false);
    let mut info = create(&host, Some(REASON));
    host.take();
    host.set_timeouts(2);
    assert!(info.set_active(true));
    host.await_parked(1);
    info.destroy();
    let logs: Vec<_> = host
        .take()
        .into_iter()
        .filter(|c| c.starts_with("log"))
        .collect();
    assert_eq!(
        logs,
        strings(&[
            "log 200 Failed to create xdg-screensaver: 2",
            "log 200 Failed to create xdg-screensaver: 2",
        ])
    );
}

/// Intentional difference: a thread that cannot start is an error and the
/// inhibitor stays inactive (C reports success, see the module docs).
#[test]
fn failed_thread_start_stays_inactive() {
    let host = Gio::with(NO_BUS, Ok(()), true);
    let mut info = create(&host, Some(REASON));
    host.take();
    assert!(!info.set_active(true));
    assert!(!info.is_active());
    assert!(!info.set_active(false));
    info.destroy();
    assert_eq!(
        host.take(),
        strings(&["log 100 Failed to create screensaver inhibitor thread"])
    );
}

/// The real spawn passes the arguments as given and returns only after the
/// program exits. `sh -c` stands in for xdg-screensaver: exec'ing a script
/// written by this test could fail with ETXTBSY while a parallel test forks.
#[test]
fn run_and_wait_runs_the_program_to_the_end() {
    let dir = ScratchDir::new("obs-nix-inhibit");
    let marker = dir.join("ran");
    let script = format!("echo \"$0 $*\" > '{}'", marker.display());
    let mut args = vec!["-c", script.as_str(), SCREENSAVER_PROGRAM];
    args.extend(SCREENSAVER_ARGS);

    assert_eq!(nix_inhibit::run_and_wait("sh".as_ref(), &args), Ok(()));
    let ran = std::fs::read_to_string(&marker).unwrap();
    assert_eq!(ran, "xdg-screensaver reset\n");

    // The exit status is not looked at.
    assert_eq!(
        nix_inhibit::run_and_wait("sh".as_ref(), &["-c", "exit 3"]),
        Ok(())
    );
}

/// A program that is not there is `ENOENT`, which C logs as its
/// `posix_spawnp` result.
#[test]
fn run_and_wait_reports_a_missing_program() {
    assert_eq!(
        nix_inhibit::run_and_wait("obs-inhibit-test-no-such-program".as_ref(), &[]),
        Err(2)
    );
}
