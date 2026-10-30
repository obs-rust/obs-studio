//! Tier 3: the Rust port of `libobs/util/platform-nix-inhibit.c` (#1500)
//! against the original C, built with GIO (`oracle_gio_*`) and without
//! (`oracle_*`).
//!
//! Both sides run the same random sequences of create, set_active (also on
//! NULL) and destroy, for every mix of portal, session D-Bus inhibitor and
//! Flatpak. What they call outside themselves goes to one set of counted C
//! stand-ins (`obs-c-oracle/oracle/nix_inhibit_stubs_posix.c`): the C
//! through `oracle/nix_inhibit_hooks.h`, the Rust core through [`Stub`].
//! Each step compares the return values and the calls made, in order: which
//! inhibitor was created, every inhibit with its reason, every destroy, the
//! Flatpak check, each reset of the xdg-screensaver thread and its log line.
//! The reset thread's wait times out on request, so its resets are compared
//! without waiting 30 s; the driver waits for the thread to park before the
//! next step, so the log order is fixed.
//!
//! The C ABI shim is compared too (`os_inhibit_sleep_*` against the oracle
//! of the same GIO setting as this build's `gio` feature). Its host is the
//! real one, so those runs leave out the Flatpak check (it reads the real
//! file), the wait (a real 30 s wait) and the reset.
//!
//! Intentional differences, not generated here:
//! - A thread that cannot start: C reports success (`pthread_create < 0`
//!   never holds) and later joins nothing. Tier 1 covers the Rust side.
//! - A reset that starts: the C then waits for any child forever on
//!   `ECHILD`. The stand-in spawn always fails with an errno.
//! - The inhibitor is a `Box` and the reason a `CString`; C uses `bzalloc`
//!   and `bstrdup`, so `bnum_allocs` moves differently while one is alive.

#![cfg(any(target_os = "linux", target_os = "freebsd"))]

use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::Mutex;

use obs_c_oracle::nix_inhibit as c;
use obs_util::ffi::platform_nix_inhibit as shim;
use obs_util::platform::Event;
use obs_util::platform::unix::nix_inhibit::{self, FLATPAK_INFO, Host, Inhibit};
use proptest::prelude::*;

/// The stand-ins are process-wide.
static STUBS: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    STUBS.lock().unwrap_or_else(|e| e.into_inner())
}

/// The Rust core's host, through the oracle's stand-ins.
struct Stub<const GIO: bool>;

fn nonnull(p: *mut c_void) -> Option<*mut c_void> {
    (!p.is_null()).then_some(p)
}

fn reason_ptr(reason: Option<&CStr>) -> *const c_char {
    reason.map_or(std::ptr::null(), CStr::as_ptr)
}

impl<const GIO: bool> Host for Stub<GIO> {
    const GIO: bool = GIO;
    type Portal = *mut c_void;
    type Dbus = *mut c_void;

    fn portal_create(&self) -> Option<*mut c_void> {
        // SAFETY: stand-in with no arguments.
        nonnull(unsafe { c::portal_inhibit_info_create() })
    }
    fn dbus_create(&self) -> Option<*mut c_void> {
        // SAFETY: stand-in with no arguments.
        nonnull(unsafe { c::dbus_sleep_info_create() })
    }
    fn portal_inhibit(&self, portal: &mut *mut c_void, reason: Option<&CStr>, active: bool) {
        // SAFETY: the stand-in only formats the handle and the C string.
        unsafe { c::portal_inhibit(*portal, reason_ptr(reason), active) }
    }
    fn dbus_inhibit(&self, dbus: &mut *mut c_void, reason: Option<&CStr>, active: bool) {
        // SAFETY: as `portal_inhibit`.
        unsafe { c::dbus_inhibit_sleep(*dbus, reason_ptr(reason), active) }
    }
    fn portal_destroy(&self, portal: *mut c_void) {
        // SAFETY: the stand-in only formats the handle.
        unsafe { c::portal_inhibit_info_destroy(portal) }
    }
    fn dbus_destroy(&self, dbus: *mut c_void) {
        // SAFETY: as `portal_destroy`.
        unsafe { c::dbus_sleep_info_destroy(dbus) }
    }
    fn log(&self, level: i32, message: &str) {
        let message = CString::new(message).unwrap();
        // SAFETY: a C string that outlives the call.
        unsafe { c::oracle_inhibit_stub_log_line(level, message.as_ptr()) }
    }
    fn in_flatpak(&self) -> bool {
        let path = CString::new(FLATPAK_INFO).unwrap();
        // SAFETY: a C string; F_OK is 0.
        unsafe { c::oracle_stub_inhibit_access(path.as_ptr(), 0) == 0 }
    }
    fn spawn(&self, program: &str, args: &[&str]) -> Result<(), i32> {
        let program = CString::new(program).unwrap();
        let arg = args.first().map(|a| CString::new(*a).unwrap());
        let arg = arg.as_deref();
        // SAFETY: C strings (or NULL) that outlive the call.
        let err = unsafe {
            c::oracle_inhibit_stub_spawn(program.as_ptr(), program.as_ptr(), reason_ptr(arg))
        };
        if err == 0 { Ok(()) } else { Err(err) }
    }
    fn wait_for_stop(&self, stop: &Event, milliseconds: u64) -> bool {
        // SAFETY: plain counters behind the stand-ins' lock.
        if unsafe { c::oracle_inhibit_stub_wait_enter(milliseconds) } != 0 {
            return false;
        }
        let stopped = stop.timed_wait(milliseconds);
        // SAFETY: as above.
        unsafe { c::oracle_inhibit_stub_wait_leave() };
        stopped
    }
}

