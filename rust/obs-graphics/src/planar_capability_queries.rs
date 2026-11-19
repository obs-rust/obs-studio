//! Optional planar capability queries over a caller-owned graphics context.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Nv12,
    P010,
}

impl Entry {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Nv12 => "gs_nv12_available",
            Self::P010 => "gs_p010_available",
        }
    }
}

/// Field access and dispatch are reached only after the corresponding checks.
pub trait Host {
    fn has_context(&self) -> bool;
    fn debug(&mut self, entry: Entry);
    fn has_callback(&self, entry: Entry) -> bool;
    fn dispatch(&mut self, entry: Entry) -> bool;
}

pub fn available<H: Host>(_host: &mut H, _entry: Entry) -> bool {
    todo!("planar capability query swap")
}
