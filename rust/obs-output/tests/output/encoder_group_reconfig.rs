//! Tier 1: `obs_output::encoder_group_reconfig`, the encoder group
//! reconfigure rules of `obs-encoder.c`.
//!
//! The `steps_*` cases port `test/cmocka/test_encoder_group_reconfig_steps.c`
//! 1:1: its 22 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones
//! included, and its group simulator. The C calls the hidden
//! `obs_encoder_group_reconfig_*` functions; here the safe core is called
//! directly.
//!
//! The `edge_*` cases are new: proptests of [`next_shared_pts`] and [`lcm`]
//! against models in `i128`/`u64`, random groups of 1 to 4 encoders with
//! random divisors and random updates (every round applies once on each
//! encoder, all on the same pts, a multiple of the timestep, after its
//! update), and the intentional differences where C is undefined.

#![allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use obs_output::encoder_group_reconfig::{
    Encoder, FrameInput, Group, frame, lcm, new_request, next_shared_pts, reset, update,
};
use proptest::prelude::*;

/// `timebase_num` of 29.97 fps.
const NTSC_NUM: u32 = 1001;

// ---------------------------------------------------------------------------
// helpers

fn group_of(request: u64, next_pts: i64, updated: u32, reconfigured: u32, again: bool) -> Group {
    Group {
        reconfigure_request: request,
        next_pts,
        encoders_updated_next_pts: updated,
        encoders_reconfigured: reconfigured,
        reconfigure_again: again,
    }
}

fn enc(last: u64, handled: u64) -> Encoder {
    Encoder {
        last_reconfigure_request: last,
        last_handled_reconfigure_request: handled,
    }
}

/// `obs_encoder_group_reconfig_frame()`'s argument order.
#[allow(clippy::too_many_arguments)]
fn step(
    group: &mut Group,
    num_encoders: usize,
    num_encoders_started: u32,
    frame_rate_divisors_lcm: u32,
    encoder: &mut Encoder,
    cur_pts: i64,
    timebase_num: u32,
    frame_rate_divisor: u32,
) -> bool {
    frame(
        group,
        encoder,
        &FrameInput {
            num_encoders,
            num_encoders_started,
            frame_rate_divisors_lcm,
            cur_pts,
            timebase_num,
            frame_rate_divisor,
        },
    )
}

/// The C test's group simulator: each video frame, every encoder whose
/// divisor the frame count meets asks [`frame`] at its `cur_pts`, in the
/// group's order, then encodes (`cur_pts` steps).
struct SimEncoder {
    reconfig: Encoder,
    divisor: u32,
    cur_pts: i64,
    applied: Vec<i64>,
}

struct Sim {
    group: Group,
    lcm: u32,
    timebase_num: u32,
    encoders: Vec<SimEncoder>,
}

impl Sim {
    fn new(timebase_num: u32, divisors: &[u32]) -> Self {
        let mut l = 1;
        for &d in divisors {
            l = lcm(l, d);
        }
        Self {
            group: Group::default(),
            lcm: l,
            timebase_num,
            encoders: divisors
                .iter()
                .map(|&divisor| SimEncoder {
                    reconfig: Encoder::default(),
                    divisor,
                    cur_pts: 0,
                    applied: Vec::new(),
                })
                .collect(),
        }
    }

    fn update(&mut self, idx: usize) {
        let handled = self.encoders[idx].reconfig.last_handled_reconfigure_request;
        update(&mut self.group, self.encoders.len(), handled);
    }

    /// One encoder's turn on a frame.
    fn turn(&mut self, idx: usize) {
        let num = self.encoders.len();
        let e = &mut self.encoders[idx];
        let input = FrameInput {
            num_encoders: num,
            num_encoders_started: num as u32,
            frame_rate_divisors_lcm: self.lcm,
            cur_pts: e.cur_pts,
            timebase_num: self.timebase_num,
            frame_rate_divisor: e.divisor,
        };
        if frame(&mut self.group, &mut e.reconfig, &input) {
            e.applied.push(e.cur_pts);
        }
        e.cur_pts += i64::from(self.timebase_num) * i64::from(e.divisor);
    }

