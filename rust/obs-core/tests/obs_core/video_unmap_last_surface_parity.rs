//! Tier 3: the original C staging cleanup and the Rust one through the
//! private ABI, over the same real mix and the production bridge's accessors.
//! No intentional differences: every unmap (its surface and every slot as the
//! callback sees it) and the final slots match exactly.
use super::video_unmap_last_surface::{CHANNELS, Model, slots, unmaps};
use core::ffi::c_void;
use obs_c_oracle::video_unmap_last_surface::*;
use obs_core::ffi::video_unmap_last_surface::obs_video_unmap_last_surface;
use obs_core::video_unmap_last_surface::unmap_last_surface;
use proptest::prelude::*;
use std::cell::RefCell;

type Slots = [Option<usize>; CHANNELS];

struct Run {
    video: *mut c_void,
    unmaps: Vec<(usize, Slots)>,
}
thread_local! { static RUN: RefCell<Option<Run>> = const { RefCell::new(None) }; }

/// The mix's slots, read directly.
fn read(video: *mut c_void) -> Slots {
    core::array::from_fn(|c| {
        // SAFETY: a live mix, c < NUM_CHANNELS.
        let surface = unsafe { oracle_unmap_last_surface_slot(video, c) };
        (!surface.is_null()).then_some(surface as usize)
    })
}

unsafe extern "C" fn unmap(surface: *mut c_void) {
    RUN.with_borrow_mut(|run| {
        let run = run.as_mut().expect("a run is active");
        let seen = read(run.video);
        run.unmaps.push((surface as usize, seen));
    })
}

/// The slots `start` after `cleanups` runs, and every unmap.
fn run(start: &Slots, cleanups: usize, original: bool) -> (Slots, Vec<(usize, Slots)>) {
    // SAFETY: freed below; a NULL mix fails the assert first.
    let video = unsafe { oracle_unmap_last_surface_new() };
    assert!(!video.is_null());
    for (c, slot) in start.iter().enumerate() {
        // SAFETY: a live mix, c < NUM_CHANNELS; the token is never read.
        unsafe { oracle_unmap_last_surface_set(video, c, slot.unwrap_or(0) as *mut c_void) };
    }
    RUN.with_borrow_mut(|run| {
        *run = Some(Run {
            video,
            unmaps: Vec::new(),
        })
    });
    for _ in 0..cleanups {
        // SAFETY: a live mix of NUM_CHANNELS slots, the bridge's accessors
        // and a non-unwinding unmap (an assert in it aborts the test).
        unsafe {
            if original {
                oracle_unmap_last_surface_original(video, unmap);
            } else {
                obs_video_unmap_last_surface(
                    video,
                    oracle_unmap_last_surface_channels(),
                    oracle_unmap_last_surface_mapped,
                    oracle_unmap_last_surface_clear,
                    unmap,
                );
            }
        }
    }
    let unmaps = RUN.with_borrow_mut(|run| run.take().expect("still active").unmaps);
    let end = read(video);
    // SAFETY: the same live mix.
    unsafe { oracle_unmap_last_surface_free(video) };
    (end, unmaps)
}

/// `cleanups` (at least one) runs over `start`.
fn check(start: Slots, cleanups: usize) {
    let expected = run(&start, cleanups, true);
    assert_eq!(run(&start, cleanups, false), expected);
    // The C unmaps each mapped slot once, whatever the number of cleanups.
    assert_eq!(expected.0, [None; CHANNELS]);
    assert_eq!(expected.1.len(), start.iter().flatten().count());
    // The safe core over a model makes the same unmaps.
    let mut model = Model {
        slots: start,
        ..Model::default()
    };
    for _ in 0..cleanups {
        unmap_last_surface(&mut model);
    }
    assert_eq!(model.slots, expected.0);
    assert_eq!(unmaps(&model.calls), expected.1);
}

#[test]
fn the_bridge_covers_num_channels() {
    // SAFETY: a constant.
    assert_eq!(unsafe { oracle_unmap_last_surface_channels() }, CHANNELS);
}

#[test]
fn every_channel_mask_once_and_repeated() {
    for mask in 0..(1u32 << CHANNELS) {
        for cleanups in 1..=3 {
            check(slots(mask), cleanups);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]
    #[test]
    fn arbitrary_surfaces(
        start in proptest::array::uniform3(proptest::option::of(1usize..=usize::MAX)),
        cleanups in 1usize..=3,
    ) {
        check(start, cleanups);
    }
}
