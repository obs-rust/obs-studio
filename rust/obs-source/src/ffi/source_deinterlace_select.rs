//! C ABI for `libobs/obs-source-deinterlace-select.c.h`, the deinterlace
//! frame selection of `first_frame()`, `ready_deinterlace_frames()`,
//! `deinterlace_get_closest_frames()` and `deinterlace_update_async_video()`,
//! declared in `libobs/obs-source-deinterlace-select.h` without `EXPORT`.
//!
//! The callers in `obs-source-deinterlace.c.h` (still C, under
//! `async_mutex`) copy the queued frames' timestamps, oldest first, into an
//! array of their own, read each of the source's fields where they always
//! did, pass the values to these shims and apply the answers: the erases,
//! `remove_async_frame()`, the stores of `last_frame_ts`,
//! `deinterlace_offset` and `deinterlace_frame_ts`, the `prev_frame` writes,
//! the frame pointers and the textures. So `struct obs_source` is never
//! mirrored, no frame pointer crosses, and the shims read only the
//! timestamps, write nothing through a pointer, take no lock and call
//! nothing in libobs. They stay hidden (`rust-hidden.txt`).
//!
//! Intentional difference from C: a null `timestamps` with a count above 0
//! is read as an empty queue (nothing dropped or ready, `last_frame_ts`
//! kept); the C reads through the null pointer. libobs never calls the walk
//! with an empty queue, and passes its own array of `count` timestamps.

use crate::deinterlace_select::{self as core, OffsetKind, State, Update};

/// `struct obs_source_deinterlace_select_first_result`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_source_deinterlace_select_first_result {
    pub first: bool,
    pub flag_front: bool,
}

/// `struct obs_source_deinterlace_select_walk_result`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_source_deinterlace_select_walk_result {
    pub drop_front: usize,
    pub last_frame_ts: u64,
    pub offset_reset: bool,
    pub pairs: bool,
    pub flag_front: bool,
    pub ready: bool,
}

/// `struct obs_source_deinterlace_select_pair_result`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct obs_source_deinterlace_select_pair_result {
    pub prev_frame: bool,
    pub set_frame_ts: bool,
    pub frame_ts: u64,
}

/// `enum obs_source_deinterlace_select_offset_kind`, as the C's enum is
/// returned (an `int` or `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_source_deinterlace_select_offset_kind_t = u32;

/// `enum obs_source_deinterlace_select_update_step`, likewise.
#[allow(non_camel_case_types)]
pub type obs_source_deinterlace_select_update_step_t = u32;

/// `struct obs_source_deinterlace_select_first_result
/// obs_source_deinterlace_select_first(uint64_t last_frame_ts, size_t
/// count)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_select_first(
    last_frame_ts: u64,
    count: usize,
) -> obs_source_deinterlace_select_first_result {
    let first = core::first(last_frame_ts, count);
    obs_source_deinterlace_select_first_result {
        first: first.first,
        flag_front: first.flag_front,
    }
}

/// `struct obs_source_deinterlace_select_walk_result
/// obs_source_deinterlace_select_walk(const uint64_t *timestamps, size_t
/// count, uint64_t sys_time, uint64_t last_frame_ts, uint64_t
/// last_sys_timestamp, bool async_unbuffered)`.
///
/// # Safety
///
/// When `count` is above 0, `timestamps` is null or points to `count`
/// initialized, aligned `uint64_t`s that nothing writes during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_deinterlace_select_walk(
    timestamps: *const u64,
    count: usize,
    sys_time: u64,
    last_frame_ts: u64,
    last_sys_timestamp: u64,
    async_unbuffered: bool,
) -> obs_source_deinterlace_select_walk_result {
    let timestamps: &[u64] = if count == 0 || timestamps.is_null() {
        &[]
    } else {
        // SAFETY: non-null, with count readable timestamps (caller contract).
        unsafe { std::slice::from_raw_parts(timestamps, count) }
    };
    let walk = core::walk(
        timestamps,
        sys_time,
        State {
            last_frame_ts,
            last_sys_timestamp,
            async_unbuffered,
        },
    );
    obs_source_deinterlace_select_walk_result {
        drop_front: walk.drop_front,
        last_frame_ts: walk.last_frame_ts,
        offset_reset: walk.offset_reset,
        pairs: walk.pairs,
        flag_front: walk.flag_front,
        ready: walk.ready,
    }
}

/// `struct obs_source_deinterlace_select_pair_result
/// obs_source_deinterlace_select_unbuffered_pair(uint64_t timestamp,
/// uint64_t next_timestamp, uint64_t offset, uint64_t sys_time)`. The
/// `frame_ts` of a pair that does not wait is 0, as the C leaves it.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_select_unbuffered_pair(
    timestamp: u64,
    next_timestamp: u64,
    offset: u64,
    sys_time: u64,
) -> obs_source_deinterlace_select_pair_result {
    let pair = core::unbuffered_pair(timestamp, next_timestamp, offset, sys_time);
    obs_source_deinterlace_select_pair_result {
        prev_frame: pair.prev_frame,
        set_frame_ts: pair.frame_ts.is_some(),
        frame_ts: pair.frame_ts.unwrap_or(0),
    }
}

/// `bool obs_source_deinterlace_select_take_reads_flag(size_t remaining)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_select_take_reads_flag(remaining: usize) -> bool {
    core::take_reads_flag(remaining)
}

/// `bool obs_source_deinterlace_select_take_pair(size_t remaining, bool
/// prev_frame)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_select_take_pair(
    remaining: usize,
    prev_frame: bool,
) -> bool {
    core::take_pair(remaining, prev_frame)
}

/// `bool obs_source_deinterlace_select_take_init_last(uint64_t
/// last_frame_ts)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_select_take_init_last(last_frame_ts: u64) -> bool {
    core::take_init_last(last_frame_ts)
}

/// `enum obs_source_deinterlace_select_offset_kind
/// obs_source_deinterlace_select_take_offset(bool async_unbuffered)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_select_take_offset(
    async_unbuffered: bool,
) -> obs_source_deinterlace_select_offset_kind_t {
    let kind: OffsetKind = core::take_offset(async_unbuffered);
    u32::from(kind)
}

/// `enum obs_source_deinterlace_select_update_step
/// obs_source_deinterlace_select_update(bool has_prev, bool updated)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_deinterlace_select_update(
    has_prev: bool,
    updated: bool,
) -> obs_source_deinterlace_select_update_step_t {
    let step: Update = core::update(has_prev, updated);
    u32::from(step)
}