    fn frame(&mut self, n: u64) {
        for i in 0..self.encoders.len() {
            if n.is_multiple_of(u64::from(self.encoders[i].divisor)) {
                self.turn(i);
            }
        }
    }

    fn frames(&mut self, from: u64, to: u64) {
        for n in from..to {
            self.frame(n);
        }
    }
}

// ---------------------------------------------------------------------------
// obs_encoder_update(): the request

#[test]
fn steps_update_first_request() {
    let mut group = group_of(0, 600, 2, 1, true);
    update(&mut group, 3, 0);
    assert_eq!(group, group_of(1, 600, 0, 0, false), "first request");
}

#[test]
fn steps_update_while_encoders_apply() {
    let mut group = group_of(1, 600, 3, 1, false);
    let want = group;
    update(&mut group, 3, 0);
    assert_eq!(group, want, "encoder not applied yet: no change");
}

#[test]
fn steps_update_after_this_encoder_applied() {
    let mut group = group_of(1, 600, 3, 1, false);
    update(&mut group, 3, 1);
    let want = group_of(1, 600, 3, 1, true);
    assert_eq!(group, want, "encoder applied: reconfigure_again");
    update(&mut group, 3, 1);
    assert_eq!(group, want, "a third update: still held");
}

#[test]
fn steps_update_after_all_applied() {
    let mut group = group_of(1, 600, 3, 3, false);
    update(&mut group, 3, 1);
    assert_eq!(group, group_of(2, 600, 0, 0, false), "all applied");
}

#[test]
fn steps_update_all_applied_wins_over_again() {
    let mut group = group_of(4, 0, 2, 2, true);
    update(&mut group, 2, 4);
    assert_eq!(group, group_of(5, 0, 0, 0, false), "all applied and again");
}

#[test]
fn steps_update_stale_handled_request() {
    let mut group = group_of(3, 90, 1, 1, false);
    let want = group;
    update(&mut group, 2, 2);
    assert_eq!(group, want, "older handled request: no change");
}

#[test]
fn steps_update_empty_group() {
    // characterized, not endorsed
    let mut group = group_of(7, 0, 0, 0, true);
    update(&mut group, 0, 0);
    assert_eq!(group, group_of(8, 0, 0, 0, false), "empty group");
}

#[test]
fn steps_new_request_counter() {
    // characterized, not endorsed: the counter wraps to 0
    let mut group = group_of(u64::MAX, -3, 5, 4, true);
    new_request(&mut group);
    assert_eq!(group, group_of(0, -3, 0, 0, false), "wraps");
}

// ---------------------------------------------------------------------------
// handle_encoder_group_reconfigure_request(): one frame

#[test]
fn steps_frame_new_request_sets_next_pts() {
    let mut group = group_of(1, 0, 0, 0, false);
    let mut e = enc(0, 0);
    assert!(!step(&mut group, 2, 2, 3, &mut e, 0, NTSC_NUM, 1));
    assert_eq!(
        group,
        group_of(1, 3003, 1, 0, false),
        "pts 0 + 1001, up to 3003"
    );
    assert_eq!(e, enc(1, 0), "takes the request");
}

#[test]
fn steps_frame_next_pts_on_the_boundary() {
    let mut group = group_of(1, 0, 0, 0, false);
    let mut e = enc(0, 0);
    assert!(!step(&mut group, 2, 2, 3, &mut e, 2002, NTSC_NUM, 1));
    assert_eq!(group, group_of(1, 3003, 1, 0, false), "2002 + 1001 = 3003");

    let mut group = group_of(1, 0, 0, 0, false);
    let mut e = enc(0, 0);
    assert!(!step(&mut group, 2, 2, 3, &mut e, 3003, NTSC_NUM, 1));
    assert_eq!(
        group,
        group_of(1, 6006, 1, 0, false),
        "3003 + 1001, up to 6006"
    );

    let mut group = group_of(1, 0, 0, 0, false);
    let mut e = enc(0, 0);
    assert!(!step(&mut group, 2, 2, 3, &mut e, 3003, NTSC_NUM, 3));
    assert_eq!(group, group_of(1, 6006, 1, 0, false), "divisor 3: 6006");
}

