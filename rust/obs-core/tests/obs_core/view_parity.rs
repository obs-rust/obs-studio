//! Tier 3: the view shims (`obs_core::ffi::view`) against the original
//! `libobs/obs-view-api.c.h` (oracle/view.c, its functions renamed).
//!
//! Both sides call the same test doubles for the hidden view accessors and
//! the source calls (oracle/view.c), each of which traces its step. A
//! sequence of operations runs once through the C and once through the Rust,
//! each on its own view and its own copies of four sources, and compares,
//! after every operation: the trace (lock, references, activations,
//! renders, channel writes, mix steps, in order), the return value (a
//! source by name, a `video_t`, or the enumerated video infos), every
//! channel of the view and the mixes list.
//!
//! Fixed sequences cover each function's cases; a proptest (512 cases)
//! covers random sequences, including channels past `MAX_CHANNELS`, sources
//! removed or being destroyed, a failed mutex and a missing main mix.
//! Excluded: a NULL `enum_proc` with a matching mix (the C calls through
//! it, the Rust panics). The oracle is built with `NDEBUG`, as release
//! libobs is, so an out-of-range channel returns instead of asserting.

use std::ffi::{CStr, c_char, c_longlong, c_void};
use std::ptr;
use std::sync::Mutex;

use obs_c_oracle::view::{
    OracleViewSource, oracle_obs_view_add, oracle_obs_view_add2, oracle_obs_view_create,
    oracle_obs_view_destroy, oracle_obs_view_enum_video_info, oracle_obs_view_free,
    oracle_obs_view_get_source, oracle_obs_view_init, oracle_obs_view_remove,
    oracle_obs_view_render, oracle_obs_view_set_source, oracle_view_channel_at, oracle_view_mix_at,
    oracle_view_mix_num, oracle_view_reset, oracle_view_set_trace, oracle_view_type_of,
};
use obs_core::ffi::view as shim;
use obs_core::view::MAX_CHANNELS;
use proptest::prelude::*;

/// The doubles' state (trace hook, mixes) is process-wide.
pub(super) static LOCK: Mutex<()> = Mutex::new(());
static TRACE: Mutex<Vec<String>> = Mutex::new(Vec::new());

unsafe extern "C" fn record(event: *const c_char, name: *const c_char, value: c_longlong) {
    // SAFETY: the doubles pass static C strings or a test source's name.
    let event = unsafe { CStr::from_ptr(event) }.to_string_lossy();
    let name = if name.is_null() {
        "-".into()
    } else {
        // SAFETY: as above.
        unsafe { CStr::from_ptr(name) }
            .to_string_lossy()
            .into_owned()
    };
    TRACE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(format!("{event} {name} {value}"));
}

fn take_trace() -> Vec<String> {
    std::mem::take(&mut *TRACE.lock().unwrap_or_else(|e| e.into_inner()))
}

#[derive(Debug, Clone)]
enum Op {
    Set(u32, Option<usize>),
    Get(u32),
    Render,
    Removed(usize),
    Dying(usize),
    Add,
    Add2(bool),
    AddNull,
    Remove,
    Enum(usize),
    EnumNull,
    Recreate,
}

const NAMES: [&CStr; 4] = [c"a", c"b", c"c", c"d"];

type Fns = (
    unsafe extern "C" fn() -> *mut c_void,
    unsafe extern "C" fn(*mut c_void),
    unsafe extern "C" fn(*mut c_void, u32) -> *mut c_void,
    unsafe extern "C" fn(*mut c_void, u32, *mut c_void),
    unsafe extern "C" fn(*mut c_void),
    unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void,
    unsafe extern "C" fn(*mut c_void),
);

fn c_fns() -> Fns {
    (
        oracle_obs_view_create,
        oracle_obs_view_destroy,
        oracle_obs_view_get_source,
        oracle_obs_view_set_source,
        oracle_obs_view_render,
        oracle_obs_view_add,
        oracle_obs_view_add2,
        oracle_obs_view_remove,
    )
}

