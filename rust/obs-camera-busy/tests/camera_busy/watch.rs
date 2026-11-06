//! The inotify watcher (#3724): a real child process opens a file, and the
//! watcher reports it as the holder after the settle time. One watcher
//! thread and one child process per test.

#![cfg(target_os = "linux")]

use std::fs::File;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use obs_camera_busy::watch_holders;

fn node(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "obs-camera-busy-watch-{name}-{}",
        std::process::id()
    ));
    std::fs::write(&p, b"").unwrap();
    std::fs::canonicalize(p).unwrap()
}

#[test]
fn reports_who_opened_the_device() {
    let dev = node("open");
    let (tx, rx) = mpsc::channel();
    let watcher = watch_holders(
        &dev,
        Path::new("/proc"),
        std::process::id(),
        obs_camera_busy::current_uid(),
        50,
        Box::new(move |holders| {
            let _ = tx.send(holders);
        }),
    )
    .unwrap();

    // The child opens the "device" itself, as its stdin, and keeps it open.
    let mut child = Command::new("sh")
        .args(["-c", "exec sleep 30 < \"$0\""])
        .arg(&dev)
        .stdin(Stdio::null())
        .spawn()
        .unwrap();

    let mut found = None;
    while let Ok(holders) = rx.recv_timeout(Duration::from_secs(5)) {
        if let Some(h) = holders.into_iter().find(|h| h.pid == child.id()) {
            found = Some(h);
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    drop(watcher);
    let _ = std::fs::remove_file(&dev);

    let h = found.expect("the watcher reported the child");
    assert_eq!(h.name, "sleep");
    assert!(h.can_close);
}

#[test]
fn an_open_by_obs_itself_reports_nobody() {
    let dev = node("self");
    let (tx, rx) = mpsc::channel();
    let watcher = watch_holders(
        &dev,
        Path::new("/proc"),
        std::process::id(),
        None,
        20,
        Box::new(move |holders| {
            let _ = tx.send(holders);
        }),
    )
    .unwrap();
    let f = File::open(&dev).unwrap();
    let holders = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(f);
    drop(watcher);
    let _ = std::fs::remove_file(&dev);
    assert!(holders.is_empty());
}

#[test]
fn a_missing_device_cannot_be_watched() {
    let missing = std::env::temp_dir().join("obs-camera-busy-no-such-device");
    assert!(watch_holders(&missing, Path::new("/proc"), 1, None, 10, Box::new(|_| {})).is_err());
}
