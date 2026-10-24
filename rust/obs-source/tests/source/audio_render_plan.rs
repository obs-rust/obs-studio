//! Tier 1: `obs_source::audio_render_plan`, the audio render mix plan of
//! `obs_source_audio_render()`, `custom_audio_render()` and
//! `process_audio_source_tick()`.
//!
//! The `steps_*` cases port `test/cmocka/test_source_audio_render_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the stand-in source and the copies of the three
//! callers (the lock, the peeks, every memset and memcpy, the callbacks and
//! the volume recorded) included.
//!
//! The `edge_*` cases are new: every decision against its rule written
//! another way, over every mix index up to 64 and the mask edges, and
//! proptests over any masks, any `u64` timestamps and any `usize` sizes;
//! the callers over any inputs against the rules of the C written out; and
//! the intentional difference, a mix of 32 or more naming no mix.

use obs_source::audio_render_plan::{
    MAX_AUDIO_MIXES, Route, custom_clear_after, custom_clear_before, custom_in_tick,
    custom_pending, custom_ts, custom_unmixed, route, tick_clear_main, tick_copy, tick_short,
    tick_stop, tick_unset, tick_volume,
};
use proptest::prelude::*;

const MIXES: usize = 6;
const CHANNELS: usize = 8;
const FRAMES: usize = 1024;
const TICK_BYTES: usize = FRAMES * 4;

const MASKS: [u32; 18] = [
    0,
    1,
    2,
    0x20,
    0x3e,
    0x3f,
    0x40,
    0x41,
    0x7f,
    0x80,
    0xff,
    0x100,
    0xfc0,
    0x10000,
    0x7fff_ffff,
    0x8000_0000,
    0xffff_fffe,
    u32::MAX,
];

fn has_mix(mask: u32, mix: usize) -> bool {
    mix < 32 && (mask >> mix) & 1 != 0
}

// ---------------------------------------------------------------------------
// The callers, copied from obs-source.c onto a stand-in source

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Lock,
    Unlock,
    Peek(usize, usize),
    Memset(usize, usize),
    Memcpy(usize, usize),
    Render(u32, usize),
    Submix(usize),
    Volume(u32, usize),
}

#[derive(Debug, Clone)]
struct StandIn {
    has_buffer: bool,
    custom: bool,
    has_data: bool,
    has_mix: bool,
    submix: bool,
    audio_ts: u64,
    audio_pending: bool,
    audio_mixers: u32,
    input_size: usize,
    cb_success: bool,
    cb_ts: u64,
    calls: Vec<Call>,
}

fn source() -> StandIn {
    StandIn {
        has_buffer: true,
        custom: false,
        has_data: true,
        has_mix: false,
        submix: false,
        audio_ts: 1000,
        audio_pending: false,
        audio_mixers: 0x3f,
        input_size: TICK_BYTES,
        cb_success: true,
        cb_ts: 2000,
        calls: Vec::new(),
    }
}

fn custom_audio_render(s: &mut StandIn, mixers: u32, channels: usize) {
    for mix in 0..MIXES {
        if custom_clear_before(s.audio_mixers, mixers, mix) {
            s.calls.push(Call::Memset(mix, TICK_BYTES * channels));
        }
    }
    s.calls.push(Call::Render(mixers, channels));
    let success = s.cb_success;
    let ts = s.cb_ts;
    s.calls.push(Call::Lock);
    s.audio_ts = custom_ts(success, ts);
    s.audio_pending = custom_pending(success);
    if custom_pending(success) || custom_unmixed(s.audio_ts, mixers) {
        s.calls.push(Call::Unlock);
        return;
    }
    for mix in 0..MIXES {
        if !custom_in_tick(mixers, mix) {
            continue;
        }
        if custom_clear_after(s.audio_mixers, mix) {
            s.calls.push(Call::Memset(mix, TICK_BYTES * channels));
        }
    }
    s.calls.push(Call::Volume(mixers, channels));
    s.calls.push(Call::Unlock);
}