fn rust_fns() -> Fns {
    (
        shim::obs_view_create,
        shim::obs_view_destroy,
        shim::obs_view_get_source,
        shim::obs_view_set_source,
        shim::obs_view_render,
        shim::obs_view_add,
        shim::obs_view_add2,
        shim::obs_view_remove,
    )
}

unsafe extern "C" fn collect(param: *mut c_void, ovi: *mut c_void) -> bool {
    // SAFETY: param is the (Vec, stop) pair `run` passes.
    let (seen, stop) = unsafe { &mut *param.cast::<(Vec<usize>, usize)>() };
    seen.push(ovi.addr());
    seen.len() < *stop
}

/// Everything one side shows after an op.
#[derive(Debug, PartialEq)]
struct Step {
    trace: Vec<String>,
    result: String,
    channels: Vec<String>,
    mixes: Vec<bool>,
}

struct Run {
    /// Never pushed to after creation, so the sources do not move.
    sources: Vec<OracleViewSource>,
    view: *mut c_void,
    ovi: Box<[u8; 8]>,
    main: Box<[u8; 8]>,
    fns: Fns,
    enum_fn: unsafe extern "C" fn(
        *mut c_void,
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool>,
        *mut c_void,
    ),
}

impl Run {
    fn name_of(&self, p: *mut c_void) -> String {
        if p.is_null() {
            return "null".into();
        }
        let i = self
            .sources
            .iter()
            .position(|s| ptr::eq(ptr::from_ref::<OracleViewSource>(s).cast(), p))
            .expect("a test source");
        NAMES[i].to_string_lossy().into_owned()
    }

    fn src(&mut self, i: usize) -> *mut c_void {
        ptr::from_mut::<OracleViewSource>(&mut self.sources[i]).cast()
    }

    fn ovi_name(&self, a: usize) -> String {
        if a == ptr::from_ref(&*self.ovi).addr() {
            "ovi".into()
        } else if a == ptr::from_ref(&*self.main).addr() {
            "main".into()
        } else {
            format!("{a:#x}")
        }
    }

    fn apply(&mut self, op: &Op, mutex_fails: bool) -> String {
        let (create, destroy, get, set, render, add, add2, remove) = self.fns;
        // SAFETY (whole match): the functions on this run's own view and
        // sources, under LOCK.
        unsafe {
            match *op {
                Op::Set(ch, s) => {
                    let s = s.map_or(ptr::null_mut(), |i| self.src(i));
                    set(self.view, ch, s);
                    String::new()
                }
                Op::Get(ch) => self.name_of(get(self.view, ch)),
                Op::Render => {
                    render(self.view);
                    String::new()
                }
                Op::Removed(i) => {
                    self.sources[i].removed = true;
                    String::new()
                }
                Op::Dying(i) => {
                    self.sources[i].dying = true;
                    String::new()
                }
                Op::Add => format!("{:#x}", add(self.view).addr()),
                Op::Add2(with_ovi) => {
                    let ovi = if with_ovi {
                        ptr::from_mut(&mut *self.ovi).cast()
                    } else {
                        ptr::null_mut()
                    };
                    format!("{:#x}", add2(self.view, ovi).addr())
                }
                Op::AddNull => format!("{:#x}", add(ptr::null_mut()).addr()),
                Op::Remove => {
                    remove(self.view);
                    String::new()
                }
                Op::Enum(stop) => {
                    let mut st = (Vec::new(), stop.max(1));
                    (self.enum_fn)(self.view, Some(collect), (&raw mut st).cast());
                    let names: Vec<_> = st.0.iter().map(|&a| self.ovi_name(a)).collect();
                    names.join(",")
                }
                Op::EnumNull => {
                    let mut st = (Vec::new(), usize::MAX);
                    (self.enum_fn)(ptr::null_mut(), Some(collect), (&raw mut st).cast());
                    let names: Vec<_> = st.0.iter().map(|&a| self.ovi_name(a)).collect();
                    names.join(",")
                }
                Op::Recreate => {
                    destroy(self.view);
                    oracle_view_reset(mutex_fails, ptr::null_mut());
                    self.view = create();
                    format!("{}", self.view.is_null())
                }
            }
        }
    }

