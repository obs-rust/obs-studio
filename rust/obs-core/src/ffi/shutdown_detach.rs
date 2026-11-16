//! C ABI for `libobs/obs-shutdown-detach.c.h`, shutdown's detachment of
//! scene and group items, declared in `libobs/obs-shutdown-detach.h` without
//! `EXPORT` (hidden, `rust-hidden.d`).
//!
//! The C bridge passes the core's `struct obs_core_data` and a table of
//! calls: the lock and unlock of `sources_mutex`, a walk of the UUID source
//! table (the `HASH_ITER` stays in C), the libobs source and scene calls and
//! the destruction queue wait. Nothing here reads `struct obs_core_data` or
//! names a libobs symbol. No Rust borrow is held across a call that can call
//! back.
use crate::shutdown_detach::{self, Core};
use core::ffi::c_void;

/// `struct obs_core_data *`, opaque here.
pub type CoreData = c_void;
/// `obs_source_t *`, opaque here.
pub type Source = c_void;
/// `obs_scene_t *`, opaque here.
pub type Scene = c_void;
/// `obs_sceneitem_t *`, opaque here.
pub type Item = c_void;

/// The table walk's visitor: `void (*)(void *param, obs_source_t *)`.
pub type Visit = unsafe extern "C" fn(*mut c_void, *mut Source);
/// `obs_scene_enum_items()`'s callback.
pub type ItemCallback = unsafe extern "C" fn(*mut Scene, *mut Item, *mut c_void) -> bool;

/// `struct obs_shutdown_detach_calls`; the bridge sets every member.
#[repr(C)]
pub struct Calls {
    pub lock: unsafe extern "C" fn(*mut CoreData),
    pub unlock: unsafe extern "C" fn(*mut CoreData),
    pub each_source: unsafe extern "C" fn(*mut CoreData, Visit, *mut c_void),
    pub get_ref: unsafe extern "C" fn(*mut Source) -> *mut Source,
    pub scene_from_source: unsafe extern "C" fn(*const Source) -> *mut Scene,
    pub group_from_source: unsafe extern "C" fn(*const Source) -> *mut Scene,
    pub enum_items: unsafe extern "C" fn(*mut Scene, ItemCallback, *mut c_void),
    pub item_remove: unsafe extern "C" fn(*mut Item),
    pub release: unsafe extern "C" fn(*mut Source),
    pub wait_destruction: unsafe extern "C" fn() -> bool,
}

struct Bridge<'a> {
    data: *mut CoreData,
    calls: &'a Calls,
}

type SourceVisitor<'v> = &'v mut dyn FnMut(*mut Source);
type ItemVisitor<'v> = &'v mut dyn FnMut(*mut Item) -> bool;

unsafe extern "C" fn visit_source(param: *mut c_void, source: *mut Source) {
    // SAFETY: param is the `&mut SourceVisitor` each_source passed, live
    // for the walk, which calls back on this thread only.
    let visit = unsafe { &mut *param.cast::<SourceVisitor<'_>>() };
    visit(source);
}

unsafe extern "C" fn visit_item(scene: *mut Scene, item: *mut Item, param: *mut c_void) -> bool {
    let _ = scene;
    // SAFETY: param is the `&mut ItemVisitor` enum_items passed, live for
    // the enumeration, which calls back on this thread only.
    let visit = unsafe { &mut *param.cast::<ItemVisitor<'_>>() };
    visit(item)
}

/// A null pointer as `None`.
fn non_null(ptr: *mut c_void) -> Option<*mut c_void> {
    (!ptr.is_null()).then_some(ptr)
}

impl Core for Bridge<'_> {
    type Source = *mut Source;
    type Scene = *mut Scene;
    type Item = *mut Item;
    fn lock(&self) {
        // SAFETY: the bridge's call locks this live core's sources_mutex.
        unsafe { (self.calls.lock)(self.data) }
    }
    fn unlock(&self) {
        // SAFETY: the detachment calls this only after its own lock().
        unsafe { (self.calls.unlock)(self.data) }
    }
    fn each_source(&self, visit: &mut dyn FnMut(*mut Source)) {
        let mut visit: SourceVisitor<'_> = visit;
        // SAFETY: under sources_mutex; visit_source gets back `visit`,
        // which outlives the walk.
        unsafe {
            (self.calls.each_source)(self.data, visit_source, (&raw mut visit).cast::<c_void>())
        }
    }
    fn get_ref(&self, source: *mut Source) -> Option<*mut Source> {
        // SAFETY: a member of the table, under its mutex, so not yet freed.
        non_null(unsafe { (self.calls.get_ref)(source) })
    }
    fn scene_from_source(&self, source: *mut Source) -> Option<*mut Scene> {
        // SAFETY: a source the snapshot holds a reference to.
        non_null(unsafe { (self.calls.scene_from_source)(source) })
    }
    fn group_from_source(&self, source: *mut Source) -> Option<*mut Scene> {
        // SAFETY: a source the snapshot holds a reference to.
        non_null(unsafe { (self.calls.group_from_source)(source) })
    }
    fn enum_items(&self, scene: *mut Scene, visit: &mut dyn FnMut(*mut Item) -> bool) {
        let mut visit: ItemVisitor<'_> = visit;
        // SAFETY: the scene of a source the snapshot holds; visit_item gets
        // back `visit`, which outlives the enumeration.
        unsafe { (self.calls.enum_items)(scene, visit_item, (&raw mut visit).cast::<c_void>()) }
    }
    fn item_remove(&self, item: *mut Item) {
        // SAFETY: an item obs_scene_enum_items handed out, live for the
        // callback.
        unsafe { (self.calls.item_remove)(item) }
    }
    fn release(&self, source: *mut Source) {
        // SAFETY: the reference get_ref took for the snapshot, released once.
        unsafe { (self.calls.release)(source) }
    }
    fn wait_destruction(&self) -> bool {
        // SAFETY: the bridge's call waits on the live core's queue.
        unsafe { (self.calls.wait_destruction)() }
    }
}

/// Detach every scene and group item at shutdown, then wait until the
/// destruction queue is quiet.
///
/// # Safety
/// `data` is the live core's `struct obs_core_data` and `calls` points to
/// the bridge's calls (or ones that behave the same) for it, every member
/// set. None of them may unwind.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_shutdown_detach_scene_items_run(
    data: *mut CoreData,
    calls: *const Calls,
) {
    // SAFETY: the caller passes the bridge's static, fully set calls.
    let calls = unsafe { &*calls };
    shutdown_detach::detach_scene_items(&Bridge { data, calls });
}
