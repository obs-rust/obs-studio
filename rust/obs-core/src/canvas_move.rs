//! Scene-first canvas migration over the existing host ownership/enumeration seams.

/// Storage and item lifetimes remain owned by libobs.
pub trait Host: Sized {
    type Handle: Copy;
    fn scene_source(&self, scene: Self::Handle) -> Self::Handle;
    fn item_source(&self, item: Self::Handle) -> Self::Handle;
    fn is_group(&self, source: Self::Handle) -> bool;
    fn remove_source(&mut self, source: Self::Handle);
    fn insert_source(&mut self, canvas: Self::Handle, source: Self::Handle);
    fn enumerate_items<F: FnMut(&mut Self, Self::Handle)>(&mut self, scene: Self::Handle, visit: F);
}

/// Remove and insert the scene before enumerating its current items.
pub fn move_scene<H: Host>(host: &mut H, scene: H::Handle, dst: H::Handle) {
    let source = host.scene_source(scene);
    host.remove_source(source);
    host.insert_source(dst, source);
    host.enumerate_items(scene, |host, item| {
        let source = host.item_source(item);
        if host.is_group(source) {
            host.remove_source(source);
            host.insert_source(dst, source);
        }
    });
}
