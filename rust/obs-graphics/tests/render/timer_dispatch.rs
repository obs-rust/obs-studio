use obs_graphics::timer_dispatch::{Command, Host, command, get_data};
#[derive(Default)]
struct Fixture {
    context: bool,
    logs: Vec<(&'static str, &'static str)>,
    events: String,
    ticks: u64,
    value: u64,
    result: bool,
    write: bool,
    timer: Option<Box<u8>>,
}
impl Host<usize> for Fixture {
    type Output = usize;
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, function: &'static str, reason: &'static str) {
        self.logs.push((function, reason));
    }
    fn command(&mut self, op: Command, timer: usize) {
        assert!(self.context);
        assert_eq!(timer, 123);
        self.events.push(match op {
            Command::Destroy => 'D',
            Command::Begin => 'B',
            Command::End => 'E',
        });
        if op == Command::Destroy {
            assert!(self.timer.take().is_some());
        }
    }
    fn data(&mut self, timer: usize, ticks: usize) -> bool {
        assert!(self.context);
        assert_eq!(timer, 123);
        assert_eq!(ticks, 456);
        self.events.push('G');
        if self.write {
            self.ticks = self.value;
        }
        self.result
    }
}
#[test]
fn test_timer_dispatch_command_guards() {
    let mut h = Fixture::default();
    for op in [Command::Destroy, Command::Begin, Command::End] {
        for timer in [None, Some(123)] {
            command(&mut h, timer, op);
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
fn test_timer_dispatch_data_guards() {
    for context in [false, true] {
        for timer in [None, Some(123)] {
            for ticks in [None, Some(456)] {
                if context && timer.is_some() && ticks.is_some() {
                    continue;
                }
                let mut h = Fixture {
                    context,
                    ticks: 0xfedcba9876543210,
                    ..Fixture::default()
                };
                assert!(!get_data(&mut h, timer, ticks));
                let reason = if !context {
                    "called while not in a graphics context"
                } else if timer.is_none() {
                    "Null 'timer' parameter"
                } else {
                    "Null 'ticks' parameter"
                };
                assert_eq!(h.logs, [("gs_timer_get_data", reason)]);
                assert!(h.events.is_empty());
                assert_eq!(h.ticks, 0xfedcba9876543210);
            }
        }
    }
}
#[test]
fn test_timer_dispatch_and_ownership() {
    let mut h = Fixture {
        context: true,
        timer: Some(Box::new(7)),
        ..Fixture::default()
    };
    for (op, expected) in [
        (Command::End, "E"),
        (Command::Begin, "EB"),
        (Command::Begin, "EBB"),
        (Command::End, "EBBE"),
    ] {
        command(&mut h, Some(123), op);
        assert_eq!(h.events, expected);
        assert!(h.timer.is_some());
    }
    for result in [false, true] {
        for write in [false, true] {
            for value in [0, 1, 0xfedcba9876543210, u64::MAX] {
                h.ticks = 42;
                h.result = result;
                h.write = write;
                h.value = value;
                let before = h.events.len();
                assert_eq!(get_data(&mut h, Some(123), Some(456)), result);
                assert_eq!(h.events.len(), before + 1);
                assert!(h.events.ends_with('G'));
                assert_eq!(h.ticks, if write { value } else { 42 });
                assert!(h.timer.is_some());
            }
        }
    }
    command(&mut h, Some(123), Command::Destroy);
    assert!(h.timer.is_none());
    assert_eq!(h.events, "EBBEGGGGGGGGGGGGGGGGD");
    assert!(h.logs.is_empty());
}

pub(super) fn run(
    context: bool,
    timer: bool,
    ticks: bool,
    op: u32,
    result: bool,
    write: bool,
    value: u64,
) -> (bool, u64, String, String) {
    let mut h = Fixture {
        context,
        ticks: 42,
        timer: Some(Box::new(7)),
        result,
        write,
        value,
        ..Fixture::default()
    };
    let timer = timer.then_some(123);
    let result = if op == 3 {
        get_data(&mut h, timer, ticks.then_some(456))
    } else {
        command(
            &mut h,
            timer,
            [Command::Destroy, Command::Begin, Command::End][op as usize],
        );
        false
    };
    let log = h
        .logs
        .first()
        .map(|(f, r)| format!("{f}: {r}"))
        .unwrap_or_default();
    (result, h.ticks, log, h.events)
}
