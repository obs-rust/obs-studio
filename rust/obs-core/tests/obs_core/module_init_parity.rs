//! Lifecycle traces from the verbatim C unit and production Rust FFI.
use obs_c_oracle as _;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_module_init_run(
        rust: bool,
        present: bool,
        core: bool,
        loaded: bool,
        results: u32,
        count: u32,
        mutations: u32,
        report: *mut u64,
    );
    fn oracle_module_init_layout(layout: *mut usize);
}
fn compare(present: bool, core: bool, loaded: bool, results: u32, count: u32, mutations: u32) {
    let mut c = [0u64; 128];
    let mut rust = [0u64; 128];
    // SAFETY: the oracle owns real-header fixtures for each run, writes exactly
    // 128 u64 values and resets globals. This test runs in its own child process.
    unsafe {
        oracle_module_init_run(
            false,
            present,
            core,
            loaded,
            results,
            count,
            mutations,
            c.as_mut_ptr(),
        );
        oracle_module_init_run(
            true,
            present,
            core,
            loaded,
            results,
            count,
            mutations,
            rust.as_mut_ptr(),
        );
    }
    assert_eq!(
        rust, c,
        "present={present} core={core} loaded={loaded} results={results} count={count} mutations={mutations}"
    );
    assert!(
        !c.contains(&99),
        "invalid loading identity, profile or warning arguments"
    );
}
#[test]
fn original_c_lifecycle_parity_and_real_header_layout() {
    const CHILD: &str = "OBS_MODULE_INIT_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(format!(
                "{}::original_c_lifecycle_parity_and_real_header_layout",
                module_path!().split_once("::").unwrap().1
            ))
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let mut layout = [0usize; 6];
    // SAFETY: C writes six layout values measured from obs-internal.h.
    unsafe {
        oracle_module_init_layout(layout.as_mut_ptr());
    }
    assert_eq!(layout[0], size_of::<bool>());
    assert_eq!(layout[1], align_of::<bool>());
    assert_eq!(layout[2], layout[3]);
    assert_eq!(layout[4], size_of::<unsafe extern "C" fn() -> bool>());
    assert_eq!(layout[5], align_of::<unsafe extern "C" fn() -> bool>());
    for present in [false, true] {
        for core in [false, true] {
            for loaded in [false, true] {
                for results in [0, 1, 2, 0x55, 0xff] {
                    for mutations in 0..4 {
                        compare(present, core, loaded, results, 8, mutations);
                    }
                }
            }
        }
    }
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..Default::default()
    });
    runner
        .run(
            &(
                any::<bool>(),
                any::<bool>(),
                any::<bool>(),
                0u32..256,
                1u32..9,
                0u32..4,
            ),
            |(present, core, loaded, results, count, mutations)| {
                compare(present, core, loaded, results, count, mutations);
                Ok(())
            },
        )
        .unwrap();
}
