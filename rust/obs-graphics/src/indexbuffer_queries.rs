//! Index-buffer queries without graphics layouts or GPU dependencies.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Data,
    Type,
}

impl Entry {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Data => "gs_indexbuffer_get_data",
            Self::Type => "gs_indexbuffer_get_type",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    Context,
    Buffer,
}

impl Rejection {
    pub const fn message(self) -> &'static str {
        match self {
            Self::Context => "called while not in a graphics context",
            Self::Buffer => "Null 'indexbuffer' parameter",
        }
    }
}

/// Callbacks are mandatory and reached only after context and buffer checks.
pub trait Host {
    type Buffer: Copy;
    type Data;
    fn has_context(&self) -> bool;
    fn debug(&mut self, entry: Entry, reason: Rejection);
    fn data(&mut self, buffer: Self::Buffer) -> Option<Self::Data>;
    fn index_type(&mut self, buffer: Self::Buffer) -> u32;
}

fn valid<H: Host>(host: &mut H, entry: Entry, buffer: Option<H::Buffer>) -> Option<H::Buffer> {
    if !host.has_context() {
        host.debug(entry, Rejection::Context);
        return None;
    }
    if buffer.is_none() {
        host.debug(entry, Rejection::Buffer);
    }
    buffer
}

pub fn get_data<H: Host>(host: &mut H, buffer: Option<H::Buffer>) -> Option<H::Data> {
    valid(host, Entry::Data, buffer).and_then(|buffer| host.data(buffer))
}

pub fn get_type<H: Host>(host: &mut H, buffer: Option<H::Buffer>) -> u32 {
    valid(host, Entry::Type, buffer).map_or(0, |buffer| host.index_type(buffer))
}
