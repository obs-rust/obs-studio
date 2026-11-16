//! C ABI for `libobs/obs-scene-item-audio-actions.c.h`, the scene item's
//! side of the visibility audio ramp: `apply_scene_item_audio_actions()`,
//! `apply_scene_item_volume()` and `process_all_audio_actions()`, declared
//! in `obs-internal.h` without `EXPORT`, so they stay hidden
//! (`rust-hidden.d`). `obs-scene.c.h` calls them from the video tick and
//! `scene_audio_render()`.
//!
//! The item is opaque here. Every field goes through the live adapter
//! `libobs/obs-scene-item-audio-actions-fields.c.h`, one C function per
//! operation the C made: the `actions_mutex` lock and unlock, the address
//! of the queue, the atomic load and store of `visible`, the atomic
//! decrement of `active_refs`, and `item->parent->source` and
//! `item->source`, read only for the removal, after the unlock, as the C
//! reads them. The queue is applied by the Rust core's own shim,
//! [`obs_scene_audio_actions_apply`], on the item's darray in place, and
//! the window by [`crate::audio_actions::action_due`].
//!
//! The removal of the active child is `obs_source_remove_active_child()`
//! in the exports; the `*_with` functions take it as a parameter so the
//! parity test can record it.
//!
//! Intentional difference from C, where C has undefined behaviour and
//! libobs never gets there: a null item does nothing (and
//! `apply_scene_item_volume()` returns false); the C dereferences it.

use core::ffi::{c_long, c_void};
use core::ptr::{self, NonNull};

use crate::audio_actions::{Gains, ItemAction};
use crate::ffi::scene_audio_actions::{
    obs_scene_audio_actions_apply, obs_scene_item_action_array_t,
};
use crate::item_audio_actions::{self as item_side, ItemAudio};

unsafe extern "C" {
    fn obs_sceneitem_audio_actions_lock(item: *mut c_void);
    fn obs_sceneitem_audio_actions_unlock(item: *mut c_void);
    fn obs_sceneitem_audio_actions_queue(item: *mut c_void) -> *mut obs_scene_item_action_array_t;
    fn obs_sceneitem_audio_actions_visible(item: *mut c_void) -> bool;
    fn obs_sceneitem_audio_actions_set_visible(item: *mut c_void, visible: bool);
    fn obs_sceneitem_audio_actions_release(item: *mut c_void) -> c_long;
    fn obs_sceneitem_audio_actions_parent_source(item: *mut c_void) -> *mut c_void;
    fn obs_sceneitem_audio_actions_source(item: *mut c_void) -> *mut c_void;
    fn obs_source_remove_active_child(parent: *mut c_void, child: *mut c_void);
}

/// `obs_source_remove_active_child()`'s signature.
pub type RemoveActiveChild = unsafe extern "C" fn(parent: *mut c_void, child: *mut c_void);

/// A live `struct obs_scene_item`, through the field adapter.
struct Item {
    item: NonNull<c_void>,
    remove: RemoveActiveChild,
}

impl Item {
    fn ptr(&self) -> *mut c_void {
        self.item.as_ptr()
    }
}

impl ItemAudio for Item {
    fn lock(&self) {
        // SAFETY: a live item (the caller's contract); the adapter locks
        // its actions_mutex.
        unsafe { obs_sceneitem_audio_actions_lock(self.ptr()) }
    }

    fn unlock(&self) {
        // SAFETY: as `lock`; this thread holds the mutex.
        unsafe { obs_sceneitem_audio_actions_unlock(self.ptr()) }
    }

    fn visible(&self) -> bool {
        // SAFETY: a live item; one atomic load.
        unsafe { obs_sceneitem_audio_actions_visible(self.ptr()) }
    }

    fn set_visible(&self, visible: bool) {
        // SAFETY: a live item; one atomic store.
        unsafe { obs_sceneitem_audio_actions_set_visible(self.ptr(), visible) }
    }

    fn first_action(&self) -> Option<ItemAction> {
        // SAFETY: a live item, so its queue is a valid darray, and the lock
        // is held, so nobody changes it; `array[0]` exists when `num > 0`.
        unsafe {
            let queue = &*obs_sceneitem_audio_actions_queue(self.ptr());
            if queue.num > 0 {
                Some(ItemAction::from(*queue.array))
            } else {
                None
            }
        }
    }

    fn apply_queue(
        &self,
        visible: &mut bool,
        ts: u64,
        sample_rate: usize,
        buf: Option<&mut Gains>,
    ) -> usize {
        let buf = buf.map_or(ptr::null_mut(), |gains| gains.as_mut_ptr());
        // SAFETY: the queue is the live item's darray and the lock is held;
        // `visible` is a local and `buf` null or a whole tick of gains.
        unsafe {
            obs_scene_audio_actions_apply(
                obs_sceneitem_audio_actions_queue(self.ptr()),
                visible,
                ts,
                sample_rate,
                buf,
            )
        }
    }

