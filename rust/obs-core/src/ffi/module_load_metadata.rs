//! C conversion and ownership for the manifest loader.
use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use std::ffi::{CStr, CString};

use super::module_paths::bstrdup;
use crate::module_load_metadata::{self, ManifestSource};

/// Measured against obs-internal.h in the differential suite.
#[repr(C)]
pub struct ModuleMetadata {
    pub display_name: *mut c_char,
    pub version: *mut c_char,
    pub id: *mut c_char,
    pub os_arch: *mut c_char,
    pub description: *mut c_char,
    pub long_description: *mut c_char,
    pub has_icon: bool,
    pub has_banner: bool,
    pub repository_url: *mut c_char,
    pub support_url: *mut c_char,
    pub website_url: *mut c_char,
    pub name: *mut c_char,
}

unsafe extern "C" {
    fn obs_module_load_metadata_slot(module: *mut c_void) -> *mut *mut ModuleMetadata;
    fn obs_module_metadata_data_path_field(module: *mut c_void) -> *const *const c_char;
    fn obs_module_names_manifest_path(path: *const c_char) -> *mut c_char;
    fn os_file_exists(path: *const c_char) -> bool;
    fn obs_data_create_from_json_file(path: *const c_char) -> *mut c_void;
    fn obs_data_get_string(data: *mut c_void, key: *const c_char) -> *const c_char;
    fn obs_data_get_bool(data: *mut c_void, key: *const c_char) -> bool;
    fn obs_data_get_obj(data: *mut c_void, key: *const c_char) -> *mut c_void;
    fn obs_data_release(data: *mut c_void);
    fn bmalloc(size: usize) -> *mut c_void;
    fn bfree(ptr: *mut c_void);
}

struct Reader {
    path: *mut c_char,
    data: *mut c_void,
    urls: *mut c_void,
}
impl Reader {
    fn text(data: *mut c_void, key: &str) -> Vec<u8> {
        let key = CString::new(key).expect("manifest keys contain no NUL");
        // SAFETY: OBS accepts NULL data, returns a live NUL-terminated string.
        unsafe {
            CStr::from_ptr(obs_data_get_string(data, key.as_ptr()))
                .to_bytes()
                .to_vec()
        }
    }
}
impl ManifestSource for Reader {
    fn open(&mut self) -> bool {
        // SAFETY: path is owned and NUL-terminated; OBS accepts NULL data on parse failure.
        unsafe {
            if !os_file_exists(self.path) {
                return false;
            }
            self.data = obs_data_create_from_json_file(self.path);
            self.urls = obs_data_get_obj(self.data, c"urls".as_ptr());
        }
        true
    }
    fn string(&self, key: &str) -> Vec<u8> {
        Self::text(self.data, key)
    }
    fn url(&self, key: &str) -> Vec<u8> {
        Self::text(self.urls, key)
    }
    fn boolean(&self, key: &str) -> bool {
        let key = CString::new(key).expect("manifest keys contain no NUL");
        // SAFETY: reader owns data or NULL; key lives through the call.
        unsafe { obs_data_get_bool(self.data, key.as_ptr()) }
    }
}
impl Drop for Reader {
    fn drop(&mut self) {
        // SAFETY: all handles belong to this reader, and release accepts NULL.
        unsafe {
            obs_data_release(self.urls);
            obs_data_release(self.data);
            bfree(self.path.cast());
        }
    }
}

/// # Safety
/// `module` points to a live non-NULL module whose previous metadata, if any, is caller-owned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_load_metadata(module: *mut c_void) -> c_int {
    assert!(!module.is_null(), "module loader requires a live module");
    // SAFETY: the caller supplies the same non-NULL module required by C.
    let path =
        unsafe { obs_module_names_manifest_path(*obs_module_metadata_data_path_field(module)) };
    let mut reader = Reader {
        path,
        data: ptr::null_mut(),
        urls: ptr::null_mut(),
    };
    let metadata = module_load_metadata::load(&mut reader).map_or(ptr::null_mut(), |metadata| {
        let [
            display_name,
            id,
            version,
            os_arch,
            name,
            description,
            long_description,
            repository_url,
            website_url,
            support_url,
        ] = metadata.strings.map(|value| bstrdup(&value));
        let value = ModuleMetadata {
            display_name,
            version,
            id,
            os_arch,
            description,
            long_description,
            has_icon: metadata.has_icon,
            has_banner: metadata.has_banner,
            repository_url,
            support_url,
            website_url,
            name,
        };
        // SAFETY: bmalloc is aligned and aborts on failure; C owns the allocation and strings.
        unsafe {
            let allocation = bmalloc(size_of::<ModuleMetadata>()).cast::<ModuleMetadata>();
            allocation.write(value);
            allocation
        }
    });
    // SAFETY: adapter returns this live module's metadata pointer slot.
    unsafe {
        *obs_module_load_metadata_slot(module) = metadata;
    }
    0 // MODULE_SUCCESS, including absent/unreadable/malformed manifests.
}
