use std::ffi::{c_char, c_void};
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_ver() -> u32 {
    crate::platform::version()
}
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_set_pointer(_: *mut c_void) {}
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_load() -> bool {
    crate::platform::load()
}
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_unload() {
    crate::platform::unload();
}
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_name() -> *const c_char {
    c"OBS Control API".as_ptr()
}
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_description() -> *const c_char {
    c"Owner-only MCP socket for the running OBS instance".as_ptr()
}
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_author() -> *const c_char {
    c"obs-rust".as_ptr()
}
