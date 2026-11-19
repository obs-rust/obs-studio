//! The staging cleanup at the start of each output stage
//! (`unmap_last_surface()`), over a mix's mapped staging surfaces.
//!
//! `stage_output_texture()` calls it once per frame before it stages the next
//! one: every channel whose staging surface is still mapped from the last
//! download is unmapped, in channel order, and its slot reset to NULL only
//! after the unmap returns. Unmapped slots make no graphics call, so a second
//! cleanup does nothing.

/// A mix's `mapped_surfaces[NUM_CHANNELS]` as the cleanup sees it, and
/// `gs_stagesurface_unmap()`.
pub trait MappedSurfaces {
    /// A `gs_stagesurf_t *`, passed through unread.
    type Surface: Copy;
    /// `NUM_CHANNELS`, the number of slots.
    fn channels(&self) -> usize;
    /// Slot `channel`'s surface; `None` for NULL.
    fn mapped(&mut self, channel: usize) -> Option<Self::Surface>;
    /// `gs_stagesurface_unmap(surface)`.
    fn unmap(&mut self, surface: Self::Surface);
    /// Resets slot `channel` to NULL.
    fn clear(&mut self, channel: usize);
}

/// Unmaps every mapped slot in channel order, resetting each slot after its
/// own unmap has returned.
pub fn unmap_last_surface(surfaces: &mut impl MappedSurfaces) {
    let _ = surfaces;
    todo!("unmap_last_surface")
}
