//! Close it: SIGTERM, then SIGKILL after the grace (#3724). A fake
//! process here; real child processes below on Linux.

use obs_camera_busy::{Closed, POLL_MS, Signals, close_with};

/// A process that dies after `term_after` SIGTERMs polls (None: ignores
/// SIGTERM), and on SIGKILL unless `unkillable`.
#[derive(Default)]
struct Fake {
    alive: bool,
    term_dies: bool,
    unkillable: bool,
    can_signal: bool,
    sent: Vec<&'static str>,
    slept: u64,
}

impl Fake {
    fn new() -> Self {
        Self {
            alive: true,
            can_signal: true,
            ..Self::default()
        }
    }
}

impl Signals for Fake {
    fn terminate(&mut self, _pid: u32) -> bool {
        self.sent.push("TERM");
        if self.can_signal && self.term_dies {
            self.alive = false;
        }
        self.can_signal
    }

    fn kill(&mut self, _pid: u32) -> bool {
        self.sent.push("KILL");
        if self.can_signal && !self.unkillable {
            self.alive = false;
        }
        self.can_signal
    }

    fn alive(&mut self, _pid: u32) -> bool {
        self.alive
    }

    fn sleep_ms(&mut self, ms: u64) {
        self.slept += ms;
    }
}

#[test]
fn exits_on_sigterm() {
    let mut p = Fake {
        term_dies: true,
        ..Fake::new()
    };
    assert_eq!(close_with(&mut p, 12, 2_000), Closed::Exited);
    assert_eq!(p.sent, ["TERM"]);
}

#[test]
fn sigkill_after_the_grace() {
    let mut p = Fake::new();
    assert_eq!(close_with(&mut p, 12, 2_000), Closed::Killed);
    assert_eq!(p.sent, ["TERM", "KILL"]);
    assert!(p.slept >= 2_000 && p.slept < 2_000 + POLL_MS);
}

#[test]
fn fails_when_even_sigkill_does_not_help() {
    let mut p = Fake {
        unkillable: true,
        ..Fake::new()
    };
    assert_eq!(close_with(&mut p, 12, 200), Closed::Failed);
    assert_eq!(p.sent, ["TERM", "KILL"]);
}

#[test]
fn fails_when_it_cannot_be_signalled() {
    let mut p = Fake {
        can_signal: false,
        ..Fake::new()
    };
    assert_eq!(close_with(&mut p, 12, 200), Closed::Failed);
    assert_eq!(p.sent, ["TERM"]);
}

#[test]
fn already_gone_is_exited() {
    let mut p = Fake {
        alive: false,
        ..Fake::new()
    };
    assert_eq!(close_with(&mut p, 12, 200), Closed::Exited);
    assert!(p.sent.is_empty());
}

#[cfg(target_os = "linux")]
mod real {
    use std::process::{Command, Stdio};

    use obs_camera_busy::{Closed, ProcessSignals, close_with};

    fn spawn(script: &str) -> std::process::Child {
        Command::new("sh")
            .args(["-c", script])
            .stdin(Stdio::null())
            .spawn()
            .unwrap()
    }

    #[test]
    fn a_real_process_exits_on_sigterm() {
        let mut child = spawn("exec sleep 30");
        let closed = close_with(&mut ProcessSignals, child.id(), 2_000);
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(closed, Closed::Exited);
    }

    #[test]
    fn a_real_process_that_ignores_sigterm_is_killed() {
        // An ignored signal stays ignored across exec.
        let mut child = spawn("trap '' TERM; exec sleep 30");
        // Let sh run the trap before the signal.
        std::thread::sleep(std::time::Duration::from_millis(200));
        let closed = close_with(&mut ProcessSignals, child.id(), 200);
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(closed, Closed::Killed);
    }
}
