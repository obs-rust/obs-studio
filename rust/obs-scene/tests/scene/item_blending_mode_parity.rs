//! Exact real-item parity, including invalid enum bits; no intentional differences.
use core::ffi::c_void;
use obs_scene::ffi::item_blending_mode::obs_sceneitem_set_blending_mode;
use proptest::prelude::*;

type Setter = unsafe extern "C" fn(*mut c_void, u32);
unsafe extern "C" {
    fn oracle_scene_item_blending_mode_probe(
        initial: u32,
        value: u32,
        flags: u32,
        setter: Setter,
    ) -> u32;
    fn oracle_scene_item_blending_mode_layout(field: u32) -> usize;
    fn oracle_scene_item_blending_mode_null(setter: Setter) -> u32;
}

#[test]
fn real_header_layout() {
    // No repr(C) struct crosses this opaque boundary. Measure the enum storage
    // addressed by the private adapter; the probe checks its exact field address.
    // SAFETY: these queries return measurements of the real C header.
    unsafe {
        assert_eq!(oracle_scene_item_blending_mode_layout(0), size_of::<u32>());
        assert_eq!(oracle_scene_item_blending_mode_layout(1), align_of::<u32>());
        assert!(oracle_scene_item_blending_mode_layout(2) > size_of::<u32>());
    }
}

fn check(initial: u32, value: u32, flags: u32) {
    // SAFETY: the C probe owns complete live items throughout each callback.
    assert_eq!(
        unsafe {
            oracle_scene_item_blending_mode_probe(
                initial,
                value,
                flags,
                obs_sceneitem_set_blending_mode,
            )
        },
        0,
        "initial={initial:#x}, value={value:#x}, flags={flags:#x}"
    );
}

#[test]
fn characterized_edges() {
    for value in [0, 1, 2, 3, 4, 5, 6, 7, u32::MAX, 0x8000_0000] {
        for flags in 0..32 {
            check(!value, value, flags);
            check(value, value, flags);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_values(initial in any::<u32>(), value in any::<u32>(), flags in any::<u32>()) {
        check(initial, value, flags);
    }
}

#[test]
fn null_diagnostics() {
    const CHILD: &str = "OBS_SCENE_BLEND_SET_LOG_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // SAFETY: isolated one-test child; the probe restores the global logger.
        assert_eq!(
            unsafe { oracle_scene_item_blending_mode_null(obs_sceneitem_set_blending_mode) },
            0
        );
        return;
    }
    let module = module_path!().split_once("::").unwrap().1;
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .env(CHILD, "1")
        .args([
            "--exact",
            &format!("{module}::null_diagnostics"),
            "--nocapture",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}