#[test]
fn steps_frame_largest_next_pts_wins() {
    let mut group = group_of(1, 9009, 1, 0, false);
    let mut e = enc(0, 0);
    assert!(!step(&mut group, 2, 2, 3, &mut e, 0, NTSC_NUM, 1));
    assert_eq!(group, group_of(1, 9009, 2, 0, false), "smaller: kept");
}

#[test]
fn steps_frame_recomputes_on_the_shared_frame() {
    let mut group = group_of(1, 4, 1, 0, false);
    let mut e = enc(1, 0);
    assert!(!step(&mut group, 2, 2, 2, &mut e, 4, 1, 1));
    assert_eq!(group, group_of(1, 6, 1, 0, false), "pts 4 + 1, up to 6");
    assert_eq!(e, enc(1, 0), "same request");
}

#[test]
fn steps_frame_applies_on_the_shared_frame() {
    let mut group = group_of(1, 6, 2, 0, false);
    let mut e = enc(1, 0);
    assert!(step(&mut group, 2, 2, 2, &mut e, 6, 1, 1));
    assert_eq!(group, group_of(1, 6, 2, 1, false), "first applies");
    assert_eq!(e, enc(1, 1), "handled");

    // characterized, not endorsed: the shared frame moves on (6 + 1, up to 8)
    let mut before = group;
    assert!(!step(&mut group, 2, 2, 2, &mut e, 6, 1, 1));
    before.next_pts = 8;
    assert_eq!(group, before, "again on the shared frame");
    assert_eq!(e, enc(1, 1), "still handled");
}

#[test]
fn steps_frame_each_apply_condition() {
    let mut group = group_of(1, 6, 1, 0, false);
    let mut e = enc(1, 0);
    assert!(
        !step(&mut group, 2, 2, 2, &mut e, 6, 1, 1),
        "not all took it"
    );

    let mut group = group_of(1, 6, 2, 0, false);
    let want = group;
    let mut e = enc(1, 0);
    assert!(!step(&mut group, 2, 2, 2, &mut e, 5, 1, 1));
    assert_eq!(group, want, "pts before the shared frame");
    assert_eq!(e, enc(1, 0), "pts before the shared frame");

    let mut group = group_of(1, 6, 2, 1, false);
    let want = group;
    let mut e = enc(1, 1);
    assert!(!step(&mut group, 2, 2, 2, &mut e, 7, 1, 1));
    assert_eq!(group, want, "handled, later pts");

    let mut group = group_of(0, 0, 0, 0, false);
    let want = group;
    let mut e = enc(0, 0);
    assert!(!step(&mut group, 2, 2, 2, &mut e, 1, 1, 1));
    assert_eq!(group, want, "no request");

    // characterized, not endorsed: the first frame moves next_pts
    let mut group = group_of(0, 0, 0, 0, false);
    let mut e = enc(0, 0);
    assert!(!step(&mut group, 2, 2, 2, &mut e, 0, 1, 1));
    assert_eq!(
        group,
        group_of(0, 2, 0, 0, false),
        "no request, first frame"
    );
}

#[test]
fn steps_frame_last_apply_starts_held_request() {
    let mut group = group_of(1, 6, 2, 1, true);
    let mut e = enc(1, 0);
    assert!(step(&mut group, 2, 2, 2, &mut e, 6, 1, 2));
    assert_eq!(group, group_of(2, 6, 0, 0, false), "last apply with again");
    assert_eq!(e, enc(1, 1), "handled 1");

    let mut group = group_of(1, 6, 3, 1, true);
    let mut e = enc(1, 0);
    assert!(step(&mut group, 3, 3, 2, &mut e, 6, 1, 2));
    assert_eq!(group, group_of(1, 6, 3, 2, true), "second of three: held");

    let mut group = group_of(1, 6, 2, 1, false);
    let mut e = enc(1, 0);
    assert!(step(&mut group, 2, 2, 2, &mut e, 6, 1, 2));
    assert_eq!(group, group_of(1, 6, 2, 2, false), "without again: done");
}