    fn release_active_ref(&self) -> bool {
        // SAFETY: a live item; one atomic decrement, whose own result is
        // compared, as the C does.
        unsafe { obs_sceneitem_audio_actions_release(self.ptr()) == 0 }
    }

    fn remove_active_child(&self) {
        // SAFETY: a live item in a live scene: the adapter reads
        // item->parent->source and item->source, and the removal takes them
        // as obs_source_remove_active_child() does.
        unsafe {
            (self.remove)(
                obs_sceneitem_audio_actions_parent_source(self.ptr()),
                obs_sceneitem_audio_actions_source(self.ptr()),
            )
        }
    }
}

/// `buf` as a tick of gains.
///
/// # Safety
///
/// `buf` is null or points to `AUDIO_OUTPUT_FRAMES` writable floats that
/// nothing else uses for the call.
unsafe fn gains<'a>(buf: *mut f32) -> Option<&'a mut Gains> {
    // SAFETY: the caller's contract.
    unsafe { buf.cast::<Gains>().as_mut() }
}

/// [`apply_scene_item_audio_actions`] with the removal passed in.
///
/// # Safety
///
/// `item` is null or a live `struct obs_scene_item` whose parent scene is
/// live; `buf` is null or holds `AUDIO_OUTPUT_FRAMES` writable floats, used
/// by nothing else for the call; `remove` may be called with the item's
/// parent source and source.
pub unsafe fn apply_scene_item_audio_actions_with(
    item: *mut c_void,
    buf: *mut f32,
    ts: u64,
    sample_rate: usize,
    remove: RemoveActiveChild,
) {
    let Some(item) = NonNull::new(item) else {
        return;
    };
    // SAFETY: the caller's contract.
    let buf = unsafe { gains(buf) };
    item_side::apply_scene_item_audio_actions(&Item { item, remove }, buf, ts, sample_rate);
}

/// [`apply_scene_item_volume`] with the removal passed in.
///
/// # Safety
///
/// As [`apply_scene_item_audio_actions_with`].
pub unsafe fn apply_scene_item_volume_with(
    item: *mut c_void,
    buf: *mut f32,
    ts: u64,
    sample_rate: usize,
    remove: RemoveActiveChild,
) -> bool {
    let Some(item) = NonNull::new(item) else {
        return false;
    };
    // SAFETY: the caller's contract.
    let buf = unsafe { gains(buf) };
    item_side::apply_scene_item_volume(&Item { item, remove }, buf, ts, sample_rate)
}

/// [`process_all_audio_actions`] with the removal passed in.
///
/// # Safety
///
/// As [`apply_scene_item_audio_actions_with`], with no buffer.
pub unsafe fn process_all_audio_actions_with(
    item: *mut c_void,
    sample_rate: usize,
    remove: RemoveActiveChild,
) {
    let Some(item) = NonNull::new(item) else {
        return;
    };
    item_side::process_all_audio_actions(&Item { item, remove }, sample_rate);
}

/// `void apply_scene_item_audio_actions(struct obs_scene_item *item, float
/// *buf, uint64_t ts, size_t sample_rate)`.
///
/// # Safety
///
/// As [`apply_scene_item_audio_actions_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn apply_scene_item_audio_actions(
    item: *mut c_void,
    buf: *mut f32,
    ts: u64,
    sample_rate: usize,
) {
    // SAFETY: the caller's contract; the removal is libobs's own.
    unsafe {
        apply_scene_item_audio_actions_with(
            item,
            buf,
            ts,
            sample_rate,
            obs_source_remove_active_child,
        )
    }
}

/// `bool apply_scene_item_volume(struct obs_scene_item *item, float *buf,
/// uint64_t ts, size_t sample_rate)`.
///
/// # Safety
///
/// As [`apply_scene_item_audio_actions_with`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn apply_scene_item_volume(
    item: *mut c_void,
    buf: *mut f32,
    ts: u64,
    sample_rate: usize,
) -> bool {
    // SAFETY: the caller's contract; the removal is libobs's own.
    unsafe {
        apply_scene_item_volume_with(item, buf, ts, sample_rate, obs_source_remove_active_child)
    }
}

/// `void process_all_audio_actions(struct obs_scene_item *item, size_t
/// sample_rate)`.
///
/// # Safety
///
/// As [`apply_scene_item_audio_actions_with`], with no buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn process_all_audio_actions(item: *mut c_void, sample_rate: usize) {
    // SAFETY: the caller's contract; the removal is libobs's own.
    unsafe { process_all_audio_actions_with(item, sample_rate, obs_source_remove_active_child) }
}
