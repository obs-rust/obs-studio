//! Timestamped push-hotkey events; queue ownership and source layout stay in C.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Mute,
    Talk,
}

pub trait Host {
    fn clock(&mut self) -> u64;
    fn lock(&mut self);
    fn append(&mut self, timestamp: u64, kind: Kind, pressed: bool);
    fn unlock(&mut self);
    fn store(&mut self, kind: Kind, pressed: bool);
}

pub fn event(host: &mut impl Host, kind: Kind, pressed: bool) {
    let timestamp = host.clock();
    host.lock();
    host.append(timestamp, kind, pressed);
    host.unlock();
    host.store(kind, pressed);
}
