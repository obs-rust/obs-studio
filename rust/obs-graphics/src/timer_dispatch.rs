//! Timer validation and synchronous opaque backend dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Destroy,
    Begin,
    End,
}
impl Command {
    pub fn function(self) -> &'static str {
        match self {
            Self::Destroy => "gs_timer_destroy",
            Self::Begin => "gs_timer_begin",
            Self::End => "gs_timer_end",
        }
    }
}
/// The backend owns both timer storage and output writes.
pub trait Host<T> {
    type Output;
    fn has_context(&self) -> bool;
    fn debug(&mut self, function: &'static str, reason: &'static str);
    fn command(&mut self, command: Command, timer: T);
    fn data(&mut self, timer: T, ticks: Self::Output) -> bool;
}
pub fn command<T>(host: &mut impl Host<T>, timer: Option<T>, command: Command) {
    if !host.has_context() {
        host.debug(command.function(), "called while not in a graphics context");
        return;
    }
    if let Some(timer) = timer {
        host.command(command, timer);
    }
}
pub fn get_data<T, H: Host<T>>(host: &mut H, timer: Option<T>, ticks: Option<H::Output>) -> bool {
    const FUNCTION: &str = "gs_timer_get_data";
    if !host.has_context() {
        host.debug(FUNCTION, "called while not in a graphics context");
        return false;
    }
    let Some(timer) = timer else {
        host.debug(FUNCTION, "Null 'timer' parameter");
        return false;
    };
    let Some(ticks) = ticks else {
        host.debug(FUNCTION, "Null 'ticks' parameter");
        return false;
    };
    host.data(timer, ticks)
}