fn process_audio_source_tick(s: &mut StandIn, mixers: u32, channels: usize, size: usize) {
    let audio_submix = s.submix;
    s.calls.push(Call::Lock);
    if tick_unset(s.audio_ts) || tick_short(s.input_size, size) {
        s.audio_pending = true;
        s.calls.push(Call::Unlock);
        return;
    }
    for ch in 0..channels {
        s.calls.push(Call::Peek(ch, size));
    }
    for mix in 1..MIXES {
        if tick_stop(audio_submix, mix) {
            break;
        }
        if !tick_copy(audio_submix, s.audio_mixers, mixers, mix) {
            s.calls.push(Call::Memset(mix, size.wrapping_mul(channels)));
            continue;
        }
        for ch in 0..channels {
            s.calls.push(Call::Memcpy(mix * CHANNELS + ch, size));
        }
    }
    if !tick_volume(audio_submix) {
        s.audio_pending = false;
        s.calls.push(Call::Unlock);
        return;
    }
    if tick_clear_main(s.audio_mixers, mixers) {
        s.calls.push(Call::Memset(0, size.wrapping_mul(channels)));
    }
    s.calls.push(Call::Volume(mixers, channels));
    s.audio_pending = false;
    s.calls.push(Call::Unlock);
}

fn audio_render(s: &mut StandIn, mixers: u32, channels: usize, size: usize) {
    let has_buffer = s.has_buffer;
    let custom = has_buffer && s.custom;
    let has_data = custom && s.has_data;
    match route(has_buffer, custom, has_data) {
        Route::Pending => {
            s.audio_pending = true;
            return;
        }
        Route::Custom => {
            custom_audio_render(s, mixers, channels);
            return;
        }
        Route::Tick => {}
    }
    if s.has_mix {
        s.calls.push(Call::Submix(channels));
    }
    process_audio_source_tick(s, mixers, channels, size);
}

// ---------------------------------------------------------------------------
// steps_*: test_source_audio_render_steps.c, 1:1

#[test]
fn steps_route_values() {
    assert_eq!(u32::from(Route::Pending), 0);
    assert_eq!(u32::from(Route::Custom), 1);
    assert_eq!(u32::from(Route::Tick), 2);
    assert_eq!(Route::Pending as u32, 0);
    assert_eq!(Route::Custom as u32, 1);
    assert_eq!(Route::Tick as u32, 2);
    assert_eq!(MAX_AUDIO_MIXES, MIXES);
}

#[test]
fn steps_route_every_input() {
    for bits in 0..8_u8 {
        let (has_buffer, custom, has_data) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
        let expected = if !has_buffer {
            Route::Pending
        } else if !custom {
            Route::Tick
        } else if has_data {
            Route::Custom
        } else {
            Route::Pending
        };
        assert_eq!(route(has_buffer, custom, has_data), expected, "bits {bits}");
    }
    assert_eq!(route(false, false, false), Route::Pending);
    assert_eq!(route(false, true, true), Route::Pending);
    assert_eq!(route(true, true, false), Route::Pending);
    assert_eq!(route(true, true, true), Route::Custom);
    // data does not matter without a custom render
    assert_eq!(route(true, false, false), Route::Tick);
    assert_eq!(route(true, false, true), Route::Tick);
}

#[test]
fn steps_custom_clear_before_masks() {
    assert!(custom_clear_before(0x3f, 0x3f, 0));
    assert!(custom_clear_before(0x3f, 0x3f, 5));
    assert!(!custom_clear_before(0x01, 0x3f, 1));
    assert!(!custom_clear_before(0x3f, 0x01, 1));
    assert!(!custom_clear_before(0, u32::MAX, 0));
    assert!(!custom_clear_before(u32::MAX, 0, 0));
    assert!(custom_clear_before(u32::MAX, u32::MAX, 3));
    // bits past the six mixes are never asked for
    assert!(!custom_clear_before(0x40, 0x40, 0));

    for a in MASKS {
        for m in MASKS {
            for mix in 0..MIXES {
                assert_eq!(
                    custom_clear_before(a, m, mix),
                    has_mix(a, mix) && has_mix(m, mix),
                    "{a:#x} {m:#x} {mix}"
                );
            }
        }
    }
    for a in 0..64_u32 {
        for m in 0..64_u32 {
            for mix in 0..MIXES {
                assert_eq!(custom_clear_before(a, m, mix), has_mix(a & m, mix));
            }
        }
    }
}

