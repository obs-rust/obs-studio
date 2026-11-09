//! Stencil commands preserve every call, including repeated boolean values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Test(bool),
    Write(bool),
    Function {
        side: i32,
        test: i32,
    },
    Op {
        side: i32,
        fail: i32,
        zfail: i32,
        zpass: i32,
    },
}

/// The current device's stencil callbacks.
pub trait Backend {
    fn test(&mut self, enable: bool);
    fn write(&mut self, enable: bool);
    fn function(&mut self, side: i32, test: i32);
    fn op(&mut self, side: i32, fail: i32, zfail: i32, zpass: i32);
}

/// Dispatch once to a valid context; absent contexts never reach the device.
pub fn dispatch(backend: Option<&mut impl Backend>, command: Command) {
    let Some(backend) = backend else { return };
    match command {
        Command::Test(enable) => backend.test(enable),
        Command::Write(enable) => backend.write(enable),
        Command::Function { side, test } => backend.function(side, test),
        Command::Op {
            side,
            fail,
            zfail,
            zpass,
        } => backend.op(side, fail, zfail, zpass),
    }
}
