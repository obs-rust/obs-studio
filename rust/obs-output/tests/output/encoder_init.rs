use obs_output::encoder_init::{Boundary, initialize};

#[derive(Default)]
struct Encoder {
    events: Vec<(u32, bool)>,
    fail: Option<u32>,
    callbacks: u32,
    shared_user: Option<i32>,
    legacy: i32,
    modern: i32,
    shared: i32,
    order: u32,
}

impl Boundary for Encoder {
    fn sentinel(&mut self, index: u32) {
        self.events.push((index, false));
    }
    fn context(&mut self) -> bool {
        self.events.push((6, false));
        self.fail != Some(0)
    }
    fn mutex(&mut self, index: u32, recursive: bool) -> bool {
        self.events.push((7 + index, recursive));
        self.fail != Some(1 + index)
    }
    fn defaults(&mut self) {
        if self.callbacks & 1 != 0 {
            assert_eq!(self.order, 0);
            self.order = 1;
            self.legacy = 11;
            self.shared = 21;
        }
    }
    fn defaults2(&mut self) {
        if self.callbacks & 2 != 0 {
            assert_eq!(self.order, self.callbacks & 1);
            if self.callbacks & 1 != 0 {
                assert_eq!((self.legacy, self.shared), (11, 21));
            }
            self.order = self.order * 10 + 2;
            self.modern = 12;
            self.shared = 22;
        }
    }
}

// Mirrors test_encoder_init.c's four callback tables and settings precedence.
// Context ownership, lookup, handlers and real mutex usability remain covered
// at the unchanged public C boundary; the safe core delegates those operations.
#[test]
fn test_encoder_init_characterize_defaults() {
    for callbacks in 0..4 {
        for supplied in [false, true] {
            let mut encoder = Encoder {
                callbacks,
                shared_user: supplied.then_some(99),
                ..Encoder::default()
            };
            assert!(initialize(&mut encoder));
            assert_eq!(encoder.order, if callbacks == 3 { 12 } else { callbacks });
            assert_eq!(encoder.legacy, if callbacks & 1 != 0 { 11 } else { 0 });
            assert_eq!(encoder.modern, if callbacks & 2 != 0 { 12 } else { 0 });
            let shared = if callbacks & 2 != 0 {
                22
            } else if callbacks & 1 != 0 {
                21
            } else {
                0
            };
            assert_eq!(encoder.shared, shared);
            assert_eq!(
                encoder.shared_user.unwrap_or(encoder.shared),
                if supplied { 99 } else { shared }
            );
            let expected: Vec<_> = (0..13).map(|n| (n, n == 7 || n == 8)).collect();
            assert_eq!(encoder.events, expected);
        }
    }
}

#[test]
fn every_failure_preserves_partial_initialization_and_skips_defaults() {
    for fail in 0..7 {
        let mut encoder = Encoder {
            fail: Some(fail),
            callbacks: 3,
            ..Encoder::default()
        };
        assert!(!initialize(&mut encoder));
        let expected: Vec<_> = (0..=6 + fail).map(|n| (n, n == 7 || n == 8)).collect();
        assert_eq!(encoder.events, expected);
        assert_eq!(
            (
                encoder.order,
                encoder.legacy,
                encoder.modern,
                encoder.shared
            ),
            (0, 0, 0, 0)
        );
    }
}
