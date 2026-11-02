//! Startup notes: a missing audio subsystem is logged, not written to stderr.

use obs_mcp::startup::{Note, report};

fn route(notes: &[Note]) -> (String, Vec<String>) {
    let mut stderr = Vec::new();
    let mut logged = Vec::new();
    report(notes, &mut stderr, &mut |line| logged.push(line.to_owned())).expect("report");
    (String::from_utf8(stderr).expect("utf8"), logged)
}

#[test]
fn missing_audio_is_logged_not_stderr() {
    let (stderr, logged) = route(&[Note::warning("cannot start audio at 48 kHz stereo")]);
    assert_eq!(stderr, "", "a missing audio driver must not reach stderr");
    assert_eq!(logged.len(), 1);
    assert!(logged[0].contains("cannot start audio"), "{logged:?}");
}

#[test]
fn errors_still_reach_stderr() {
    let (stderr, logged) = route(&[
        Note::error("cannot start video"),
        Note::warning("cannot start audio"),
    ]);
    assert_eq!(stderr, "obs-mcp: cannot start video\n");
    assert_eq!(logged, ["cannot start audio"]);
}
