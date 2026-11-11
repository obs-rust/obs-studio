//! Exact pair/order parity against original C. NULL callbacks are stored but
//! never dispatched; invalid core pointers and shutdown races are C UB.
use obs_core::ffi::tick_callbacks::{Entry, Tick, obs_add_tick_callback, obs_remove_tick_callback};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;

unsafe extern "C" {
    fn oracle_tick_callbacks_new() -> *mut c_void;
    fn oracle_tick_callbacks_free(core: *mut c_void);
    fn oracle_tick_callbacks_select(core: *mut c_void);
    fn oracle_core_initialized_set(core: *mut c_void);
    fn oracle_tick_callbacks_num(core: *mut c_void) -> usize;
    fn oracle_tick_callbacks_get(core: *mut c_void, index: usize) -> Entry;
    fn oracle_tick_callbacks_layout(index: usize) -> usize;
    fn oracle_tick_callbacks_dispatch(core: *mut c_void, seconds: f32);
    fn oracle_obs_add_tick_callback(tick: Tick, param: *mut c_void);
    fn oracle_obs_remove_tick_callback(tick: Tick, param: *mut c_void);
}
thread_local! {
    static TRACE: RefCell<Vec<(u8, usize, u32)>> = const { RefCell::new(Vec::new()) };
    static RUST: Cell<bool> = const { Cell::new(false) };
}
unsafe extern "C" fn first(param: *mut c_void, seconds: f32) {
    TRACE.with_borrow_mut(|trace| trace.push((1, param as usize, seconds.to_bits())));
}
unsafe extern "C" fn second(param: *mut c_void, seconds: f32) {
    TRACE.with_borrow_mut(|trace| trace.push((2, param as usize, seconds.to_bits())));
}
unsafe extern "C" fn remove_self(param: *mut c_void, seconds: f32) {
    TRACE.with_borrow_mut(|trace| trace.push((3, param as usize, seconds.to_bits())));
    // SAFETY: dispatch holds the live core's recursive draw mutex. Reentering
    // removal on that same thread must use that exact mutex and erase one row.
    unsafe {
        if RUST.get() {
            obs_remove_tick_callback(Some(remove_self), param);
        } else {
            oracle_obs_remove_tick_callback(Some(remove_self), param);
        }
    }
}
unsafe extern "C" fn add_during_tick(param: *mut c_void, seconds: f32) {
    TRACE.with_borrow_mut(|trace| trace.push((4, param as usize, seconds.to_bits())));
    let inserted = std::ptr::without_provenance_mut(7);
    // SAFETY: dispatch holds the live recursive draw mutex on this thread;
    // the added callback is valid and treats its parameter as opaque identity.
    unsafe {
        if RUST.get() {
            obs_add_tick_callback(Some(second), inserted);
        } else {
            oracle_obs_add_tick_callback(Some(second), inserted);
        }
    }
}
struct Cores {
    c: *mut c_void,
    rust: *mut c_void,
}
impl Cores {
    fn new() -> Self {
        // SAFETY: oracle allocates initialized, independently owned cores.
        unsafe {
            let this = Self {
                c: oracle_tick_callbacks_new(),
                rust: oracle_tick_callbacks_new(),
            };
            assert!(!this.c.is_null() && !this.rust.is_null());
            oracle_tick_callbacks_select(this.c);
            oracle_core_initialized_set(this.rust);
            this
        }
    }
    fn operation(&self, add: bool, entry: Entry) {
        // SAFETY: both selected cores are live; callback pointers are valid and
        // parameters are opaque identities, never dereferenced by these callbacks.
        unsafe {
            if add {
                oracle_obs_add_tick_callback(entry.tick, entry.param);
                obs_add_tick_callback(entry.tick, entry.param);
            } else {
                oracle_obs_remove_tick_callback(entry.tick, entry.param);
                obs_remove_tick_callback(entry.tick, entry.param);
            }
        }
        self.compare();
    }
    fn compare(&self) {
        // SAFETY: single-threaded isolated process owns both core arrays.
        unsafe {
            let count = oracle_tick_callbacks_num(self.c);
            assert_eq!(count, oracle_tick_callbacks_num(self.rust));
            for i in 0..count {
                let a = oracle_tick_callbacks_get(self.c, i);
                let b = oracle_tick_callbacks_get(self.rust, i);
                assert_eq!(a.tick.map(|f| f as usize), b.tick.map(|f| f as usize));
                assert_eq!(a.param, b.param);
            }
        }
    }
    fn dispatch(&self, seconds: f32) {
        TRACE.with_borrow_mut(Vec::clear);
        RUST.set(false);
        // SAFETY: all dispatched callbacks in this case are non-NULL and valid.
        unsafe { oracle_tick_callbacks_dispatch(self.c, seconds) };
        let expected = TRACE.with_borrow(Clone::clone);
        TRACE.with_borrow_mut(Vec::clear);
        RUST.set(true);
        // SAFETY: same callbacks/parameters on the independently owned Rust core.
        unsafe { oracle_tick_callbacks_dispatch(self.rust, seconds) };
        assert_eq!(TRACE.with_borrow(Clone::clone), expected);
        self.compare();
    }
}
impl Drop for Cores {
    fn drop(&mut self) {
        // SAFETY: clear globals before destroying the cores we exclusively own.
        unsafe {
            oracle_tick_callbacks_select(std::ptr::null_mut());
            oracle_core_initialized_set(std::ptr::null_mut());
            oracle_tick_callbacks_free(self.c);
            oracle_tick_callbacks_free(self.rust);
        }
    }
}
fn entry(function: u8, param: u8) -> Entry {
    let tick: Tick = match function {
        0 => None,
        1 => Some(first),
        _ => Some(second),
    };
    Entry {
        tick,
        param: std::ptr::without_provenance_mut(usize::from(param)),
    }
}

