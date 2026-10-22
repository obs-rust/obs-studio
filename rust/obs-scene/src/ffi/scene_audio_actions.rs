//! C ABI for `libobs/obs-scene-audio-actions.c.h`, the core of the scene
//! item visibility audio ramp, declared in `libobs/obs-scene-audio-actions.h`
//! without `EXPORT`.
//!
//! The item side stays C (`obs-scene-item-audio-actions.c.h`): it takes the
//! item's `actions_mutex`, calls [`obs_scene_audio_actions_apply`] with
//! `&item->audio_actions` and `&item->visible`, releases the lock, and drops
//! one active reference per hide it returns. `apply_scene_item_volume()`
//! peeks the first action under the lock and asks
//! [`obs_scene_audio_action_due`]. So `struct obs_scene_item` is never
//! mirrored, and these shims take no lock and call nothing in libobs. Both
//! stay hidden (`rust-hidden.txt`).
//!
//! The queue is the item's `DARRAY(struct item_action)`, mirrored as
//! [`obs_scene_item_action_array_t`] (the layout of `struct darray`). The
//! shim reads it in place, and erases the consumed actions from the front
//! one at a time, each as the C's `da_erase(*actions, i--)` with `i` 0
//! does (`darray_erase()` in `util/darray.h`: `num` minus one, then a
//! memmove of the rest down by one item), so the memory left in the array,
//! the stale items past `num` included, is the C's byte for byte. A single
//! move of the rest by `consumed` items would leave different stale bytes.
//! The erase allocates and frees nothing. It writes `*visible` only when it
//! consumed an action, as the C does.
//!
//! `obs-scene` does not use `obs-util`'s darray or `util_mul_div64` port:
//! `test_scene_bounds_place_rust` links the staticlib for `obs-scene` alone,
//! and `obs-util` would bring in its system libraries.
//!
//! Intentional differences from C, where C has undefined behaviour and
//! libobs never gets there:
//! - with `ts` 0 and a buffer, the C writes past the `AUDIO_OUTPUT_FRAMES`
//!   gains; this writes the same gains inside them and none past them;
//! - `obs_scene_audio_action_due()` with a rate of 0 (the C divides by 0)
//!   returns true;
//! - a null `actions` or `visible` does nothing and returns 0 (the C
//!   dereferences it).
//!
//! - on MSVC x64, `util_mul_div64` is exact 128-bit math and faults when
//!   the quotient does not fit in 64 bits, which needs a rate above 1 GHz;
//!   the core saturates there.

use std::ptr;
use std::slice;

use crate::audio_actions::{self as core, Gains, ItemAction};

/// `struct item_action` (`obs-scene-audio-actions.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct item_action {
    pub visible: bool,
    pub timestamp: u64,
}

/// `obs_scene_item_action_array_t` (`obs-scene-audio-actions.h`), a
/// `DARRAY(struct item_action)`: the layout of `struct darray`, typed.
#[repr(C)]
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub struct obs_scene_item_action_array_t {
    pub array: *mut item_action,
    pub num: usize,
    pub capacity: usize,
}

/// `da_erase(*actions, 0)`, `n` times: each is `darray_erase()` of
/// `util/darray.h` at index 0, which drops `num` by one and, when items are
/// left, moves them down by one.
///
/// # Safety
///
/// `da` must be a valid darray of `struct item_action` with at least `n`
/// items.
unsafe fn erase_front(da: &mut obs_scene_item_action_array_t, n: usize) {
    for _ in 0..n {
        if da.num == 0 {
            return;
        }
        da.num -= 1;
        if da.num == 0 {
            return;
        }
        // SAFETY: the array held `num + 1` items: the `num` after index 0
        // move down by one, within it (an overlapping move, as memmove).
        unsafe { ptr::copy(da.array.add(1), da.array, da.num) };
    }
}

impl From<item_action> for ItemAction {
    fn from(a: item_action) -> Self {
        ItemAction {
            visible: a.visible,
            timestamp: a.timestamp,
        }
    }
}

/// `size_t obs_scene_audio_actions_apply(obs_scene_item_action_array_t
/// *actions, bool *visible, uint64_t ts, size_t sample_rate, float *buf)`.
///
/// Consumes the queued actions that fall in the tick at `ts` (all of them
/// when `ts` is 0), sets `*visible`, fills `buf` when it is not null and
/// returns the number of hides consumed.
///
/// # Safety
///
/// `actions` must point to a valid darray of `struct item_action` and
/// `visible` to a `bool`, both readable and writable and not used by anyone
/// else for the call (the C caller holds `actions_mutex`). `buf` is null or
/// points to `AUDIO_OUTPUT_FRAMES` writable `float`s that alias neither.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_scene_audio_actions_apply(
    actions: *mut obs_scene_item_action_array_t,
    visible: *mut bool,
    ts: u64,
    sample_rate: usize,
    buf: *mut f32,
) -> usize {
    if actions.is_null() || visible.is_null() {
        return 0;
    }

    // SAFETY: the caller guarantees `buf` is null or holds a whole tick of
    // gains that nothing else uses for the call.
    let gains: Option<&mut Gains> = unsafe { buf.cast::<Gains>().as_mut() };

    let applied = {
        // SAFETY: the caller guarantees `actions` is a valid darray.
        let da = unsafe { &*actions };
        let queue: &[item_action] = if da.num == 0 || da.array.is_null() {
            &[]
        } else {
            // SAFETY: a valid darray of `struct item_action` holds `num`
            // initialized items at `array`.
            unsafe { slice::from_raw_parts(da.array, da.num) }
        };
        // SAFETY: the caller guarantees `visible` is readable.
        let start = unsafe { *visible };
        core::apply(
            queue.iter().map(|&a| ItemAction::from(a)),
            start,
            ts,
            sample_rate as u64,
            gains,
        )
    };

    // SAFETY: `actions` is valid and not used by anyone else for the call,
    // and the consumed actions are the first `consumed` of its `num`.
    unsafe { erase_front(&mut *actions, applied.consumed) };
    if applied.consumed != 0 {
        // SAFETY: the caller guarantees `visible` is writable.
        unsafe { *visible = applied.visible };
    }

    applied.hides
}

/// `bool obs_scene_audio_action_due(uint64_t timestamp, uint64_t ts, size_t
/// sample_rate)`: whether an action at `timestamp` is due in the tick at
/// `ts`. Returns true for a rate of 0, where the C divides by 0.
#[unsafe(no_mangle)]
pub extern "C" fn obs_scene_audio_action_due(timestamp: u64, ts: u64, sample_rate: usize) -> bool {
    core::action_due(timestamp, ts, sample_rate as u64)
}
