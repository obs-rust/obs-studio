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
    todo!()
}
pub fn get_data<T, H: Host<T>>(host: &mut H, timer: Option<T>, ticks: Option<H::Output>) -> bool {
    todo!()
}
