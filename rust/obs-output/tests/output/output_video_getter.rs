//! Tier 1 mirrors of test/cmocka/test_output_video_getter.c over a model
//! output. The encoded branch composes the getter with the real safe cores of
//! `obs_output_get_video_encoder()` and `obs_encoder_video()`, as the shim
//! does; every read and diagnostic is recorded in order.
use core::cell::RefCell;
use core::ffi::CStr;
use obs_encoder::media_getters::{Getter, Media, get};
use obs_output::output_encoder_getters::video_encoder;
use obs_output::output_slots::{MAX_OUTPUT_VIDEO_ENCODERS, OBS_ENCODER_AUDIO, OBS_ENCODER_VIDEO};
use obs_output::output_video_getter::video;
use obs_output::video_checks::{
    OBS_OUTPUT_AUDIO, OBS_OUTPUT_ENCODED, OBS_OUTPUT_SERVICE, OBS_OUTPUT_VIDEO,
};

const LOG_WARNING: i32 = 200;
const LOG_DEBUG: i32 = 400;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    NullOutput(&'static CStr),
    Flags,
    Raw,
    Occupied(usize),
    Slot(usize),
    Log(i32, &'static CStr),
}

/// A borrowed video identity: the address of a token, as in the C fixture.
type Video = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Output {
    flags: u32,
    video: Option<Video>,
    slots: [Option<Media<Video>>; MAX_OUTPUT_VIDEO_ENCODERS],
}

fn encoder(media: Option<Video>) -> Media<Video> {
    Media {
        encoder_type: OBS_ENCODER_VIDEO,
        parent: media,
        fps_override: None,
    }
}

/// `init_fixture()`: two video encoders in the first two slots.
fn fixture() -> Output {
    let mut slots = [None; MAX_OUTPUT_VIDEO_ENCODERS];
    slots[0] = Some(encoder(None));
    slots[1] = Some(encoder(None));
    Output {
        flags: 0,
        video: None,
        slots,
    }
}

/// One `obs_output_video()` call on the model (`None` is a NULL output).
fn call(output: Option<&Output>) -> (Option<Video>, Vec<Event>) {
    let events = RefCell::new(Vec::new());
    let push = |event| events.borrow_mut().push(event);
    let got = video(
        output.is_some(),
        || {
            push(Event::Flags);
            output.map_or(0, |o| o.flags)
        },
        || {
            push(Event::Raw);
            output.and_then(|o| o.video)
        },
        || {
            let output = output.expect("encoded branch of a valid output");
            let first = video_encoder(
                true,
                MAX_OUTPUT_VIDEO_ENCODERS,
                |i| {
                    push(Event::Occupied(i));
                    output.slots[i].is_some()
                },
                |i| {
                    push(Event::Slot(i));
                    output.slots[i]
                },
                |name| push(Event::NullOutput(name)),
            );
            get(first, Getter::Video, |level, message| {
                push(Event::Log(level, message))
            })
        },
        |name| push(Event::NullOutput(name)),
    );
    (got, events.into_inner())
}

/// `check_borrowed()`: two calls return `expected` and leave the fixture as it was.
fn check_borrowed(output: &Output, expected: Option<Video>) -> Vec<Event> {
    let before = output.clone();
    let (first, events) = call(Some(output));
    let (second, again) = call(Some(output));
    assert_eq!(first, expected);
    assert_eq!(second, expected);
    assert_eq!(events, again);
    assert_eq!(output, &before);
    events
}

fn encoded_reads(first: usize) -> Vec<Event> {
    let mut events = vec![Event::Flags];
    events.extend((0..MAX_OUTPUT_VIDEO_ENCODERS).map(Event::Occupied));
    events.push(Event::Slot(first));
    events
}

#[test]
fn null_output() {
    let (got, events) = call(None);
    assert_eq!(got, None);
    // LOG_DEBUG "obs_output_video: Null 'output' parameter"; nothing is read.
    assert_eq!(events, [Event::NullOutput(c"obs_output_video")]);
}

#[test]
fn raw_video() {
    let raw_token = 0x1000;
    let mut output = fixture();
    // Unreadable slots in C: the raw branch never follows them here either.
    for slot in &mut output.slots {
        *slot = Some(encoder(Some(0x1)));
    }
    for flags in [
        0,
        OBS_OUTPUT_VIDEO,
        OBS_OUTPUT_AUDIO | OBS_OUTPUT_SERVICE,
        u32::MAX ^ OBS_OUTPUT_ENCODED,
    ] {
        output.flags = flags;
        for media in [Some(raw_token), None] {
            output.video = media;
            assert_eq!(check_borrowed(&output, media), [Event::Flags, Event::Raw]);
        }
    }
}

#[test]
fn encoded_video() {
    let (raw_token, first_token, second_token, override_token) = (0x1000, 0x2000, 0x3000, 0x4000);
    let mut output = fixture();
    output.slots[1] = Some(encoder(Some(second_token)));
    for flags in [
        OBS_OUTPUT_ENCODED,
        OBS_OUTPUT_ENCODED | OBS_OUTPUT_AUDIO,
        u32::MAX,
    ] {
        output.flags = flags;
        for raw in [None, Some(raw_token)] {
            output.video = raw;
            for media in [None, Some(first_token)] {
                for fps_override in [None, Some(override_token)] {
                    output.slots[0] = Some(Media {
                        encoder_type: OBS_ENCODER_VIDEO,
                        parent: media,
                        fps_override,
                    });
                    // The override wins over the encoder's media; raw media is ignored.
                    let expected = fps_override.or(media);
                    assert_eq!(check_borrowed(&output, expected), encoded_reads(0));
                }
            }
        }
    }
}

#[test]
fn first_occupied_and_missing_encoder() {
    let (raw_token, second_token) = (0x1000, 0x3000);
    let mut output = fixture();
    output.flags = OBS_OUTPUT_ENCODED | OBS_OUTPUT_VIDEO;
    output.video = Some(raw_token);
    output.slots = [None; MAX_OUTPUT_VIDEO_ENCODERS];
    // "First" means first occupied slot, including gaps before the final slot.
    for i in 1..MAX_OUTPUT_VIDEO_ENCODERS {
        output.slots[i] = Some(encoder(Some(second_token)));
        assert_eq!(
            check_borrowed(&output, Some(second_token)),
            encoded_reads(i)
        );
        output.slots[i] = None;
    }
    // No occupied encoder: the downstream diagnostic, with no raw fallback.
    let before = output.clone();
    let (got, events) = call(Some(&output));
    assert_eq!(got, None);
    let mut expected = vec![Event::Flags];
    expected.extend((0..MAX_OUTPUT_VIDEO_ENCODERS).map(Event::Occupied));
    expected.push(Event::Log(
        LOG_DEBUG,
        c"obs_encoder_video: Null 'encoder' parameter",
    ));
    assert_eq!(events, expected);
    assert_eq!(output, before);
}

#[test]
fn wrong_encoder_type() {
    let (raw_token, encoder_token) = (0x1000, 0x2000);
    let mut output = fixture();
    output.flags = OBS_OUTPUT_ENCODED;
    output.video = Some(raw_token);
    output.slots[0] = Some(Media {
        encoder_type: OBS_ENCODER_AUDIO,
        parent: Some(encoder_token),
        fps_override: None,
    });
    let before = output.clone();
    let (got, events) = call(Some(&output));
    assert_eq!(got, None);
    // Historical setter label from obs_encoder_video: characterized, not endorsed.
    let mut expected = encoded_reads(0);
    expected.push(Event::Log(
        LOG_WARNING,
        c"obs_encoder_set_video: encoder '%s' is not a video encoder",
    ));
    assert_eq!(events, expected);
    assert_eq!(output, before);
}