/// What the desktop has, and the errno a reset's spawn fails with.
#[derive(Debug, Clone, Copy)]
struct Desk {
    portal: bool,
    dbus: bool,
    flatpak: bool,
    spawn_error: i32,
}

#[derive(Debug, Clone)]
enum Op {
    Create(Option<Vec<u8>>),
    Set {
        slot: usize,
        active: bool,
        timeouts: u8,
    },
    SetNull(bool),
    Destroy(usize),
}

const SLOTS: usize = 3;

/// One implementation of the three functions.
trait Api {
    type Obj;
    /// Whether this run waits for reset threads to park (stand-in waits).
    const PARKS: bool;
    fn create(&mut self, reason: Option<&CStr>) -> Option<Self::Obj>;
    fn set(&mut self, obj: Option<&mut Self::Obj>, active: bool) -> bool;
    fn destroy(&mut self, obj: Self::Obj);
}

struct Oracle {
    gio: bool,
}

impl Api for Oracle {
    type Obj = *mut c_void;
    const PARKS: bool = true;
    fn create(&mut self, reason: Option<&CStr>) -> Option<*mut c_void> {
        // SAFETY: NULL or a C string; the oracle copies it.
        nonnull(unsafe {
            if self.gio {
                c::oracle_gio_os_inhibit_sleep_create(reason_ptr(reason))
            } else {
                c::oracle_os_inhibit_sleep_create(reason_ptr(reason))
            }
        })
    }
    fn set(&mut self, obj: Option<&mut *mut c_void>, active: bool) -> bool {
        let p = obj.map_or(std::ptr::null_mut(), |o| *o);
        // SAFETY: NULL or a live oracle inhibitor.
        unsafe {
            if self.gio {
                c::oracle_gio_os_inhibit_sleep_set_active(p, active)
            } else {
                c::oracle_os_inhibit_sleep_set_active(p, active)
            }
        }
    }
    fn destroy(&mut self, obj: *mut c_void) {
        // SAFETY: a live oracle inhibitor, destroyed once.
        unsafe {
            if self.gio {
                c::oracle_gio_os_inhibit_sleep_destroy(obj)
            } else {
                c::oracle_os_inhibit_sleep_destroy(obj)
            }
        }
    }
}

/// The C oracle with this build's GIO setting, for the shim runs, which
/// cannot park (the shim's waits are real).
struct OracleNoPark(Oracle);

impl Api for OracleNoPark {
    type Obj = *mut c_void;
    const PARKS: bool = false;
    fn create(&mut self, reason: Option<&CStr>) -> Option<*mut c_void> {
        self.0.create(reason)
    }
    fn set(&mut self, obj: Option<&mut *mut c_void>, active: bool) -> bool {
        self.0.set(obj, active)
    }
    fn destroy(&mut self, obj: *mut c_void) {
        self.0.destroy(obj)
    }
}

struct Core<const GIO: bool>;

impl<const GIO: bool> Api for Core<GIO> {
    type Obj = Inhibit<Stub<GIO>>;
    const PARKS: bool = true;
    fn create(&mut self, reason: Option<&CStr>) -> Option<Self::Obj> {
        Inhibit::create(std::sync::Arc::new(Stub::<GIO>), reason)
    }
    fn set(&mut self, obj: Option<&mut Self::Obj>, active: bool) -> bool {
        nix_inhibit::set_active(obj, active)
    }
    fn destroy(&mut self, obj: Self::Obj) {
        nix_inhibit::destroy(Some(obj))
    }
}

