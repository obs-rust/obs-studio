//! Tier 1: the safe core of `libobs/obs-module-load-rules.c.h`.
//!
//! The first seventeen tests mirror the cases of
//! `test/cmocka/test_module_load_steps.c` one to one, in the same order. The
//! first thirteen call the rules directly, as the C test calls the moved
//! functions; `load_all_callback()`'s decision goes through [`by_rules`],
//! which calls them as `obs-module.c` does, and [`by_c`], the decisions read
//! off the C before the move. The last four go through [`Loader`], which
//! stands for `obs_open_module()`, `obs_get_module_allow_disable()`,
//! `obs_is_core_module()` and `obs_is_legacy_module()`: it checks the open
//! gates, opens nothing that does not exist, logs what the C logs, and asks
//! the rules where the C asks them.
//!
//! Cases marked "characterized, not endorsed" are odd but real (#2801) and
//! kept as the C has them. The tests after those pin the edges of each
//! function, and proptests cover any code, type, version and list length.

use obs_core::module_load_rules::{
    List, MODULE_ERROR, MODULE_FAILED_TO_OPEN, MODULE_HARDCODED_SKIP, MODULE_INCOMPATIBLE_VER,
    MODULE_MISSING_EXPORTS, MODULE_SUCCESS, ROUTE_CORE_CONTINUES, ROUTE_DISABLE, ROUTE_FAIL,
    ROUTE_FREE, ROUTE_KEEP_TYPE, ROUTE_SET_TYPE, ROUTE_STOP, Route, STATE_DISABLED,
    STATE_DISABLED_SAFE, STATE_FAILED_TO_INITIALIZE, STATE_FAILED_TO_OPEN, STATE_INVALID,
    TYPE_CORE, TYPE_LEGACY_PLUGIN, TYPE_PLUGIN, TYPE_UNDEFINED, VERSION_MASK, allow_disable,
    arg_missing, core_loaded, count_failure, disabled_list, export_missing, init_route, is_core,
    is_legacy, open_route, safe_list, version_ok, version_word,
};
use proptest::prelude::*;

/// `LIBOBS_API_VER` of this tree: 33.0.0 (`MAKE_SEMANTIC_VERSION`).
const API_VER: u32 = 33 << 24;

const ALL_TYPES: [u32; 7] = [
    TYPE_UNDEFINED,
    TYPE_CORE,
    TYPE_PLUGIN,
    TYPE_LEGACY_PLUGIN,
    4,
    0x101,
    0xFFFF_FFFF,
];

const ALL_CODES: [i32; 8] = [
    MODULE_SUCCESS,
    MODULE_ERROR,
    MODULE_FAILED_TO_OPEN,
    MODULE_MISSING_EXPORTS,
    MODULE_INCOMPATIBLE_VER,
    MODULE_HARDCODED_SKIP,
    1,
    -6,
];

fn bits(route: Route) -> (u32, u32) {
    (route.bits(), route.state())
}

// ------------------------------------------------------------------ steps

/// `test_arg_missing`.
#[test]
fn test_arg_missing() {
    assert!(arg_missing(false));
    assert!(!arg_missing(true));
    // obs_open_module()'s gate, asked as its || chain asks.
    for b in 0..8u8 {
        let (m, p, c) = (b & 1 != 0, b & 2 != 0, b & 4 != 0);
        let mut asked = 0;
        let mut ask = |present| {
            asked += 1;
            arg_missing(present)
        };
        let missing = ask(m) || ask(p) || ask(c);
        assert_eq!(missing, b != 7, "{m} {p} {c}");
        let expected = if !m {
            1
        } else if !p {
            2
        } else {
            3
        };
        assert_eq!(asked, expected, "{m} {p} {c}");
    }
}

/// `test_export_missing`.
#[test]
fn test_export_missing() {
    assert!(export_missing(false));
    assert!(!export_missing(true));
}

