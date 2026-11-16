//! Exact C parity including every surrounding item byte; no intentional differences.
use core::ffi::c_void;
use obs_c_oracle as _;
use obs_scene::ffi::item_blending_method::obs_sceneitem_set_blending_method;
use proptest::prelude::*;

type Setter = unsafe extern "C" fn(*mut c_void, u32);
unsafe extern "C" {
    fn oracle_scene_item_blending_method_probe(method: u32, flags: u32, setter: Setter) -> u32;
    fn oracle_scene_item_blending_method_null_probe(setter: Setter) -> u32;
    fn oracle_scene_item_blending_method_size() -> usize;
}

fn check(method: u32, flags: u32) {
    // SAFETY: C owns complete live items and calls the setter synchronously.
    assert_eq!(
        unsafe {
            oracle_scene_item_blending_method_probe(
                method,
                flags,
                obs_sceneitem_set_blending_method,
            )
        },
        0
    );
}

#[test]
fn characterized_values_and_state() {
    // SAFETY: the oracle returns the real C enum size, with no pointer access.
    assert_eq!(
        unsafe { oracle_scene_item_blending_method_size() },
        core::mem::size_of::<u32>()
    );
    for dirty in 0..2 {
        for method in [0, 1, 17, 0x7fff_ffff] {
            check(method, dirty);
        }
    }
}

#[test]
fn null_diagnostic() {
    const CHILD: &str = "OBS_SCENE_ITEM_METHOD_LOG_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // SAFETY: the isolated child saves and restores its global log handler.
        assert_eq!(
            unsafe {
                oracle_scene_item_blending_method_null_probe(obs_sceneitem_set_blending_method)
            },
            0
        );
        return;
    }
    let module = module_path!().split_once("::").unwrap().1;
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .env(CHILD, "1")
        .args([
            "--exact",
            &format!("{module}::null_diagnostic"),
            "--nocapture",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(512), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn nonnegative_methods(method in 0u32..=0x7fff_ffff, flags in 0u32..2) {
        check(method, flags);
    }
}
