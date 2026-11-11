//! Tier 1 mirrors test_output_encoder_getters' null_output, video_slots, audio_slots.
use obs_output::output_encoder_getters::{audio_encoder, video_encoder};
use obs_output::output_slots::{MAX_OUTPUT_AUDIO_ENCODERS, MAX_OUTPUT_VIDEO_ENCODERS};

#[test]
fn null_output_logs_once_per_getter() {
    let mut logs = Vec::new();
    assert_eq!(
        video_encoder::<Option<usize>>(
            false,
            || panic!("null mask"),
            |_| panic!("null read"),
            |name| logs.push(name),
        ),
        None,
    );
    assert_eq!(logs, [c"obs_output_get_video_encoder"]);
    for idx in [0, MAX_OUTPUT_AUDIO_ENCODERS, usize::MAX] {
        logs = Vec::new();
        assert_eq!(
            audio_encoder::<Option<usize>>(
                false,
                idx,
                |_| panic!("null read"),
                |name| logs.push(name),
            ),
            None,
        );
        assert_eq!(logs, [c"obs_output_get_audio_encoder"]);
    }
}

#[test]
fn first_occupied_video_slot_never_checks_flags() {
    let mut logs = Vec::new();
    let mut slots = [0; MAX_OUTPUT_VIDEO_ENCODERS];
    assert_eq!(
        video_encoder::<Option<usize>>(
            true,
            || 0,
            |_| panic!("unoccupied"),
            |name| logs.push(name),
        ),
        None,
    );
    for first in 0..MAX_OUTPUT_VIDEO_ENCODERS {
        slots[first] = first + 1;
        for last in first..MAX_OUTPUT_VIDEO_ENCODERS {
            slots[last] = last + 1;
            assert_eq!(
                video_encoder::<Option<usize>>(
                    true,
                    || mask_of(slots),
                    |i| slot_of(slots, i),
                    |name| logs.push(name),
                ),
                Some(first + 1),
            );
        }
        /* Characterized, not endorsed: clearing the lowest occupied slot advances. */
        for idx in first..MAX_OUTPUT_VIDEO_ENCODERS {
            assert_eq!(
                video_encoder::<Option<usize>>(
                    true,
                    || mask_of(slots),
                    |i| slot_of(slots, i),
                    |name| logs.push(name),
                ),
                Some(idx + 1),
            );
            slots[idx] = 0;
        }
        assert_eq!(
            video_encoder::<Option<usize>>(
                true,
                || mask_of(slots),
                |i| slot_of(slots, i),
                |name| logs.push(name),
            ),
            None,
        );
    }
    assert_eq!(logs.len(), 0);
}

fn mask_of(slots: [usize; MAX_OUTPUT_VIDEO_ENCODERS]) -> u32 {
    let mut mask = 0u32;
    for idx in 0..MAX_OUTPUT_VIDEO_ENCODERS {
        if slots[idx] != 0 {
            mask |= 1u32 << idx;
        }
    }
    mask
}

fn slot_of(slots: [usize; MAX_OUTPUT_VIDEO_ENCODERS], idx: usize) -> Option<usize> {
    if idx < MAX_OUTPUT_VIDEO_ENCODERS && slots[idx] != 0 {
        Some(slots[idx])
    } else {
        None
    }
}

#[test]
fn audio_getter_bounds_only_never_flags() {
    let mut logs = Vec::new();
    let mut slots = [0; MAX_OUTPUT_AUDIO_ENCODERS];
    for idx in 0..MAX_OUTPUT_AUDIO_ENCODERS {
        assert_eq!(
            audio_encoder::<Option<usize>>(
                true,
                idx,
                |_| panic!("empty read"),
                |name| logs.push(name),
            ),
            None,
        );
        slots[idx] = idx + 1;
    }
    for empty in 0..MAX_OUTPUT_AUDIO_ENCODERS {
        slots[empty] = 0;
        for idx in 0..MAX_OUTPUT_AUDIO_ENCODERS {
            assert_eq!(
                audio_encoder::<Option<usize>>(
                    true,
                    idx,
                    |i| if slots[i] != 0 { Some(slots[i]) } else { None },
                    |name| logs.push(name),
                ),
                if slots[idx] != 0 {
                    Some(slots[idx])
                } else {
                    None
                },
            );
        }
        for idx in [
            MAX_OUTPUT_AUDIO_ENCODERS,
            MAX_OUTPUT_AUDIO_ENCODERS + 1,
            usize::MAX,
        ] {
            assert_eq!(
                audio_encoder::<Option<usize>>(
                    true,
                    idx,
                    |_| panic!("out of bounds"),
                    |name| logs.push(name),
                ),
                None,
            );
        }
        slots[empty] = empty + 1;
    }
    assert_eq!(logs.len(), 0);
}