    fn snapshot(&self) -> (Vec<String>, Vec<bool>) {
        let channels = if self.view.is_null() {
            Vec::new()
        } else {
            (0..MAX_CHANNELS)
                // SAFETY: a live view from this run.
                .map(|i| self.name_of(unsafe { oracle_view_channel_at(self.view, i) }))
                .collect()
        };
        // SAFETY: the doubles' mixes list, under LOCK.
        let mixes = unsafe {
            (0..oracle_view_mix_num())
                .map(|i| ptr::eq(oracle_view_mix_at(i), self.view))
                .collect()
        };
        (channels, mixes)
    }
}

fn run_side(
    fns: Fns,
    enum_fn: unsafe extern "C" fn(
        *mut c_void,
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool>,
        *mut c_void,
    ),
    ops: &[Op],
    mutex_fails: bool,
    main_mix: bool,
) -> Vec<Step> {
    let mut run = Run {
        sources: NAMES
            .iter()
            .map(|n| OracleViewSource {
                name: n.as_ptr(),
                removed: false,
                dying: false,
            })
            .collect(),
        view: ptr::null_mut(),
        ovi: Box::new([0; 8]),
        main: Box::new([0; 8]),
        fns,
        enum_fn,
    };
    let main = if main_mix {
        ptr::from_mut(&mut *run.main).cast()
    } else {
        ptr::null_mut()
    };
    let mut steps = Vec::new();
    // SAFETY: under LOCK; the view is this run's.
    unsafe {
        oracle_view_reset(mutex_fails, main);
        take_trace();
        run.view = (run.fns.0)();
        let (channels, mixes) = run.snapshot();
        steps.push(Step {
            trace: take_trace(),
            result: format!("{}", run.view.is_null()),
            channels,
            mixes,
        });
        oracle_view_reset(mutex_fails, main);
        for op in ops {
            if run.view.is_null() && !matches!(op, Op::Recreate | Op::AddNull | Op::EnumNull) {
                continue;
            }
            let result = run.apply(op, mutex_fails);
            let (channels, mixes) = run.snapshot();
            steps.push(Step {
                trace: take_trace(),
                result,
                channels,
                mixes,
            });
        }
        (run.fns.1)(run.view);
        steps.push(Step {
            trace: take_trace(),
            result: "destroy".into(),
            channels: Vec::new(),
            mixes: Vec::new(),
        });
    }
    steps
}

fn compare(ops: &[Op], mutex_fails: bool, main_mix: bool) {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: the hook is a plain function; the doubles call it under LOCK.
    unsafe { oracle_view_set_trace(Some(record)) };
    let c = run_side(
        c_fns(),
        oracle_obs_view_enum_video_info,
        ops,
        mutex_fails,
        main_mix,
    );
    let r = run_side(
        rust_fns(),
        shim::obs_view_enum_video_info,
        ops,
        mutex_fails,
        main_mix,
    );
    // SAFETY: as above.
    unsafe { oracle_view_set_trace(None) };
    assert_eq!(c.len(), r.len());
    for (i, (c, r)) in c.iter().zip(&r).enumerate() {
        let op = if i == 0 {
            "create".to_string()
        } else {
            format!("{:?}", ops.get(i - 1))
        };
        assert_eq!(c, r, "step {i} ({op}) of {ops:?}");
    }
}

