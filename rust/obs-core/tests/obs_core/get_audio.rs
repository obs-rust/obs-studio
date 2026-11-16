//! Safe mirrors of test_get_audio.c; audio lifecycle belongs to the caller.
use obs_core::get_audio::get_audio;

#[derive(Debug, PartialEq, Eq)]
struct Audio {
    rate: u32,
    channels: u8,
    active: bool,
    sinks: u8,
}

fn idle(rate: u32, channels: u8) -> Audio {
    Audio {
        rate,
        channels,
        active: false,
        sinks: 0,
    }
}

fn expect_current_output(audio: &Audio, rate: u32, channels: u8) {
    for _ in 0..32 {
        let actual = get_audio(Some(audio)).expect("configured output");
        assert!(core::ptr::eq(actual, audio));
        assert_eq!(actual.rate, rate);
        assert_eq!(actual.channels, channels);
        assert!(!actual.active);
        assert_eq!(actual.sinks, 0);
    }
    assert_eq!(audio.rate, rate);
    assert_eq!(audio.channels, channels);
}

#[test]
fn test_get_audio_initialized_core_without_audio() {
    let current: Option<&Audio> = None;
    for _ in 0..32 {
        assert!(get_audio(current).is_none());
    }
    assert!(current.is_none());
}

#[test]
fn test_get_audio_borrowed_output_tracks_reset_and_disable() {
    let stereo = idle(48000, 2);
    expect_current_output(&stereo, 48000, 2);
    let mono = idle(44100, 1);
    expect_current_output(&mono, 44100, 1);
    for _ in 0..32 {
        assert!(get_audio(None::<&Audio>).is_none());
    }
    let reopened = idle(48000, 2);
    expect_current_output(&reopened, 48000, 2);
}

#[test]
fn test_get_audio_active_output_is_unchanged() {
    let mut audio = Audio {
        rate: 48000,
        channels: 2,
        active: true,
        sinks: 1,
    };
    for _ in 0..32 {
        let result = get_audio(Some(&audio)).expect("configured output");
        assert!(core::ptr::eq(result, &audio));
        assert!(result.active);
        assert_eq!(result.sinks, 1, "existing sink remains registered");
    }
    // The caller disconnects the sink, then the getter returns the same output.
    audio.sinks = 0;
    audio.active = false;
    let result = get_audio(Some(&audio)).expect("configured output");
    assert!(core::ptr::eq(result, &audio));
    assert!(!result.active);
    assert_eq!(result.sinks, 0);
}
