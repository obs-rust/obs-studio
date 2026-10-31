//! C ABI for `libobs/obs-view-api.c.h` (#1933).
//!
//! The nine `EXPORT` functions of `obs.h` are exported
//! (`libobs/cmake/rust-exports.d/obs-core-view.txt`); `obs_view_init` and
//! `obs_view_free`, which `obs-canvas.c` calls on the view embedded in each
//! canvas, are declared in `obs-internal.h` without `EXPORT`, so their shims
//! stay hidden (`libobs/cmake/rust-hidden.d`).
//!
//! `struct obs_view` stays C. [`LibobsHost`] reaches it, a source's removed
//! flag and the video mixes list only through the hidden accessors in
//! `obs-view.c` (`obs_view_lock()`, `obs_view_channel()`, `obs_view_mix_*()`,
//! ...), and the sources through the `obs_source_*` calls the C makes. Cargo
//! tests link stand-ins for all of them (`obs-c-oracle/oracle/view.c`).

use core::ffi::{c_char, c_int, c_void};
use core::ptr;

use obs_util::ffi::bmem::{bfree, bmalloc};

use crate::view::{self, Handle, Host};

/// `LOG_ERROR` in `util/base.h`.
const LOG_ERROR: c_int = 100;

/// `bool (*enum_proc)(void *, struct obs_video_info *)`.
pub type EnumVideoInfoProc =
    Option<unsafe extern "C" fn(param: *mut c_void, ovi: *mut c_void) -> bool>;

unsafe extern "C" {
    fn blog(level: c_int, format: *const c_char, ...);

    fn obs_view_struct_size() -> usize;
    fn obs_view_mutex_init(view: *mut c_void) -> bool;
    fn obs_view_mutex_destroy(view: *mut c_void);
    fn obs_view_lock(view: *mut c_void);
    fn obs_view_unlock(view: *mut c_void);
    fn obs_view_get_type(view: *const c_void) -> u32;
    fn obs_view_set_type(view: *mut c_void, view_type: u32);
    fn obs_view_channel(view: *const c_void, channel: usize) -> *mut c_void;
    fn obs_view_set_channel(view: *mut c_void, channel: usize, source: *mut c_void);
    fn obs_view_clear_channels(view: *mut c_void);
    fn obs_view_source_removed(source: *const c_void) -> bool;
    fn obs_view_main_mix_info() -> *mut c_void;
    fn obs_view_mix_add(view: *mut c_void, ovi: *mut c_void) -> *mut c_void;
    fn obs_view_mixes_lock();
    fn obs_view_mixes_unlock();
    fn obs_view_mix_count() -> usize;
    fn obs_view_mix_view(i: usize) -> *mut c_void;
    fn obs_view_mix_clear_view(i: usize);
    fn obs_view_mix_info(i: usize) -> *mut c_void;

    fn obs_source_get_ref(source: *mut c_void) -> *mut c_void;
    fn obs_source_release(source: *mut c_void);
    fn obs_source_activate(source: *mut c_void, view_type: u32);
    fn obs_source_deactivate(source: *mut c_void, view_type: u32);
    fn obs_source_video_render(source: *mut c_void);
}

fn p(h: Handle) -> *mut c_void {
    ptr::with_exposed_provenance_mut(h)
}

fn h(p: *const c_void) -> Handle {
    p.expose_provenance()
}

/// libobs, for the view core: each method is one C call.
struct LibobsHost;