#[test]
fn view_parity_channels() {
    compare(
        &[
            Op::Set(0, Some(0)),
            Op::Set(0, Some(1)),
            Op::Set(5, Some(1)),
            Op::Get(5),
            Op::Get(6),
            Op::Set(5, None),
            Op::Set(63, Some(2)),
            Op::Get(63),
            Op::Dying(3),
            Op::Set(1, Some(3)),
            Op::Get(1),
            Op::Removed(2),
            Op::Render,
            Op::Render,
            Op::Set(2, Some(0)),
        ],
        false,
        false,
    );
}

#[test]
fn view_parity_mixes() {
    compare(
        &[
            Op::Add,
            Op::Add2(true),
            Op::Add2(false),
            Op::AddNull,
            Op::Enum(10),
            Op::Enum(1),
            Op::EnumNull,
            Op::Remove,
            Op::Enum(10),
            Op::EnumNull,
            Op::Recreate,
            Op::Add2(true),
            Op::Enum(10),
        ],
        false,
        true,
    );
    compare(&[Op::Add, Op::AddNull, Op::Enum(10)], false, false);
}

#[test]
fn view_parity_failed_mutex() {
    compare(&[Op::AddNull, Op::EnumNull, Op::Recreate], true, true);
}

#[test]
fn view_parity_init_and_free() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: zeroed struct obs_view buffers of the C's size; the doubles
    // only trace the mutex.
    unsafe {
        oracle_view_set_trace(Some(record));
        let size = obs_c_oracle::view::obs_view_struct_size();
        let mut traces = Vec::new();
        for side in 0..2 {
            oracle_view_reset(false, ptr::null_mut());
            take_trace();
            let mut buf = vec![0u64; size.div_ceil(8)];
            let view: *mut c_void = buf.as_mut_ptr().cast();
            let mut a = OracleViewSource {
                name: c"a".as_ptr(),
                removed: false,
                dying: false,
            };
            let a: *mut c_void = (&raw mut a).cast();
            let (init, set, free): (
                unsafe extern "C" fn(*mut c_void, u32) -> bool,
                unsafe extern "C" fn(*mut c_void, u32, *mut c_void),
                unsafe extern "C" fn(*mut c_void),
            ) = if side == 0 {
                (
                    oracle_obs_view_init,
                    oracle_obs_view_set_source,
                    oracle_obs_view_free,
                )
            } else {
                (
                    shim::obs_view_init,
                    shim::obs_view_set_source,
                    shim::obs_view_free,
                )
            };
            assert!(!init(ptr::null_mut(), 1));
            assert!(init(view, 1));
            assert_eq!(oracle_view_type_of(view), 1);
            set(view, 7, a);
            free(view);
            free(ptr::null_mut());
            assert!(oracle_view_channel_at(view, 7).is_null());
            traces.push(take_trace());
        }
        oracle_view_set_trace(None);
        assert_eq!(traces[0], traces[1]);
        assert!(traces[0].contains(&"deactivate a 1".to_string()));
    }
}

fn op() -> impl Strategy<Value = Op> {
    let channel = prop_oneof![
        4 => 0u32..4,
        1 => Just(63u32),
        1 => Just(64u32),
        1 => Just(u32::MAX),
    ];
    prop_oneof![
        6 => (channel.clone(), prop::option::of(0usize..4)).prop_map(|(c, s)| Op::Set(c, s)),
        3 => channel.prop_map(Op::Get),
        2 => Just(Op::Render),
        1 => (0usize..4).prop_map(Op::Removed),
        1 => (0usize..4).prop_map(Op::Dying),
        1 => Just(Op::Add),
        1 => any::<bool>().prop_map(Op::Add2),
        1 => Just(Op::AddNull),
        1 => Just(Op::Remove),
        1 => (0usize..4).prop_map(Op::Enum),
        1 => Just(Op::EnumNull),
        1 => Just(Op::Recreate),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn view_parity_random(
        ops in prop::collection::vec(op(), 0..40),
        mutex_fails in prop::bool::weighted(0.1),
        main_mix in any::<bool>(),
    ) {
        compare(&ops, mutex_fails, main_mix);
    }
}
