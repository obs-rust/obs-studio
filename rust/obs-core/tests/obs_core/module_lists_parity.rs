//! Tier 3: `obs_core::ffi::module_lists` against the original C.
//!
//! The original `libobs/obs-module-lists.c.h` (oracle/module_lists.c,
//! renamed `oracle_obs_*`) and the shims run the same calls on two copies of
//! the same world: a stand-in core (or none) with its module config path and
//! modules with their files, data paths and names, some on the core's list
//! of loaded modules. The shims reach that core through the module list
//! seam, compiled in the oracle over its thread-local core; `blog()` is the
//! oracle's recording stand-in on both sides. After every call the test
//! compares what it returned and what it logged, and at the end every list
//! of the core and of each module: each entry (NULL against empty
//! included), its size and its capacity, so a push that allocates
//! differently shows too. The search paths' type is compared as well.
//!
//! The file lookups run on a directory of the test's own in the temporary
//! directory, with data paths in it, past it and relative, and both sides
//! run libobs's own `obs_module_paths_find_file()` and
//! `obs_module_names_config_path()` (already Rust), so this compares how
//! the calls reach them.
//!
//! Intentional difference, left out of the comparison and pinned on the
//! Rust side alone: `obs_log_loaded_modules()` with no core logs nothing.
//! The C logs the heading and then reads the core's first module, which
//! crashes, so the C side never runs it.
//!
//! Also never run, on either side: a module's calls with no core. A module
//! exists only in a core, and `obs_module_get_config_path()` reads the
//! core's config path unchecked on both sides.

use core::ffi::{CStr, c_char, c_int, c_void};
use std::ffi::CString;
use std::mem::{align_of, offset_of, size_of};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use obs_c_oracle::module_lists as c;
use obs_core::ffi::module_lists::{
    self as rs, Blog, Calls, HEADING_FORMAT, LOG_INFO, MODULE_FORMAT, OBS_MODULE_TYPE_UNDEFINED,
    obs_module_path,
};
use obs_core::module_lists::{HEADING, MODULE_FORMAT as CORE_MODULE_FORMAT};
use obs_util::ffi::bmem::bfree;
use proptest::prelude::*;

/// The shims' `blog()`: the oracle's recording stand-in.
enum Oracle {}

impl Calls for Oracle {
    fn blog() -> Blog {
        c::oracle_module_lists_blog
    }
}

/// A directory of the test's own with one file, removed on drop.
struct Dir {
    path: PathBuf,
    /// The directory as C writes paths: '/' separators.
    prefix: String,
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

const PRESENT: &str = "present.txt";

impl Dir {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("obs-module-lists-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join(PRESENT), b"x").unwrap();
        let prefix = path.to_str().unwrap().replace('\\', "/");
        Self { path, prefix }
    }