#[test]
fn steps_custom_result() {
    for ts in [0, 1, 1_000_000_000, 0x8000_0000_0000_0000, u64::MAX] {
        assert_eq!(custom_ts(true, ts), ts);
        assert_eq!(custom_ts(false, ts), 0);
    }
    assert!(!custom_pending(true));
    assert!(custom_pending(false));
}

#[test]
fn steps_custom_unmixed() {
    for ts in [0, 1, u64::MAX] {
        for m in MASKS {
            assert_eq!(custom_unmixed(ts, m), ts == 0 || m == 0);
        }
    }
    assert!(custom_unmixed(0, u32::MAX));
    assert!(custom_unmixed(1, 0));
    assert!(!custom_unmixed(1, 1));
    assert!(!custom_unmixed(1, 0x40));
    assert!(!custom_unmixed(u64::MAX, 0x8000_0000));
}

#[test]
fn steps_custom_clear_after_masks() {
    for m in MASKS {
        for mix in 0..MIXES {
            assert_eq!(custom_in_tick(m, mix), has_mix(m, mix));
            assert_eq!(custom_clear_after(m, mix), !has_mix(m, mix));
        }
    }
    assert!(!custom_in_tick(0, 0));
    assert!(custom_in_tick(1, 0));
    assert!(custom_in_tick(0x20, 5));
    assert!(!custom_in_tick(0x40, 5));
    assert!(custom_clear_after(0, 0));
    assert!(!custom_clear_after(0x3f, 5));
    assert!(custom_clear_after(0x1f, 5));
    assert!(!custom_clear_after(u32::MAX, 0));
}

#[test]
fn steps_tick_pending_edges() {
    assert!(tick_unset(0));
    assert!(!tick_unset(1));
    assert!(!tick_unset(u64::MAX));

    let size = TICK_BYTES;
    assert!(tick_short(size - 1, size));
    assert!(!tick_short(size, size));
    assert!(!tick_short(size + 1, size));
    assert!(tick_short(0, size));
    assert!(!tick_short(0, 0));
    assert!(!tick_short(usize::MAX, size));
    assert!(tick_short(usize::MAX - 1, usize::MAX));
    assert!(!tick_short(usize::MAX, usize::MAX));
    assert!(tick_short(0, 1));
}

#[test]
fn steps_tick_stop_mixes() {
    for mix in 0..=64 {
        assert!(!tick_stop(false, mix));
        assert_eq!(tick_stop(true, mix), mix > 1);
    }
    assert!(tick_stop(true, usize::MAX));
    assert!(tick_volume(false));
    assert!(!tick_volume(true));
}

#[test]
fn steps_tick_copy_masks() {
    for a in 0..64_u32 {
        for m in 0..64_u32 {
            for mix in 1..MIXES {
                assert_eq!(
                    tick_copy(false, a, m, mix),
                    has_mix(a, mix) && has_mix(m, mix)
                );
                assert_eq!(tick_copy(true, a, m, mix), has_mix(a, 0));
            }
        }
    }
    for a in MASKS {
        for m in MASKS {
            for mix in 0..MIXES {
                assert_eq!(
                    tick_copy(false, a, m, mix),
                    has_mix(a, mix) && has_mix(m, mix)
                );
                assert_eq!(tick_copy(true, a, m, mix), has_mix(a, 0));
            }
        }
    }
    // submix: mix 1 from bit 0 even with no tick mixers, not from bit 1
    assert!(tick_copy(true, 1, 0, 1));
    assert!(!tick_copy(true, 2, u32::MAX, 1));
    assert!(tick_copy(false, 2, 2, 1));
    assert!(!tick_copy(false, 1, 1, 1));
    // a submix source's mix index is not shifted, so any index answers
    assert!(tick_copy(true, 1, 0, 64));
}

