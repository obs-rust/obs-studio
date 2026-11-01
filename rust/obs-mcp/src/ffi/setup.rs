//! Brings libobs up like the application does: video, audio, modules and the
//! record and stream outputs (#3317).

use super::Output;
use crate::host::StartOptions;

/// What [`configure`] set up. Dropping it releases the outputs, which must
/// happen before `obs_shutdown`.
pub struct Pipeline {
    _outputs: Vec<Output>,
    video_error: Option<String>,
    output_error: Option<String>,
    notes: Vec<String>,
}

impl Pipeline {
    /// Why there is no video subsystem, if there is none.
    pub fn video_error(&self) -> Option<&str> {
        self.video_error.as_deref()
    }

    /// Why the record or stream output could not be created, if it could not.
    pub fn output_error(&self) -> Option<&str> {
        self.output_error.as_deref()
    }

    /// Every problem found, in order, for the log.
    pub fn notes(&self) -> &[String] {
        &self.notes
    }
}

/// Resets video and audio, loads the modules and creates the outputs.
///
/// Never fails: a step that cannot complete is recorded in the result.
pub fn configure(_options: &StartOptions) -> Pipeline {
    todo!("#3317: video, audio, modules and outputs")
}
