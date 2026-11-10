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
pub fn command<T>(_host: &mut impl Host<T>, _range: Option<T>, _command: Command) {
    todo!("timer-range command port")
}
pub fn get_data<T, H: Host<T>>(
    _host: &mut H,
    _range: T,
    _disjoint: Option<H::Disjoint>,
    _frequency: Option<H::Frequency>,
) -> bool {
    todo!("timer-range data port")
}