#[test]
fn original_c_registration_and_recursive_dispatch() {
    // Existing initialized-query tests also mutate the global pointer slot.
    // Re-exec this exact case so its globals cannot overlap any other module.
    const CHILD: &str = "OBS_TICK_CALLBACKS_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let name = format!(
            "{}::original_c_registration_and_recursive_dispatch",
            module_path!()
        );
        let name = name.split_once("::").unwrap().1;
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture"])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let layout = [
        size_of::<Entry>(),
        align_of::<Entry>(),
        std::mem::offset_of!(Entry, tick),
        std::mem::offset_of!(Entry, param),
    ];
    for (index, expected) in layout.into_iter().enumerate() {
        // SAFETY: oracle returns a scalar for every index.
        assert_eq!(unsafe { oracle_tick_callbacks_layout(index) }, expected);
    }
    {
        let cores = Cores::new();
        for (add, function, param) in [
            (false, 1, 0),
            (true, 1, 0),
            (true, 2, 1),
            (true, 1, 0),
            (true, 1, 1),
            (false, 1, 0),
            (false, 2, 0),
        ] {
            cores.operation(add, entry(function, param));
        }
        cores.dispatch(0.125);
        let trace = TRACE.with_borrow(Clone::clone);
        assert_eq!(
            trace.iter().map(|&(f, p, _)| (f, p)).collect::<Vec<_>>(),
            [(1, 0), (2, 1), (1, 1)]
        );
    }
    {
        let cores = Cores::new();
        cores.operation(true, entry(1, 0));
        cores.operation(
            true,
            Entry {
                tick: Some(remove_self),
                param: std::ptr::null_mut(),
            },
        );
        cores.operation(true, entry(2, 1));
        cores.dispatch(0.25);
        cores.dispatch(0.5);
        assert_eq!(TRACE.with_borrow(|trace| trace.len()), 2);
    }
    {
        let cores = Cores::new();
        cores.operation(
            true,
            Entry {
                tick: Some(add_during_tick),
                param: std::ptr::null_mut(),
            },
        );
        cores.operation(true, entry(2, 1));
        cores.dispatch(0.75);
        // Characterized, not endorsed: insertion shifts the live array while
        // reverse traversal retains its index, skipping the old second row and
        // visiting the newly inserted row during this same dispatch.
        assert_eq!(
            TRACE.with_borrow(|trace| trace.iter().map(|&(f, p, _)| (f, p)).collect::<Vec<_>>()),
            [(4, 0), (2, 7)]
        );
    }
    let mut runner = TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(
            &prop::collection::vec((any::<bool>(), 0u8..3, 0u8..8), 0..96),
            |operations| {
                let cores = Cores::new();
                for (add, function, param) in operations {
                    cores.operation(add, entry(function, param));
                }
                Ok(())
            },
        )
        .unwrap();
}
