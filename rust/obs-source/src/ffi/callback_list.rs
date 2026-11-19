//! A source's private C callback darray, held under its mutex, as the safe
//! ordered-registry interface. The darray and its bmem storage stay C-owned.
use crate::callback_registry::Registry;
use core::ffi::{CStr, c_char, c_int, c_void};
use core::marker::PhantomData;
use obs_util::ffi::darray::{darray, darray_erase, darray_push_back};

unsafe extern "C" {
    fn blog(level: c_int, format: *const c_char, ...);
}

/// The C adapters of one list: its address and its mutex operations.
pub(crate) struct Adapters {
    pub fields: unsafe extern "C" fn(*mut c_void) -> *mut darray,
    pub lock: unsafe extern "C" fn(*mut c_void),
    pub unlock: unsafe extern "C" fn(*mut c_void),
}

pub(crate) struct Locked<P> {
    source: *mut c_void,
    list: *mut darray,
    unlock: unsafe extern "C" fn(*mut c_void),
    pair: PhantomData<P>,
}

impl<P: Copy + Eq> Registry for Locked<P> {
    type Pair = P;
    fn count(&self) -> usize {
        // SAFETY: the source owns this list and its mutex is held.
        unsafe { (*self.list).num }
    }
    fn pair(&self, index: usize) -> P {
        assert!(index < self.count());
        // SAFETY: the index is in the initialized list under its mutex; P
        // mirrors the C element layout (checked against the real headers).
        unsafe { *(*self.list).array.cast::<P>().add(index) }
    }
    fn append(&mut self, pair: P) {
        // SAFETY: exclusive mutex access; darray uses libobs's bmem allocator.
        unsafe { darray_push_back(size_of::<P>(), self.list, (&pair as *const P).cast()) };
    }
    fn erase(&mut self, index: usize) {
        // SAFETY: the core chooses an initialized index; the mutex is held.
        unsafe { darray_erase(size_of::<P>(), self.list, index) };
    }
}

impl<P> Drop for Locked<P> {
    fn drop(&mut self) {
        // SAFETY: construction acquired this live source's list mutex.
        unsafe { (self.unlock)(self.source) };
    }
}

pub(crate) fn log(name: &'static CStr) {
    // SAFETY: static NUL-terminated format and argument match %s.
    unsafe { blog(400, c"%s: Null 'source' parameter".as_ptr(), name.as_ptr()) };
}

/// Rejects NULL before any adapter, else locks the source's list.
///
/// # Safety
/// A non-null source must be live, and `adapters` must name its list.
pub(crate) unsafe fn lock<P>(source: *mut c_void, adapters: &Adapters) -> Option<Locked<P>> {
    if source.is_null() {
        return None;
    }
    // SAFETY: the caller supplies a live source for these adapters.
    unsafe {
        (adapters.lock)(source);
        Some(Locked {
            source,
            list: (adapters.fields)(source),
            unlock: adapters.unlock,
            pair: PhantomData,
        })
    }
}