struct Shim;

impl Api for Shim {
    type Obj = *mut c_void;
    const PARKS: bool = false;
    fn create(&mut self, reason: Option<&CStr>) -> Option<*mut c_void> {
        // SAFETY: NULL or a C string.
        nonnull(unsafe { shim::os_inhibit_sleep_create(reason_ptr(reason)) })
    }
    fn set(&mut self, obj: Option<&mut *mut c_void>, active: bool) -> bool {
        let p = obj.map_or(std::ptr::null_mut(), |o| *o);
        // SAFETY: NULL or a live shim inhibitor.
        unsafe { shim::os_inhibit_sleep_set_active(p, active) }
    }
    fn destroy(&mut self, obj: *mut c_void) {
        // SAFETY: a live shim inhibitor, destroyed once.
        unsafe { shim::os_inhibit_sleep_destroy(obj) }
    }
}

fn take_log() -> Vec<String> {
    let mut buf = vec![0u8; 1 << 16];
    // SAFETY: `buf` is writable for its length.
    let len = unsafe { c::oracle_inhibit_stub_take_log(buf.as_mut_ptr().cast(), buf.len()) };
    assert!(len < buf.len(), "stand-in log overflow");
    String::from_utf8_lossy(&buf[..len])
        .lines()
        .map(String::from)
        .collect()
}

/// Whether an inhibitor with this desk uses the reset thread (the test's
/// own model, only to know how many threads to wait for).
fn uses_thread(gio: bool, desk: Desk) -> bool {
    !(gio && (desk.portal || desk.dbus))
}

/// Runs `ops` and returns, per step, the result and the calls it made.
fn run<A: Api>(api: &mut A, gio: bool, desk: Desk, ops: &[Op]) -> Vec<String> {
    // SAFETY: plain setters on the stand-ins.
    unsafe {
        c::oracle_inhibit_stub_reset(
            desk.portal.into(),
            desk.dbus.into(),
            desk.flatpak.into(),
            desk.spawn_error,
        )
    };
    let threaded = uses_thread(gio, desk);
    let mut slots: Vec<Option<(A::Obj, bool)>> = (0..SLOTS).map(|_| None).collect();
    let mut parked = 0;
    let mut out = Vec::new();

    for op in ops {
        match op {
            Op::Create(reason) => {
                let reason = reason.as_ref().map(|r| CString::new(r.clone()).unwrap());
                match slots.iter().position(Option::is_none) {
                    Some(free) => {
                        let obj = api.create(reason.as_deref());
                        out.push(format!("create {}", obj.is_some()));
                        slots[free] = obj.map(|o| (o, false));
                    }
                    None => out.push("create full".into()),
                }
            }
            Op::Set {
                slot,
                active,
                timeouts,
            } => {
                let Some((obj, is_active)) = slots[*slot].as_mut() else {
                    out.push("set empty".into());
                    continue;
                };
                if A::PARKS {
                    // SAFETY: plain setter.
                    unsafe { c::oracle_inhibit_stub_set_timeouts((*timeouts).into()) };
                }
                let changed = api.set(Some(obj), *active);
                out.push(format!("set {active} -> {changed}"));
                if changed {
                    *is_active = *active;
                    if threaded {
                        parked = if *active { parked + 1 } else { parked - 1 };
                    }
                    // SAFETY: waits on the stand-ins' condition variable.
                    if A::PARKS && unsafe { c::oracle_inhibit_stub_await_parked(parked) } == 0 {
                        out.push(format!("never parked {parked}"));
                    }
                }
                if A::PARKS {
                    // SAFETY: plain setter.
                    unsafe { c::oracle_inhibit_stub_set_timeouts(0) };
                }
            }
            Op::SetNull(active) => {
                out.push(format!("set NULL -> {}", api.set(None, *active)));
            }
            Op::Destroy(slot) => match slots[*slot].take() {
                Some((obj, was_active)) => {
                    api.destroy(obj);
                    if threaded && was_active {
                        parked -= 1;
                    }
                    out.push("destroy".into());
                }
                None => out.push("destroy empty".into()),
            },
        }
        out.extend(take_log().into_iter().map(|l| format!("  {l}")));
    }

    for slot in slots.iter_mut() {
        if let Some((obj, _)) = slot.take() {
            api.destroy(obj);
            out.push("destroy at end".into());
        }
    }
    out.extend(take_log().into_iter().map(|l| format!("  {l}")));
    out
}

