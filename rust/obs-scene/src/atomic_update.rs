//! Scene callback orchestration, independent of C layout and locking primitives.

/// Operations on a scene owned by the caller. Acquiring may fail for an expired
/// scene; removal alone does not imply expiration.
pub trait Scene: Sized {
    /// Acquire a temporary strong reference, if the source is still alive.
    fn acquire(&self) -> Option<Self>;
    /// Lock video, then audio.
    fn lock(&self);
    /// Unlock audio, then video.
    fn unlock(&self);
}

/// Run exactly once under both locks and a temporary reference. Dropping the
/// acquired scene releases that reference, after unlocking.
pub fn update<S: Scene>(scene: Option<&S>, callback: impl FnOnce(&S)) {
    let _ = (scene, callback);
    todo!("scene atomic update")
}