#[test]
fn steps_frame_u32_products_wrap() {
    // characterized, not endorsed
    let mut group = group_of(1, 0, 0, 0, false);
    let mut e = enc(0, 0);
    assert!(!step(&mut group, 1, 1, 1, &mut e, 5, 0x8000_0000, 2));
    assert_eq!(group, group_of(1, 0x8000_0000, 1, 0, false), "wrapped step");

    let mut group = group_of(1, 0, 0, 0, false);
    let mut e = enc(0, 0);
    assert!(!step(&mut group, 1, 1, 0x1_0000, &mut e, 1, 0x1_0001, 1));
    assert_eq!(
        group,
        group_of(1, 0x2_0000, 1, 0, false),
        "wrapped timestep"
    );
}

// ---------------------------------------------------------------------------
// the group over frames

#[test]
fn steps_walk_two_encoders() {
    let mut sim = Sim::new(1, &[1, 2]);
    assert_eq!(sim.lcm, 2);
    sim.frames(0, 3);
    sim.update(0);
    sim.frames(3, 20);
    assert_eq!(sim.encoders[0].applied, [6], "encoder 1");
    assert_eq!(sim.encoders[1].applied, [6], "encoder 2");
}

#[test]
fn steps_walk_three_encoders_ntsc() {
    let mut sim = Sim::new(NTSC_NUM, &[1, 2, 3]);
    assert_eq!(sim.lcm, 6);
    sim.frames(0, 7);
    sim.update(2);
    sim.frames(7, 40);
    let want = [12 * i64::from(NTSC_NUM)];
    for (i, e) in sim.encoders.iter().enumerate() {
        assert_eq!(e.applied, want, "encoder {}", i + 1);
    }
}

#[test]
fn steps_walk_second_update_while_pending() {
    let mut sim = Sim::new(1, &[1, 2]);
    sim.frames(0, 3);
    sim.update(0);
    sim.frame(3);
    sim.update(1);
    sim.frames(4, 20);
    assert_eq!(sim.encoders[0].applied, [6], "encoder 1");
    assert_eq!(sim.encoders[1].applied, [6], "encoder 2");
}

#[test]
fn steps_walk_second_update_after_one_applied() {
    let mut sim = Sim::new(1, &[1, 2]);
    sim.frames(0, 3);
    sim.update(0);
    sim.frames(3, 6);
    sim.turn(0);
    sim.update(0);
    assert!(sim.group.reconfigure_again);
    sim.turn(1);
    assert_eq!(sim.group.reconfigure_request, 2, "request 2");
    sim.frames(7, 30);
    assert_eq!(sim.encoders[0].applied, [6, 10], "encoder 1");
    assert_eq!(sim.encoders[1].applied, [6, 10], "encoder 2");
}

// ---------------------------------------------------------------------------
// add_connection() and remove_connection()

#[test]
fn steps_lcm_of_divisors() {
    for (a, b, want) in [
        (1, 1, 1),
        (1, 2, 2),
        (2, 2, 2),
        (2, 3, 6),
        (3, 2, 6),
        (2, 4, 4),
        (4, 2, 4),
        (4, 6, 12),
        (6, 4, 12),
        (60, 7, 420),
        (0x7fff_ffff, 1, 0x7fff_ffff),
        (46340, 46339, 2_147_349_260),
    ] {
        assert_eq!(lcm(a, b), want, "{a}, {b}");
    }
}

#[test]
fn steps_reset_when_idle() {
    let mut group = group_of(9, 12012, 3, 2, true);
    reset(&mut group);
    assert_eq!(group, Group::default(), "reset");
}

// ---------------------------------------------------------------------------
// edges

/// The next shared frame from C's expression, in `i128`, where C is
/// defined (a timestep above 0, no `i64` overflow).
fn model_next(cur_pts: i64, timebase_num: u32, divisor: u32, l: u32) -> Option<i64> {
    let step = i128::from(timebase_num.wrapping_mul(divisor));
    let timestep = i128::from(timebase_num.wrapping_mul(l));
    let next = i128::from(cur_pts) + step;
    if timestep == 0 || i64::try_from(next).is_err() {
        return None;
    }
    let rounded = if next % timestep != 0 {
        (next / timestep + 1) * timestep
    } else {
        next
    };
    i64::try_from(rounded).ok()
}

fn gcd_u64(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd_u64(b, a % b) }
}