/// The shim's host is real: drop what only the stand-ins log.
fn without_host_lines(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .filter(|l| {
            let l = l.trim_start();
            !(l.starts_with("access ") || l.starts_with("wait ") || l.starts_with("spawn "))
        })
        .collect()
}

fn compare_core(gio: bool, desk: Desk, ops: &[Op]) -> Result<(), TestCaseError> {
    let _guard = lock();
    let expected = run(&mut Oracle { gio }, gio, desk, ops);
    let actual = if gio {
        run(&mut Core::<true>, gio, desk, ops)
    } else {
        run(&mut Core::<false>, gio, desk, ops)
    };
    prop_assert_eq!(actual, expected, "gio {} {:?}", gio, desk);
    Ok(())
}

fn compare_shim(desk: Desk, ops: &[Op]) -> Result<(), TestCaseError> {
    let _guard = lock();
    let gio = cfg!(feature = "gio");
    let desk = Desk {
        flatpak: false,
        ..desk
    };
    let ops: Vec<Op> = ops
        .iter()
        .map(|op| match op {
            Op::Set { slot, active, .. } => Op::Set {
                slot: *slot,
                active: *active,
                timeouts: 0,
            },
            op => op.clone(),
        })
        .collect();
    let expected = without_host_lines(run(&mut OracleNoPark(Oracle { gio }), gio, desk, &ops));
    let actual = without_host_lines(run(&mut Shim, gio, desk, &ops));
    prop_assert_eq!(actual, expected, "gio {} {:?}", gio, desk);
    Ok(())
}

fn desk() -> impl Strategy<Value = Desk> {
    (
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        prop_oneof![Just(2), Just(13)],
    )
        .prop_map(|(portal, dbus, flatpak, spawn_error)| Desk {
            portal,
            dbus,
            flatpak,
            spawn_error,
        })
}

fn reason() -> impl Strategy<Value = Option<Vec<u8>>> {
    prop_oneof![
        1 => Just(None),
        4 => proptest::collection::vec(1u8..=255, 0..48).prop_map(Some),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => reason().prop_map(Op::Create),
        6 => (0..SLOTS, any::<bool>(), 0u8..3).prop_map(|(slot, active, timeouts)| Op::Set {
            slot,
            active,
            timeouts,
        }),
        1 => any::<bool>().prop_map(Op::SetNull),
        2 => (0..SLOTS).prop_map(Op::Destroy),
    ]
}

/// The step lists of test_inhibit_sleep.c and test_inhibit_sleep_dbus.c,
/// for each desk.
#[test]
fn cmocka_sequences_match_oracle() {
    let create = Op::Create(Some(b"OBS Video/audio".to_vec()));
    let set = |active, timeouts| Op::Set {
        slot: 0,
        active,
        timeouts,
    };
    let sequences = [
        // set_active returns true only on a change.
        vec![
            create.clone(),
            set(false, 0),
            set(true, 2),
            set(true, 0),
            set(false, 0),
            set(false, 0),
            Op::Destroy(0),
        ],
        // Destroy while active.
        vec![create.clone(), set(true, 1), Op::Destroy(0)],
        // Odd reasons and NULL.
        vec![
            Op::Create(None),
            Op::Create(Some(Vec::new())),
            Op::Create(Some("Aufnahme läuft – ✓".into())),
            Op::Set {
                slot: 1,
                active: true,
                timeouts: 0,
            },
            Op::Set {
                slot: 2,
                active: true,
                timeouts: 0,
            },
            set(true, 0),
            Op::SetNull(true),
            Op::SetNull(false),
        ],
    ];
    for gio in [false, true] {
        for portal in [false, true] {
            for dbus in [false, true] {
                for flatpak in [false, true] {
                    let desk = Desk {
                        portal,
                        dbus,
                        flatpak,
                        spawn_error: 2,
                    };
                    for ops in &sequences {
                        compare_core(gio, desk, ops).unwrap();
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(96)))]

    #[test]
    fn random_sequences_match_oracle(
        gio in any::<bool>(),
        desk in desk(),
        ops in proptest::collection::vec(op(), 1..16),
    ) {
        compare_core(gio, desk, &ops)?;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(32)))]

    #[test]
    fn shim_sequences_match_oracle(
        desk in desk(),
        ops in proptest::collection::vec(op(), 1..12),
    ) {
        compare_shim(desk, &ops)?;
    }
}
