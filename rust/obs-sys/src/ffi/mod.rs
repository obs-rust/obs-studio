//! Declarations from the unchanged libobs public headers.
unsafe extern "C" {
    /// Returns the libobs C API version from `libobs/obs-config.h`.
    pub fn obs_get_version() -> u32;
    /// Initializes libobs; a null name store asks libobs to create its own.
    pub fn obs_startup(
        locale: *const std::ffi::c_char,
        module_config_path: *const std::ffi::c_char,
        store: *mut std::ffi::c_void,
    ) -> bool;
    pub fn obs_initialized() -> bool;
    pub fn obs_shutdown();
}
