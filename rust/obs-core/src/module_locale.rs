//! Safe core of `libobs/obs-module-locale.c.h`: module locale files.
//!
//! The names `obs_module_load_locale()` looks up in a module's data path:
//! the default locale's file ([`default_file`], `locale/<default>.ini`) and
//! the locale's file ([`locale_file`], `/locale/<locale>.ini`), and whether
//! the locale is the default one, so that its file is not added
//! ([`is_default`]). `obs-module.c` keeps the module, the lookups
//! (`obs_find_module_file()`, `text_lookup_create()`, `text_lookup_add()`)
//! and the logging, and passes the locales in.
//!
//! Locales are bytes, not text: they come from the configuration and the
//! module, so they can hold anything but NUL. `None` is a NULL string, which
//! the C's `dstr_cat` and `astrcmpi` take as empty. Nothing here panics,
//! indexes or overflows for any input.
//!
//! Kept as the C has it, characterized and not endorsed (#1876):
//!
//! - [`locale_file`] starts with '/' and [`default_file`] does not, so after
//!   a data path the locale's file has two ('<data>//locale/<locale>.ini',
//!   which still resolves), and with an empty data path it is absolute;
//! - [`is_default`] ignores ASCII case, so a locale that differs from the
//!   default only in case is never added, even when its own file exists;
//! - the locales are not checked: '/' and ".." pass through.

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

/// What the default locale's file name starts with.
pub const DEFAULT_PREFIX: &[u8] = b"locale/";
/// What the locale's file name starts with.
pub const LOCALE_PREFIX: &[u8] = b"/locale/";
/// What both file names end with.
pub const LOCALE_EXT: &[u8] = b".ini";

fn file(prefix: &[u8], locale: Option<&[u8]>) -> Vec<u8> {
    [prefix, locale.unwrap_or_default(), LOCALE_EXT].concat()
}

/// Same contract as C `obs_module_locale_default_file`: `locale/`, the
/// default locale, `.ini`.
pub fn default_file(default_locale: Option<&[u8]>) -> Vec<u8> {
    file(DEFAULT_PREFIX, default_locale)
}

/// Same contract as C `obs_module_locale_is_default`: `astrcmpi(locale,
/// default_locale) == 0`, the two equal ignoring ASCII case, NULL as empty.
pub fn is_default(locale: Option<&[u8]>, default_locale: Option<&[u8]>) -> bool {
    obs_util::dstr::astrcmpi(locale, default_locale) == 0
}

/// Same contract as C `obs_module_locale_file`: `/locale/`, the locale,
/// `.ini`.
pub fn locale_file(locale: Option<&[u8]>) -> Vec<u8> {
    file(LOCALE_PREFIX, locale)
}
