//! The watchdog process (#4589).
//!
//! - `args_*`: [`HelperArgs::read`] on every host.
//! - `windows::*`: the helper on Windows. A child arms it and then blocks
//!   inside `ExitProcess`, in a TLS callback of this test executable on
//!   `DLL_PROCESS_DETACH`: Windows runs the executable's TLS callbacks
//!   there after it has ended every other thread, the watchdog thread too,
//!   which is the state a plugin DLL's static destructor runs in. The
//!   helper is this test executable too, started with
//!   [`helper_process_entry`] as its selector.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use obs_exit_watchdog::HelperArgs;
use obs_exit_watchdog::helper::{
    ENV_ARM, ENV_LOG, ENV_PARENT, ENV_TABLE, ENV_TIMEOUT_MS, decode_arm, encode_arm,
};
use obs_exit_watchdog::images::{Entry, Owner, Table};

fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |key| {
        pairs
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| OsString::from(v))
    }
}

#[test]
fn args_read_every_value() {
    let args = HelperArgs::read(env(&[
        (ENV_PARENT, "1234"),
        (ENV_ARM, "88"),
        (ENV_TIMEOUT_MS, "10000"),
        (ENV_LOG, "C:/obs/logs/a.txt"),
        (ENV_TABLE, "77"),
    ]));
    assert_eq!(
        args,
        Some(HelperArgs {
            parent: 1234,
            arm: 88,
            timeout: Duration::from_secs(10),
            log: Some(PathBuf::from("C:/obs/logs/a.txt")),
            table: Some(77),
        })
    );
}

#[test]
fn args_without_a_log_have_none() {
    let args = HelperArgs::read(env(&[
        (ENV_PARENT, "1"),
        (ENV_ARM, "2"),
        (ENV_TIMEOUT_MS, "0"),
    ]));
    assert_eq!(args.map(|a| a.log), Some(None));
}

#[test]
fn args_need_both_handles_and_the_timeout() {
    let full = [(ENV_PARENT, "1"), (ENV_ARM, "2"), (ENV_TIMEOUT_MS, "5")];
    for missing in 0..full.len() {
        let pairs: Vec<_> = full
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != missing)
            .map(|(_, p)| *p)
            .collect();
        assert_eq!(
            HelperArgs::read(env(&pairs)),
            None,
            "without {}",
            full[missing].0
        );
    }
}

#[test]
fn args_reject_bad_numbers_and_null_handles() {
    for (key, bad) in [
        (ENV_PARENT, "0"),
        (ENV_ARM, "0"),
        (ENV_PARENT, "-1"),
        (ENV_ARM, "x"),
        (ENV_TIMEOUT_MS, ""),
        (ENV_TIMEOUT_MS, "1.5"),
        (ENV_PARENT, "18446744073709551616"),
    ] {
        let mut pairs = vec![(ENV_PARENT, "1"), (ENV_ARM, "2"), (ENV_TIMEOUT_MS, "5")];
        for p in &mut pairs {
            if p.0 == key {
                p.1 = bad;
            }
        }
        assert_eq!(HelperArgs::read(env(&pairs)), None, "{key}={bad:?}");
    }
}

/// The section is optional: without it, or with a bad value, the helper
/// still ends a hang and writes the line without a library.
#[test]
fn args_without_a_good_table_have_none() {
    let base = [(ENV_PARENT, "1"), (ENV_ARM, "2"), (ENV_TIMEOUT_MS, "5")];
    assert_eq!(HelperArgs::read(env(&base)).map(|a| a.table), Some(None));
    for bad in ["", "0", "x", "-3"] {
        let mut pairs = base.to_vec();
        pairs.push((ENV_TABLE, bad));
        assert_eq!(
            HelperArgs::read(env(&pairs)).map(|a| a.table),
            Some(None),
            "{bad:?}"
        );
    }
}

fn entry(start: usize, file: &str, owner: Owner) -> Entry {
    Entry {
        start,
        end: start + 0x1000,
        file: file.to_string(),
        owner,
    }
}

