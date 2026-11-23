//! Out-of-process discovery for the VST3 filter.

#![deny(unsafe_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Run the scanner against the supplied installation roots.
pub fn run(
    _cache: Option<&Path>,
    _roots: &[PathBuf],
    _stdout: &mut impl Write,
    _stderr: &mut impl Write,
) -> ExitCode {
    todo!("VST3 scanner port")
}
