use obs_graphics::timer_range_dispatch::{Command, Host, command, get_data};

#[derive(Clone, Copy, Debug)]
pub(super) struct Case {
    pub context: bool,
    pub range: bool,
    pub disjoint: bool,
    pub frequency: bool,
    pub op: u32,
    pub result: bool,
    pub writes: u32,
    pub value_disjoint: bool,
    pub value: u64,
}
#[derive(Default)]
struct Fixture {
    context: bool,
    logs: Vec<(&'static str, &'static str)>,
    events: String,
    disjoint: bool,
    frequency: u64,
    expected_range: usize,
    result: bool,
    writes: u32,
    value_disjoint: bool,
    value: u64,
    allocation: Option<Box<u8>>,
}
impl Host<usize> for Fixture {
    type Disjoint = usize;
    type Frequency = usize;
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, function: &'static str, reason: &'static str) {
        self.logs.push((function, reason));
    }
    fn command(&mut self, op: Command, range: usize) {
        assert!(self.context);
        assert_eq!(range, 123);
        self.events.push(match op {
            Command::Destroy => 'D',
            Command::Begin => 'B',
            Command::End => 'E',
        });
        if op == Command::Destroy {
            assert!(self.allocation.take().is_some());
        }
    }
    fn data(&mut self, range: usize, disjoint: usize, frequency: usize) -> bool {
        assert!(self.context);
        assert_eq!(range, self.expected_range);
        assert_eq!(disjoint, 456);
        assert_eq!(frequency, 789);
        self.events.push('G');
        if self.writes & 1 != 0 {
            self.disjoint = self.value_disjoint;
        }
        if self.writes & 2 != 0 {
            self.frequency = self.value;
        }
        self.result
    }
}
#[test]
fn test_timer_range_dispatch_command_guards() {
    let mut h = Fixture::default();
    for op in [Command::Destroy, Command::Begin, Command::End] {
        for range in [None, Some(123)] {
            command(&mut h, range, op);
            assert_eq!(
                h.logs,
                [(op.function(), "called while not in a graphics context")]
            );
            assert!(h.events.is_empty());
            h.logs.clear();
        }
    }
    h.context = true;
    for op in [Command::Destroy, Command::Begin, Command::End] {
        command(&mut h, None, op);
    }
    assert!(h.logs.is_empty());
    assert!(h.events.is_empty());
}
#[test]
fn test_timer_range_dispatch_data_guards() {
    for context in [false, true] {
        for range in [0, 123] {
            for outputs in 0..4 {
                if context && outputs == 3 {
                    continue;
                }
                let mut h = Fixture {
                    context,
                    disjoint: true,
                    frequency: 0xfedcba9876543210,
                    ..Fixture::default()
                };
                assert!(!get_data(
                    &mut h,
                    range,
                    (outputs & 1 != 0).then_some(456),
                    (outputs & 2 != 0).then_some(789)
                ));
                let reason = if !context {
                    "called while not in a graphics context"
                } else if outputs & 1 == 0 {
                    "Null 'disjoint' parameter"
                } else {
                    "Null 'frequency' parameter"
                };
                assert_eq!(h.logs, [("gs_timer_range_get_data", reason)]);
                assert!(h.events.is_empty());
                assert!(h.disjoint);
                assert_eq!(h.frequency, 0xfedcba9876543210);
            }
        }
    }
}
#[test]
fn test_timer_range_dispatch_and_ownership() {
    let mut h = Fixture {
        context: true,
        allocation: Some(Box::new(7)),
        ..Fixture::default()
    };
    for (op, result, expected) in [
        (Command::End, false, "E"),
        (Command::Begin, true, "EB"),
        (Command::Begin, false, "EBB"),
        (Command::End, true, "EBBE"),
    ] {
        h.result = result;
        command(&mut h, Some(123), op);
        assert_eq!(h.events, expected);
        assert!(h.allocation.is_some());
    }
    for range in [0, 123] {
        h.expected_range = range;
        for result in [false, true] {
            h.result = result;
            for writes in 0..4 {
                h.writes = writes;
                for (i, value) in [0, 1, 0xfedcba9876543210, u64::MAX].into_iter().enumerate() {
                    h.value = value;
                    h.value_disjoint = i & 1 != 0;
                    h.disjoint = !h.value_disjoint;
                    h.frequency = 42;
                    let before = h.events.len();
                    assert_eq!(get_data(&mut h, range, Some(456), Some(789)), result);
                    assert_eq!(h.events.len(), before + 1);
                    assert!(h.events.ends_with('G'));
                    assert_eq!(
                        h.disjoint,
                        if writes & 1 != 0 {
                            h.value_disjoint
                        } else {
                            !h.value_disjoint
                        }
                    );
                    assert_eq!(h.frequency, if writes & 2 != 0 { value } else { 42 });
                    assert!(h.allocation.is_some());
                }
            }
        }
    }
    assert_eq!(h.events.len(), 68);
    command(&mut h, Some(123), Command::Destroy);
    assert!(h.allocation.is_none());
    assert_eq!(h.events.len(), 69);
    assert!(h.events.ends_with('D'));
    assert!(h.logs.is_empty());
}
pub(super) fn run(c: Case) -> (bool, bool, u64, String, String) {
    let range = if c.range { 123 } else { 0 };
    let mut h = Fixture {
        context: c.context,
        expected_range: range,
        result: c.result,
        writes: c.writes,
        value_disjoint: c.value_disjoint,
        value: c.value,
        disjoint: !c.value_disjoint,
        frequency: 42,
        allocation: Some(Box::new(7)),
        ..Fixture::default()
    };
    let result = if c.op == 3 {
        get_data(
            &mut h,
            range,
            c.disjoint.then_some(456),
            c.frequency.then_some(789),
        )
    } else {
        command(
            &mut h,
            c.range.then_some(range),
            [Command::Destroy, Command::Begin, Command::End][c.op as usize],
        );
        false
    };
    let log = h
        .logs
        .first()
        .map(|(f, r)| format!("{f}: {r}"))
        .unwrap_or_default();
    (result, h.disjoint, h.frequency, log, h.events)
}
