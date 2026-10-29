//! Tier 2 layout and Tier 3 parity: the shims of
//! `obs_output::ffi::encoder_group_reconfig` (and the safe core under them)
//! against the original `libobs/obs-encoder-group-reconfig.c.h`, compiled
//! as `oracle_obs_encoder_group_reconfig_*` (obs-c-oracle,
//! oracle/encoder_group_reconfig.c).
//!
//! Every call runs the shim, the C and the safe core on the same state, and
//! compares the answer and every field written, exactly.
//!
//! Runs: the layout of `struct obs_encoder_group_reconfig` and `struct
//! obs_encoder_reconfig`; exhaustive over the counters, flags and request
//! numbers at 0, 1, 2, 3 and their maximum for the update, the frame step
//! and the new request; exhaustive over pts, timebases, divisors and LCMs
//! at the common values and the edges for the next shared frame; every
//! `lcm()` of two values in 1..=256 and at the `INT_MAX` edges; groups of 1
//! to 4 encoders over random frames and updates in lockstep (C and shim
//! side by side, every state compared after every step); and proptests
//! over the full ranges.
//!
//! The C is undefined for an LCM product above `INT_MAX`, a 0 or a value
//! above `INT_MAX` in `lcm()`, a timestep of 0 and `next_pts` beyond
//! `int64_t`. Those inputs are not given to the C; there the shim is
//! compared with the core, which gives the intentional differences. A NULL
//! struct pointer is not given to the C (it dereferences it); the shim
//! changes nothing. `test_encoder_group_reconfig_steps` runs the C test
//! against the C and (`_rust`) against these shims;
//! `test_encoder_group_reconfig_walk` runs grouped encoders through the
//! hybrid libobs, where these shims replace the C.

use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::encoder_group_reconfig::{
    LAYOUT_NAMES, OracleEncoder, OracleGroup, oracle_encoder_group_reconfig_layout,
    oracle_obs_encoder_group_reconfig_frame, oracle_obs_encoder_group_reconfig_lcm,
    oracle_obs_encoder_group_reconfig_new_request, oracle_obs_encoder_group_reconfig_reset,
    oracle_obs_encoder_group_reconfig_update,
};
use obs_output::encoder_group_reconfig::{self as core, Encoder, FrameInput, Group};
use obs_output::ffi::encoder_group_reconfig::{
    obs_encoder_group_reconfig, obs_encoder_group_reconfig_frame, obs_encoder_group_reconfig_lcm,
    obs_encoder_group_reconfig_new_request, obs_encoder_group_reconfig_reset,
    obs_encoder_group_reconfig_update, obs_encoder_reconfig,
};
use proptest::prelude::*;

const INT_MAX: u32 = 0x7fff_ffff;

// ---------------------------------------------------------------------------
// conversions

fn to_ffi(g: Group) -> obs_encoder_group_reconfig {
    g.into()
}

fn to_oracle(g: Group) -> OracleGroup {
    OracleGroup {
        reconfigure_request: g.reconfigure_request,
        next_pts: g.next_pts,
        encoders_updated_next_pts: g.encoders_updated_next_pts,
        encoders_reconfigured: g.encoders_reconfigured,
        reconfigure_again: g.reconfigure_again,
    }
}

fn from_oracle(g: OracleGroup) -> Group {
    Group {
        reconfigure_request: g.reconfigure_request,
        next_pts: g.next_pts,
        encoders_updated_next_pts: g.encoders_updated_next_pts,
        encoders_reconfigured: g.encoders_reconfigured,
        reconfigure_again: g.reconfigure_again,
    }
}

fn enc_to_oracle(e: Encoder) -> OracleEncoder {
    OracleEncoder {
        last_reconfigure_request: e.last_reconfigure_request,
        last_handled_reconfigure_request: e.last_handled_reconfigure_request,
    }
}

fn enc_from_oracle(e: OracleEncoder) -> Encoder {
    Encoder {
        last_reconfigure_request: e.last_reconfigure_request,
        last_handled_reconfigure_request: e.last_handled_reconfigure_request,
    }
}

// ---------------------------------------------------------------------------
// one call on each side

/// C is defined for this frame step: a timestep above 0 and `next_pts`
/// within `i64`.
fn frame_defined(input: &FrameInput) -> bool {
    let timestep = input
        .timebase_num
        .wrapping_mul(input.frame_rate_divisors_lcm);
    let step = input.timebase_num.wrapping_mul(input.frame_rate_divisor);
    let next = i128::from(input.cur_pts) + i128::from(step);
    if timestep == 0 || i64::try_from(next).is_err() {
        return false;
    }
    // The rounding's product must stay within i64 too.
    let t = i128::from(timestep);
    let rounded = if next % t != 0 {
        (next / t + 1) * t
    } else {
        next
    };
    i64::try_from(rounded).is_ok()
}

