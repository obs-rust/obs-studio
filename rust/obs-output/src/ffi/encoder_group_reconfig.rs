//! C ABI for `libobs/obs-encoder-group-reconfig.c.h`, the encoder group
//! reconfigure rules of `obs-encoder.c`, declared in
//! `libobs/obs-encoder-group-reconfig.h` without `EXPORT`.
//!
//! These only decide. `obs-encoder.c.h` (still C) takes the group mutex,
//! copies the reconfigure fields of the group and the encoder into `struct
//! obs_encoder_group_reconfig` and `struct obs_encoder_reconfig` and back
//! around each call, passes the other values by value, sets
//! `reconfigure_requested` on a true from the frame step, and calls
//! `info.update()`. The two structs cross as `#[repr(C)]` mirrors whose
//! layout Tier 3 checks against the C; no encoder or group pointer crosses.
//! All five stay hidden (`rust-hidden.txt`).
//!
//! Intentional differences, unreachable: the core's (where C is
//! undefined), and a NULL struct pointer, which the C dereferences, leaves
//! everything as it is (the frame step answers false). libobs passes its
//! own stack copies.

use crate::encoder_group_reconfig::{self as core, Encoder, FrameInput, Group};

/// `struct obs_encoder_group_reconfig` (`obs-encoder-group-reconfig.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_encoder_group_reconfig {
    pub reconfigure_request: u64,
    pub next_pts: i64,
    pub encoders_updated_next_pts: u32,
    pub encoders_reconfigured: u32,
    pub reconfigure_again: bool,
}

/// `struct obs_encoder_reconfig` (`obs-encoder-group-reconfig.h`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(non_camel_case_types)]
pub struct obs_encoder_reconfig {
    pub last_reconfigure_request: u64,
    pub last_handled_reconfigure_request: u64,
}

impl From<obs_encoder_group_reconfig> for Group {
    fn from(group: obs_encoder_group_reconfig) -> Self {
        Self {
            reconfigure_request: group.reconfigure_request,
            next_pts: group.next_pts,
            encoders_updated_next_pts: group.encoders_updated_next_pts,
            encoders_reconfigured: group.encoders_reconfigured,
            reconfigure_again: group.reconfigure_again,
        }
    }
}

impl From<Group> for obs_encoder_group_reconfig {
    fn from(group: Group) -> Self {
        Self {
            reconfigure_request: group.reconfigure_request,
            next_pts: group.next_pts,
            encoders_updated_next_pts: group.encoders_updated_next_pts,
            encoders_reconfigured: group.encoders_reconfigured,
            reconfigure_again: group.reconfigure_again,
        }
    }
}

impl From<obs_encoder_reconfig> for Encoder {
    fn from(encoder: obs_encoder_reconfig) -> Self {
        Self {
            last_reconfigure_request: encoder.last_reconfigure_request,
            last_handled_reconfigure_request: encoder.last_handled_reconfigure_request,
        }
    }
}

impl From<Encoder> for obs_encoder_reconfig {
    fn from(encoder: Encoder) -> Self {
        Self {
            last_reconfigure_request: encoder.last_reconfigure_request,
            last_handled_reconfigure_request: encoder.last_handled_reconfigure_request,
        }
    }
}

/// Runs `step` on a copy of `*group` and writes it back.
///
/// # Safety
///
/// `group` is NULL, or points to an initialized, aligned struct that
/// nothing else reads or writes during the call.
unsafe fn with_group<R: Default>(
    group: *mut obs_encoder_group_reconfig,
    step: impl FnOnce(&mut Group) -> R,
) -> R {
    // SAFETY: the caller's contract: NULL or a valid, exclusive struct.
    let Some(group) = (unsafe { group.as_mut() }) else {
        return R::default();
    };
    let mut core_group = Group::from(*group);
    let result = step(&mut core_group);
    *group = core_group.into();
    result
}

/// `void obs_encoder_group_reconfig_new_request(struct
/// obs_encoder_group_reconfig *group)`.
///
/// # Safety
///
/// `group` points to an initialized, aligned struct that nothing else reads
/// or writes during the call. NULL does nothing.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_group_reconfig_new_request(
    group: *mut obs_encoder_group_reconfig,
) {
    // SAFETY: forwarded from the caller.
    unsafe { with_group(group, core::new_request) }
}

/// `void obs_encoder_group_reconfig_update(struct obs_encoder_group_reconfig
/// *group, size_t num_encoders, uint64_t last_handled_reconfigure_request)`.
///
/// # Safety
///
/// As [`obs_encoder_group_reconfig_new_request`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_group_reconfig_update(
    group: *mut obs_encoder_group_reconfig,
    num_encoders: usize,
    last_handled_reconfigure_request: u64,
) {
    // SAFETY: forwarded from the caller.
    unsafe {
        with_group(group, |group| {
            core::update(group, num_encoders, last_handled_reconfigure_request);
        });
    }
}

/// `bool obs_encoder_group_reconfig_frame(struct obs_encoder_group_reconfig
/// *group, size_t num_encoders, uint32_t num_encoders_started, uint32_t
/// frame_rate_divisors_lcm, struct obs_encoder_reconfig *encoder, int64_t
/// cur_pts, uint32_t timebase_num, uint32_t frame_rate_divisor)`.
///
/// # Safety
///
/// `group` and `encoder` each point to an initialized, aligned struct that
/// nothing else reads or writes during the call. A NULL one changes
/// nothing and answers false.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn obs_encoder_group_reconfig_frame(
    group: *mut obs_encoder_group_reconfig,
    num_encoders: usize,
    num_encoders_started: u32,
    frame_rate_divisors_lcm: u32,
    encoder: *mut obs_encoder_reconfig,
    cur_pts: i64,
    timebase_num: u32,
    frame_rate_divisor: u32,
) -> bool {
    // SAFETY: the caller's contract: NULL or a valid, exclusive struct.
    let Some(encoder) = (unsafe { encoder.as_mut() }) else {
        return false;
    };
    let input = FrameInput {
        num_encoders,
        num_encoders_started,
        frame_rate_divisors_lcm,
        cur_pts,
        timebase_num,
        frame_rate_divisor,
    };
    // SAFETY: forwarded from the caller.
    unsafe {
        with_group(group, |group| {
            let mut core_encoder = Encoder::from(*encoder);
            let apply = core::frame(group, &mut core_encoder, &input);
            *encoder = core_encoder.into();
            apply
        })
    }
}

/// `uint32_t obs_encoder_group_reconfig_lcm(uint32_t lcm, uint32_t
/// frame_rate_divisor)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_encoder_group_reconfig_lcm(lcm: u32, frame_rate_divisor: u32) -> u32 {
    core::lcm(lcm, frame_rate_divisor)
}

/// `void obs_encoder_group_reconfig_reset(struct obs_encoder_group_reconfig
/// *group)`.
///
/// # Safety
///
/// As [`obs_encoder_group_reconfig_new_request`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_group_reconfig_reset(group: *mut obs_encoder_group_reconfig) {
    // SAFETY: forwarded from the caller.
    unsafe { with_group(group, core::reset) }
}
