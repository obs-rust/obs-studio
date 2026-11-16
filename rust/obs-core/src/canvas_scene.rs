//! Canvas scene lifecycle wrappers, with storage and ownership retained by the host.

/// The four operations performed by the original C wrappers.
pub trait Host {
    /// Opaque identity of a canvas, source or scene.
    type Handle: Copy;
    /// Borrowed scene name, including the host's absent-name representation.
    type Name: Copy;
    fn create_scene_source(&mut self, canvas: Self::Handle, name: Self::Name) -> Self::Handle;
    fn source_scene(&self, source: Self::Handle) -> Self::Handle;
    fn scene_source(&self, scene: Self::Handle) -> Self::Handle;
    fn remove_source(&mut self, source: Self::Handle);
}

/// Create a scene source on the requested canvas and return its context data.
pub fn create<H: Host>(host: &mut H, canvas: H::Handle, name: H::Name) -> H::Handle {
    let source = host.create_scene_source(canvas, name);
    host.source_scene(source)
}

/// Remove the scene's source from its canvas, leaving the scene alive.
pub fn remove<H: Host>(host: &mut H, scene: H::Handle) {
    let source = host.scene_source(scene);
    host.remove_source(source);
}
