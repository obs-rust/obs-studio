//! Tier 3 over the original C obs_canvas_release and the original weak
//! release, with a traced destroy seam. No intentional differences. Excludes
//! signed overflow and release without strong ownership. The core pointer,
//! log handler and allocation probes are global, so this re-execs in isolation.
use core::ffi::{c_long, c_void};
use obs_core::ffi::canvas_release::obs_canvas_release;
use proptest::test_runner::{Config, TestRunner};

unsafe extern "C" {
    fn oracle_canvas_release_probe(
        flags: u32,
        releases: u32,
        refs: c_long,
        weak: c_long,
        release: unsafe extern "C" fn(*mut c_void),
    ) -> bool;
    fn oracle_canvas_release_final_free(release: unsafe extern "C" fn(*mut c_void)) -> bool;
}

fn check(flags: u32, releases: u32, refs: c_long, weak: c_long) {
    // SAFETY: C owns the real-header fixtures. Callers keep weak >= 1, so the
    // stack control outlives a final release, and never release past
    // ownership (releases <= refs + 1) while the core is live.
    assert!(
        unsafe { oracle_canvas_release_probe(flags, releases, refs, weak, obs_canvas_release) },
        "flags={flags} releases={releases} refs={refs} weak={weak}"
    );
}

#[test]
fn characterized_edges_and_random_sequences_match_c() {
    const CHILD: &str = "OBS_CANVAS_RELEASE_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let name = format!(
            "{}::characterized_edges_and_random_sequences_match_c",
            module_path!().split_once("::").unwrap().1
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    // Shutdown: NULL is silent, a live or invalid handle only warns, whatever
    // its counts say (even expired or about to be destroyed).
    for refs in [c_long::MIN, -3, -1, 0, 1, 1023, c_long::MAX] {
        for flags in [2, 3, 6] {
            for releases in [1, 2] {
                check(flags, releases, refs, 1);
            }
        }
    }
    // Live core: NULL is a no-op; owned handles decrement, and the release
    // that reaches -1 destroys once and then drops the canvas's weak owner.
    for refs in [0, 1, 2, 7, 1023, c_long::MAX - 2] {
        for weak in [1, 2, 9] {
            check(1, 3, refs, weak);
            for releases in 1..=3u32 {
                if i128::from(releases) <= i128::from(refs) + 1 {
                    check(0, releases, refs, weak);
                }
            }
        }
    }
    // SAFETY: the C fixture hands one bzalloc control to the final release
    // and checks destroy and the allocation balance without reading freed data.
    assert!(unsafe { oracle_canvas_release_final_free(obs_canvas_release) });

    let strong: std::ops::Range<c_long> = 0..4096;
    let weak: std::ops::Range<c_long> = 1..16;
    let strategy = (0u32..4, 1u32..5, strong, weak);
    TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Config::default()
    })
    .run(&strategy, |(flags, releases, refs, weak)| {
        let releases = if flags & 2 == 0 {
            releases.min(u32::try_from(refs + 1).unwrap_or(u32::MAX))
        } else {
            releases
        };
        check(flags, releases, refs, weak);
        Ok(())
    })
    .unwrap();
}
