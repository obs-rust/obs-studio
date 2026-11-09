//! Tier 1 mirrors test_graphics_stencil.c's recording-device assertions.
use obs_graphics::stencil::{Backend, Command, dispatch};

#[derive(Default)]
struct Device {
    calls: Vec<Command>,
}
impl Backend for Device {
    fn test(&mut self, value: bool) {
        self.calls.push(Command::Test(value));
    }
    fn write(&mut self, value: bool) {
        self.calls.push(Command::Write(value));
    }
    fn function(&mut self, side: i32, test: i32) {
        self.calls.push(Command::Function { side, test });
    }
    fn op(&mut self, side: i32, fail: i32, zfail: i32, zpass: i32) {
        self.calls.push(Command::Op {
            side,
            fail,
            zfail,
            zpass,
        });
    }
}

#[test]
fn test_graphics_stencil_dispatch() {
    let mut device = Device::default();
    let mut commands = Vec::new();
    for value in [false, true] {
        for _ in 0..2 {
            commands.extend([Command::Test(value), Command::Write(value)]);
        }
    }
    for side in 0..3 {
        for test in 0..8 {
            commands.push(Command::Function { side, test });
        }
        for fail in 0..6 {
            for zfail in 0..6 {
                for zpass in 0..6 {
                    commands.push(Command::Op {
                        side,
                        fail,
                        zfail,
                        zpass,
                    });
                }
            }
        }
    }
    for command in commands.iter().copied() {
        dispatch(Some(&mut device), command);
        assert_eq!(device.calls.len(), 1);
        assert_eq!(device.calls.pop(), Some(command));
    }
    assert_eq!(commands.len(), 680);
    for _ in 0..2 {
        for command in [
            Command::Test(true),
            Command::Write(false),
            Command::Function { side: 1, test: 1 },
            Command::Op {
                side: 2,
                fail: 2,
                zfail: 3,
                zpass: 4,
            },
        ] {
            dispatch(None::<&mut Device>, command);
            assert!(device.calls.is_empty());
        }
    }
}
