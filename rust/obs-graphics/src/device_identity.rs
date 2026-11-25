//! Borrowed device identities, with context admission before native dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Object,
    Name,
}

impl Entry {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Object => "gs_get_device_obj",
            Self::Name => "gs_get_device_name",
        }
    }
}

/// Values retain their backend identity; the default represents NULL.
/// Both callbacks are required once a context has been admitted.
pub trait Host {
    type Value: Default;
    fn has_context(&self) -> bool;
    fn debug(&mut self, entry: Entry);
    fn dispatch(&mut self, entry: Entry) -> Self::Value;
}

pub fn identity<H: Host>(host: &mut H, entry: Entry) -> H::Value {
    if !host.has_context() {
        host.debug(entry);
        return H::Value::default();
    }
    host.dispatch(entry)
}
