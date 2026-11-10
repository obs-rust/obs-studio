//! Exact original-C parity; no intentional differences.
use obs_c_oracle as _;
use obs_scene::ffi::item_render_getters::SceneItemRenderPrefix;
use obs_scene::ffi::item_scale_filter::obs_sceneitem_set_scale_filter;
use proptest::prelude::*;

type Setter = unsafe extern "C" fn(*mut SceneItemRenderPrefix, u32);
unsafe extern "C" {
    fn oracle_scene_scale_filter_probe(
        initial: u32,
        filter: u32,
        dirty: bool,
        setter: Setter,
    ) -> u32;
    fn oracle_scene_scale_filter_null_probe(setter: Setter) -> u32;
}

fn check(initial: u32, filter: u32, dirty: bool) {
    // SAFETY: oracle owns complete live items and synchronously calls the setter.
    assert_eq!(
        unsafe {
            oracle_scene_scale_filter_probe(initial, filter, dirty, obs_sceneitem_set_scale_filter)
        },
        0,
        "initial={initial} filter={filter} dirty={dirty}"
    );
}

#[test]
fn characterized_edges() {
    for initial in [0, 1, 2, 3, 4, 5, 6, 17, 0x7fff_ffff, 0x8000_0000, u32::MAX] {
        for filter in [0, 1, 2, 3, 4, 5, 6, 17, 0x7fff_ffff, 0x8000_0000, u32::MAX] {
            for dirty in [false, true] {
                check(initial, filter, dirty);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn arbitrary_values(initial in any::<u32>(), filter in any::<u32>(), dirty in any::<bool>()) {
        check(initial, filter, dirty);
    }
}

#[test]
fn null_diagnostic() {
    const CHILD: &str = "OBS_SCENE_SCALE_FILTER_LOG_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // SAFETY: one-test child owns global logging; oracle saves/restores it.
        assert_eq!(
            unsafe { oracle_scene_scale_filter_null_probe(obs_sceneitem_set_scale_filter) },
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
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}
