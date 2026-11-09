//! Tier 3: original extracted C, real canvas layout and actual callbacks.
//! No excluded inputs or intentional differences. A child isolates global fixtures.
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

unsafe extern "C" {
    fn oracle_canvas_signals_run(
        rust: bool,
        private: bool,
        global: bool,
        local: bool,
        source_mode: bool,
        mutate: bool,
        source: usize,
        trace: *mut usize,
    );
}
fn compare(
    private: bool,
    global: bool,
    local: bool,
    source_mode: bool,
    mutate: bool,
    source: usize,
) {
    let mut c = [0; 17];
    let mut rust = [0; 17];
    // SAFETY: the fixture owns every live canvas/handler, borrows source only
    // as an opaque token, fills the 17-word buffers, and destroys every handler.
    unsafe {
        oracle_canvas_signals_run(
            false,
            private,
            global,
            local,
            source_mode,
            mutate,
            source,
            c.as_mut_ptr(),
        );
        oracle_canvas_signals_run(
            true,
            private,
            global,
            local,
            source_mode,
            mutate,
            source,
            rust.as_mut_ptr(),
        );
    }
    assert_eq!(
        rust, c,
        "private={private} global={global} local={local} source_mode={source_mode} mutate={mutate} source={source}"
    );
    let count = if source_mode {
        1
    } else {
        usize::from(global && !private) + usize::from(local)
    };
    assert_eq!(rust[0], count);
    for row in rust[1..1 + count * 8].as_chunks::<8>().0 {
        assert_eq!(row[1], 1, "canvas identity");
        assert_eq!(row[4], 128, "fixed stack capacity");
        assert_eq!(row[5], 1, "fixed storage flag");
    }
}
#[test]
fn original_helpers_match_rust() {
    const CHILD: &str = "OBS_CANVAS_SIGNALS_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let selector = format!(
            "{}::original_helpers_match_rust",
            module_path!().split_once("::").unwrap().1
        );
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &selector, "--nocapture"])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    for private in [false, true] {
        for global in [false, true] {
            for local in [false, true] {
                for source_mode in [false, true] {
                    for mutate in [false, true] {
                        for source in [0, 1, usize::MAX] {
                            compare(private, global, local, source_mode, mutate, source);
                        }
                    }
                }
            }
        }
    }
    let mut runner = TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(
            &(
                any::<bool>(),
                any::<bool>(),
                any::<bool>(),
                any::<bool>(),
                any::<bool>(),
                any::<usize>(),
            ),
            |(private, global, local, source_mode, mutate, source)| {
                compare(private, global, local, source_mode, mutate, source);
                Ok(())
            },
        )
        .unwrap();
}