proptest! {
    #[test]
    fn edge_next_shared_pts_matches_the_model(
        cur_pts in prop_oneof![-1_000_000i64..1_000_000, any::<i64>()],
        timebase_num in prop_oneof![1u32..2000, any::<u32>()],
        divisor in prop_oneof![1u32..8, any::<u32>()],
        l in prop_oneof![1u32..64, any::<u32>()],
    ) {
        if let Some(want) = model_next(cur_pts, timebase_num, divisor, l) {
            prop_assert_eq!(next_shared_pts(cur_pts, timebase_num, divisor, l), want);
        }
    }

    #[test]
    fn edge_lcm_matches_the_model(a in 1u32..=0x7fff_ffff, b in 1u32..=0x7fff_ffff) {
        let want = u64::from(a) / gcd_u64(u64::from(a), u64::from(b)) * u64::from(b);
        // C overflows int beyond INT_MAX; there the result wraps.
        prop_assert_eq!(lcm(a, b), want as u32);
        prop_assert_eq!(lcm(a, b), lcm(b, a));
    }

    #[test]
    fn edge_any_group_applies_together(
        divisors in prop::collection::vec(1u32..=4, 1..=4),
        timebase_num in prop_oneof![Just(1u32), Just(NTSC_NUM), 1u32..100],
        updates in prop::collection::vec((0u64..60, 0usize..4), 1..6),
    ) {
        let mut sim = Sim::new(timebase_num, &divisors);
        let n = divisors.len();
        let mut first_update = None;
        for f in 0..120u64 {
            for &(at, who) in &updates {
                if at == f {
                    first_update.get_or_insert(f);
                    sim.update(who % n);
                }
            }
            sim.frame(f);
        }
        let timestep = i64::from(timebase_num) * i64::from(sim.lcm);
        let rounds = sim.encoders[0].applied.clone();
        prop_assert!(!rounds.is_empty(), "an update applies");
        for e in &sim.encoders {
            prop_assert_eq!(&e.applied, &rounds, "same rounds on the same pts");
        }
        let first = first_update.unwrap_or(0) as i64 * i64::from(timebase_num);
        for (k, &pts) in rounds.iter().enumerate() {
            prop_assert_eq!(pts % timestep, 0, "round {} on a shared frame", k);
            prop_assert!(pts >= first, "round {} after the update", k);
        }
        prop_assert!(rounds.windows(2).all(|w| w[0] < w[1]), "rounds in order");
    }

    #[test]
    fn edge_frame_never_panics(
        request in any::<u64>(), next_pts in any::<i64>(), updated in any::<u32>(),
        reconfigured in any::<u32>(), again in any::<bool>(), last in any::<u64>(),
        handled in any::<u64>(), num in any::<usize>(), started in any::<u32>(),
        l in any::<u32>(), cur_pts in any::<i64>(), timebase_num in any::<u32>(),
        divisor in any::<u32>(),
    ) {
        let mut group = group_of(request, next_pts, updated, reconfigured, again);
        let mut e = enc(last, handled);
        let _ = step(&mut group, num, started, l, &mut e, cur_pts, timebase_num, divisor);
        update(&mut group, num, handled);
        new_request(&mut group);
        let _ = lcm(l, divisor);
        reset(&mut group);
        prop_assert_eq!(group, Group::default());
    }
}

#[test]
fn edge_intentional_differences() {
    // C divides by 0 in gcd(): 0 here.
    assert_eq!(lcm(0, 3), 0);
    assert_eq!(lcm(3, 0), 0);
    assert_eq!(lcm(0, 0), 0);
    // Above INT_MAX C's gcd() goes negative; here the unsigned LCM.
    assert_eq!(lcm(0x8000_0000, 2), 0x8000_0000);
    assert_eq!(lcm(0x8000_0000, 3), 0x8000_0000_u32.wrapping_mul(3));
    // A timestep of 0 (C divides by 0): unrounded.
    assert_eq!(next_shared_pts(5, 0, 1, 1), 5);
    assert_eq!(next_shared_pts(5, 3, 1, 0), 8);
    // Beyond i64: wraps.
    assert_eq!(next_shared_pts(i64::MAX, 1, 1, 1), i64::MIN);
}