/// What OBS writes at arm reads back the same: the thread and every entry
/// with its owner.
#[test]
fn the_armed_thread_and_the_table_read_back() {
    let table = Table::from_entries(vec![
        entry(0x1000, "obs64.exe", Owner::Obs),
        entry(
            0x5000,
            "libremidi.dll",
            Owner::Module("advanced-scene-switcher.dll".to_string()),
        ),
        entry(
            0x9000,
            "obs-avip.dll",
            Owner::Module("obs-avip.dll".to_string()),
        ),
        entry(0xd000, "pipewire.dll", Owner::PostLoad),
        entry(0x11000, "nss3.dll", Owner::AfterStartup),
        entry(0x15000, "ünïcode.dll", Owner::Obs),
    ]);
    let bytes = encode_arm(4242, &table);
    assert_eq!(decode_arm(&bytes), Some((4242, table)));
    let empty = encode_arm(1, &Table::default());
    assert_eq!(decode_arm(&empty), Some((1, Table::default())));
}

/// A cut or damaged section reads as nothing (the line then names no
/// library), never as a wrong table.
#[test]
fn a_cut_or_damaged_section_reads_as_nothing() {
    let table = Table::from_entries(vec![
        entry(0x1000, "obs64.exe", Owner::Obs),
        entry(0x5000, "a.dll", Owner::Module("a.dll".to_string())),
    ]);
    let bytes = encode_arm(7, &table);
    for len in 0..bytes.len() {
        assert_eq!(decode_arm(&bytes[..len]), None, "cut at {len}");
    }
    assert_eq!(decode_arm(&[0u8; 64]), None, "a section nobody wrote");
    let mut damaged = bytes.clone();
    damaged[0] ^= 0xff;
    assert_eq!(decode_arm(&damaged), None, "a damaged header");
    // Trailing bytes (the rest of a larger section) are ignored.
    let mut longer = bytes.clone();
    longer.extend([0u8; 32]);
    assert_eq!(decode_arm(&longer), Some((7, table)));
}

/// The libtest selector of [`helper_process_entry`].
#[cfg(windows)]
pub(crate) fn entry_selector() -> &'static str {
    super::selector(concat!(module_path!(), "::helper_process_entry"))
}

/// The helper process: this test executable, run with this test as its
/// only selector and the helper's environment. In an ordinary run it does
/// nothing.
#[test]
fn helper_process_entry() {
    if std::env::var_os(ENV_PARENT).is_some() {
        std::process::exit(obs_exit_watchdog::run_helper());
    }
}

#[cfg(not(windows))]
#[test]
fn start_does_nothing_off_windows() {
    assert!(!obs_exit_watchdog::start(Duration::from_secs(1), None));
}

#[cfg(windows)]
mod windows {
    use std::ffi::{OsStr, c_void};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use obs_exit_watchdog::images::{Entry, Owner, Table, current, file_name};
    use obs_exit_watchdog::{EXIT_CODE, arm, arm_with, line, start_with};

    use super::super::{LOG, block, count_lines, in_child, log_path, run_child, selector};

