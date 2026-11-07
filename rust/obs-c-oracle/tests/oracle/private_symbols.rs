//! The oracle archive must not define libobs' public threading symbols.
//! obs-util exports the Rust implementations of the same names, so a test
//! binary that links both archives (obs-media-io's audio_io) fails with
//! duplicate symbols whenever the linker pulls both definitions in, which
//! non-incremental builds do. The oracle's C callers use the private copy in
//! oracle/threading_posix.c on Unix and oracle/threading_windows.c on Windows.
#![cfg(unix)]

use std::path::Path;
use std::process::Command;

const PUBLIC_THREADING: [&str; 12] = [
    "os_event_init",
    "os_event_destroy",
    "os_event_wait",
    "os_event_timedwait",
    "os_event_try",
    "os_event_signal",
    "os_event_reset",
    "os_sem_init",
    "os_sem_destroy",
    "os_sem_post",
    "os_sem_wait",
    "os_set_thread_name",
];

#[test]
fn encoder_metadata_has_no_production_getter_dependencies() {
    let archive = Path::new(obs_c_oracle::ORACLE_ARCHIVE_DIR).join("libobs_c_oracle.a");
    let out = Command::new("nm")
        .args(["-A", "-u"])
        .arg(&archive)
        .output()
        .expect("run nm");
    assert!(out.status.success(), "nm {} failed", archive.display());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let dependencies: Vec<_> = stdout
        .lines()
        .filter(|line| line.contains("encoder_metadata.o:"))
        .filter(|line| {
            line.split_whitespace()
                .last()
                .is_some_and(|symbol| symbol.trim_start_matches('_').starts_with("obs_encoder_get_"))
        })
        .collect();
    assert!(
        dependencies.is_empty(),
        "metadata oracle must link without obs-output: {dependencies:?}"
    );
}

#[test]
fn oracle_archive_keeps_threading_helpers_private() {
    let archive = Path::new(obs_c_oracle::ORACLE_ARCHIVE_DIR).join("libobs_c_oracle.a");
    let out = Command::new("nm")
        .arg("-g")
        .arg(&archive)
        .output()
        .expect("run nm");
    assert!(out.status.success(), "nm {} failed", archive.display());

    // Defined symbols print as "<address> <type> <name>"; undefined ones as
    // "U <name>" and are skipped. Mach-O prefixes C names with '_'.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let exported: Vec<&str> = stdout
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            match fields.as_slice() {
                [_, ty, name] if *ty != "U" => Some(name.trim_start_matches('_')),
                _ => None,
            }
        })
        .filter(|name| PUBLIC_THREADING.contains(name))
        .collect();
    assert!(
        exported.is_empty(),
        "the oracle defines public libobs threading symbols: {exported:?}"
    );
}
