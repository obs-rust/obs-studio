//! Requests to the background shortcut client under the registry mutex.

/// The fields owned by background shortcut control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub retry: bool,
    pub configure: bool,
    pub probe: bool,
    pub flags: u32,
}

#[derive(Clone, Copy, Debug)]
pub enum Request {
    Retry,
    Configure,
    Probe(bool),
}

/// The existing registry lock and client callback, with fields accessed only
/// between a successful lock and its matching unlock.
pub trait BackgroundControl {
    fn lock(&mut self) -> bool;
    fn read(&self) -> State;
    fn write(&mut self, state: State);
    fn wake(&mut self);
    fn unlock(&mut self);
}

pub fn request<B: BackgroundControl>(_background: &mut B, _request: Request) {
    todo!()
}