/// The frame step on the shim, the core and (where defined) the C; panics
/// on any difference. Returns the shim's state and answer.
fn frame_all(group: Group, encoder: Encoder, input: &FrameInput) -> (Group, Encoder, bool) {
    let mut fg = to_ffi(group);
    let mut fe = obs_encoder_reconfig::from(encoder);
    // SAFETY: two exclusive, initialized locals.
    let shim = unsafe {
        obs_encoder_group_reconfig_frame(
            &raw mut fg,
            input.num_encoders,
            input.num_encoders_started,
            input.frame_rate_divisors_lcm,
            &raw mut fe,
            input.cur_pts,
            input.timebase_num,
            input.frame_rate_divisor,
        )
    };
    let (sg, se) = (Group::from(fg), Encoder::from(fe));

    let (mut cg, mut ce) = (group, encoder);
    let core_answer = core::frame(&mut cg, &mut ce, input);
    assert_eq!(
        (sg, se, shim),
        (cg, ce, core_answer),
        "shim vs core: {group:?} {encoder:?} {input:?}"
    );

    if frame_defined(input) {
        let mut og = to_oracle(group);
        let mut oe = enc_to_oracle(encoder);
        // SAFETY: two exclusive, initialized locals; C is defined here.
        let c = unsafe {
            oracle_obs_encoder_group_reconfig_frame(
                &raw mut og,
                input.num_encoders,
                input.num_encoders_started,
                input.frame_rate_divisors_lcm,
                &raw mut oe,
                input.cur_pts,
                input.timebase_num,
                input.frame_rate_divisor,
            )
        };
        assert_eq!(
            (sg, se, shim),
            (from_oracle(og), enc_from_oracle(oe), c),
            "shim vs C: {group:?} {encoder:?} {input:?}"
        );
    }
    (sg, se, shim)
}

fn update_all(group: Group, num_encoders: usize, handled: u64) -> Group {
    let mut fg = to_ffi(group);
    // SAFETY: an exclusive, initialized local.
    unsafe { obs_encoder_group_reconfig_update(&raw mut fg, num_encoders, handled) };
    let mut og = to_oracle(group);
    // SAFETY: an exclusive, initialized local; defined for every input.
    unsafe { oracle_obs_encoder_group_reconfig_update(&raw mut og, num_encoders, handled) };
    let mut cg = group;
    core::update(&mut cg, num_encoders, handled);
    let shim = Group::from(fg);
    assert_eq!(
        shim,
        from_oracle(og),
        "shim vs C: {group:?} {num_encoders} {handled}"
    );
    assert_eq!(shim, cg, "shim vs core: {group:?} {num_encoders} {handled}");
    shim
}

fn new_request_all(group: Group) {
    let mut fg = to_ffi(group);
    // SAFETY: an exclusive, initialized local.
    unsafe { obs_encoder_group_reconfig_new_request(&raw mut fg) };
    let mut og = to_oracle(group);
    // SAFETY: an exclusive, initialized local; the counter is unsigned.
    unsafe { oracle_obs_encoder_group_reconfig_new_request(&raw mut og) };
    let mut cg = group;
    core::new_request(&mut cg);
    assert_eq!(Group::from(fg), from_oracle(og), "shim vs C: {group:?}");
    assert_eq!(Group::from(fg), cg, "shim vs core: {group:?}");
}

fn reset_all(group: Group) {
    let mut fg = to_ffi(group);
    // SAFETY: an exclusive, initialized local.
    unsafe { obs_encoder_group_reconfig_reset(&raw mut fg) };
    let mut og = to_oracle(group);
    // SAFETY: an exclusive, initialized local.
    unsafe { oracle_obs_encoder_group_reconfig_reset(&raw mut og) };
    assert_eq!(Group::from(fg), from_oracle(og), "shim vs C: {group:?}");
    assert_eq!(Group::from(fg), Group::default(), "shim vs core: {group:?}");
}

/// C is defined for `lcm()`: both in 1..=INT_MAX and the LCM within it.
fn lcm_defined(a: u32, b: u32) -> bool {
    if a == 0 || b == 0 || a > INT_MAX || b > INT_MAX {
        return false;
    }
    let (mut x, mut y) = (u64::from(a), u64::from(b));
    while y != 0 {
        (x, y) = (y, x % y);
    }
    u64::from(a) / x * u64::from(b) <= u64::from(INT_MAX)
}

