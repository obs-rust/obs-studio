//! The source layout stays behind the counter address adapters of
//! `obs-source-showing-active-fields.c.h`; the shim loads the C `long`
//! counter atomically itself, as `os_atomic_load_long()` does.
use crate::showing_active::{Counter, nonzero};
use core::ffi::{CStr, c_char, c_int, c_long, c_void};
use core::sync::atomic::{AtomicI32, AtomicI64, Ordering};

unsafe extern "C" {
    fn obs_source_activate_refs_field(source: *const c_void) -> *const c_long;
    fn obs_source_show_refs_field(source: *const c_void) -> *const c_long;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `os_atomic_load_long()`: a sequentially consistent load of a C `long`,
/// 32 bits on LLP64 hosts and 64 bits elsewhere.
///
/// # Safety
/// `value` points at a live, aligned C `long`.
unsafe fn atomic_load_long(value: *const c_long) -> i64 {
    // SAFETY: the caller's pointer is a live aligned C long, which has the
    // size and alignment of the atomic type chosen by its width.
    unsafe {
        if size_of::<c_long>() == size_of::<i32>() {
            i64::from((*value.cast::<AtomicI32>()).load(Ordering::SeqCst))
        } else {
            (*value.cast::<AtomicI64>()).load(Ordering::SeqCst)
        }
    }
}

fn ask(source: *const c_void, counter: Counter) -> bool {
    let load = (!source.is_null()).then_some(|counter: Counter| {
        // SAFETY: NULL was rejected before the private C layout accessor; the
        // adapter returns the address of the live source's counter.
        unsafe {
            atomic_load_long(match counter {
                Counter::Activate => obs_source_activate_refs_field(source),
                Counter::Show => obs_source_show_refs_field(source),
            })
        }
    });
    nonzero(counter, load, |name: &'static CStr| {
        // SAFETY: static strings match the two %s arguments of the format.
        unsafe {
            blog(
                400,
                c"%s: Null '%s' parameter".as_ptr(),
                name.as_ptr(),
                c"source".as_ptr(),
            );
        }
    })
}

/// # Safety
/// A non-null source must be live under the original libobs ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_active(source: *const c_void) -> bool {
    ask(source, Counter::Activate)
}

/// # Safety
/// A non-null source must be live under the original libobs ownership contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_showing(source: *const c_void) -> bool {
    ask(source, Counter::Show)
}
