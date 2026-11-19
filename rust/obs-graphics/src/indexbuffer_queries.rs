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

pub fn get_data<H: Host>(_host: &mut H, _buffer: Option<H::Buffer>) -> Option<H::Data> {
    todo!("indexbuffer query port")
}

pub fn get_type<H: Host>(_host: &mut H, _buffer: Option<H::Buffer>) -> u32 {
    todo!("indexbuffer query port")
}
