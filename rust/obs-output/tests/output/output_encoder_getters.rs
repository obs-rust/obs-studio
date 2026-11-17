//! Tier 1 mirrors of test_output_encoder_getters.c over a model output: the
//! slots are a plain array, a NULL output is `valid == false`, and every read
//! and call is recorded in order.
use core::cell::RefCell;
use core::ffi::CStr;
use obs_output::output_encoder_getters::{audio_encoder, video_encoder};
use obs_output::output_slots::{MAX_OUTPUT_AUDIO_ENCODERS, MAX_OUTPUT_VIDEO_ENCODERS};

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Log(&'static CStr),
    Occupied(usize),
    Get(usize),
    Read(usize),
}

/// The model output's video slots, read through the getter's closures.
fn video(valid: bool, slots: &[Option<u32>]) -> (Option<u32>, Vec<Event>) {
    let events = RefCell::new(Vec::new());
    let got = video_encoder(
        valid,
        slots.len(),
        |i| {
            events.borrow_mut().push(Event::Occupied(i));
            slots[i].is_some()
        },
        |i| {
            events.borrow_mut().push(Event::Get(i));
            slots[i]
        },
        |name| events.borrow_mut().push(Event::Log(name)),
    );
    (got, events.into_inner())
}

fn audio(valid: bool, slots: &[Option<u32>], idx: usize) -> (Option<u32>, Vec<Event>) {
    let events = RefCell::new(Vec::new());
    let got = audio_encoder(
        valid,
        idx,
        |i| {
            events.borrow_mut().push(Event::Read(i));
            slots[i]
        },
        |name| events.borrow_mut().push(Event::Log(name)),
    );
    (got, events.into_inner())
}

/// The reads of the occupancy mask: every slot, in order.
fn mask_reads() -> Vec<Event> {
    (0..MAX_OUTPUT_VIDEO_ENCODERS)
        .map(Event::Occupied)
        .collect()
}

#[test]
fn null_output() {
    let slots = [Some(1); MAX_OUTPUT_VIDEO_ENCODERS];
    assert_eq!(
        video(false, &slots),
        (None, vec![Event::Log(c"obs_output_get_video_encoder")])
    );
    let audio_slots = [Some(1); MAX_OUTPUT_AUDIO_ENCODERS];
    for idx in [0, MAX_OUTPUT_AUDIO_ENCODERS, usize::MAX] {
        assert_eq!(
            audio(false, &audio_slots, idx),
            (None, vec![Event::Log(c"obs_output_get_audio_encoder")])
        );
    }
}

#[test]
fn video_slots() {
    let mut slots = [None; MAX_OUTPUT_VIDEO_ENCODERS];
    assert_eq!(video(true, &slots), (None, mask_reads()));

    // Characterized, not endorsed: the flags are never read, so even a
    // non-video output returns the first occupied slot, a nonzero one too.
    for first in 0..MAX_OUTPUT_VIDEO_ENCODERS {
        slots[first] = Some(100 + u32::try_from(first).unwrap());
        for last in first..MAX_OUTPUT_VIDEO_ENCODERS {
            slots[last] = Some(100 + u32::try_from(last).unwrap());
            let mut reads = mask_reads();
            reads.push(Event::Get(first));
            assert_eq!(video(true, &slots), (slots[first], reads));
        }
        // Removing the lowest occupied slot advances the borrowed result.
        for idx in first..MAX_OUTPUT_VIDEO_ENCODERS {
            assert_eq!(video(true, &slots).0, slots[idx]);
            slots[idx] = None;
        }
        assert_eq!(video(true, &slots), (None, mask_reads()));
    }
}

#[test]
fn video_get_result_is_returned_as_is() {
    // The C returns obs_output_get_video_encoder2()'s result unchanged.
    let events = RefCell::new(Vec::new());
    let got = video_encoder(
        true,
        MAX_OUTPUT_VIDEO_ENCODERS,
        |i| i == 3,
        |i| {
            events.borrow_mut().push(Event::Get(i));
            Some(7_u32)
        },
        |_| panic!("valid"),
    );
    assert_eq!(got, Some(7));
    assert_eq!(events.into_inner(), [Event::Get(3)]);
}

#[test]
fn video_mask_ignores_slots_past_the_mask_width() {
    // Slots from 32 up have no bit in the C's uint32_t mask.
    let mut slots = vec![None; 40];
    slots[35] = Some(9);
    let (got, events) = video(true, &slots);
    assert_eq!(got, None);
    assert_eq!(events.len(), 40);
}

#[test]
fn audio_slots() {
    let mut slots = [None; MAX_OUTPUT_AUDIO_ENCODERS];
    for idx in 0..MAX_OUTPUT_AUDIO_ENCODERS {
        assert_eq!(audio(true, &slots, idx), (None, vec![Event::Read(idx)]));
        slots[idx] = Some(200 + u32::try_from(idx).unwrap());
    }
    // Characterized, not endorsed: no flag or multitrack check for a
    // nonzero index.
    for empty in 0..MAX_OUTPUT_AUDIO_ENCODERS {
        let saved = slots[empty].take();
        for idx in 0..MAX_OUTPUT_AUDIO_ENCODERS {
            let expected = if idx == empty { None } else { slots[idx] };
            assert_eq!(audio(true, &slots, idx), (expected, vec![Event::Read(idx)]));
        }
        for idx in [
            MAX_OUTPUT_AUDIO_ENCODERS,
            MAX_OUTPUT_AUDIO_ENCODERS + 1,
            usize::MAX,
        ] {
            assert_eq!(audio(true, &slots, idx), (None, vec![]));
        }
        slots[empty] = saved;
    }
}
