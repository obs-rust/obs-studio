//! Writable access through the existing real-header transition field adapter.
use super::item_transition_getters::Fields;
use crate::item_transition_setters::Item;
use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

unsafe extern "C" {
    fn obs_scene_item_transition_fields(item: *const c_void) -> Fields;
    fn obs_source_release(source: *mut c_void);
    fn obs_source_get_ref(source: *mut c_void) -> *mut c_void;
}

/// Source lifetime calls, injectable for differential tests.
pub trait Calls {
    /// # Safety
    /// `source` is a live reference owned by the selected item slot.
    unsafe fn release(source: *mut c_void);
    /// # Safety
    /// `source` satisfies the original obs_source_get_ref contract (including NULL).
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void;
}

/// Calls supplied by hybrid libobs.
pub enum Libobs {}
impl Calls for Libobs {
    unsafe fn release(source: *mut c_void) {
        // SAFETY: forwarded from the caller's source lifetime contract.
        unsafe { obs_source_release(source) }
    }
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: forwarded from the caller's source lifetime contract.
        unsafe { obs_source_get_ref(source) }
    }
}

struct BorrowedItem<C> {
    fields: Fields,
    calls: PhantomData<C>,
}
impl<C: Calls> Item for BorrowedItem<C> {
    type Source = NonNull<c_void>;
    fn transition(&self, show: bool) -> Option<Self::Source> {
        let field = if show {
            self.fields.show_transition
        } else {
            self.fields.hide_transition
        };
        // SAFETY: constructed only for a live item; read only the selected slot.
        NonNull::new(unsafe { *field })
    }
    fn release(&mut self, source: Self::Source) {
        // SAFETY: the selected slot owns this reference, just as in C.
        unsafe { C::release(source.as_ptr()) }
    }
    fn get_ref(&mut self, source: Option<Self::Source>) -> Option<Self::Source> {
        // SAFETY: the original source contract is required by set_transition_with.
        NonNull::new(unsafe { C::get_ref(source.map_or(core::ptr::null_mut(), NonNull::as_ptr)) })
    }
    fn store_transition(&mut self, show: bool, source: Option<Self::Source>) {
        let field = if show {
            self.fields.show_transition
        } else {
            self.fields.hide_transition
        };
        // SAFETY: the real adapter addresses writable fields of the live mutable
        // item. No Rust field reference is held across release/get_ref callbacks.
        unsafe { *field.cast_mut() = source.map_or(core::ptr::null_mut(), NonNull::as_ptr) };
    }
    fn store_duration(&mut self, show: bool, duration: u32) {
        let field = if show {
            self.fields.show_duration
        } else {
            self.fields.hide_duration
        };
        // SAFETY: the adapter addresses a writable uint32_t in the live item.
        unsafe { *field.cast_mut() = duration };
    }
}

unsafe fn item<C>(item: *mut c_void) -> Option<BorrowedItem<C>> {
    NonNull::new(item).map(|item| {
        // SAFETY: NULL is excluded before forming addresses; caller supplies a
        // live mutable item under the original C synchronization contract.
        let fields = unsafe { obs_scene_item_transition_fields(item.as_ptr()) };
        BorrowedItem {
            fields,
            calls: PhantomData,
        }
    })
}

/// # Safety
/// Non-NULL item and source obey the original setter's lifetime and external
/// synchronization contract; C implements those same source reference operations.
pub unsafe fn set_transition_with<C: Calls>(
    item_ptr: *mut c_void,
    show: bool,
    source: *mut c_void,
) {
    // SAFETY: caller supplies the original mutable-item contract.
    let mut item = unsafe { item::<C>(item_ptr) };
    crate::item_transition_setters::set_transition(item.as_mut(), show, NonNull::new(source));
}

/// # Safety
/// Non-NULL item and source obey the original setter's lifetime and external
/// synchronization contract, including source validity across release/get_ref.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_transition(
    item: *mut c_void,
    show: bool,
    source: *mut c_void,
) {
    // SAFETY: caller supplies the C setter contract; production uses libobs calls.
    unsafe { set_transition_with::<Libobs>(item, show, source) }
}

/// # Safety
/// A non-NULL item is live and writable under the original C setter contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_sceneitem_set_transition_duration(
    item_ptr: *mut c_void,
    show: bool,
    duration: u32,
) {
    // SAFETY: caller supplies the original mutable-item contract.
    let mut item = unsafe { item::<Libobs>(item_ptr) };
    crate::item_transition_setters::set_duration(item.as_mut(), show, duration);
}
