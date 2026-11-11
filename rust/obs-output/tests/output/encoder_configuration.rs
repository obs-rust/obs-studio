//! Tier 1: direct setter characterization and validation-order edges.
use core::ffi::CStr;
use obs_output::encoder_configuration::{Configuration, Effects, Request, configure};

#[derive(Default)]
struct Recorder {
    size: Option<(u32, u32)>,
    calls: Vec<String>,
    logs: Vec<(i32, &'static CStr)>,
}
impl Effects for Recorder {
    fn log(&mut self, level: i32, format: &'static CStr) {
        self.logs.push((level, format));
    }
    fn output_size(&mut self) -> Option<(u32, u32)> {
        self.calls.push("info".into());
        self.size
    }
    fn scaled_size(&mut self, w: u32, h: u32) {
        self.calls.push(format!("size {w} {h}"));
    }
    fn gpu_scale(&mut self, scale: i32) {
        self.calls.push(format!("gpu {scale}"));
    }
    fn divisor(&mut self, divisor: u32) {
        self.calls.push(format!("divisor {divisor}"));
    }
    fn free_override(&mut self) {
        self.calls.push("free".into());
    }
    fn create_override(&mut self, divisor: u32) {
        self.calls.push(format!("create {divisor}"));
    }
}
fn state() -> Configuration {
    Configuration {
        kind: 1,
        active: false,
        initialized: false,
        has_override: false,
        has_media: false,
    }
}

#[test]
fn test_video_encoder_getters_and_test_enum_order_scaled() {
    let mut r = Recorder::default();
    assert!(configure(Some(state()), Request::GpuScale(1), &mut r));
    assert!(configure(Some(state()), Request::Divisor(2), &mut r));
    assert!(configure(
        Some(state()),
        Request::ScaledSize(1280, 720),
        &mut r
    ));
    assert_eq!(r.calls, ["gpu 1", "divisor 2", "info", "size 1280 720"]);
    assert!(r.logs.is_empty());
    assert!(!configure(Some(state()), Request::Divisor(0), &mut r));
    assert_eq!(
        r.logs.last().unwrap().1,
        c"encoder '%s': Cannot set frame rate divisor to 0"
    );
}

#[test]
fn refusals_are_ordered_and_never_mutate() {
    for request in [
        Request::ScaledSize(1, 2),
        Request::GpuScale(3),
        Request::Divisor(0),
    ] {
        let mut r = Recorder::default();
        assert!(!configure(None, request, &mut r));
        assert_eq!(r.logs.len(), 1);
        assert_eq!(r.logs[0].0, 400);
        assert!(r.calls.is_empty());
        for (kind, active, initialized, diagnostic) in [
            (0, true, true, "not a video encoder"),
            (1, true, true, "while the encoder is active"),
            (1, false, true, "after the encoder has been initialized"),
        ] {
            let mut r = Recorder::default();
            assert!(!configure(
                Some(Configuration {
                    kind,
                    active,
                    initialized,
                    ..state()
                }),
                request,
                &mut r
            ));
            assert!(r.calls.is_empty());
            assert_eq!(r.logs.len(), 1);
            assert_eq!(r.logs[0].0, 200);
            assert!(r.logs[0].1.to_str().unwrap().contains(diagnostic));
        }
    }
}

#[test]
fn dimensions_match_only_with_media_and_full_width_values_survive() {
    for size in [None, Some((0, 0)), Some((1920, 1080))] {
        for (w, h) in [(0, 0), (1920, 1080), (u32::MAX, 0), (0, u32::MAX)] {
            let mut r = Recorder {
                size,
                ..Recorder::default()
            };
            assert!(configure(Some(state()), Request::ScaledSize(w, h), &mut r));
            let matches = size == Some((w, h));
            assert_eq!(
                r.calls[1],
                if matches {
                    "size 0 0".into()
                } else {
                    format!("size {w} {h}")
                }
            );
            assert_eq!(r.logs.len(), usize::from(matches));
        }
    }
}

#[test]
fn divisor_lifecycle_is_ordered_including_one_and_maximum() {
    for has_override in [false, true] {
        for has_media in [false, true] {
            for divisor in [1, 2, u32::MAX] {
                let mut r = Recorder::default();
                assert!(configure(
                    Some(Configuration {
                        has_override,
                        has_media,
                        ..state()
                    }),
                    Request::Divisor(divisor),
                    &mut r
                ));
                let mut expected = vec![format!("divisor {divisor}")];
                if has_override {
                    expected.push("free".into());
                }
                if has_media {
                    expected.push(format!("create {divisor}"));
                }
                assert_eq!(r.calls, expected);
                assert!(r.logs.is_empty());
            }
        }
    }
}
