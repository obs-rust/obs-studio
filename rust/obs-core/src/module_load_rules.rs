//! Safe core of `libobs/obs-module-load-rules.c.h`: the module load rules.
//!
//! The pure decisions of the module loader in `obs-module.c`, each made where
//! the C makes it, from the values the caller reads at that point:
//!
//! - `obs_open_module()`: an argument it needs (the module pointer, the path,
//!   the core) is missing, asked for each in turn as the C's `||` chain
//!   reads them ([`arg_missing`]); a required export is missing, asked
//!   after each of the three lookups (`obs_module_load`,
//!   `obs_module_set_pointer`, `obs_module_ver`, [`export_missing`]); the
//!   version a module reports without its patch number ([`version_word`])
//!   is not above the libobs it runs in ([`version_ok`]);
//! - the safe and disabled lists: from the length alone, every module is on
//!   the list, none is, or the name decides ([`safe_list`],
//!   [`disabled_list`], [`List`]); the name match is
//!   [`listed`](crate::module_names::listed);
//! - `obs_get_module_allow_disable()` ([`allow_disable`]), and the core and
//!   legacy plugin types ([`is_core`], [`is_legacy`]) that
//!   `load_all_callback()`, `find_core_module()`, `obs_is_core_module()` and
//!   `obs_is_legacy_module()` ask about;
//! - what `load_all_callback()` does with the code `obs_open_module()`
//!   returned ([`open_route`]) and with the result of `obs_init_module()`
//!   ([`init_route`]), as a [`Route`]; whether a failure is counted
//!   ([`count_failure`]);
//! - `obs_load_core_modules()`: loaded when no failure was counted
//!   ([`core_loaded`]).
//!
//! `obs-module.c` keeps `get_plugin_info()`, `obs_get_module()`,
//! `obs_host_skip_module()`, `os_dlopen()`, `os_dlsym()`, the `ver()` call,
//! the module set-up, `obs_create_disabled_module()`, `obs_init_module()`,
//! `free_module()`, the failure list, the module walks and every log
//! message, in the same order and under the same conditions, and passes the
//! values in. Module types and states are plain `u32`s, as the C enums are,
//! and codes are `i32`s, so any value is kept as its bits. Nothing here
//! panics, indexes or overflows for any input.
//!
//! Kept as the C has it, characterized and not endorsed (#2801): an open
//! code `obs_open_module()` never returns goes on to `obs_init_module()`, as
//! the C's switch has no default.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `MODULE_SUCCESS`.
pub const MODULE_SUCCESS: i32 = 0;
/// `MODULE_ERROR`.
pub const MODULE_ERROR: i32 = -1;
/// `MODULE_FAILED_TO_OPEN`.
pub const MODULE_FAILED_TO_OPEN: i32 = -2;
/// `MODULE_MISSING_EXPORTS`.
pub const MODULE_MISSING_EXPORTS: i32 = -3;
/// `MODULE_INCOMPATIBLE_VER`.
pub const MODULE_INCOMPATIBLE_VER: i32 = -4;
/// `MODULE_HARDCODED_SKIP`.
pub const MODULE_HARDCODED_SKIP: i32 = -5;

/// `OBS_MODULE_TYPE_UNDEFINED`.
pub const TYPE_UNDEFINED: u32 = 0;
/// `OBS_MODULE_TYPE_CORE`.
pub const TYPE_CORE: u32 = 1;
/// `OBS_MODULE_TYPE_PLUGIN`.
pub const TYPE_PLUGIN: u32 = 2;
/// `OBS_MODULE_TYPE_LEGACY_PLUGIN`.
pub const TYPE_LEGACY_PLUGIN: u32 = 3;

/// `OBS_MODULE_INVALID`: no disabled module.
pub const STATE_INVALID: u32 = 0;
/// `OBS_MODULE_DISABLED`.
pub const STATE_DISABLED: u32 = 3;
/// `OBS_MODULE_DISABLED_SAFE`.
pub const STATE_DISABLED_SAFE: u32 = 4;
/// `OBS_MODULE_FAILED_TO_OPEN`.
pub const STATE_FAILED_TO_OPEN: u32 = 5;
/// `OBS_MODULE_FAILED_TO_INITIALIZE`.
pub const STATE_FAILED_TO_INITIALIZE: u32 = 6;

/// The bits of the version a module reports that [`version_word`] keeps:
/// the major and minor numbers, without the patch.
pub const VERSION_MASK: u32 = 0xFFFF_0000;

/// `OBS_MODULE_LOAD_RULES_DISABLE`.
pub const ROUTE_DISABLE: u32 = 1 << 0;
/// `OBS_MODULE_LOAD_RULES_KEEP_TYPE`.
pub const ROUTE_KEEP_TYPE: u32 = 1 << 1;
/// `OBS_MODULE_LOAD_RULES_SET_TYPE`.
pub const ROUTE_SET_TYPE: u32 = 1 << 2;
/// `OBS_MODULE_LOAD_RULES_FREE`.
pub const ROUTE_FREE: u32 = 1 << 3;
/// `OBS_MODULE_LOAD_RULES_FAIL`.
pub const ROUTE_FAIL: u32 = 1 << 4;
/// `OBS_MODULE_LOAD_RULES_STOP`.
pub const ROUTE_STOP: u32 = 1 << 5;
/// `OBS_MODULE_LOAD_RULES_CORE_CONTINUES`.
pub const ROUTE_CORE_CONTINUES: u32 = 1 << 6;

