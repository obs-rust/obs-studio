//! Tier 1 mirrors of test_source_audio_pending_speaker_layout.c without FFI
//! or unsafe code.
use core::ffi::{CStr, c_int};
use obs_source::audio_pending_speaker_layout::{
    OBS_SOURCE_AUDIO, OBS_SOURCE_COMPOSITE, PENDING_FUNCTION, Pending, SPEAKER_LAYOUT_FUNCTION,
    SPEAKERS_UNKNOWN, audio_pending, speaker_layout,
};
use std::cell::RefCell;

const OBS_SOURCE_VIDEO: u32 = 1 << 0;
const OBS_SOURCE_ASYNC_VIDEO: u32 = (1 << 2) | OBS_SOURCE_VIDEO;

/// The flag words the cmocka grid walks.
const FLAG_WORDS: [u32; 12] = [
    0,
    OBS_SOURCE_VIDEO,
    OBS_SOURCE_ASYNC_VIDEO,
    OBS_SOURCE_AUDIO,
    OBS_SOURCE_COMPOSITE,
    OBS_SOURCE_AUDIO | OBS_SOURCE_COMPOSITE,
    OBS_SOURCE_AUDIO | OBS_SOURCE_VIDEO,
    OBS_SOURCE_COMPOSITE | OBS_SOURCE_VIDEO,
    !(OBS_SOURCE_AUDIO | OBS_SOURCE_COMPOSITE),
    !OBS_SOURCE_AUDIO,
    !OBS_SOURCE_COMPOSITE,
    u32::MAX,
];

/// A model source that records which fields were read, in order.
struct Model {
    removed: bool,
    flags: u32,
    pending: bool,
    reads: RefCell<Vec<&'static str>>,
}

impl Model {
    fn new(removed: bool, flags: u32, pending: bool) -> Self {
        Self {
            removed,
            flags,
            pending,
            reads: RefCell::new(Vec::new()),
        }
    }
}

impl Pending for Model {
    fn removed(&self) -> bool {
        self.reads.borrow_mut().push("removed");
        self.removed
    }
    fn output_flags(&self) -> u32 {
        self.reads.borrow_mut().push("flags");
        self.flags
    }
    fn audio_pending(&self) -> bool {
        self.reads.borrow_mut().push("pending");
        self.pending
    }
}

fn no_log(name: &'static CStr) {
    panic!("a live source logged {name:?}");
}

/// null_source_answers_the_defaults_and_names_the_caller: NULL answers true
/// and SPEAKERS_UNKNOWN, reads nothing, and logs once under each name (the
/// layout's being obs_source_get_audio_channels).
#[test]
fn null_source_answers_the_defaults_and_names_the_caller() {
    let mut logs = Vec::new();
    assert!(audio_pending(None::<&Model>, |name| logs.push(name)));
    assert_eq!(logs, [c"obs_source_audio_pending"]);

    let mut logs = Vec::new();
    let answer = speaker_layout(None::<fn() -> c_int>, |name| logs.push(name));
    assert_eq!(answer, SPEAKERS_UNKNOWN);
    assert_eq!(logs, [c"obs_source_get_audio_channels"]);

    assert_eq!(PENDING_FUNCTION, c"obs_source_audio_pending");
    assert_eq!(SPEAKER_LAYOUT_FUNCTION, c"obs_source_get_audio_channels");
    assert_eq!(SPEAKERS_UNKNOWN, 0);
    assert_eq!(OBS_SOURCE_AUDIO, 1 << 1);
    assert_eq!(OBS_SOURCE_COMPOSITE, 1 << 6);
}

/// audio_pending_follows_removed_flags_and_field: removed answers pending,
/// neither audio nor composite answers pending, otherwise the field; the
/// reads stop as soon as the answer is known.
#[test]
fn audio_pending_follows_removed_flags_and_field() {
    for flags in FLAG_WORDS {
        for removed in [false, true] {
            for field in [false, true] {
                let reads_field = flags & (OBS_SOURCE_AUDIO | OBS_SOURCE_COMPOSITE) != 0;
                let expected = removed || !reads_field || field;
                for repeat in 0..2 {
                    let source = Model::new(removed, flags, field);
                    assert_eq!(
                        audio_pending(Some(&source), no_log),
                        expected,
                        "flags={flags:#x} removed={removed} field={field} repeat={repeat}"
                    );
                    let reads = source.reads.borrow();
                    let want: &[&str] = if removed {
                        &["removed"]
                    } else if reads_field {
                        &["removed", "flags", "pending"]
                    } else {
                        &["removed", "flags"]
                    };
                    assert_eq!(reads.as_slice(), want);
                }
            }
        }
    }
}

/// audio_pending_named_cases: the named cases of the cmocka test.
#[test]
fn audio_pending_named_cases() {
    // A removed audio source with nothing pending still answers pending.
    assert!(audio_pending(
        Some(&Model::new(true, OBS_SOURCE_AUDIO, false)),
        no_log
    ));
    // A video-only source has no audio to wait for: always pending.
    assert!(audio_pending(
        Some(&Model::new(false, OBS_SOURCE_VIDEO, false)),
        no_log
    ));
    // Audio and composite sources answer their field.
    for flags in [OBS_SOURCE_AUDIO, OBS_SOURCE_COMPOSITE] {
        assert!(!audio_pending(
            Some(&Model::new(false, flags, false)),
            no_log
        ));
        assert!(audio_pending(Some(&Model::new(false, flags, true)), no_log));
    }
}

/// speaker_layout_answers_sample_info_speakers: the answer is the field,
/// read once per call, for every layout and values outside the enum.
#[test]
fn speaker_layout_answers_sample_info_speakers() {
    for layout in [0, 1, 2, 3, 4, 5, 6, 8, 7, 0x5a, -1, c_int::MAX] {
        for repeat in 0..2 {
            let mut reads = 0;
            let answer = speaker_layout(
                Some(|| {
                    reads += 1;
                    layout
                }),
                no_log,
            );
            assert_eq!(answer, layout, "repeat {repeat}");
            assert_eq!(reads, 1);
        }
    }
}