/// `test_version_word`.
#[test]
fn test_version_word() {
    assert_eq!(version_word(0), 0);
    assert_eq!(version_word(0xFFFF), 0);
    assert_eq!(version_word(0x10000), 0x10000);
    assert_eq!(version_word(0x1F_0042), 0x1F_0000);
    assert_eq!(version_word(0xFFFF_FFFF), 0xFFFF_0000);
    assert_eq!(version_word(API_VER), API_VER & 0xFFFF_0000);
}

/// `test_version_ok`.
#[test]
fn test_version_ok() {
    let api = API_VER;
    let major_minor = api & 0xFFFF_0000;

    // Same major and minor, any patch.
    for patch in (0..=0xFFFF_u32).step_by(0x1111) {
        assert!(
            version_ok(version_word(major_minor | patch), api),
            "{patch:x}"
        );
    }
    // Older minors and majors load.
    assert!(version_ok(major_minor - 0x10000, api));
    assert!(version_ok(0, api));
    // One minor above, one major above and the top word don't.
    assert!(!version_ok(major_minor + 0x10000, api));
    assert!(!version_ok(major_minor + 0x0100_0000, api));
    assert!(!version_ok(0xFFFF_0000, api));
    // The whole 32 bits against the libobs version as given, patch included
    // (characterized, not endorsed).
    assert!(version_ok(api, api));
    assert!(!version_ok(api + 1, api));
    assert!(version_ok(0, 0));
    assert!(!version_ok(1, 0));
    assert!(version_ok(0xFFFF_FFFF, 0xFFFF_FFFF));
}

/// `test_lists`.
#[test]
fn test_lists() {
    assert_eq!(safe_list(0), List::All);
    assert_eq!(disabled_list(0), List::None);
    for num in [1, 2, 3, 1000, usize::MAX] {
        assert_eq!(safe_list(num), List::Match, "{num}");
        assert_eq!(disabled_list(num), List::Match, "{num}");
    }
}

/// `test_allow_disable`.
#[test]
fn test_allow_disable() {
    assert!(!allow_disable(true));
    assert!(allow_disable(false));
}

/// `test_types`.
#[test]
fn test_types() {
    for t in [
        TYPE_UNDEFINED,
        TYPE_CORE,
        TYPE_PLUGIN,
        TYPE_LEGACY_PLUGIN,
        4,
        0x101,
        0x103,
        0xFFFF_FFFF,
    ] {
        assert_eq!(is_core(t), t == TYPE_CORE, "{t}");
        assert_eq!(is_legacy(t), t == TYPE_LEGACY_PLUGIN, "{t}");
    }
}

/// `test_open_route`.
#[test]
fn test_open_route() {
    assert_eq!(
        bits(open_route(MODULE_MISSING_EXPORTS)),
        (ROUTE_STOP, STATE_INVALID)
    );
    // The type is not an input: the C asks is_core() only on CORE_CONTINUES.
    assert_eq!(
        bits(open_route(MODULE_FAILED_TO_OPEN)),
        (
            ROUTE_DISABLE | ROUTE_CORE_CONTINUES | ROUTE_FAIL | ROUTE_STOP,
            STATE_FAILED_TO_OPEN
        )
    );
    assert_eq!(
        bits(open_route(MODULE_ERROR)),
        (ROUTE_FAIL | ROUTE_STOP, STATE_INVALID)
    );
    assert_eq!(
        bits(open_route(MODULE_INCOMPATIBLE_VER)),
        (
            ROUTE_DISABLE | ROUTE_FAIL | ROUTE_STOP,
            STATE_FAILED_TO_OPEN
        )
    );
    assert_eq!(
        bits(open_route(MODULE_HARDCODED_SKIP)),
        (ROUTE_STOP, STATE_INVALID)
    );
    // Characterized, not endorsed: an unknown code goes on to init.
    for code in [MODULE_SUCCESS, 1, 2, -6, -100, i32::MIN, i32::MAX] {
        assert_eq!(bits(open_route(code)), (0, STATE_INVALID), "{code}");
    }
}

