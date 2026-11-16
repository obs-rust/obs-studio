//! Safe 1:1 mirror of test_get_output_source.c. The fixture models the existing
//! downstream channel getter and setter; only the wrapper is under test.
use obs_core::get_output_source::get_output_source;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Fixture {
    slots: [Option<usize>; 64],
    refs: [i64; 2],
    active: [i64; 2],
    showing: [i64; 2],
    canvas_refs: i64,
    signals: usize,
    names: [&'static str; 2],
}

impl Fixture {
    fn new() -> Self {
        let f = Self {
            slots: [None; 64],
            refs: [0; 2],
            active: [0; 2],
            showing: [0; 2],
            canvas_refs: 1,
            signals: 0,
            names: ["get output a", "get output b"],
        };
        assert_eq!(f.refs, [0; 2]);
        f
    }

    fn set(&mut self, channel: usize, source: Option<usize>) {
        let previous = self.slots[channel];
        if previous == source {
            return;
        }
        if let Some(s) = source {
            self.refs[s] += 1;
            self.active[s] += 1;
            self.showing[s] += 1;
        }
        self.signals += 2;
        self.slots[channel] = source;
        if let Some(s) = previous {
            self.refs[s] -= 1;
            self.active[s] -= 1;
            self.showing[s] -= 1;
        }
    }

    fn direct(&mut self, canvas: usize, channel: u32) -> Option<usize> {
        assert_eq!(canvas, 73, "current main canvas identity");
        let channel = usize::try_from(channel).ok()?;
        let source = *self.slots.get(channel)?;
        if let Some(s) = source {
            self.refs[s] += 1;
        }
        source
    }

    fn get(&mut self, channel: u32) -> Option<usize> {
        get_output_source(73, channel, |c, ch| self.direct(c, ch))
    }

    fn release(&mut self, source: Option<usize>) {
        if let Some(s) = source {
            self.refs[s] -= 1;
        }
    }

    fn expect_read(&mut self, channel: u32, expected: Option<usize>) {
        let before = self.clone();
        let got = self.get(channel);
        assert_eq!(got, expected);
        for i in 0..2 {
            assert_eq!(
                self.refs[i],
                before.refs[i] + i64::from(expected == Some(i))
            );
        }
        let direct = self.direct(73, channel);
        assert_eq!(direct, got);
        for i in 0..2 {
            assert_eq!(
                self.refs[i],
                before.refs[i] + 2 * i64::from(expected == Some(i))
            );
        }
        self.release(direct);
        self.release(got);
        assert_eq!(self.slots, before.slots);
        assert_eq!(self.canvas_refs, before.canvas_refs);
        assert_eq!(self.signals, before.signals);
        assert_eq!(self.refs, before.refs);
        assert_eq!(self.active, before.active);
        assert_eq!(self.showing, before.showing);
    }
}

#[test]
fn test_get_output_source_empty_and_channel_selection() {
    let mut f = Fixture::new();
    for channel in 0..64 {
        f.expect_read(channel, None);
    }
    f.set(0, Some(0));
    f.set(3, Some(0));
    f.set(63, Some(1));
    for channel in 0..64 {
        let expected = match channel {
            63 => Some(1),
            0 | 3 => Some(0),
            _ => None,
        };
        f.expect_read(channel, expected);
    }
    f.set(0, None);
    f.expect_read(0, None);
    f.expect_read(3, Some(0));
    f.expect_read(63, Some(1));
}

#[test]
fn test_get_output_source_acquired_references_survive_clear() {
    let mut f = Fixture::new();
    f.set(0, Some(0));
    assert_eq!(f.refs[0], 1);
    let first = f.get(0);
    assert_eq!(first, Some(0));
    assert_eq!(f.refs[0], 2);
    let second = f.get(0);
    assert_eq!(second, Some(0));
    assert_eq!(f.refs[0], 3);
    f.set(0, None);
    assert_eq!(f.refs[0], 2);
    assert_eq!(f.names[first.unwrap()], "get output a");
    assert_eq!(f.names[second.unwrap()], "get output a");
    f.expect_read(0, None);
    f.release(first);
    assert_eq!(f.refs[0], 1);
    f.release(second);
    assert_eq!(f.refs[0], 0);
}

#[test]
fn test_get_output_source_invalid_channels() {
    let mut f = Fixture::new();
    f.set(0, Some(0));
    f.set(63, Some(1));
    // Model the downstream release guard. The real C debug assert is unchanged.
    for channel in [64, 65, u32::MAX] {
        f.expect_read(channel, None);
    }
    f.expect_read(0, Some(0));
    f.expect_read(63, Some(1));
}

#[test]
fn forwards_current_canvas_and_full_width_channel_once_and_returns_owned_value() {
    struct Canvas(u64);
    for canvas in [0, 73, u64::MAX] {
        for channel in [0, 63, 64, 65, u32::MAX] {
            let mut calls = 0;
            let result = get_output_source(Canvas(canvas), channel, |c, ch| {
                calls += 1;
                assert_eq!(c.0, canvas);
                assert_eq!(ch, channel);
                Box::new((c.0, ch))
            });
            assert_eq!(calls, 1);
            assert_eq!(*result, (canvas, channel));
        }
    }
}