// SAFETY (every method): each passes the C's own pointers back to the C
// call that the original made at the same point, with the same arguments.
#[allow(clippy::undocumented_unsafe_blocks)]
impl Host for LibobsHost {
    fn alloc_view(&self) -> Handle {
        unsafe {
            let size = obs_view_struct_size();
            let view = bmalloc(size);
            ptr::write_bytes(view.cast::<u8>(), 0, size);
            h(view)
        }
    }
    fn free_view(&self, view: Handle) {
        unsafe { bfree(p(view)) }
    }
    fn mutex_init(&self, view: Handle) -> bool {
        unsafe { obs_view_mutex_init(p(view)) }
    }
    fn mutex_destroy(&self, view: Handle) {
        unsafe { obs_view_mutex_destroy(p(view)) }
    }
    fn lock(&self, view: Handle) {
        unsafe { obs_view_lock(p(view)) }
    }
    fn unlock(&self, view: Handle) {
        unsafe { obs_view_unlock(p(view)) }
    }
    fn view_type(&self, view: Handle) -> u32 {
        unsafe { obs_view_get_type(p(view)) }
    }
    fn set_view_type(&self, view: Handle, view_type: u32) {
        unsafe { obs_view_set_type(p(view), view_type) }
    }
    fn channel(&self, view: Handle, channel: usize) -> Handle {
        unsafe { h(obs_view_channel(p(view), channel)) }
    }
    fn set_channel(&self, view: Handle, channel: usize, source: Handle) {
        unsafe { obs_view_set_channel(p(view), channel, p(source)) }
    }
    fn clear_channels(&self, view: Handle) {
        unsafe { obs_view_clear_channels(p(view)) }
    }
    fn log_mutex_failed(&self) {
        unsafe { blog(LOG_ERROR, c"obs_view_init: Failed to create mutex".as_ptr()) }
    }
    fn source_get_ref(&self, source: Handle) -> Handle {
        unsafe { h(obs_source_get_ref(p(source))) }
    }
    fn source_release(&self, source: Handle) {
        unsafe { obs_source_release(p(source)) }
    }
    fn source_activate(&self, source: Handle, view_type: u32) {
        unsafe { obs_source_activate(p(source), view_type) }
    }
    fn source_deactivate(&self, source: Handle, view_type: u32) {
        unsafe { obs_source_deactivate(p(source), view_type) }
    }
    fn source_video_render(&self, source: Handle) {
        unsafe { obs_source_video_render(p(source)) }
    }
    fn source_removed(&self, source: Handle) -> bool {
        unsafe { obs_view_source_removed(p(source)) }
    }
    fn main_mix_info(&self) -> Handle {
        unsafe { h(obs_view_main_mix_info()) }
    }
    fn mix_add(&self, view: Handle, ovi: Handle) -> Handle {
        unsafe { h(obs_view_mix_add(p(view), p(ovi))) }
    }
    fn mixes_lock(&self) {
        unsafe { obs_view_mixes_lock() }
    }
    fn mixes_unlock(&self) {
        unsafe { obs_view_mixes_unlock() }
    }
    fn mix_count(&self) -> usize {
        unsafe { obs_view_mix_count() }
    }
    fn mix_view(&self, index: usize) -> Handle {
        unsafe { h(obs_view_mix_view(index)) }
    }
    fn mix_clear_view(&self, index: usize) {
        unsafe { obs_view_mix_clear_view(index) }
    }
    fn mix_info(&self, index: usize) -> Handle {
        unsafe { h(obs_view_mix_info(index)) }
    }
}

/// `obs_view_init` (hidden).
///
/// # Safety
/// `view` is NULL or a `struct obs_view` the caller owns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_view_init(view: *mut c_void, view_type: u32) -> bool {
    view::init(&LibobsHost, h(view), view_type)
}

/// `obs_view_create`.
///
/// # Safety
/// libobs is started.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_view_create() -> *mut c_void {
    p(view::create(&LibobsHost))
}

/// `obs_view_free` (hidden).
///
/// # Safety
/// `view` is NULL or an initialised view.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_view_free(view: *mut c_void) {
    view::free(&LibobsHost, h(view));
}

/// `obs_view_destroy`.
///
/// # Safety
/// `view` is NULL or a view from `obs_view_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_view_destroy(view: *mut c_void) {
    view::destroy(&LibobsHost, h(view));
}

/// `obs_view_get_source`.
///
/// # Safety
/// `view` is NULL or a live view.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_view_get_source(view: *mut c_void, channel: u32) -> *mut c_void {
    p(view::get_source(&LibobsHost, h(view), channel))
}

/// `obs_view_set_source`.
///
/// # Safety
/// `view` is NULL or a live view; `source` is NULL or a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_view_set_source(view: *mut c_void, channel: u32, source: *mut c_void) {
    view::set_source(&LibobsHost, h(view), channel, h(source));
}

/// `obs_view_render`.
///
/// # Safety
/// `view` is NULL or a live view; called on the graphics thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_view_render(view: *mut c_void) {
    view::render(&LibobsHost, h(view));
}

/// `obs_view_add`.
///
/// # Safety
/// libobs is started; `view` is NULL or a live view.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_view_add(view: *mut c_void) -> *mut c_void {
    p(view::add(&LibobsHost, h(view)))
}

/// `obs_view_add2`.
///
/// # Safety
/// `view` and `ovi` are NULL or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_view_add2(view: *mut c_void, ovi: *mut c_void) -> *mut c_void {
    p(view::add2(&LibobsHost, h(view), h(ovi)))
}

/// `obs_view_remove`.
///
/// # Safety
/// libobs is started.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_view_remove(view: *mut c_void) {
    view::remove(&LibobsHost, h(view));
}

/// `obs_view_enum_video_info`. `enum_proc` is called for each of the view's
/// mixes; as in the C, it must not be NULL when the view has a mix.
///
/// # Safety
/// libobs is started; `enum_proc` is callable with `param`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_view_enum_video_info(
    view: *mut c_void,
    enum_proc: EnumVideoInfoProc,
    param: *mut c_void,
) {
    view::enum_video_info(&LibobsHost, h(view), |ovi| {
        let f = enum_proc.expect("obs_view_enum_video_info: NULL enum_proc");
        // SAFETY: the caller's callback with its own param, as the C calls it.
        unsafe { f(param, p(ovi)) }
    });
}