/// `test_init_route`.
#[test]
fn test_init_route() {
    assert_eq!(bits(init_route(true)), (ROUTE_SET_TYPE, STATE_INVALID));
    assert_eq!(
        bits(init_route(false)),
        (
            ROUTE_FREE | ROUTE_DISABLE | ROUTE_KEEP_TYPE,
            STATE_FAILED_TO_INITIALIZE
        )
    );
}

/// `test_counts`.
#[test]
fn test_counts() {
    assert!(count_failure(true));
    assert!(!count_failure(false));
    assert!(core_loaded(0));
    assert!(!core_loaded(1));
    assert!(!core_loaded(2));
    assert!(!core_loaded(usize::MAX));
}

// -------------------------------------------- load_all_callback() decision

/// What `load_all_callback()` reads.
#[derive(Clone, Copy, Debug, Default)]
struct World {
    is_plugin: bool,
    safe_num: usize,
    safe_listed: bool,
    disabled_num: usize,
    disabled_listed: bool,
    module_type: u32,
    core_name: bool,
    existing: bool,
    code: i32,
    init_ok: bool,
    has_fail_info: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Skip {
    #[default]
    None,
    NotPlugin,
    NotSafe,
    Disabled,
    LegacyCore,
    CoreLoaded,
    LegacyLoaded,
    PluginLoaded,
    Open,
}

/// What it did, and which lookups ran.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
struct Outcome {
    skip: Skip,
    disabled: u32,
    disabled_typed: bool,
    init: bool,
    freed: bool,
    typed: bool,
    counted: bool,
    core_warning: bool,
    asked_type: bool,
    asked_safe: bool,
    asked_disabled: bool,
    asked_core: bool,
    asked_existing: bool,
    opened: bool,
}

fn list_answer(answer: List, listed: bool, asked: &mut bool) -> bool {
    match answer {
        List::All => true,
        List::None => false,
        List::Match => {
            *asked = true;
            listed
        }
    }
}

/// `load_all_callback()` on the rules, as `obs-module.c` calls them.
fn by_rules(w: &World) -> Outcome {
    let mut o = Outcome::default();
    if !w.is_plugin {
        o.skip = Skip::NotPlugin;
        return o;
    }
    if !list_answer(safe_list(w.safe_num), w.safe_listed, &mut o.asked_safe) {
        o.skip = Skip::NotSafe;
        o.disabled = STATE_DISABLED_SAFE;
        return o;
    }
    if !is_core(w.module_type)
        && list_answer(
            disabled_list(w.disabled_num),
            w.disabled_listed,
            &mut o.asked_disabled,
        )
    {
        o.skip = Skip::Disabled;
        o.disabled = STATE_DISABLED;
        return o;
    }
    if is_legacy(w.module_type) {
        o.asked_core = true;
        if w.core_name {
            o.skip = Skip::LegacyCore;
            return o;
        }
    }
    o.asked_existing = true;
    if w.existing {
        o.skip = if is_core(w.module_type) {
            Skip::CoreLoaded
        } else if is_legacy(w.module_type) {
            Skip::LegacyLoaded
        } else {
            Skip::PluginLoaded
        };
        return o;
    }

    o.opened = true;
    let route = open_route(w.code);
    if let Some(state) = route.disable {
        o.disabled = state;
    }
    if route.core_continues {
        o.asked_type = true;
        if is_core(w.module_type) {
            o.disabled_typed = true;
            o.core_warning = true;
            o.skip = Skip::Open;
            return o;
        }
    }
    if route.fail {
        o.skip = Skip::Open;
        o.counted = count_failure(w.has_fail_info);
        return o;
    }
    if route.stop {
        o.skip = Skip::Open;
        return o;
    }

    o.init = true;
    let route = init_route(w.init_ok);
    o.freed = route.free;
    if let Some(state) = route.disable {
        o.disabled = state;
    }
    if route.keep_type {
        o.disabled_typed = true;
    }
    o.typed = route.set_type;
    o
}

/// `load_all_callback()` as the C read before the move.
fn by_c(w: &World) -> Outcome {
    let mut o = Outcome::default();
    if !w.is_plugin {
        o.skip = Skip::NotPlugin;
        return o;
    }
    let mut safe = true;
    if w.safe_num != 0 {
        o.asked_safe = true;
        safe = w.safe_listed;
    }
    if !safe {
        o.skip = Skip::NotSafe;
        o.disabled = STATE_DISABLED_SAFE;
        return o;
    }
    if w.module_type != TYPE_CORE {
        let mut disabled = false;
        if w.disabled_num != 0 {
            o.asked_disabled = true;
            disabled = w.disabled_listed;
        }
        if disabled {
            o.skip = Skip::Disabled;
            o.disabled = STATE_DISABLED;
            return o;
        }
    }
    if w.module_type == TYPE_LEGACY_PLUGIN {
        o.asked_core = true;
        if w.core_name {
            o.skip = Skip::LegacyCore;
            return o;
        }
    }
    o.asked_existing = true;
    if w.existing {
        o.skip = match w.module_type {
            TYPE_CORE => Skip::CoreLoaded,
            TYPE_LEGACY_PLUGIN => Skip::LegacyLoaded,
            _ => Skip::PluginLoaded,
        };
        return o;
    }

    o.opened = true;
    let fail = match w.code {
        MODULE_MISSING_EXPORTS | MODULE_HARDCODED_SKIP => {
            o.skip = Skip::Open;
            return o;
        }
        MODULE_FAILED_TO_OPEN => {
            o.disabled = STATE_FAILED_TO_OPEN;
            o.asked_type = true;
            if w.module_type == TYPE_CORE {
                o.disabled_typed = true;
                o.core_warning = true;
                o.skip = Skip::Open;
                return o;
            }
            true
        }
        MODULE_ERROR => true,
        MODULE_INCOMPATIBLE_VER => {
            o.disabled = STATE_FAILED_TO_OPEN;
            true
        }
        _ => false,
    };
    if fail {
        o.skip = Skip::Open;
        o.counted = w.has_fail_info;
        return o;
    }

    o.init = true;
    if w.init_ok {
        o.typed = true;
    } else {
        o.freed = true;
        o.disabled = STATE_FAILED_TO_INITIALIZE;
        o.disabled_typed = true;
    }
    o
}

fn world(b: u32, module_type: u32, code: i32) -> World {
    World {
        is_plugin: b & 1 != 0,
        safe_num: if b & 2 != 0 { 2 } else { 0 },
        safe_listed: b & 4 != 0,
        disabled_num: usize::from(b & 8 != 0),
        disabled_listed: b & 16 != 0,
        module_type,
        core_name: b & 32 != 0,
        existing: b & 64 != 0,
        code,
        init_ok: b & 128 != 0,
        has_fail_info: b & 256 != 0,
    }
}

/// `test_route_every_combination`.
#[test]
fn test_route_every_combination() {
    let mut cases = 0;
    for b in 0..512 {
        for t in ALL_TYPES {
            for code in ALL_CODES {
                let w = world(b, t, code);
                assert_eq!(by_rules(&w), by_c(&w), "{w:?}");
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 512 * ALL_TYPES.len() * ALL_CODES.len());
}

/// `test_route_examples`.
#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "scenario table: one block per ported step"
)]
fn test_route_examples() {
    let base = World {
        is_plugin: true,
        module_type: TYPE_PLUGIN,
        init_ok: true,
        has_fail_info: true,
        ..World::default()
    };

    let o = by_rules(&base);
    assert_eq!(o.skip, Skip::None);
    assert!(o.init && o.typed);
    assert_eq!(o.disabled, 0);

    let o = by_rules(&World {
        safe_num: 1,
        module_type: TYPE_CORE,
        ..base
    });
    assert_eq!(o.skip, Skip::NotSafe);
    assert_eq!(o.disabled, STATE_DISABLED_SAFE);
    assert!(o.asked_safe);

    let o = by_rules(&World {
        module_type: TYPE_CORE,
        disabled_num: 1,
        disabled_listed: true,
        ..base
    });
    assert_eq!(o.skip, Skip::None);
    assert!(!o.asked_disabled);
    assert!(o.typed);

    let o = by_rules(&World {
        module_type: TYPE_LEGACY_PLUGIN,
        core_name: true,
        ..base
    });
    assert_eq!(o.skip, Skip::LegacyCore);
    assert_eq!(o.disabled, 0);
    assert!(!o.asked_existing);

    let o = by_rules(&World {
        core_name: true,
        ..base
    });
    assert_eq!(o.skip, Skip::None);
    assert!(!o.asked_core);

    let o = by_rules(&World {
        module_type: TYPE_CORE,
        code: MODULE_FAILED_TO_OPEN,
        ..base
    });
    assert_eq!(o.disabled, STATE_FAILED_TO_OPEN);
    assert!(o.disabled_typed && o.core_warning);
    assert!(!o.counted && !o.init);

    let w = World {
        code: MODULE_FAILED_TO_OPEN,
        ..base
    };
    let o = by_rules(&w);
    assert!(o.counted);
    assert!(!o.disabled_typed);
    let o = by_rules(&World {
        has_fail_info: false,
        ..w
    });
    assert!(!o.counted);
    assert_eq!(o.disabled, STATE_FAILED_TO_OPEN);

    let o = by_rules(&World {
        init_ok: false,
        ..base
    });
    assert!(o.freed);
    assert_eq!(o.disabled, STATE_FAILED_TO_INITIALIZE);
    assert!(o.disabled_typed);
    assert!(!o.typed && !o.counted);
}

/// `obs_load_core_modules()`: the cached result, else loaded only when no
/// failure was counted.
struct CoreLoad {
    cached: bool,
}

impl CoreLoad {
    /// The result, and whether the core module list was walked.
    fn load(&mut self, fails: usize) -> (bool, bool) {
        if self.cached {
            return (true, false);
        }
        self.cached = core_loaded(fails);
        (self.cached, true)
    }
}

/// `test_core_load_twice`.
#[test]
fn test_core_load_twice() {
    let mut c = CoreLoad { cached: false };
    assert_eq!(c.load(0), (true, true));
    assert_eq!(c.load(5), (true, false));

    let mut c = CoreLoad { cached: false };
    assert_eq!(c.load(1), (false, true));
    assert_eq!(c.load(3), (false, true));
    assert_eq!(c.load(0), (true, true));
    assert_eq!(c.load(1), (true, false));
}

// ---------------------------------------------------------------- callers

/// Stands for the real libobs callers the C test reaches through the public
/// API: no module exists, so every open that gets past the gates fails to
/// open, and the warning is logged.
struct Loader {
    started: bool,
    log: Vec<String>,
}

impl Loader {
    fn open_module(&mut self, has_module: bool, path: Option<&str>) -> i32 {
        if arg_missing(has_module) || arg_missing(path.is_some()) || arg_missing(self.started) {
            return MODULE_ERROR;
        }
        let path = path.unwrap_or_default();
        self.log.push(format!("Module '{path}' not loaded"));
        MODULE_FAILED_TO_OPEN
    }

