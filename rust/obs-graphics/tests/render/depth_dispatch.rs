//! Safe 1:1 mirror of test_depth_dispatch.c, without GPU or FFI types.
use obs_graphics::depth_dispatch::{self, Command, Host};
#[derive(Default)]
pub(super) struct Fixture {
    pub device: Option<usize>,
    pub events: Vec<(usize, Command)>,
    pub logs: Vec<String>,
}
impl Host for Fixture {
    fn has_context(&self) -> bool { self.device.is_some() }
    fn debug(&mut self, function: &'static str, reason: &'static str) {
        self.logs.push(format!("{function}: {reason}"));
    }
    fn dispatch(&mut self, command: Command) {
        self.events.push((self.device.unwrap(), command));
    }
}
#[test]
fn test_depth_dispatch_missing_context() {
    let mut host = Fixture::default();
    for phase in 0..2 {
        if phase == 1 { host.device = Some(0); host.device = None; }
        for command in [Command::Enable(false), Command::Enable(true)].into_iter()
            .chain((0..8).map(Command::Function)) {
            depth_dispatch::command(&mut host, command);
            assert_eq!(host.logs, [format!("{}: called while not in a graphics context", command.function())]);
            assert!(host.events.is_empty());
            host.logs.clear();
        }
    }
}
#[test]
fn test_depth_dispatch_forwarding() {
    let mut host = Fixture::default();
    let enables = [true, true, false, false, true, false, true, false];
    let mut enable_calls = 0;
    let mut function_calls = 0;
    for device in 0..2 {
        host.device = Some(device);
        for (value, enable) in enables.into_iter().enumerate() {
            let before = host.events.len();
            let command = Command::Enable(enable);
            depth_dispatch::command(&mut host, command);
            enable_calls += 1;
            assert_eq!(host.events.len(), before + 1);
            assert_eq!(host.events.last(), Some(&(device, command)));
            let command = Command::Function(value as u32);
            for repeat in 1..=2 {
                depth_dispatch::command(&mut host, command);
                function_calls += 1;
                assert_eq!(host.events.len(), before + 1 + repeat);
                assert_eq!(host.events.last(), Some(&(device, command)));
                assert_eq!(host.events.iter().filter(|(_, c)| matches!(c, Command::Enable(_))).count(), enable_calls);
                assert_eq!(host.events.iter().filter(|(_, c)| matches!(c, Command::Function(_))).count(), function_calls);
                assert!(host.logs.is_empty());
            }
        }
        host.device = None;
        assert!(!host.has_context());
    }
    assert_eq!(enable_calls, 16);
    assert_eq!(function_calls, 32);
}
