//! The holder list and the log lines (#3724).

use obs_camera_busy::{
    Answer, Cause, Closed, Holder, Show, answer_log_line, close_log_line, closable_names, holder_list,
    log_line,
};

fn h(pid: u32, name: &str) -> Holder {
    Holder {
        pid,
        name: name.into(),
        can_close: true,
    }
}

#[test]
fn holder_list_names_app_and_pid() {
    assert_eq!(holder_list(&[]), "");
    assert_eq!(holder_list(&[h(12, "zoom")]), "zoom (PID 12)");
    assert_eq!(
        holder_list(&[h(12, "zoom"), h(40, "cheese")]),
        "zoom (PID 12), cheese (PID 40)"
    );
}

#[test]
fn busy_log_lines() {
    assert_eq!(
        log_line(Cause::Busy, "/dev/video0", &[h(12, "zoom")], Show::Dialog),
        "[camera-busy] /dev/video0 is busy, in use by zoom (PID 12): warning shown"
    );
    assert_eq!(
        log_line(Cause::Busy, "/dev/video0", &[], Show::Dialog),
        "[camera-busy] /dev/video0 is busy, in use by another program: warning shown"
    );
}

#[test]
fn opened_log_lines() {
    assert_eq!(
        log_line(
            Cause::Opened,
            "/dev/video0",
            &[h(12, "zoom")],
            Show::Allowed
        ),
        "[camera-busy] zoom (PID 12) opened /dev/video0 while OBS uses it: allowed, not warned"
    );
}

#[test]
fn every_decision_has_its_words() {
    let d = |s| log_line(Cause::Busy, "/dev/v", &[h(1, "a")], s);
    assert!(d(Show::Duplicate).ends_with(": a warning for it is already open"));
    assert!(d(Show::RateLimited).ends_with(": warned recently, not warned again"));
    assert!(d(Show::NoHolder).ends_with(": nobody else holds it"));
}

#[test]
fn answer_and_close_log_lines() {
    let holders = [h(12, "zoom")];
    assert_eq!(
        answer_log_line("/dev/video0", &holders, Answer::CloseIt),
        "[camera-busy] /dev/video0: Close it, closing zoom (PID 12)"
    );
    assert_eq!(
        answer_log_line("/dev/video0", &holders, Answer::Ignore),
        "[camera-busy] /dev/video0: Ignore"
    );
    assert_eq!(
        answer_log_line("/dev/video0", &holders, Answer::AlwaysAllow),
        "[camera-busy] /dev/video0: Always allow zoom (PID 12)"
    );
    assert_eq!(
        close_log_line("zoom", 12, Closed::Exited),
        "[camera-busy] zoom (PID 12) exited on SIGTERM"
    );
    assert_eq!(
        close_log_line("zoom", 12, Closed::Killed),
        "[camera-busy] zoom (PID 12) did not exit on SIGTERM and was killed (SIGKILL)"
    );
    assert_eq!(
        close_log_line("zoom", 12, Closed::Failed),
        "[camera-busy] zoom (PID 12) could not be closed"
    );
}

/// The Quit button names the programs it quits: only those this user may
/// close (#3726).
#[test]
fn closable_names_name_only_what_quit_closes() {
    let other = Holder {
        pid: 7,
        name: "pipewire".into(),
        can_close: false,
    };
    assert_eq!(closable_names(&[]), "");
    assert_eq!(closable_names(&[h(12, "zoom")]), "zoom");
    assert_eq!(closable_names(&[h(12, "zoom"), other.clone()]), "zoom");
    assert_eq!(closable_names(&[other]), "");
    assert_eq!(
        closable_names(&[h(12, "zoom"), h(40, "cheese")]),
        "zoom, cheese"
    );
}
