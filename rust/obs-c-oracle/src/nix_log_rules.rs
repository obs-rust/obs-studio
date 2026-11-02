//! `oracle_*` build of `libobs/obs-nix-log-rules.c.h`
//! (oracle/nix_log_rules.c): the distribution, Flatpak and desktop log rules
//! of `obs-nix.c.h`, renamed. The two enums are declared as `c_int`, with
//! the values `obs-nix-system-info.h` spells out; a returned position is a
//! pointer into the string passed in, or to a static string.

use std::os::raw::{c_char, c_int};

unsafe extern "C" {
    /// `const char *obs_nix_system_info_os_release_field(const char *line,
    /// enum obs_nix_os_release_key *key)`.
    pub fn oracle_obs_nix_system_info_os_release_field(
        line: *const c_char,
        key: *mut c_int,
    ) -> *const c_char;
    /// `const char *obs_nix_system_info_os_release_value(const char *start,
    /// size_t *len)`.
    pub fn oracle_obs_nix_system_info_os_release_value(
        start: *const c_char,
        len: *mut usize,
    ) -> *const c_char;
    /// `const char *obs_nix_system_info_version_fallback(bool has_build_id,
    /// const char *distro)`.
    pub fn oracle_obs_nix_system_info_version_fallback(
        has_build_id: bool,
        distro: *const c_char,
    ) -> *const c_char;
    /// `const char *obs_nix_system_info_extension_name(const char *ext,
    /// size_t *len)`.
    pub fn oracle_obs_nix_system_info_extension_name(
        ext: *const c_char,
        len: *mut usize,
    ) -> *const c_char;
    /// `enum obs_nix_desktop_form obs_nix_system_info_desktop_form(bool
    /// has_current, bool has_session)`.
    pub fn oracle_obs_nix_system_info_desktop_form(has_current: bool, has_session: bool) -> c_int;
}
