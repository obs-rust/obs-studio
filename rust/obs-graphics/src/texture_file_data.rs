//! Legacy image-file orchestration; FFmpeg state and callbacks stay C-owned.

/// Post-decode metadata, published only when decoding returns pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub format: i32,
    pub width: u32,
    pub height: u32,
}

/// A private image session. Implementations retain ownership of decoder state;
/// the returned pixel allocation belongs to the caller.
pub trait Backend {
    type Pixels;
    fn init(&mut self) -> bool;
    fn decode_straight(&mut self) -> Option<Self::Pixels>;
    fn metadata(&mut self) -> Metadata;
    fn publish(&mut self, metadata: Metadata);
    fn free(&mut self);
}

/// Load legacy straight-alpha pixels and publish post-decode metadata before
/// releasing an initialized session. Failed initialization owns its cleanup.
pub fn create_texture_file_data<B: Backend>(backend: &mut B) -> Option<B::Pixels> {
    if !backend.init() {
        return None;
    }
    let data = backend.decode_straight();
    if data.is_some() {
        let metadata = backend.metadata();
        backend.publish(metadata);
    }
    backend.free();
    data
}
