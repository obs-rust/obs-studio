//! Out-of-process discovery for the VST3 filter.

#![deny(unsafe_code)]

mod cache;
mod discovery;
#[allow(unsafe_code)]
mod ffi;
mod metadata;
mod platform;

use cache::Cache;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

/// Run the scanner against the supplied installation roots.
pub fn run(
    cache: Option<&Path>,
    roots: &[PathBuf],
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let Some(cache_path) = cache.filter(|path| !path.as_os_str().is_empty()) else {
        let _ = writeln!(stderr, "Usage: obs-vst3-scanner <vst3list.json>");
        return ExitCode::FAILURE;
    };
    match scan(cache_path, roots, stdout, stderr) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            let _ = writeln!(stderr, "Scan failed: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Run the scanner executable with the platform's VST3 installation roots.
pub fn command(cache: Option<&Path>) -> ExitCode {
    platform::prepare_process();
    run_installed(cache)
}

fn run_installed(cache: Option<&Path>) -> ExitCode {
    run(
        cache,
        &platform::search_roots(),
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
}

fn scan(
    cache_path: &Path,
    roots: &[PathBuf],
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> io::Result<bool> {
    if let Some(parent) = cache_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let start = Instant::now();
    let candidates = discovery::find_candidates(roots);
    if candidates.is_empty() {
        writeln!(stdout, "NO_VST3")?;
        return Ok(false);
    }

    let cached = Cache::restore(cache_path);
    let updated = discovery::scan(&candidates, cached);
    updated.save(cache_path)?;
    let effects = updated.plugins.iter().filter(|class| class.is_fx).count();
    writeln!(
        stderr,
        "[VST3 Scanner] Completed scan in {} ms, found {effects} plugins",
        start.elapsed().as_millis()
    )?;
    writeln!(stdout, "VST3_SCAN_DONE")?;
    Ok(true)
}