    /// `obs_get_module_allow_disable()`: no name in this test is a core
    /// module's.
    fn allow_disable(name: &str) -> bool {
        let core_name = name == "obs-core-module-of-this-test";
        allow_disable(core_name)
    }

    fn is_core_module(module_type: Option<u32>) -> bool {
        module_type.is_some_and(is_core)
    }

    fn is_legacy_module(module_type: Option<u32>) -> bool {
        module_type.is_some_and(is_legacy)
    }
}

/// `test_open_module_gates`, and the check before `obs_startup()` in the C
/// test's group setup.
#[test]
fn test_open_module_gates() {
    let mut l = Loader {
        started: false,
        log: Vec::new(),
    };
    assert_eq!(
        l.open_module(true, Some("module-load-steps-none")),
        MODULE_ERROR
    );
    l.started = true;
    assert_eq!(
        l.open_module(false, Some("module-load-steps-none")),
        MODULE_ERROR
    );
    assert_eq!(l.open_module(true, None), MODULE_ERROR);
    assert!(l.log.is_empty());
}

/// `test_open_module_missing_file`.
#[test]
fn test_open_module_missing_file() {
    let mut l = Loader {
        started: true,
        log: Vec::new(),
    };
    assert_eq!(
        l.open_module(true, Some("module-load-steps-does-not-exist")),
        MODULE_FAILED_TO_OPEN
    );
    assert_eq!(
        l.log,
        ["Module 'module-load-steps-does-not-exist' not loaded"]
    );
}

/// `test_allow_disable_caller`.
#[test]
fn test_allow_disable_caller() {
    assert!(Loader::allow_disable("module-load-steps-not-core"));
    assert!(Loader::allow_disable(""));
}

/// `test_is_core_legacy_null`.
#[test]
fn test_is_core_legacy_null() {
    assert!(!Loader::is_core_module(None));
    assert!(!Loader::is_legacy_module(None));
}

// ------------------------------------------------------------------ edges

#[test]
fn route_bits_and_state() {
    assert_eq!(Route::default().bits(), 0);
    assert_eq!(Route::default().state(), STATE_INVALID);
    let all = Route {
        disable: Some(STATE_FAILED_TO_OPEN),
        keep_type: true,
        set_type: true,
        free: true,
        fail: true,
        stop: true,
        core_continues: true,
    };
    assert_eq!(all.bits(), 0x7F);
    assert_eq!(all.state(), STATE_FAILED_TO_OPEN);
}

#[test]
fn list_codes() {
    assert_eq!(List::None.code(), 0);
    assert_eq!(List::All.code(), 1);
    assert_eq!(List::Match.code(), 2);
}

#[test]
fn open_route_never_sets_type_or_frees() {
    for code in ALL_CODES {
        let r = open_route(code);
        assert!(!r.set_type && !r.free && !r.keep_type, "{code}");
        // Anything that disables or fails stops before init.
        if r.disable.is_some() || r.fail || r.core_continues {
            assert!(r.stop, "{code}");
        }
    }
}

#[test]
fn version_word_keeps_the_top_bits() {
    assert_eq!(VERSION_MASK, 0xFFFF_0000);
    assert_eq!(version_word(0x1234_5678), 0x1234_0000);
    assert_eq!(version_word(0x0000_FFFF), 0);
}

fn any_type() -> impl Strategy<Value = u32> {
    prop_oneof![
        3 => prop::sample::select(ALL_TYPES.to_vec()),
        1 => any::<u32>(),
    ]
}

fn any_code() -> impl Strategy<Value = i32> {
    prop_oneof![
        3 => prop::sample::select(ALL_CODES.to_vec()),
        1 => any::<i32>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn version_word_drops_only_the_patch(ver in any::<u32>()) {
        prop_assert_eq!(version_word(ver), ver & VERSION_MASK);
        prop_assert_eq!(version_word(version_word(ver)), version_word(ver));
    }

    #[test]
    fn version_ok_is_not_above(word in any::<u32>(), api in any::<u32>()) {
        prop_assert_eq!(version_ok(word, api), word <= api);
    }

    #[test]
    fn same_major_minor_always_loads(api in any::<u32>(), patch in 0..=0xFFFF_u32) {
        prop_assert!(version_ok(version_word((api & VERSION_MASK) | patch), api));
    }

    #[test]
    fn lists_by_length(num in any::<usize>()) {
        let empty = num == 0;
        prop_assert_eq!(safe_list(num), if empty { List::All } else { List::Match });
        prop_assert_eq!(disabled_list(num), if empty { List::None } else { List::Match });
    }

    #[test]
    fn routes_match_the_c(b in 0..512_u32, t in any_type(), code in any_code()) {
        let w = world(b, t, code);
        prop_assert_eq!(by_rules(&w), by_c(&w));
    }

    #[test]
    fn core_loaded_only_without_failures(fails in any::<usize>()) {
        prop_assert_eq!(core_loaded(fails), fails == 0);
    }
}
