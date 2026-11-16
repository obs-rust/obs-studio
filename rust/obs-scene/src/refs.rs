//! Scene ownership delegates to the source; scenes have no separate counter.

/// Source access and lifetime effects supplied by the C boundary.
pub trait Host {
    type Scene: Copy;
    type Source: Copy;
    fn source(&mut self, scene: Self::Scene) -> Self::Source;
    fn acquire(&mut self, source: Self::Source) -> bool;
    fn release(&mut self, source: Self::Source);
}

/// Retain the source and return the argument scene only on successful promotion.
pub fn get_ref<H: Host>(host: &mut H, scene: Option<H::Scene>) -> Option<H::Scene> {
    let scene = scene?;
    let source = host.source(scene);
    host.acquire(source).then_some(scene)
}

/// Delegate exactly one source release for a present scene.
pub fn release<H: Host>(host: &mut H, scene: Option<H::Scene>) {
    if let Some(scene) = scene {
        let source = host.source(scene);
        host.release(source);
    }
}