#[test]
fn steps_tick_clear_main_masks() {
    for a in MASKS {
        for m in MASKS {
            assert_eq!(tick_clear_main(a, m), !(has_mix(a, 0) && has_mix(m, 0)));
        }
    }
    assert!(!tick_clear_main(1, 1));
    assert!(tick_clear_main(0x3e, 0x3f));
    assert!(tick_clear_main(0x3f, 0x3e));
    assert!(!tick_clear_main(u32::MAX, u32::MAX));
    assert!(tick_clear_main(0, 0));
}

#[test]
fn steps_render_no_buffer() {
    for bits in 0..8_u8 {
        let mut s = source();
        s.has_buffer = false;
        s.custom = bits & 1 != 0;
        s.has_data = bits & 2 != 0;
        s.has_mix = bits & 4 != 0;
        audio_render(&mut s, 0x3f, 2, TICK_BYTES);
        assert!(s.audio_pending);
        assert!(s.calls.is_empty());
        assert_eq!(s.audio_ts, 1000);
    }
}

#[test]
fn steps_render_custom_no_data() {
    let mut s = source();
    s.custom = true;
    s.has_data = false;
    s.has_mix = true;
    audio_render(&mut s, 0x3f, 2, TICK_BYTES);
    assert!(s.audio_pending);
    assert!(s.calls.is_empty());
    assert_eq!(s.audio_ts, 1000);
}

#[test]
fn steps_render_custom_success() {
    use Call::*;
    let mut s = source();
    s.custom = true;
    s.has_mix = true;
    s.audio_pending = true;
    s.audio_mixers = 0x05;
    audio_render(&mut s, 0x03, 2, TICK_BYTES);
    assert_eq!(
        s.calls,
        [
            Memset(0, 2 * TICK_BYTES),
            Render(0x03, 2),
            Lock,
            Memset(1, 2 * TICK_BYTES),
            Volume(0x03, 2),
            Unlock
        ]
    );
    assert_eq!(s.audio_ts, 2000);
    assert!(!s.audio_pending);

    // every mix in both masks, so nothing cleared after
    let mut s = source();
    s.custom = true;
    audio_render(&mut s, 0x3f, 8, TICK_BYTES);
    let mut expected: Vec<Call> = (0..MIXES).map(|m| Memset(m, 8 * TICK_BYTES)).collect();
    expected.extend([Render(0x3f, 8), Lock, Volume(0x3f, 8), Unlock]);
    assert_eq!(s.calls, expected);

    // the source on no mix: nothing before, every tick mix after
    let mut s = source();
    s.custom = true;
    s.audio_mixers = 0;
    audio_render(&mut s, 0x21, 1, TICK_BYTES);
    assert_eq!(
        s.calls,
        [
            Render(0x21, 1),
            Lock,
            Memset(0, TICK_BYTES),
            Memset(5, TICK_BYTES),
            Volume(0x21, 1),
            Unlock
        ]
    );

    // characterized, not endorsed: tick mixers past the six mixes clear
    // nothing and still reach the volume
    let mut s = source();
    s.custom = true;
    audio_render(&mut s, 0x40, 2, TICK_BYTES);
    assert_eq!(s.calls, [Render(0x40, 2), Lock, Volume(0x40, 2), Unlock]);
    let mut s = source();
    s.custom = true;
    s.audio_mixers = u32::MAX;
    audio_render(&mut s, u32::MAX, 2, TICK_BYTES);
    assert_eq!(s.calls.len(), 6 + 4);
}

#[test]
fn steps_render_custom_failure() {
    use Call::*;
    let mut s = source();
    s.custom = true;
    s.cb_success = false;
    s.cb_ts = 5000;
    s.audio_mixers = 0x01;
    audio_render(&mut s, 0x3f, 2, TICK_BYTES);
    assert_eq!(
        s.calls,
        [Memset(0, 2 * TICK_BYTES), Render(0x3f, 2), Lock, Unlock]
    );
    assert_eq!(s.audio_ts, 0);
    assert!(s.audio_pending);
}

