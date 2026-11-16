//! Shutdown's detachment of scene and group items
//! (`libobs/obs-shutdown-detach.c.h`: `obs_shutdown_detach_scene_items()`
//! and its item callback `obs_shutdown_remove_scene_item()`).
//!
//! `obs_free_data()` calls it once, after the destruction queue has drained
//! and before it walks the source tables, so that a scene the host left
//! alive releases what its items hold. A scene can release the last
//! reference of another member of the table, so the C:
//!
//! 1. takes `sources_mutex` and, walking the UUID table, takes a strong
//!    reference to every source it can (one whose reference fails is
//!    skipped), then releases the mutex;
//! 2. for every source of that snapshot, in table order, that is a scene
//!    (or else a group), removes each of its items, with the mutex released;
//! 3. only then releases every snapshot reference, so no member is freed
//!    while another scene still lists it;
//! 4. waits on the destruction queue until it reports no more work, so the
//!    destroys those releases queued finish before the tables are walked.
//!
//! The item callback always continues the enumeration. The drain has no
//! bound, as in the C. This keeps the same order of calls.

/// What the detachment reaches of the core: the source table and its
/// mutex, the source and scene calls and the destruction queue. Every call
/// takes `&self`, so a snapshot reference can be taken while the table is
/// being walked.
pub trait Core {
    /// An `obs_source_t *`.
    type Source: Copy;
    /// An `obs_scene_t *` (a scene or a group).
    type Scene: Copy;
    /// An `obs_sceneitem_t *`.
    type Item: Copy;
    /// Take `sources_mutex`.
    fn lock(&self);
    /// Release `sources_mutex`.
    fn unlock(&self);
    /// Hand every member of the UUID source table to `visit`, in table
    /// order, under the mutex.
    fn each_source(&self, visit: &mut dyn FnMut(Self::Source));
    /// `obs_source_get_ref()`: a strong reference, or `None` when the
    /// source is already being destroyed.
    fn get_ref(&self, source: Self::Source) -> Option<Self::Source>;
    /// `obs_scene_from_source()`.
    fn scene_from_source(&self, source: Self::Source) -> Option<Self::Scene>;
    /// `obs_group_from_source()`.
    fn group_from_source(&self, source: Self::Source) -> Option<Self::Scene>;
    /// `obs_scene_enum_items()`: hand each item to `visit` while it
    /// returns true.
    fn enum_items(&self, scene: Self::Scene, visit: &mut dyn FnMut(Self::Item) -> bool);
    /// `obs_sceneitem_remove()`.
    fn item_remove(&self, item: Self::Item);
    /// `obs_source_release()`.
    fn release(&self, source: Self::Source);
    /// `os_task_queue_wait(obs->destruction_task_thread)`: whether the
    /// queue had work to wait for.
    fn wait_destruction(&self) -> bool;
}

/// `obs_shutdown_remove_scene_item()`: remove the item and continue.
pub fn remove_scene_item<C: Core>(core: &C, item: C::Item) -> bool {
    core.item_remove(item);
    true
}

/// `obs_shutdown_detach_scene_items()`.
pub fn detach_scene_items(core: &impl Core) {
    let mut sources = Vec::new();
    core.lock();
    core.each_source(&mut |member| {
        if let Some(source) = core.get_ref(member) {
            sources.push(source);
        }
    });
    core.unlock();

    for &source in &sources {
        let scene = core
            .scene_from_source(source)
            .or_else(|| core.group_from_source(source));
        if let Some(scene) = scene {
            core.enum_items(scene, &mut |item| remove_scene_item(core, item));
        }
    }
    for &source in &sources {
        core.release(source);
    }
    drop(sources);

    while core.wait_destruction() {}
}