fn lcm_all(a: u32, b: u32) -> u32 {
    let shim = obs_encoder_group_reconfig_lcm(a, b);
    assert_eq!(shim, core::lcm(a, b), "shim vs core: {a}, {b}");
    if lcm_defined(a, b) {
        // SAFETY: C is defined here.
        let c = unsafe { oracle_obs_encoder_group_reconfig_lcm(a, b) };
        assert_eq!(shim, c, "shim vs C: {a}, {b}");
    }
    shim
}

// ---------------------------------------------------------------------------
// layout

#[test]
fn layout_matches_the_c() {
    let rust = [
        size_of::<obs_encoder_group_reconfig>(),
        align_of::<obs_encoder_group_reconfig>(),
        offset_of!(obs_encoder_group_reconfig, reconfigure_request),
        offset_of!(obs_encoder_group_reconfig, next_pts),
        offset_of!(obs_encoder_group_reconfig, encoders_updated_next_pts),
        offset_of!(obs_encoder_group_reconfig, encoders_reconfigured),
        offset_of!(obs_encoder_group_reconfig, reconfigure_again),
        size_of::<obs_encoder_reconfig>(),
        align_of::<obs_encoder_reconfig>(),
        offset_of!(obs_encoder_reconfig, last_reconfigure_request),
        offset_of!(obs_encoder_reconfig, last_handled_reconfigure_request),
    ];
    for (i, (want, name)) in rust.iter().zip(LAYOUT_NAMES).enumerate() {
        // SAFETY: an index the C answers.
        let c = unsafe { oracle_encoder_group_reconfig_layout(i) };
        assert_eq!(*want, c, "{name}");
    }
    assert_eq!(
        size_of::<OracleGroup>(),
        size_of::<obs_encoder_group_reconfig>()
    );
    assert_eq!(
        size_of::<OracleEncoder>(),
        size_of::<obs_encoder_reconfig>()
    );
}

// ---------------------------------------------------------------------------
// exhaustive

const REQUESTS: [u64; 5] = [0, 1, 2, 3, u64::MAX];
const COUNTS: [u32; 5] = [0, 1, 2, 3, u32::MAX];
const NUMS: [usize; 5] = [0, 1, 2, 3, usize::MAX];

