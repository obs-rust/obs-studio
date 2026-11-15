//! The legacy `obs_output_set_preferred_size` wrapper: validate the output,
//! check its video flag, then delegate to the indexed setter at slot 0.
use crate::video_checks::OBS_OUTPUT_VIDEO;

/// What the wrapper does for one call, in the original gate order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// NULL output: the debug diagnostic under the legacy name, nothing else.
    Null,
    /// No video flag: the non-video warning under the legacy name.
    NotVideo,
    /// Delegate the unchanged width and height to the indexed setter at slot 0.
    Delegate,
}

/// The slot the legacy wrapper always delegates to.
pub const DEFAULT_SLOT: usize = 0;

/// `flags` is read only for a present output.
pub fn decide(present: bool, flags: impl FnOnce() -> u32) -> Decision {
    if !present {
        return Decision::Null;
    }
    if flags() & OBS_OUTPUT_VIDEO == 0 {
        return Decision::NotVideo;
    }
    Decision::Delegate
}