    /// A data path of `choice`: the directory, with a trailing '/', one
    /// that does not exist, empty, NULL, or a relative one.
    fn data_path(&self, choice: u8) -> Option<Vec<u8>> {
        match choice % 6 {
            0 => Some(self.prefix.clone().into_bytes()),
            1 => Some(format!("{}/", self.prefix).into_bytes()),
            2 => Some(format!("{}/missing", self.prefix).into_bytes()),
            3 => Some(Vec::new()),
            4 => None,
            _ => Some(b"module-lists-relative".to_vec()),
        }
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A file of `choice`: the one there, one that is not, empty or NULL.
fn file(choice: u8) -> Option<Vec<u8>> {
    match choice % 4 {
        0 => Some(PRESENT.as_bytes().to_vec()),
        1 => Some(b"missing.txt".to_vec()),
        2 => Some(Vec::new()),
        _ => None,
    }
}

fn cstring(s: &Option<Vec<u8>>) -> Option<CString> {
    s.as_ref().map(|s| CString::new(s.clone()).unwrap())
}

fn ptr(s: &Option<CString>) -> *const c_char {
    s.as_ref().map_or(core::ptr::null(), |s| s.as_ptr())
}

/// The bytes of a `bmalloc`'d result, freed.
fn take(p: *mut c_char) -> Option<Vec<u8>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: a NUL-terminated `bmalloc`'d string from a shim or the oracle.
    let out = unsafe { CStr::from_ptr(p) }.to_bytes().to_vec();
    // SAFETY: allocated by `bmalloc`.
    unsafe { bfree(p.cast()) };
    Some(out)
}

fn take_log() -> String {
    let mut out = vec![0u8; 1 << 16];
    // SAFETY: `out` holds `out.len()` bytes.
    let n = unsafe { c::oracle_module_lists_take_log(out.as_mut_ptr().cast(), out.len()) };
    out.truncate(n);
    String::from_utf8_lossy(&out).into_owned()
}

/// A module of the world.
#[derive(Clone, Debug)]
struct ModuleSpec {
    file: Option<Vec<u8>>,
    data_path: u8,
    name: Option<Vec<u8>>,
    loaded: bool,
}

/// One call.
#[derive(Clone, Debug)]
enum Op {
    Log,
    AddType(Option<usize>, u8, Option<Vec<u8>>),
    FindFile(Option<usize>, u8),
    ConfigPath(Option<usize>, u8),
    AddPath(Option<Vec<u8>>, Option<Vec<u8>>),
    AddName(bool, Option<Vec<u8>>),
}

/// A core (its module config path, `None` for NULL) or none, its modules,
/// and the calls.
#[derive(Clone, Debug)]
struct World {
    core: Option<Option<Vec<u8>>>,
    modules: Vec<ModuleSpec>,
    ops: Vec<Op>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    C,
    Rust,
}

/// What one call gave: its result and its log.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    result: Option<Vec<u8>>,
    log: String,
}

/// The world's modules in `core` (none with no core).
fn make_modules(world: &World, dir: &Dir, core: *mut c_void) -> Vec<*mut c_void> {
    if core.is_null() {
        return Vec::new();
    }
    world
        .modules
        .iter()
        .map(|m| {
            let (file, data, name) = (
                cstring(&m.file),
                cstring(&dir.data_path(m.data_path)),
                cstring(&m.name),
            );
            // SAFETY: a live core; NULL or NUL-terminated strings.
            unsafe {
                c::oracle_module_lists_module_new(
                    core,
                    ptr(&file),
                    ptr(&data),
                    ptr(&name),
                    m.loaded,
                )
            }
        })
        .collect()
}

/// Every list of `core` and `modules`, empty with no core.
fn state(core: *mut c_void, modules: &[*mut c_void]) -> String {
    if core.is_null() {
        return String::new();
    }
    let mut out = vec![0u8; 1 << 18];
    // SAFETY: the live core and modules; `out` holds `out.len()` bytes.
    let n = unsafe {
        c::oracle_module_lists_state(
            core,
            modules.as_ptr(),
            modules.len(),
            out.as_mut_ptr().cast(),
            out.len(),
        )
    };
    out.truncate(n);
    String::from_utf8_lossy(&out).into_owned()
}

/// Runs `world` on `side`: the outcome of each call, then every list.
fn run(world: &World, dir: &Dir, side: Side) -> (Vec<Outcome>, String) {
    // SAFETY: a fresh core of the oracle's, or none.
    let core = world.core.as_ref().map_or(core::ptr::null_mut(), |config| {
        let config = cstring(config);
        unsafe { c::oracle_module_lists_core_new(ptr(&config)) }
    });
    // SAFETY: `core` is null or the oracle's.
    unsafe { c::oracle_module_lists_set_core(core) };
    take_log();

    let modules = make_modules(world, dir, core);
    let module = |i: &Option<usize>| -> *mut c_void {
        match i {
            Some(i) if !modules.is_empty() => modules[i % modules.len()],
            _ => core::ptr::null_mut(),
        }
    };

    let mut outcomes = Vec::new();
    for op in &world.ops {
        // SAFETY: the oracle's live core (or none) and modules, and NULL or
        // NUL-terminated strings, as libobs calls them.
        let result = unsafe {
            match op {
                Op::Log => {
                    if core.is_null() {
                        // The intentional difference: never on the C side.
                        continue;
                    }
                    match side {
                        Side::C => c::oracle_obs_log_loaded_modules(),
                        Side::Rust => rs::log_loaded_modules_with::<Oracle>(),
                    }
                    None
                }
                Op::AddType(m, list, id) => {
                    let id = cstring(id);
                    let (m, id) = (module(m), ptr(&id));
                    match (side, list % 4) {
                        (Side::C, 0) => c::oracle_obs_module_add_source(m, id),
                        (Side::C, 1) => c::oracle_obs_module_add_output(m, id),
                        (Side::C, 2) => c::oracle_obs_module_add_encoder(m, id),
                        (Side::C, _) => c::oracle_obs_module_add_service(m, id),
                        (Side::Rust, 0) => rs::obs_module_add_source(m, id),
                        (Side::Rust, 1) => rs::obs_module_add_output(m, id),
                        (Side::Rust, 2) => rs::obs_module_add_encoder(m, id),
                        (Side::Rust, _) => rs::obs_module_add_service(m, id),
                    }
                    None
                }
                Op::FindFile(m, f) => {
                    let f = cstring(&file(*f));
                    let (m, f) = (module(m), ptr(&f));
                    take(match side {
                        Side::C => c::oracle_obs_find_module_file(m, f),
                        Side::Rust => rs::obs_find_module_file(m, f),
                    })
                }
                Op::ConfigPath(m, f) => {
                    let f = cstring(&file(*f));
                    let (m, f) = (module(m), ptr(&f));
                    take(match side {
                        Side::C => c::oracle_obs_module_get_config_path(m, f),
                        Side::Rust => rs::obs_module_get_config_path(m, f),
                    })
                }
                Op::AddPath(bin, data) => {
                    let (bin, data) = (cstring(bin), cstring(data));
                    match side {
                        Side::C => c::oracle_obs_add_module_path(ptr(&bin), ptr(&data)),
                        Side::Rust => rs::obs_add_module_path(ptr(&bin), ptr(&data)),
                    }
                    None
                }
                Op::AddName(safe, name) => {
                    let name = cstring(name);
                    match (side, safe) {
                        (Side::C, true) => c::oracle_obs_add_safe_module(ptr(&name)),
                        (Side::C, false) => c::oracle_obs_add_disabled_module(ptr(&name)),
                        (Side::Rust, true) => rs::obs_add_safe_module(ptr(&name)),
                        (Side::Rust, false) => rs::obs_add_disabled_module(ptr(&name)),
                    }
                    None
                }
            }
        };
        outcomes.push(Outcome {
            result,
            log: take_log(),
        });
    }

    let state = state(core, &modules);
    // SAFETY: the oracle's modules and core, each freed once; then no core.
    unsafe {
        for m in modules {
            c::oracle_module_lists_module_free(m);
        }
        c::oracle_module_lists_set_core(core::ptr::null_mut());
        if !core.is_null() {
            c::oracle_module_lists_core_free(core);
        }
    }
    (outcomes, state)
}

fn assert_same(world: &World) {
    let dir = Dir::new();
    let c_side = run(world, &dir, Side::C);
    let rust_side = run(world, &dir, Side::Rust);
    assert_eq!(rust_side, c_side, "{world:?}");
}

fn loaded(name: &str) -> ModuleSpec {
    ModuleSpec {
        file: Some(format!("bin/{name}.so").into_bytes()),
        data_path: 0,
        name: Some(name.as_bytes().to_vec()),
        loaded: true,
    }
}

fn s(text: &str) -> Option<Vec<u8>> {
    Some(text.as_bytes().to_vec())
}

#[test]
fn layout_matches_c() {
    let mut layout = [0usize; 8];
    // SAFETY: `layout` holds the 8 values the oracle writes.
    unsafe { c::oracle_module_lists_layout(layout.as_mut_ptr()) };
    assert_eq!(layout[0], size_of::<obs_module_path>());
    assert_eq!(layout[1], align_of::<obs_module_path>());
    assert_eq!(layout[2], offset_of!(obs_module_path, bin));
    assert_eq!(layout[3], offset_of!(obs_module_path, data));
    assert_eq!(layout[4], offset_of!(obs_module_path, module_type));
    assert_eq!(layout[5], size_of::<c_int>());
    assert_eq!(layout[6], OBS_MODULE_TYPE_UNDEFINED as usize);
    assert_eq!(layout[7], LOG_INFO as usize);
    assert_eq!(HEADING_FORMAT.to_bytes(), HEADING.as_bytes());
    assert_eq!(MODULE_FORMAT.to_bytes(), CORE_MODULE_FORMAT.as_bytes());
}

/// `test_module_lists`' calls, in its order, on both sides.
#[test]
fn characterization_calls_match_c() {
    let core = World {
        core: Some(s("module-lists-config")),
        modules: vec![
            ModuleSpec {
                file: s("bin/module_lists_stub.so"),
                data_path: 0,
                name: s("module_lists_stub"),
                loaded: false,
            },
            loaded("a"),
            loaded("b"),
        ],
        ops: vec![
            Op::Log,
            Op::AddType(None, 0, s("module_lists_null")),
            Op::AddType(None, 0, None),
            Op::AddType(Some(0), 0, s("module_lists_source")),
            Op::AddType(Some(0), 1, s("module_lists_output")),
            Op::AddType(Some(0), 2, s("module_lists_encoder")),
            Op::AddType(Some(0), 3, s("module_lists_service")),
            Op::AddType(Some(0), 0, s("module_lists_source")),
            Op::FindFile(Some(0), 0),
            Op::FindFile(Some(0), 1),
            Op::FindFile(None, 0),
            Op::ConfigPath(Some(0), 0),
            Op::ConfigPath(Some(0), 3),
            Op::ConfigPath(None, 0),
            Op::AddPath(None, s("data")),
            Op::AddPath(s("bin"), None),
            Op::AddPath(None, None),
            Op::AddName(true, None),
            Op::AddName(false, None),
            Op::AddPath(s("module-lists-bin"), s("module-lists-data")),
            Op::AddName(true, s("module_lists_safe")),
            Op::AddName(false, s("module_lists_disabled")),
            Op::AddPath(s(""), s("")),
            Op::AddName(true, s("")),
            Op::AddName(false, s("")),
            Op::AddType(Some(0), 0, None),
            Op::AddType(Some(0), 1, None),
            Op::AddType(Some(0), 2, None),
            Op::AddType(Some(0), 3, None),
        ],
    };
    assert_same(&core);

    let no_core = World {
        core: None,
        modules: Vec::new(),
        ops: vec![
            Op::AddPath(s("bin"), s("data")),
            Op::AddName(true, s("safe")),
            Op::AddName(false, s("disabled")),
            Op::AddType(None, 0, s("id")),
            Op::AddType(None, 1, s("id")),
            Op::AddType(None, 2, s("id")),
            Op::AddType(None, 3, s("id")),
            Op::FindFile(None, 0),
            Op::ConfigPath(None, 0),
        ],
    };
    assert_same(&no_core);
}

/// The log lines, a NULL file and a NULL config path or name included.
#[test]
fn log_and_paths_match_c() {
    let world = World {
        core: Some(None),
        modules: vec![
            loaded("first"),
            ModuleSpec {
                file: None,
                data_path: 4,
                name: None,
                loaded: true,
            },
            loaded("last"),
        ],
        ops: vec![
            Op::Log,
            Op::ConfigPath(Some(1), 0),
            Op::ConfigPath(Some(0), 2),
            Op::FindFile(Some(1), 0),
            Op::FindFile(Some(0), 2),
        ],
    };
    assert_same(&world);
}

/// The intentional difference, on the Rust side alone: with no core,
/// `obs_log_loaded_modules()` logs nothing (the C crashes).
#[test]
fn log_with_no_core_logs_nothing() {
    // SAFETY: no core on this thread.
    unsafe {
        c::oracle_module_lists_set_core(core::ptr::null_mut());
        take_log();
        rs::log_loaded_modules_with::<Oracle>();
    }
    assert_eq!(take_log(), "");
}

fn bytes() -> impl Strategy<Value = Option<Vec<u8>>> {
    proptest::option::of(proptest::collection::vec(1u8..=255, 0..6))
}

fn module_spec() -> impl Strategy<Value = ModuleSpec> {
    (bytes(), any::<u8>(), bytes(), any::<bool>()).prop_map(|(file, data_path, name, loaded)| {
        ModuleSpec {
            file,
            data_path,
            name,
            loaded,
        }
    })
}

fn op() -> impl Strategy<Value = Op> {
    let module = proptest::option::of(0usize..4);
    prop_oneof![
        Just(Op::Log),
        (module.clone(), any::<u8>(), bytes()).prop_map(|(m, l, id)| Op::AddType(m, l, id)),
        (module.clone(), any::<u8>()).prop_map(|(m, f)| Op::FindFile(m, f)),
        (module, any::<u8>()).prop_map(|(m, f)| Op::ConfigPath(m, f)),
        (bytes(), bytes()).prop_map(|(b, d)| Op::AddPath(b, d)),
        (any::<bool>(), bytes()).prop_map(|(safe, n)| Op::AddName(safe, n)),
    ]
}

fn world() -> impl Strategy<Value = World> {
    (
        proptest::option::of(bytes()),
        proptest::collection::vec(module_spec(), 0..4),
        proptest::collection::vec(op(), 0..24),
    )
        .prop_map(|(core, modules, ops)| World { core, modules, ops })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    /// Random worlds and calls, in any order: every result, log line and
    /// list is the C's.
    #[test]
    fn random_calls_match_c(world in world()) {
        assert_same(&world);
    }
}
