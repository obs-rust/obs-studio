//! Synchronous depth-state validation and backend dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Enable(bool),
    Function(u32),
}
impl Command {
    pub fn function(self) -> &'static str {
        match self {
            Self::Enable(_) => "gs_enable_depth_test",
            Self::Function(_) => "gs_depth_function",
        }
    }
}
pub trait Host {
    fn has_context(&self) -> bool;
    fn debug(&mut self, function: &'static str, reason: &'static str);
    fn dispatch(&mut self, command: Command);
}
pub fn command(host: &mut impl Host, command: Command) {
    if !host.has_context() {
        host.debug(command.function(), "called while not in a graphics context");
        return;
    }
    host.dispatch(command);
}
