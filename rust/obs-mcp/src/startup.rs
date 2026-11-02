//! Where each startup note goes: the OBS log or stderr.
//!
//! A missing audio subsystem is not a startup failure. libobs mixes audio in
//! software (`obs_init_audio` opens no device, `libobs/obs.c.h:680`), so a
//! machine without audio hardware still starts audio; when the mixer cannot
//! start the run carries on without audio and the note is a warning for the
//! OBS log, not an stderr complaint. Only errors go to stderr.

use std::io::{self, Write};

/// How serious a startup note is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// The run continues with a degraded feature; goes to the OBS log.
    Warning,
    /// A feature the tools need is missing; goes to stderr.
    Error,
}

/// One problem found while bringing libobs up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub severity: Severity,
    pub text: String,
}

impl Note {
    #[must_use]
    pub fn warning(text: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            text: text.into(),
        }
    }

    #[must_use]
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            text: text.into(),
        }
    }
}

/// Sends warnings to `log` and errors to `stderr` as `obs-mcp: <text>`.
///
/// # Errors
///
/// Returns an error when writing to `stderr` fails.
pub fn report(notes: &[Note], stderr: &mut dyn Write, log: &mut dyn FnMut(&str)) -> io::Result<()> {
    for note in notes {
        match note.severity {
            Severity::Warning => log(&note.text),
            Severity::Error => writeln!(stderr, "obs-mcp: {}", note.text)?,
        }
    }
    Ok(())
}
