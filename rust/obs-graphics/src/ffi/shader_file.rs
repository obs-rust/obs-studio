//! ABI conversions for file-backed shaders; all sequencing is in the safe core.
use crate::shader_file::{self, Backend, Failure, Kind};
use std::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn gs_get_context() -> *mut c_void;
    fn os_quick_read_utf8_file(path: *const c_char) -> *mut c_char;
    fn gs_vertexshader_create(
        source: *const c_char,
        path: *const c_char,
        error: *mut *mut c_char,
    ) -> *mut c_void;
    fn gs_pixelshader_create(
        source: *const c_char,
        path: *const c_char,
        error: *mut *mut c_char,
    ) -> *mut c_void;
    fn blog(level: c_int, format: *const c_char, ...);
}
struct FileBackend {
    path: *const c_char,
    error: *mut *mut c_char,
}
impl Backend for FileBackend {
    type Source = *mut c_char;
    type Shader = *mut c_void;
    fn context(&self) -> bool {
        // SAFETY: querying the current thread's graphics context has no preconditions.
        unsafe { !gs_get_context().is_null() }
    }
    fn path_present(&self) -> bool {
        !self.path.is_null()
    }
    fn read(&mut self) -> Option<Self::Source> {
        // SAFETY: the caller supplies a live C path; the core has checked non-NULL.
        let source = unsafe { os_quick_read_utf8_file(self.path) };
        (!source.is_null()).then_some(source)
    }
    fn create(&mut self, kind: Kind, source: &Self::Source) -> Self::Shader {
        // SAFETY: the read allocation is live and NUL terminated, and the original
        // path/error arguments retain their caller-provided lifetime and identity.
        unsafe {
            match kind {
                Kind::Vertex => gs_vertexshader_create(*source, self.path, self.error),
                Kind::Pixel => gs_pixelshader_create(*source, self.path, self.error),
            }
        }
    }
    fn free(&mut self, source: Self::Source) {
        // SAFETY: this is the single release of the reader's bmalloc allocation.
        unsafe { obs_util::ffi::darray::bfree(source.cast()) };
    }
    fn log(&mut self, kind: Kind, failure: Failure) {
        let name = match kind {
            Kind::Vertex => c"gs_vertexshader_create_from_file",
            Kind::Pixel => c"gs_pixelshader_create_from_file",
        };
        // SAFETY: literal formats match their C string arguments. The path is
        // accessed only for a read failure, after validation in the core.
        unsafe {
            match failure {
                Failure::Context => blog(
                    400,
                    c"%s: called while not in a graphics context".as_ptr(),
                    name.as_ptr(),
                ),
                Failure::Path => blog(
                    400,
                    c"%s: Null '%s' parameter".as_ptr(),
                    name.as_ptr(),
                    c"file".as_ptr(),
                ),
                Failure::Read => {
                    let kind = match kind {
                        Kind::Vertex => c"vertex",
                        Kind::Pixel => c"pixel",
                    };
                    blog(
                        100,
                        c"Could not load %s shader file '%s'".as_ptr(),
                        kind.as_ptr(),
                        self.path,
                    );
                }
            }
        }
    }
}

/// # Safety
/// `file` is NULL or a live NUL-terminated path; `error_string` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_vertexshader_create_from_file(
    file: *const c_char,
    error_string: *mut *mut c_char,
) -> *mut c_void {
    shader_file::create(
        &mut FileBackend {
            path: file,
            error: error_string,
        },
        Kind::Vertex,
    )
}

/// # Safety
/// Same pointer requirements as `gs_vertexshader_create_from_file`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_pixelshader_create_from_file(
    file: *const c_char,
    error_string: *mut *mut c_char,
) -> *mut c_void {
    shader_file::create(
        &mut FileBackend {
            path: file,
            error: error_string,
        },
        Kind::Pixel,
    )
}
