//! Safe 1:1 characterization of test_set_output_source.c. The dependency model
//! supplies the existing canvas setter's effects, never production logic.
use obs_core::set_output_source::set_output_source;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Fixture {
    slots: [Option<usize>; 64],
    refs: [i64; 3],
    active: [i64; 3],
    showing: [i64; 3],
    canvas_refs: i64,
    local: Option<Option<usize>>,
    global: Option<Option<usize>>,
    signals: usize,
}

impl Fixture {
    fn new() -> Self {
        Self {
            slots: [None; 64],
            refs: [0; 3],
            active: [0; 3],
            showing: [0; 3],
            canvas_refs: 1,
            local: None,
            global: None,
            signals: 0,
        }
    }

    fn direct(&mut self, canvas: usize, channel: u32, source: Option<usize>) {
        assert_eq!(canvas, 73, "actual current main canvas identity");
        let Ok(channel) = usize::try_from(channel) else {
            return;
        };
        if channel >= self.slots.len() {
            return;
        }
        if let Some(s) = source {
            self.refs[s] += 1;
        }
        let previous = self.slots[channel];
        if source == previous {
            if let Some(s) = source {
                self.refs[s] -= 1;
            }
            return;
        }
        let before = self.clone();
        let mut payload = source;
        for global in [false, true] {
            assert_eq!(self.signals, usize::from(global));
            assert_eq!(self.slots[channel], previous);
            assert_eq!(
                self.refs, before.refs,
                "old/new references held during signals"
            );
            assert_eq!(self.active, before.active);
            assert_eq!(self.showing, before.showing);
            assert_eq!(
                payload,
                if global {
                    self.local.unwrap_or(source)
                } else {
                    source
                }
            );
            self.signals += 1;
            if let Some(replacement) = if global { self.global } else { self.local } {
                payload = replacement;
            }
        }
        self.slots[channel] = payload;
        if let Some(s) = payload {
            self.active[s] += 1;
            self.showing[s] += 1;
        }
        if let Some(s) = previous {
            self.active[s] -= 1;
            self.showing[s] -= 1;
            self.refs[s] -= 1;
        }
    }

    fn call(&mut self, direct: bool, channel: u32, source: Option<usize>) {
        if direct {
            self.direct(73, channel, source);
        } else {
            set_output_source(73, channel, source, |c, ch, s| self.direct(c, ch, s));
        }
    }

    fn change(
        &mut self,
        direct: bool,
        channel: u32,
        source: Option<usize>,
        stored: Option<usize>,
        during: [i64; 3],
        after: [i64; 3],
    ) {
        let previous = self.slots[channel as usize];
        let before = self.clone();
        let mut held = before.refs;
        if let Some(s) = source {
            held[s] += 1;
        }
        assert_eq!(held, during);
        self.signals = 0;
        self.call(direct, channel, source);
        assert_eq!(self.signals, 2);
        assert_eq!(self.slots[channel as usize], stored);
        assert_eq!(self.canvas_refs, before.canvas_refs);
        assert_eq!(self.refs, after);
        for i in 0..3 {
            let delta = i64::from(stored == Some(i)) - i64::from(previous == Some(i));
            assert_eq!(self.active[i], before.active[i] + delta);
            assert_eq!(self.showing[i], before.showing[i] + delta);
        }
        // The unchanged channel getter returns the stored source with a new ref.
        if let Some(s) = self.slots[channel as usize] {
            self.refs[s] += 1;
            assert_eq!(Some(s), stored);
            self.refs[s] -= 1;
        } else {
            assert_eq!(stored, None);
        }
    }

    fn unchanged(&mut self, direct: bool, channel: u32, source: Option<usize>) {
        self.signals = 0;
        let before = self.clone();
        self.call(direct, channel, source);
        assert_eq!(*self, before);
    }
}

#[test]
fn test_set_output_source_bounds_and_null() {
    let mut f = Fixture::new();
    f.unchanged(false, 0, None);
    f.unchanged(false, 63, None);
    f.change(false, 0, Some(0), Some(0), [1, 0, 0], [1, 0, 0]);
    f.change(false, 63, Some(0), Some(0), [2, 0, 0], [2, 0, 0]);
    for invalid in [64, 65, u32::MAX] {
        f.unchanged(false, invalid, Some(1));
        f.unchanged(false, invalid, None);
    }
    f.change(false, 0, None, None, [2, 0, 0], [1, 0, 0]);
    f.change(false, 63, None, None, [1, 0, 0], [0, 0, 0]);
    f.unchanged(false, 0, None);
}

#[test]
fn test_set_output_source_delegates_like_main_canvas() {
    let mut f = Fixture::new();
    for direct in [false, true] {
        f.change(direct, 3, Some(0), Some(0), [1, 0, 0], [1, 0, 0]);
        f.unchanged(direct, 3, Some(0));
        f.change(direct, 3, Some(1), Some(1), [1, 1, 0], [0, 1, 0]);
        f.change(direct, 3, None, None, [0, 1, 0], [0, 0, 0]);
        f.unchanged(direct, 3, None);
    }
}

#[test]
fn test_set_output_source_mutable_signal_payload() {
    let mut f = Fixture::new();
    f.refs[2] += 1; // externally supply the replacement's channel ownership
    f.local = Some(Some(1));
    f.global = Some(Some(2));
    f.change(false, 2, Some(0), Some(2), [1, 0, 1], [1, 0, 1]);
    f.local = None;
    f.global = None;
    f.unchanged(false, 2, Some(2));
    f.change(false, 2, None, None, [1, 0, 1], [1, 0, 0]);
    f.refs[0] -= 1;
    assert_eq!(f.refs[0], 0);
    f.global = Some(None);
    f.change(false, 2, Some(0), None, [1, 0, 0], [1, 0, 0]);
    f.global = None;
    f.unchanged(false, 2, None);
    f.refs[0] -= 1;
    assert_eq!(f.refs, [0; 3]);
}

#[test]
fn forwards_every_channel_source_and_current_canvas_once() {
    for canvas in [17, 73, 991] {
        for channel in [0, 3, 63, 64, 65, u32::MAX] {
            for source in [None, Some(0), Some(1), Some(2)] {
                let mut calls = Vec::new();
                set_output_source(canvas, channel, source, |c, ch, s| calls.push((c, ch, s)));
                assert_eq!(calls, [(canvas, channel, source)]);
            }
        }
    }
}