#[test]
fn update_every_state() {
    let mut n = 0u64;
    for request in REQUESTS {
        for reconfigured in COUNTS {
            for updated in [0, 2] {
                for again in [false, true] {
                    for num in NUMS {
                        for handled in REQUESTS {
                            let group = Group {
                                reconfigure_request: request,
                                next_pts: 6,
                                encoders_updated_next_pts: updated,
                                encoders_reconfigured: reconfigured,
                                reconfigure_again: again,
                            };
                            update_all(group, num, handled);
                            n += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(n, 5 * 5 * 2 * 2 * 5 * 5);
}

#[test]
fn new_request_and_reset_every_state() {
    for request in REQUESTS {
        for count in COUNTS {
            for again in [false, true] {
                for next_pts in [i64::MIN, -1, 0, 6, i64::MAX] {
                    let group = Group {
                        reconfigure_request: request,
                        next_pts,
                        encoders_updated_next_pts: count,
                        encoders_reconfigured: count.wrapping_add(1),
                        reconfigure_again: again,
                    };
                    new_request_all(group);
                    reset_all(group);
                }
            }
        }
    }
}

#[test]
fn frame_every_state() {
    let mut applied = 0u64;
    let mut n = 0u64;
    for request in [0, 1, 2, u64::MAX] {
        for next_pts in [0, 4, 6] {
            for updated in COUNTS {
                for reconfigured in [0, 1, 2, u32::MAX] {
                    for again in [false, true] {
                        for last in [0, 1, 2, u64::MAX] {
                            for handled in [0, 1, 2] {
                                for num in [0, 1, 2, 3] {
                                    for started in [0, 2, 3] {
                                        for cur_pts in [4, 6] {
                                            let group = Group {
                                                reconfigure_request: request,
                                                next_pts,
                                                encoders_updated_next_pts: updated,
                                                encoders_reconfigured: reconfigured,
                                                reconfigure_again: again,
                                            };
                                            let encoder = Encoder {
                                                last_reconfigure_request: last,
                                                last_handled_reconfigure_request: handled,
                                            };
                                            let input = FrameInput {
                                                num_encoders: num,
                                                num_encoders_started: started,
                                                frame_rate_divisors_lcm: 2,
                                                cur_pts,
                                                timebase_num: 1,
                                                frame_rate_divisor: 1,
                                            };
                                            if frame_all(group, encoder, &input).2 {
                                                applied += 1;
                                            }
                                            n += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(n, 4 * 3 * 5 * 4 * 2 * 4 * 3 * 4 * 3 * 2);
    assert!(applied > 0, "the apply branch ran");
}

#[test]
fn next_shared_frame_every_value() {
    const PTS: [i64; 14] = [
        i64::MIN,
        i64::MIN + 1,
        -6006,
        -1001,
        -1,
        0,
        1,
        2,
        1001,
        3003,
        3004,
        1 << 40,
        i64::MAX - 6006,
        i64::MAX,
    ];
    const TIMEBASES: [u32; 9] = [0, 1, 2, 1001, 30000, 65536, 0x1_0001, 0x8000_0000, u32::MAX];
    const DIVISORS: [u32; 7] = [0, 1, 2, 3, 4, 65536, u32::MAX];
    const LCMS: [u32; 8] = [0, 1, 2, 3, 6, 12, 65536, u32::MAX];
    let mut defined = 0u64;
    for cur_pts in PTS {
        for timebase_num in TIMEBASES {
            for frame_rate_divisor in DIVISORS {
                for frame_rate_divisors_lcm in LCMS {
                    let input = FrameInput {
                        num_encoders: 2,
                        num_encoders_started: 2,
                        frame_rate_divisors_lcm,
                        cur_pts,
                        timebase_num,
                        frame_rate_divisor,
                    };
                    // A new request: the frame step proposes the next shared
                    // frame, against a group next_pts below every pts.
                    let group = Group {
                        reconfigure_request: 1,
                        next_pts: i64::MIN,
                        ..Group::default()
                    };
                    let (g, _, _) = frame_all(group, Encoder::default(), &input);
                    assert_eq!(
                        g.next_pts,
                        core::next_shared_pts(
                            cur_pts,
                            timebase_num,
                            frame_rate_divisor,
                            frame_rate_divisors_lcm
                        )
                    );
                    if frame_defined(&input) {
                        defined += 1;
                    }
                }
            }
        }
    }
    assert!(defined > 1000, "{defined} defined inputs reached the C");
}

#[test]
fn lcm_every_small_pair_and_the_edges() {
    for a in 1..=256 {
        for b in 1..=256 {
            lcm_all(a, b);
        }
    }
    for a in [
        0,
        1,
        2,
        46339,
        46340,
        65535,
        65536,
        INT_MAX - 1,
        INT_MAX,
        INT_MAX + 1,
        u32::MAX,
    ] {
        for b in [
            0,
            1,
            2,
            3,
            46339,
            46340,
            65536,
            INT_MAX,
            INT_MAX + 1,
            u32::MAX,
        ] {
            lcm_all(a, b);
        }
    }
}

// ---------------------------------------------------------------------------
// groups over frames, C and shim in lockstep

struct Side {
    group: Group,
    encoders: Vec<(Encoder, i64)>,
    applied: Vec<Vec<i64>>,
}

/// Runs the frames on both sides through `frame_all` and `update_all` (each
/// compares shim, core and C), keeping one state since all three agree.
fn run_group(timebase_num: u32, divisors: &[u32], updates: &[(u64, usize)], frames: u64) -> Side {
    let mut l = 1;
    for &d in divisors {
        l = lcm_all(l, d);
    }
    let n = divisors.len();
    let mut side = Side {
        group: Group::default(),
        encoders: vec![(Encoder::default(), 0); n],
        applied: vec![Vec::new(); n],
    };
    for f in 0..frames {
        for &(at, who) in updates {
            if at == f {
                let handled = side.encoders[who % n].0.last_handled_reconfigure_request;
                side.group = update_all(side.group, n, handled);
            }
        }
        for (i, &d) in divisors.iter().enumerate() {
            if !f.is_multiple_of(u64::from(d)) {
                continue;
            }
            let (encoder, cur_pts) = side.encoders[i];
            let input = FrameInput {
                num_encoders: n,
                num_encoders_started: u32::try_from(n).unwrap_or(u32::MAX),
                frame_rate_divisors_lcm: l,
                cur_pts,
                timebase_num,
                frame_rate_divisor: d,
            };
            let (g, e, apply) = frame_all(side.group, encoder, &input);
            side.group = g;
            if apply {
                side.applied[i].push(cur_pts);
            }
            side.encoders[i] = (e, cur_pts + i64::from(timebase_num) * i64::from(d));
        }
    }
    side
}

#[test]
fn groups_at_common_rates() {
    for timebase_num in [1, 1001] {
        for divisors in [&[1, 2][..], &[1, 2, 3], &[2, 3], &[1, 1, 4, 2], &[3]] {
            let side = run_group(
                timebase_num,
                divisors,
                &[(3, 0), (4, 1), (20, 0), (21, 2)],
                80,
            );
            assert!(side.applied.iter().all(|a| a == &side.applied[0]));
            assert_eq!(side.applied[0].len(), 2, "{divisors:?}");
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn groups_random(
        divisors in prop::collection::vec(1u32..=6, 1..=4),
        timebase_num in prop_oneof![Just(1u32), Just(1001u32), 1u32..5000],
        updates in prop::collection::vec((0u64..80, 0usize..4), 0..8),
    ) {
        let side = run_group(timebase_num, &divisors, &updates, 160);
        prop_assert!(side.applied.iter().all(|a| a == &side.applied[0]));
    }

    #[test]
    fn frame_random_state(
        request in any::<u64>(), next_pts in any::<i64>(), updated in any::<u32>(),
        reconfigured in any::<u32>(), again in any::<bool>(), last in any::<u64>(),
        handled in any::<u64>(), num in any::<usize>(), started in any::<u32>(),
        l in any::<u32>(), cur_pts in any::<i64>(), timebase_num in any::<u32>(),
        divisor in any::<u32>(),
    ) {
        let group = Group {
            reconfigure_request: request,
            next_pts,
            encoders_updated_next_pts: updated,
            encoders_reconfigured: reconfigured,
            reconfigure_again: again,
        };
        let encoder = Encoder { last_reconfigure_request: last, last_handled_reconfigure_request: handled };
        let input = FrameInput {
            num_encoders: num,
            num_encoders_started: started,
            frame_rate_divisors_lcm: l,
            cur_pts,
            timebase_num,
            frame_rate_divisor: divisor,
        };
        frame_all(group, encoder, &input);
        update_all(group, num, handled);
        new_request_all(group);
        reset_all(group);
    }

    #[test]
    fn frame_random_small_state(
        request in 0u64..4, next_pts in -4i64..16, updated in 0u32..4,
        reconfigured in 0u32..4, again in any::<bool>(), last in 0u64..4,
        handled in 0u64..4, num in 0usize..4, started in 0u32..4,
        l in 1u32..7, cur_pts in -4i64..16, timebase_num in 1u32..4,
        divisor in 1u32..7,
    ) {
        let group = Group {
            reconfigure_request: request,
            next_pts,
            encoders_updated_next_pts: updated,
            encoders_reconfigured: reconfigured,
            reconfigure_again: again,
        };
        let encoder = Encoder { last_reconfigure_request: last, last_handled_reconfigure_request: handled };
        let input = FrameInput {
            num_encoders: num,
            num_encoders_started: started,
            frame_rate_divisors_lcm: l,
            cur_pts,
            timebase_num,
            frame_rate_divisor: divisor,
        };
        frame_all(group, encoder, &input);
    }

    #[test]
    fn lcm_random(a in any::<u32>(), b in any::<u32>(), small in 1u32..=0xffff) {
        lcm_all(a, b);
        lcm_all(small, b);
        lcm_all(a, small);
    }
}

#[test]
fn null_pointers_change_nothing() {
    let mut fg = to_ffi(Group {
        reconfigure_request: 1,
        ..Group::default()
    });
    let before = fg;
    // SAFETY: NULL is allowed; the group is an exclusive local.
    unsafe {
        obs_encoder_group_reconfig_new_request(std::ptr::null_mut());
        obs_encoder_group_reconfig_update(std::ptr::null_mut(), 1, 0);
        obs_encoder_group_reconfig_reset(std::ptr::null_mut());
        assert!(!obs_encoder_group_reconfig_frame(
            std::ptr::null_mut(),
            1,
            1,
            1,
            std::ptr::null_mut(),
            0,
            1,
            1
        ));
        assert!(!obs_encoder_group_reconfig_frame(
            &raw mut fg,
            1,
            1,
            1,
            std::ptr::null_mut(),
            0,
            1,
            1
        ));
    }
    assert_eq!(fg, before);
}
