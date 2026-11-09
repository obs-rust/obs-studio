//! Converts the private C list to the safe ordered-registry interface.
use crate::caption_callbacks::{self, Registry};
use core::ffi::{CStr, c_char, c_int, c_void};
use obs_util::ffi::darray::{darray, darray_erase, darray_push_back};

pub type Callback = Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void)>;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Pair {
    pub callback: Callback,
    pub param: *mut c_void,
}
impl PartialEq for Pair {
    fn eq(&self, other: &Self) -> bool {
        self.callback.map(|f| f as usize) == other.callback.map(|f| f as usize)
            && self.param == other.param
    }
}
impl Eq for Pair {}
unsafe extern "C" {
    fn obs_source_caption_callback_fields(source: *mut c_void) -> *mut darray;
    fn obs_source_caption_callback_lock(source: *mut c_void);
    fn obs_source_caption_callback_unlock(source: *mut c_void);
    fn blog(level: c_int, format: *const c_char, ...);
}
struct Locked {
    source: *mut c_void,
    list: *mut darray,
}
impl Registry for Locked {
    type Pair = Pair;
    fn count(&self) -> usize {
        // SAFETY: the source owns this list and its mutex is held.
        unsafe { (*self.list).num }
    }
    fn pair(&self, index: usize) -> Pair {
        assert!(index < self.count());
        // SAFETY: the index is in the initialized list under its mutex.
        unsafe { *(*self.list).array.cast::<Pair>().add(index) }
    }
    fn append(&mut self, pair: Pair) {
        // SAFETY: exclusive mutex access; darray uses libobs's bmem allocator.
        unsafe { darray_push_back(size_of::<Pair>(), self.list, (&pair as *const Pair).cast()) };
    }
    fn erase(&mut self, index: usize) {
        // SAFETY: the core chooses an initialized index; the mutex is held.
        unsafe { darray_erase(size_of::<Pair>(), self.list, index) };
    }
}
impl Drop for Locked {
    fn drop(&mut self) {
        // SAFETY: construction acquired this live source's caption mutex.
        unsafe { obs_source_caption_callback_unlock(self.source) };
    }
}
fn log(name: &'static CStr) {
    // SAFETY: static NUL-terminated format and argument match %s.
    unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), name.as_ptr()) };
}
unsafe fn update(source: *mut c_void, pair: Pair, add: bool) {
    let mut locked = if source.is_null() {
        None
    } else {
        // SAFETY: caller supplies a live source; reject NULL before any adapter.
        unsafe {
            obs_source_caption_callback_lock(source);
            Some(Locked {
                source,
                list: obs_source_caption_callback_fields(source),
            })
        }
    };
    caption_callbacks::update(locked.as_mut(), pair, add, log);
}
/// # Safety
/// A non-null source must be live; callback and param follow the C API contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_add_caption_callback(
    source: *mut c_void,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: forward the caller's source lifetime contract; NULL is accepted.
    unsafe { update(source, Pair { callback, param }, true) };
}
/// # Safety
/// A non-null source must be live; callback and param follow the C API contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_remove_caption_callback(
    source: *mut c_void,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: forward the caller's source lifetime contract; NULL is accepted.
    unsafe { update(source, Pair { callback, param }, false) };
}