    /// Set in a child that blocks in `ExitProcess`.
    static HANG_ON_DETACH: AtomicBool = AtomicBool::new(false);

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateEventW(
            attributes: *mut c_void,
            manual_reset: i32,
            initial_state: i32,
            name: *const u16,
        ) -> *mut c_void;
        fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
    }

    /// A TLS callback of this executable: on `DLL_PROCESS_DETACH` (0), in
    /// a child that set [`HANG_ON_DETACH`], it waits on an event nobody
    /// sets.
    unsafe extern "system" fn on_tls(_module: *mut c_void, reason: u32, _reserved: *mut c_void) {
        if reason == 0 && HANG_ON_DETACH.load(Ordering::SeqCst) {
            // SAFETY: an unnamed event with default security; the wait on it
            // never returns, which is the point.
            unsafe {
                let event = CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null());
                WaitForSingleObject(event, u32::MAX);
            }
        }
    }

    #[used]
    #[unsafe(link_section = ".CRT$XLB")]
    static TLS_CALLBACK: unsafe extern "system" fn(*mut c_void, u32, *mut c_void) = on_tls;

    const TIMEOUT: Duration = Duration::from_secs(1);

    /// Starts this test executable as the helper of the child.
    fn start_test_helper(log: &std::path::Path) -> bool {
        let exe = std::env::current_exe().unwrap();
        let args = [super::entry_selector(), "--exact", "--test-threads=1", "-q"].map(OsStr::new);
        start_with(&exe, &args, TIMEOUT, Some(log))
    }

    #[test]
    fn the_helper_ends_a_hang_in_exit_process() {
        let test = selector(concat!(
            module_path!(),
            "::the_helper_ends_a_hang_in_exit_process"
        ));
        if in_child(test) {
            let log = PathBuf::from(std::env::var_os(LOG).unwrap());
            // Without a helper, arm() falls back to the thread, which
            // ExitProcess ends before on_tls blocks: the child hangs.
            let _ = start_test_helper(&log);
            assert!(arm(TIMEOUT, Some(&log)));
            HANG_ON_DETACH.store(true, Ordering::SeqCst);
            // The test returns, libtest returns from main, and ExitProcess
            // blocks in on_tls.
            return;
        }
        let log = log_path("helper-hang");
        std::fs::write(&log, "earlier line\n").unwrap();
        let (out, elapsed) = run_child(test, &log);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(EXIT_CODE), "{stderr}");
        assert!(elapsed >= TIMEOUT, "{elapsed:?}");
        let text = std::fs::read_to_string(&log).unwrap();
        assert_eq!(text, format!("earlier line\n{}", line(TIMEOUT)));
    }

    /// The helper names the DLL a hang inside `ExitProcess` waits in, and
    /// the module that loaded it (part 4): the child's table marks this
    /// test executable, where `on_tls` blocks, as loaded by `blocker.dll`.
    /// The child calls `exit` on the thread that armed the helper, as OBS's
    /// main thread does.
    #[test]
    fn the_helpers_line_names_the_dll() {
        let test = selector(concat!(module_path!(), "::the_helpers_line_names_the_dll"));
        let exe_path = std::env::current_exe().unwrap();
        let exe = file_name(exe_path.to_str().unwrap()).to_string();
        if in_child(test) {
            let log = PathBuf::from(std::env::var_os(LOG).unwrap());
            assert!(start_test_helper(&log), "the helper did not start");
            let table = Table::from_entries(
                current()
                    .into_iter()
                    .map(|i| Entry {
                        owner: if file_name(&i.path) == exe {
                            Owner::Module("blocker.dll".to_string())
                        } else {
                            Owner::Obs
                        },
                        file: file_name(&i.path).to_string(),
                        start: i.start,
                        end: i.end,
                    })
                    .collect(),
            );
            assert!(arm_with(TIMEOUT, Some(&log), table));
            HANG_ON_DETACH.store(true, Ordering::SeqCst);
            std::process::exit(0);
        }
        let log = log_path("helper-named");
        std::fs::write(&log, "").unwrap();
        let (out, elapsed) = run_child(test, &log);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(EXIT_CODE), "{stderr}");
        assert!(elapsed >= TIMEOUT, "{elapsed:?}");
        let text = std::fs::read_to_string(&log).unwrap();
        assert_eq!(count_lines(&text), 1, "{text}");
        if cfg!(target_arch = "x86_64") {
            assert!(
                text.contains(&format!("is blocking in {exe} (loaded by blocker.dll); ")),
                "{text}"
            );
        } else {
            // arm64: no stack capture yet, the line names no library.
            assert_eq!(text, line(TIMEOUT));
        }
    }

    #[test]
    fn the_helper_lets_a_normal_exit_go() {
        let test = selector(concat!(
            module_path!(),
            "::the_helper_lets_a_normal_exit_go"
        ));
        if in_child(test) {
            let log = PathBuf::from(std::env::var_os(LOG).unwrap());
            assert!(start_test_helper(&log), "the helper did not start");
            assert!(arm(TIMEOUT, Some(&log)));
            return;
        }
        let log = log_path("helper-normal");
        std::fs::write(&log, "").unwrap();
        let (out, elapsed) = run_child(test, &log);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(0), "{stderr}");
        assert!(elapsed < TIMEOUT, "{elapsed:?}");
        // The helper would have written by now if it had not seen the end.
        std::thread::sleep(TIMEOUT + Duration::from_millis(500));
        let text = std::fs::read_to_string(&log).unwrap();
        assert_eq!(count_lines(&text), 0, "{text}");
    }

    #[test]
    fn without_a_helper_the_thread_still_ends_a_hang_in_exit() {
        let test = selector(concat!(
            module_path!(),
            "::without_a_helper_the_thread_still_ends_a_hang_in_exit"
        ));
        if in_child(test) {
            let log = PathBuf::from(std::env::var_os(LOG).unwrap());
            let missing = log.with_file_name("no-such-helper.exe");
            assert!(!start_with(&missing, &[], TIMEOUT, Some(&log)));
            assert!(arm(Duration::from_millis(200), Some(&log)));
            block();
        }
        let log = log_path("helper-missing");
        std::fs::write(&log, "").unwrap();
        let (out, _) = run_child(test, &log);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(EXIT_CODE), "{stderr}");
        assert_eq!(count_lines(&stderr), 1, "{stderr}");
    }
}
