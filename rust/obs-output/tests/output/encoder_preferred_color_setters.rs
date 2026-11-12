//! Mirrors every case in test_encoder_preferred_color_setters.c (#4777).
use obs_output::encoder_preferred_color_setters::set;

const SPACES: [u32; 7] = [0, 1, 2, 3, 4, 5, 0x5a];
const RANGES: [u32; 4] = [0, 1, 2, 0x5a];

#[derive(Clone, Copy, Debug, PartialEq)]
struct Encoder {
    kind: i32,
    original_kind: i32,
    format: u32,
    space: u32,
    range: u32,
}

fn video() -> Encoder {
    Encoder {
        kind: 1,
        original_kind: 0,
        format: 2,
        space: 0,
        range: 0,
    }
}

#[test]
fn mirrors_test_null() {
    for value in SPACES.into_iter().chain(RANGES) {
        set(None, value);
    }
}

#[test]
fn mirrors_test_audio_unchanged() {
    let mut encoder = Encoder {
        kind: 0,
        original_kind: 1,
        space: 5,
        range: 2,
        ..video()
    };
    let before = encoder;
    for value in SPACES {
        set(Some((encoder.kind, &mut encoder.space)), value);
        assert_eq!(encoder, before);
    }
    for value in RANGES {
        set(Some((encoder.kind, &mut encoder.range)), value);
        assert_eq!(encoder, before);
    }
}

#[test]
fn mirrors_test_video_color_space_writes_only_its_field() {
    let mut encoder = video();
    for range in RANGES {
        encoder.range = range;
        for space in SPACES {
            let expected = Encoder { space, ..encoder };
            for _ in 0..2 {
                set(Some((encoder.kind, &mut encoder.space)), space);
                assert_eq!(encoder.space, space);
                assert_eq!(encoder, expected);
            }
        }
    }
}

#[test]
fn mirrors_test_video_range_writes_only_its_field() {
    let mut encoder = video();
    for space in SPACES {
        encoder.space = space;
        for range in RANGES {
            let expected = Encoder { range, ..encoder };
            for _ in 0..2 {
                set(Some((encoder.kind, &mut encoder.range)), range);
                assert_eq!(encoder.range, range);
                assert_eq!(encoder, expected);
            }
        }
    }
}

#[test]
fn exact_enum_bits_and_unknown_encoder_kinds() {
    for kind in [0, 1, 2, -1, i32::MIN, i32::MAX] {
        for value in [0, 0x5a, i32::MAX as u32, 0x8000_0000, u32::MAX] {
            let mut stored = 17;
            set(Some((kind, &mut stored)), value);
            assert_eq!(stored, if kind == 1 { value } else { 17 });
        }
    }
}