#[test]
fn steps_render_custom_unmixed() {
    use Call::*;
    let mut s = source();
    s.custom = true;
    s.cb_ts = 0;
    s.audio_pending = true;
    audio_render(&mut s, 0x01, 2, TICK_BYTES);
    assert_eq!(
        s.calls,
        [Memset(0, 2 * TICK_BYTES), Render(0x01, 2), Lock, Unlock]
    );
    assert_eq!(s.audio_ts, 0);
    assert!(!s.audio_pending);

    let mut s = source();
    s.custom = true;
    audio_render(&mut s, 0, 2, TICK_BYTES);
    assert_eq!(s.calls, [Render(0, 2), Lock, Unlock]);
    assert_eq!(s.audio_ts, 2000);
    assert!(!s.audio_pending);
}

#[test]
fn steps_render_tick_pending() {
    use Call::*;
    let mut s = source();
    s.audio_ts = 0;
    s.has_mix = true;
    audio_render(&mut s, 0x3f, 2, TICK_BYTES);
    assert_eq!(s.calls, [Submix(2), Lock, Unlock]);
    assert!(s.audio_pending);

    let mut s = source();
    s.input_size = TICK_BYTES - 1;
    audio_render(&mut s, 0x3f, 2, TICK_BYTES);
    assert_eq!(s.calls, [Lock, Unlock]);
    assert!(s.audio_pending);

    let mut s = source();
    s.input_size = 0;
    s.audio_ts = u64::MAX;
    audio_render(&mut s, 0x3f, 2, 1);
    assert!(s.audio_pending);

    // exactly enough
    let mut s = source();
    s.audio_pending = true;
    audio_render(&mut s, 0x01, 1, TICK_BYTES);
    assert_eq!(
        s.calls,
        [
            Lock,
            Peek(0, TICK_BYTES),
            Memset(1, TICK_BYTES),
            Memset(2, TICK_BYTES),
            Memset(3, TICK_BYTES),
            Memset(4, TICK_BYTES),
            Memset(5, TICK_BYTES),
            Volume(0x01, 1),
            Unlock
        ]
    );
    assert!(!s.audio_pending);

    // more than enough, and a size of 0 with no input
    let mut s = source();
    s.input_size = TICK_BYTES + 1;
    audio_render(&mut s, 0x01, 1, TICK_BYTES);
    assert!(!s.audio_pending);
    let mut s = source();
    s.input_size = 0;
    audio_render(&mut s, 0x01, 1, 0);
    assert!(!s.audio_pending);
    assert_eq!(s.calls[1], Peek(0, 0));
}

/// The tick's calls written out from the C's rules.
fn expected_tick(audio_mixers: u32, mixers: u32, channels: usize, size: usize) -> Vec<Call> {
    let mut expected = vec![Call::Lock];
    expected.extend((0..channels).map(|ch| Call::Peek(ch, size)));
    for mix in 1..MIXES {
        if has_mix(audio_mixers, mix) && has_mix(mixers, mix) {
            expected.extend((0..channels).map(|ch| Call::Memcpy(mix * CHANNELS + ch, size)));
        } else {
            expected.push(Call::Memset(mix, size.wrapping_mul(channels)));
        }
    }
    if !(has_mix(audio_mixers, 0) && has_mix(mixers, 0)) {
        expected.push(Call::Memset(0, size.wrapping_mul(channels)));
    }
    expected.extend([Call::Volume(mixers, channels), Call::Unlock]);
    expected
}

#[test]
fn steps_render_tick_every_mask() {
    let masks: Vec<u32> = (0..64).chain(MASKS).collect();
    for &audio_mixers in &masks {
        for &mixers in &masks {
            let mut s = source();
            s.audio_mixers = audio_mixers;
            audio_render(&mut s, mixers, 2, TICK_BYTES);
            assert_eq!(
                s.calls,
                expected_tick(audio_mixers, mixers, 2, TICK_BYTES),
                "{audio_mixers:#x} {mixers:#x}"
            );
            assert!(!s.audio_pending);
        }
    }
}

