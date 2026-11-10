//! Timer-range validation and synchronous opaque backend dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Destroy,
    Begin,
    End,
}
impl Command {
    pub fn function(self) -> &'static str {
        match self {
            Self::Destroy => "gs_timer_range_destroy",
            Self::Begin => "gs_timer_range_begin",
            Self::End => "gs_timer_range_end",
        }
    }
}
/// The backend owns range storage and both independent output writes.
pub trait Host<T> {
    type Disjoint;
    type Frequency;
    fn has_context(&self) -> bool;
    fn debug(&mut self, function: &'static str, reason: &'static str);
    fn command(&mut self, command: Command, range: T);
    fn data(&mut self, range: T, disjoint: Self::Disjoint, frequency: Self::Frequency) -> bool;
}
pub fn command<T>(host: &mut impl Host<T>, range: Option<T>, command: Command) {
    if !host.has_context() {
        host.debug(command.function(), "called while not in a graphics context");
        return;
    }
    if let Some(range) = range {
        host.command(command, range);
    }
}
/// Range is deliberately not validated: even NULL reaches the backend.
pub fn get_data<T, H: Host<T>>(
    host: &mut H,
    range: T,
    disjoint: Option<H::Disjoint>,
    frequency: Option<H::Frequency>,
) -> bool {
    const FUNCTION: &str = "gs_timer_range_get_data";
    if !host.has_context() {
        host.debug(FUNCTION, "called while not in a graphics context");
        return false;
    }
    let Some(disjoint) = disjoint else {
        host.debug(FUNCTION, "Null 'disjoint' parameter");
        return false;
    };
    let Some(frequency) = frequency else {
        host.debug(FUNCTION, "Null 'frequency' parameter");
        return false;
    };
    host.data(range, disjoint, frequency)
}
