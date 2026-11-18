//! Indexbuffer maintenance decisions, independent of graphics layouts.
use std::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Destroy,
    Flush,
    Direct,
    Count,
}

impl Entry {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Destroy => "gs_indexbuffer_destroy",
            Self::Flush => "gs_indexbuffer_flush",
            Self::Direct => "gs_indexbuffer_flush_direct",
            Self::Count => "gs_indexbuffer_get_num_indices",
        }
    }
    pub const fn c_name(self) -> &'static CStr {
        match self {
            Self::Destroy => c"gs_indexbuffer_destroy",
            Self::Flush => c"gs_indexbuffer_flush",
            Self::Direct => c"gs_indexbuffer_flush_direct",
            Self::Count => c"gs_indexbuffer_get_num_indices",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    Context,
    Buffer,
    Data,
}

impl Rejection {
    pub const fn message(self) -> &'static CStr {
        match self {
            Self::Context => c"called while not in a graphics context",
            Self::Buffer => c"Null 'indexbuffer' parameter",
            Self::Data => c"Null 'data' parameter",
        }
    }
}

/// Backend operations are reached only after all Rust guards succeed.
pub trait Host {
    type Buffer: Copy;
    type Data: Copy;
    fn has_context(&self) -> bool;
    fn debug(&mut self, entry: Entry, reason: Rejection);
    fn destroy(&mut self, buffer: Self::Buffer);
    fn flush(&mut self, buffer: Self::Buffer);
    fn direct(&mut self, buffer: Self::Buffer, data: Self::Data);
    fn count(&mut self, buffer: Self::Buffer) -> usize;
}

fn valid<H: Host>(host: &mut H, entry: Entry, buffer: Option<H::Buffer>) -> Option<H::Buffer> {
    if !host.has_context() {
        host.debug(entry, Rejection::Context);
        return None;
    }
    if buffer.is_none() && entry != Entry::Destroy {
        host.debug(entry, Rejection::Buffer);
    }
    buffer
}

pub fn destroy<H: Host>(host: &mut H, buffer: Option<H::Buffer>) {
    if let Some(buffer) = valid(host, Entry::Destroy, buffer) {
        host.destroy(buffer);
    }
}

pub fn flush<H: Host>(host: &mut H, buffer: Option<H::Buffer>) {
    if let Some(buffer) = valid(host, Entry::Flush, buffer) {
        host.flush(buffer);
    }
}

pub fn flush_direct<H: Host>(host: &mut H, buffer: Option<H::Buffer>, data: Option<H::Data>) {
    if let Some(buffer) = valid(host, Entry::Direct, buffer) {
        if let Some(data) = data {
            host.direct(buffer, data);
        } else {
            host.debug(Entry::Direct, Rejection::Data);
        }
    }
}

pub fn get_num_indices<H: Host>(host: &mut H, buffer: Option<H::Buffer>) -> usize {
    valid(host, Entry::Count, buffer).map_or(0, |buffer| host.count(buffer))
}