#[test]
fn steps_render_tick_submix() {
    use Call::*;
    for a in MASKS {
        for m in MASKS {
            let mut s = source();
            s.submix = true;
            s.has_mix = true;
            s.audio_pending = true;
            s.audio_mixers = a;
            audio_render(&mut s, m, 2, TICK_BYTES);
            if has_mix(a, 0) {
                assert_eq!(
                    s.calls,
                    [
                        Submix(2),
                        Lock,
                        Peek(0, TICK_BYTES),
                        Peek(1, TICK_BYTES),
                        Memcpy(CHANNELS, TICK_BYTES),
                        Memcpy(CHANNELS + 1, TICK_BYTES),
                        Unlock
                    ]
                );
            } else {
                assert_eq!(
                    s.calls,
                    [
                        Submix(2),
                        Lock,
                        Peek(0, TICK_BYTES),
                        Peek(1, TICK_BYTES),
                        Memset(1, 2 * TICK_BYTES),
                        Unlock
                    ]
                );
            }
            assert!(!s.audio_pending);
        }
    }

    // without a submix callback, and with no channels
    let mut s = source();
    s.submix = true;
    s.audio_mixers = 1;
    audio_render(&mut s, 0, 0, TICK_BYTES);
    assert_eq!(s.calls, [Lock, Unlock]);
}

#[test]
fn steps_render_mix_callback_without_flag() {
    let mut s = source();
    s.has_mix = true;
    audio_render(&mut s, 0x01, 1, TICK_BYTES);
    assert_eq!(s.calls[0], Call::Submix(1));
    assert_eq!(s.calls[s.calls.len() - 2], Call::Volume(0x01, 1));
}

// ---------------------------------------------------------------------------
// edge_*: new

/// Every mix index up to 64: the bit tests against `u64` shifts, and a mix
/// of 32 or more names no mix (an intentional difference: the C's `int`
/// shift is undefined from 31 on; libobs passes a mix below 6).
#[test]
fn edge_mix_index_past_the_mask() {
    for mix in 0..=64_usize {
        for a in MASKS {
            for m in MASKS {
                let in_a = mix < 32 && (u64::from(a) >> mix) & 1 == 1;
                let in_m = mix < 32 && (u64::from(m) >> mix) & 1 == 1;
                assert_eq!(
                    custom_clear_before(a, m, mix),
                    in_a && in_m,
                    "{a:#x} {m:#x} {mix}"
                );
                assert_eq!(custom_in_tick(m, mix), in_m);
                assert_eq!(custom_clear_after(a, mix), !in_a);
                assert_eq!(tick_copy(false, a, m, mix), in_a && in_m);
                assert_eq!(tick_copy(true, a, m, mix), a & 1 == 1);
            }
        }
    }
    assert!(custom_clear_before(0x8000_0000, 0x8000_0000, 31));
    assert!(!custom_clear_before(u32::MAX, u32::MAX, 32));
    assert!(custom_clear_after(u32::MAX, usize::MAX));
    assert!(!custom_in_tick(u32::MAX, usize::MAX));
    assert!(!tick_copy(false, u32::MAX, u32::MAX, usize::MAX));
}

/// The custom render with any inputs, against the C's rules written out.
fn expected_custom(
    audio_mixers: u32,
    mixers: u32,
    channels: usize,
    success: bool,
    ts: u64,
) -> Vec<Call> {
    let mut expected = Vec::new();
    for mix in 0..MIXES {
        if has_mix(audio_mixers & mixers, mix) {
            expected.push(Call::Memset(mix, TICK_BYTES * channels));
        }
    }
    expected.extend([Call::Render(mixers, channels), Call::Lock]);
    if !success || ts == 0 || mixers == 0 {
        expected.push(Call::Unlock);
        return expected;
    }
    for mix in 0..MIXES {
        if has_mix(mixers, mix) && !has_mix(audio_mixers, mix) {
            expected.push(Call::Memset(mix, TICK_BYTES * channels));
        }
    }
    expected.extend([Call::Volume(mixers, channels), Call::Unlock]);
    expected
}

fn mask() -> impl Strategy<Value = u32> {
    prop_oneof![
        prop::sample::select(MASKS.to_vec()),
        0..64_u32,
        any::<u32>()
    ]
}

fn timestamp() -> impl Strategy<Value = u64> {
    prop_oneof![Just(0_u64), Just(1), Just(u64::MAX), any::<u64>()]
}

