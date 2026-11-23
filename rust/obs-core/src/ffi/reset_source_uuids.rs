//! `obs_reset_source_uuids()` (#5520) over C-owned state: the core's uuid
//! table, its mutex and links, the uthash unlink and add, the uuid free and
//! generation and the head store, behind the private adapters of
//! `libobs/obs-reset-source-uuids-fields.c.h`. The walk is the safe core's
//! ([`crate::reset_source_uuids`]); the working table's head and the new
//! table's head are this walk's own locals, as in the original, and are
//! handed to the adapters by address.
use core::ffi::c_void;
use core::ptr::{self, NonNull};

use crate::reset_source_uuids::{Sources, reset_source_uuids};

/// `struct obs_context_data`, opaque here.
pub type Context = c_void;

unsafe extern "C" {
    fn obs_reset_source_uuids_lock();
    fn obs_reset_source_uuids_unlock();
    fn obs_reset_source_uuids_head() -> *mut Context;
    fn obs_reset_source_uuids_next(ctx: *const Context) -> *mut Context;
    fn obs_reset_source_uuids_delete(table: *mut *mut Context, ctx: *mut Context);
    fn obs_reset_source_uuids_free_uuid(ctx: *mut Context);
    fn obs_reset_source_uuids_new_uuid(ctx: *mut Context);
    fn obs_reset_source_uuids_add(table: *mut *mut Context, ctx: *mut Context);
    fn obs_reset_source_uuids_store(table: *mut Context);
}

/// A table head local, as the C `struct obs_context_data *` it is.
fn head_ptr(table: &mut Option<NonNull<Context>>) -> *mut *mut Context {
    // `Option<NonNull<T>>` has the layout of a nullable `*mut T`.
    ptr::from_mut(table).cast::<*mut Context>()
}

/// The core's uuid table.
struct CoreSources;

impl Sources for CoreSources {
    type Entry = NonNull<Context>;
    fn lock(&mut self) {
        // SAFETY: the initialized core's sources mutex.
        unsafe { obs_reset_source_uuids_lock() }
    }
    fn unlock(&mut self) {
        // SAFETY: paired with this walk's own `lock`.
        unsafe { obs_reset_source_uuids_unlock() }
    }
    fn head(&mut self) -> Option<NonNull<Context>> {
        // SAFETY: the table head, read under the sources mutex.
        NonNull::new(unsafe { obs_reset_source_uuids_head() })
    }
    fn next(&mut self, entry: NonNull<Context>) -> Option<NonNull<Context>> {
        // SAFETY: a listed context, live while the sources mutex is held.
        NonNull::new(unsafe { obs_reset_source_uuids_next(entry.as_ptr()) })
    }
    fn delete(&mut self, table: &mut Option<NonNull<Context>>, entry: NonNull<Context>) {
        // SAFETY: `table` is this walk's working table head and `entry` is
        // listed in it, under the sources mutex.
        unsafe { obs_reset_source_uuids_delete(head_ptr(table), entry.as_ptr()) }
    }
    fn free_uuid(&mut self, entry: NonNull<Context>) {
        // SAFETY: a context just unlinked by this walk; its uuid is the
        // table's allocation, freed once and replaced by `new_uuid`.
        unsafe { obs_reset_source_uuids_free_uuid(entry.as_ptr()) }
    }
    fn new_uuid(&mut self, entry: NonNull<Context>) {
        // SAFETY: a context unlinked by this walk, its old uuid freed.
        unsafe { obs_reset_source_uuids_new_uuid(entry.as_ptr()) }
    }
    fn add(&mut self, table: &mut Option<NonNull<Context>>, entry: NonNull<Context>) {
        // SAFETY: `table` is this walk's new table head; `entry` is in no
        // table and holds its new uuid.
        unsafe { obs_reset_source_uuids_add(head_ptr(table), entry.as_ptr()) }
    }
    fn store(&mut self, table: Option<NonNull<Context>>) {
        // SAFETY: the new table, every context of the old one moved into
        // it, stored under the sources mutex.
        unsafe { obs_reset_source_uuids_store(table.map_or(ptr::null_mut(), NonNull::as_ptr)) }
    }
}

/// Gives every source of the core's uuid table a new uuid, keeping the
/// table order.
///
/// # Safety
/// As for the C function: the core is initialized.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_reset_source_uuids() {
    reset_source_uuids(&mut CoreSources);
}