/// What the safe or disabled list says about a module before its name is
/// looked at (`enum obs_module_load_rules_list`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum List {
    /// No module is on the list (an empty disabled list).
    None,
    /// Every module is on it (an empty safe list).
    All,
    /// The name decides.
    Match,
}

impl List {
    /// The C enum value (`OBS_MODULE_LOAD_RULES_LIST_*`).
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::None => 0,
            Self::All => 1,
            Self::Match => 2,
        }
    }
}

/// What `load_all_callback()` does next.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Route {
    /// Make a disabled module in this state (`OBS_MODULE_*`).
    pub disable: Option<u32>,
    /// Give that disabled module the module's type.
    pub keep_type: bool,
    /// Give the loaded module its type.
    pub set_type: bool,
    /// Free the module first.
    pub free: bool,
    /// Count a failure.
    pub fail: bool,
    /// Go no further: no `obs_init_module()`.
    pub stop: bool,
    /// For a core module ([`is_core`]), which the C asks only then: give the
    /// disabled module its type and stop instead of counting a failure.
    pub core_continues: bool,
}

impl Route {
    /// The C's `OBS_MODULE_LOAD_RULES_*` route bits.
    #[must_use]
    pub const fn bits(self) -> u32 {
        let mut bits = 0;
        if self.disable.is_some() {
            bits |= ROUTE_DISABLE;
        }
        if self.keep_type {
            bits |= ROUTE_KEEP_TYPE;
        }
        if self.set_type {
            bits |= ROUTE_SET_TYPE;
        }
        if self.free {
            bits |= ROUTE_FREE;
        }
        if self.fail {
            bits |= ROUTE_FAIL;
        }
        if self.stop {
            bits |= ROUTE_STOP;
        }
        if self.core_continues {
            bits |= ROUTE_CORE_CONTINUES;
        }
        bits
    }

    /// The state the C is given back: the disabled module's, or
    /// [`STATE_INVALID`] when there is none.
    #[must_use]
    pub const fn state(self) -> u32 {
        match self.disable {
            Some(state) => state,
            None => STATE_INVALID,
        }
    }
}

/// An argument `obs_open_module()` needs is missing.
#[must_use]
pub fn arg_missing(present: bool) -> bool {
    !present
}

/// A required export was not found.
#[must_use]
pub fn export_missing(found: bool) -> bool {
    !found
}

/// The version a module reports without its patch number.
#[must_use]
pub fn version_word(ver: u32) -> u32 {
    ver & VERSION_MASK
}

/// The module's version word is not above `api_ver`, the libobs it runs in
/// (`LIBOBS_API_VER`, patch included).
#[must_use]
pub fn version_ok(ver_word: u32, api_ver: u32) -> bool {
    ver_word <= api_ver
}

/// The safe list, from its length: every module is safe when it is empty.
#[must_use]
pub fn safe_list(num: usize) -> List {
    if num == 0 { List::All } else { List::Match }
}

/// The disabled list, from its length: no module is disabled when it is
/// empty.
#[must_use]
pub fn disabled_list(num: usize) -> List {
    if num == 0 { List::None } else { List::Match }
}

/// A module can be disabled unless its name is a core module's.
#[must_use]
pub fn allow_disable(core_name: bool) -> bool {
    !core_name
}

/// The core module type.
#[must_use]
pub fn is_core(module_type: u32) -> bool {
    module_type == TYPE_CORE
}

/// The legacy plugin type.
#[must_use]
pub fn is_legacy(module_type: u32) -> bool {
    module_type == TYPE_LEGACY_PLUGIN
}

/// What `load_all_callback()` does with the code `obs_open_module()`
/// returned. The module type is not an input: the C reads it only for
/// [`Route::core_continues`], after making the disabled module.
#[must_use]
pub fn open_route(code: i32) -> Route {
    let stop = Route {
        stop: true,
        ..Route::default()
    };
    match code {
        MODULE_MISSING_EXPORTS | MODULE_HARDCODED_SKIP => stop,
        // Most often a library the module needs is not installed (libcef.so
        // for obs-browser): a core module carries on without it, as a plugin
        // does, instead of failing the core module load and OBS startup.
        MODULE_FAILED_TO_OPEN => Route {
            disable: Some(STATE_FAILED_TO_OPEN),
            core_continues: true,
            fail: true,
            ..stop
        },
        MODULE_INCOMPATIBLE_VER => Route {
            disable: Some(STATE_FAILED_TO_OPEN),
            fail: true,
            ..stop
        },
        MODULE_ERROR => Route { fail: true, ..stop },
        // MODULE_SUCCESS, and any code obs_open_module() never returns.
        _ => Route::default(),
    }
}

/// What `load_all_callback()` does with the result of `obs_init_module()`.
#[must_use]
pub fn init_route(ok: bool) -> Route {
    if ok {
        Route {
            set_type: true,
            ..Route::default()
        }
    } else {
        Route {
            disable: Some(STATE_FAILED_TO_INITIALIZE),
            keep_type: true,
            free: true,
            ..Route::default()
        }
    }
}

/// A failure is counted only when the caller passed somewhere to count it.
#[must_use]
pub fn count_failure(has_fail_info: bool) -> bool {
    has_fail_info
}

/// The core modules loaded: no failure was counted.
#[must_use]
pub fn core_loaded(fail_count: usize) -> bool {
    fail_count == 0
}
