//! Canvas event payload construction and ordered routing.

/// Fixed storage used by both original helpers.
pub const CAPACITY: usize = 128;

/// A destination, resolved only when its turn to dispatch is reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Global,
    Local,
}

/// Host operations over a live canvas and a single mutable event payload.
pub trait Host {
    /// Construct a fixed payload containing the canvas and optional source.
    fn init(&mut self, source: Option<usize>);
    /// Read the canvas privacy flag at the global-route decision.
    fn private(&self) -> bool;
    /// Dispatch synchronously, retaining payload edits for subsequent routes.
    fn signal(&mut self, route: Route, name: &std::ffi::CStr);
}

/// Fill the fixed calldata buffer, returning its initialized length.
pub fn payload(stack: &mut [u8; CAPACITY], canvas: usize, source: Option<usize>) -> usize {
    let mut data = obs_callback::Calldata::fixed(stack);
    data.set_ptr("canvas", canvas);
    if let Some(source) = source {
        data.set_ptr("source", source);
    }
    data.size()
}

/// Emit a canvas event, global before local, with independent optional routes.
pub fn emit(host: &mut impl Host, global: Option<&std::ffi::CStr>, local: Option<&std::ffi::CStr>) {
    host.init(None);
    if let Some(name) = global
        && !host.private()
    {
        host.signal(Route::Global, name);
    }
    if let Some(name) = local {
        host.signal(Route::Local, name);
    }
}

/// Emit a source event only on its canvas, including a null source pointer.
pub fn emit_source(host: &mut impl Host, name: &std::ffi::CStr, source: usize) {
    host.init(Some(source));
    host.signal(Route::Local, name);
}
