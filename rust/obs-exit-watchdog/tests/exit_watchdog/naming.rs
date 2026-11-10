//! The watchdog line names the library the armed thread waits in (#4525):
//! the line text, the choice of the frame, another thread's stack, and a
//! blocked child process whose line names the library and its module.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use obs_exit_watchdog::images::{Entry, Owner, Table, current, file_name};
use obs_exit_watchdog::{EXIT_CODE, LINE_MAX, arm_with, culprit, line, line_naming, stack};

use super::{LOG, block, count_lines, in_child, log_path, run_child, selector};

fn entry(start: usize, file: &str, owner: Owner) -> Entry {
    Entry {
        start,
        end: start + 0x1000,
        file: file.to_string(),
        owner,
    }
}

fn module(name: &str) -> Owner {
    Owner::Module(name.to_string())
}

#[test]
fn the_line_names_the_library_and_who_loaded_it() {
    let t = Duration::from_secs(10);
    assert_eq!(line_naming(t, None), line(t));
    let start = "exit-watchdog: exit did not finish in 10 s: a static destructor or \
                 atexit handler is blocking in ";
    let cases = [
        (
            entry(0, "libremidi.so.4", module("advanced-scene-switcher.so")),
            "libremidi.so.4 (loaded by advanced-scene-switcher.so)",
        ),
        (
            entry(0, "obs-avip.so", module("obs-avip.so")),
            "obs-avip.so (an OBS module)",
        ),
        (
            entry(0, "libpipewire-0.3.so.0", Owner::PostLoad),
            "libpipewire-0.3.so.0 (loaded during the modules' post-load)",
        ),
        (
            entry(0, "libnss3.so", Owner::AfterStartup),
            "libnss3.so (loaded after startup)",
        ),
    ];
    for (e, named) in cases {
        assert_eq!(
            line_naming(t, Some(&e)),
            format!("{start}{named}; ending the process\n")
        );
    }
}

/// A file name longer than the line buffer is cut; the line still ends.
#[test]
fn a_long_name_is_cut_and_the_line_still_ends() {
    let long = "x".repeat(2 * LINE_MAX);
    let text = line_naming(
        Duration::from_secs(10),
        Some(&entry(0, &long, module(&long))),
    );
    assert!(text.len() <= LINE_MAX, "{}", text.len());
    assert!(text.ends_with('\n'));
    assert!(text.starts_with("exit-watchdog: "));
}

/// The #3739 stack, innermost first: the wait in the C library (OBS's
/// own), then libremidi and midi.so (Advanced Scene Switcher's).
#[test]
fn the_first_frame_outside_obs_is_named() {
    let table = Table::from_entries(vec![
        entry(0x1000, "libc.so.6", Owner::Obs),
        entry(0x3000, "midi.so", module("advanced-scene-switcher.so")),
        entry(
            0x5000,
            "libremidi.so.4",
            module("advanced-scene-switcher.so"),
        ),
    ]);
    let frames = [0x1010, 0x1020, 0x9999, 0x5010, 0x3010, 0x1030];
    assert_eq!(
        culprit(&frames, &table).map(|e| e.file.as_str()),
        Some("libremidi.so.4")
    );
    assert_eq!(culprit(&[0x1010, 0x1020], &table), None, "only OBS's own");
    assert_eq!(culprit(&[], &table), None);
    assert_eq!(culprit(&frames, &Table::default()), None);
}

#[inline(never)]
fn waits_here(ready: mpsc::Sender<stack::Thread>) -> ! {
    ready.send(stack::current()).unwrap();
    block()
}

/// Another thread's stack: it has frames, and one is in this test binary
/// (the thread blocks in a function of it).
#[test]
fn another_threads_stack_is_captured() {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || waits_here(tx));
    let thread = rx.recv().unwrap();
    std::thread::sleep(Duration::from_millis(50));
    stack::prepare();
    let mut frames = [0usize; 64];
    let n = stack::capture(&thread, &mut frames);
    if !stack::SUPPORTED {
        assert_eq!(n, 0);
        return;
    }
    assert!(n > 1, "{:?}", &frames[..n]);
    let table = Table::build(&current(), &[], &[], &[]);
    let exe = std::env::current_exe().unwrap();
    let exe = exe.file_name().unwrap().to_str().unwrap();
    assert!(
        frames[..n]
            .iter()
            .any(|&f| table.find(f).map(|e| e.file.as_str()) == Some(exe)),
        "no frame in {exe}: {:?}",
        &frames[..n]
    );
    // A second capture works too.
    assert!(stack::capture(&thread, &mut frames) > 1);
}

/// A blocked child whose table marks the test binary as a module: the line
/// names the test binary and that module.
#[test]
fn a_blocked_process_names_the_library() {
    let test = selector(concat!(
        module_path!(),
        "::a_blocked_process_names_the_library"
    ));
    let exe_path = std::env::current_exe().unwrap();
    let exe = file_name(exe_path.to_str().unwrap()).to_string();
    if in_child(test) {
        let log = PathBuf::from(std::env::var_os(LOG).unwrap());
        let table = Table::from_entries(
            current()
                .into_iter()
                .map(|i| Entry {
                    owner: if file_name(&i.path) == exe {
                        module("blocker.so")
                    } else {
                        Owner::Obs
                    },
                    file: file_name(&i.path).to_string(),
                    start: i.start,
                    end: i.end,
                })
                .collect(),
        );
        assert!(arm_with(Duration::from_millis(200), Some(&log), table));
        block();
    }
    // The watchdog appends to the frontend's log, which exists by then.
    let log = log_path("named");
    std::fs::write(&log, "").unwrap();
    let (out, _) = run_child(test, &log);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(EXIT_CODE), "{stderr}");
    assert_eq!(count_lines(&stderr), 1, "{stderr}");
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(stderr.contains(text.as_str()), "{stderr}");
    if stack::SUPPORTED {
        assert!(
            text.contains(&format!("is blocking in {exe} (loaded by blocker.so); ")),
            "{text}"
        );
    } else {
        assert_eq!(text, line(Duration::from_millis(200)));
    }
}