fn size() -> impl Strategy<Value = usize> {
    prop_oneof![
        Just(0_usize),
        Just(TICK_BYTES),
        Just(usize::MAX),
        any::<usize>()
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn edge_decisions_any_values(
        a in mask(),
        m in mask(),
        mix in 0..80_usize,
        ts in timestamp(),
        input in size(),
        want in size(),
        success: bool,
        submix: bool,
    ) {
        let in_a = mix < 32 && (a >> mix) & 1 == 1;
        let in_m = mix < 32 && (m >> mix) & 1 == 1;
        prop_assert_eq!(custom_clear_before(a, m, mix), in_a && in_m);
        prop_assert_eq!(custom_in_tick(m, mix), in_m);
        prop_assert_eq!(custom_clear_after(a, mix), !in_a);
        prop_assert_eq!(custom_ts(success, ts), if success { ts } else { 0 });
        prop_assert_eq!(custom_pending(success), !success);
        prop_assert_eq!(custom_unmixed(ts, m), ts == 0 || m == 0);
        prop_assert_eq!(tick_unset(ts), ts == 0);
        prop_assert_eq!(tick_short(input, want), u128::try_from(input).unwrap() < u128::try_from(want).unwrap());
        prop_assert_eq!(tick_stop(submix, mix), submix && mix >= 2);
        prop_assert_eq!(tick_copy(submix, a, m, mix), if submix { a & 1 == 1 } else { in_a && in_m });
        prop_assert_eq!(tick_volume(submix), !submix);
        prop_assert_eq!(tick_clear_main(a, m), a & m & 1 == 0);
    }

    #[test]
    fn edge_render_any_inputs(
        bits in 0..64_u8,
        audio_ts in timestamp(),
        audio_mixers in mask(),
        mixers in mask(),
        input_size in size(),
        tick_size in size(),
        channels in 0..=CHANNELS,
        cb_ts in timestamp(),
    ) {
        let mut s = source();
        s.has_buffer = bits & 1 != 0;
        s.custom = bits & 2 != 0;
        s.has_data = bits & 4 != 0;
        s.has_mix = bits & 8 != 0;
        s.submix = bits & 16 != 0;
        s.cb_success = bits & 32 != 0;
        s.audio_ts = audio_ts;
        s.audio_mixers = audio_mixers;
        s.input_size = input_size;
        s.cb_ts = cb_ts;
        let before = s.clone();
        audio_render(&mut s, mixers, channels, tick_size);

        if !before.has_buffer || (before.custom && !before.has_data) {
            prop_assert!(s.calls.is_empty());
            prop_assert!(s.audio_pending);
            prop_assert_eq!(s.audio_ts, audio_ts);
        } else if before.custom {
            prop_assert_eq!(&s.calls, &expected_custom(audio_mixers, mixers, channels, before.cb_success, cb_ts));
            prop_assert_eq!(s.audio_ts, if before.cb_success { cb_ts } else { 0 });
            prop_assert_eq!(s.audio_pending, !before.cb_success);
        } else {
            let calls = if before.has_mix { &s.calls[1..] } else { &s.calls[..] };
            if before.has_mix {
                prop_assert_eq!(s.calls[0], Call::Submix(channels));
            }
            if audio_ts == 0 || input_size < tick_size {
                prop_assert_eq!(calls, &[Call::Lock, Call::Unlock][..]);
                prop_assert!(s.audio_pending);
            } else if before.submix {
                let mut expected = vec![Call::Lock];
                expected.extend((0..channels).map(|ch| Call::Peek(ch, tick_size)));
                if audio_mixers & 1 == 1 {
                    expected.extend((0..channels).map(|ch| Call::Memcpy(CHANNELS + ch, tick_size)));
                } else {
                    expected.push(Call::Memset(1, tick_size.wrapping_mul(channels)));
                }
                expected.push(Call::Unlock);
                prop_assert_eq!(calls, &expected[..]);
                prop_assert!(!s.audio_pending);
            } else {
                prop_assert_eq!(calls, &expected_tick(audio_mixers, mixers, channels, tick_size)[..]);
                prop_assert!(!s.audio_pending);
            }
            prop_assert_eq!(s.audio_ts, audio_ts);
        }
    }
}
