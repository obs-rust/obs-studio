//! The deprecated `obs_load_all_modules` and `obs_load_all_modules2`
//! convenience loaders, from `libobs/obs-module-load-all.c.h`. Both are
//! intentional no-ops kept for the ABI: they never start OBS, discover or load
//! modules, allocate, log or touch any state. Callers load modules with
//! `obs_load_plugins`/`obs_load_core_modules` instead, which these do not call.

/// `obs_load_all_modules`: does nothing.
pub const fn load_all_modules() {}

/// `obs_load_all_modules2`: does nothing. The C shim never reads or writes the
/// caller's failure info (count, pointer array or strings), so the safe core
/// takes none of it.
pub const fn load_all_modules2() {}
