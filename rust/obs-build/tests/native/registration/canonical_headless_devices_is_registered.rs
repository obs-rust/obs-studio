use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_qsv_update",
    "test_qsv_encode_bounds",
    "test_v4l2_device_identity",
    "test_vcam_format",
    "test_v4l2_input_caps",
    "test_windows_monitor_identity",
];
#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "headless_devices_runtime.rs"]
mod runtime;
fn expected() -> serde_json::Value {
    serde_json::json!({"schema": 1, "targets": {"test_qsv_update": {"sources": ["test/cmocka/test_qsv_update.c"], "libraries": ["OBS::libobs", "OBS::native-test-cmocka"], "requires": {"all": ["OS_LINUX", "HAVE_QSV_TEST_SDK"]}, "includes": ["."], "header_libraries": ["VPL::VPL"], "definitions": ["OBS_QSV_HOST_DEFS=\"plugins/obs-qsv11/bits/linux_defs.h\""], "compile_options_if_supported": ["-Wno-error=switch"], "watch_inputs": ["plugins/obs-qsv11/QSV_Encoder.cpp.hpp", "plugins/obs-qsv11/QSV_Encoder.h", "plugins/obs-qsv11/QSV_Encoder_Internal.cpp.hpp", "plugins/obs-qsv11/QSV_Encoder_Internal.h", "plugins/obs-qsv11/bits/linux_defs.h", "plugins/obs-qsv11/build/obs-qsv11-c.pch.h", "plugins/obs-qsv11/build/obs-qsv11.core+.hpp.cpp", "plugins/obs-qsv11/build/obs-qsv11.linux+.hpp.cpp", "plugins/obs-qsv11/build/obs-qsv11.pch.hpp", "plugins/obs-qsv11/common_directx11.cpp.hpp", "plugins/obs-qsv11/common_directx11.h", "plugins/obs-qsv11/common_utils.cpp.hpp", "plugins/obs-qsv11/common_utils.h", "plugins/obs-qsv11/obs-qsv11.c.h", "plugins/obs-qsv11/qsv-host.h", "plugins/obs-qsv11/qsv-probe.h"]}, "test_qsv_encode_bounds": {"sources": ["test/cmocka/test_qsv_encode_bounds.cpp"], "libraries": ["OBS::libobs", "OBS::native-test-cmocka"], "requires": {"all": ["OS_LINUX", "HAVE_QSV_TEST_SDK"]}, "includes": ["."], "header_libraries": ["VPL::VPL"], "definitions": ["OBS_QSV_HOST_DEFS=\"plugins/obs-qsv11/bits/linux_defs.h\""], "compile_options_if_supported": ["-Wno-error=switch"], "watch_inputs": ["plugins/obs-qsv11/QSV_Encoder.cpp.hpp", "plugins/obs-qsv11/QSV_Encoder.h", "plugins/obs-qsv11/QSV_Encoder_Internal.cpp.hpp", "plugins/obs-qsv11/QSV_Encoder_Internal.h", "plugins/obs-qsv11/bits/linux_defs.h", "plugins/obs-qsv11/build/obs-qsv11-c.pch.h", "plugins/obs-qsv11/build/obs-qsv11.core+.hpp.cpp", "plugins/obs-qsv11/build/obs-qsv11.linux+.hpp.cpp", "plugins/obs-qsv11/build/obs-qsv11.pch.hpp", "plugins/obs-qsv11/common_directx11.cpp.hpp", "plugins/obs-qsv11/common_directx11.h", "plugins/obs-qsv11/common_utils.cpp.hpp", "plugins/obs-qsv11/common_utils.h", "plugins/obs-qsv11/obs-qsv11.c.h", "plugins/obs-qsv11/qsv-host.h", "plugins/obs-qsv11/qsv-probe.h"]}, "test_v4l2_device_identity": {"sources": ["test/cmocka/test_v4l2_device_identity.c", "plugins/linux-v4l2/v4l2-busy.c.h", "plugins/linux-v4l2/v4l2-controls.c.h", "plugins/linux-v4l2/v4l2-decoder.c.h", "plugins/linux-v4l2/v4l2-frame-action.c.h", "plugins/linux-v4l2/v4l2-framerate.c.h", "plugins/linux-v4l2/v4l2-helpers.c.h", "plugins/linux-v4l2/v4l2-input-caps.c.h", "plugins/linux-v4l2/v4l2-input.c.h"], "libraries": ["OBS::libobs", "FFmpeg::avcodec", "FFmpeg::avformat", "FFmpeg::avutil", "OBS::native-test-cmocka"], "requires": {"all": ["OS_LINUX", "HAVE_OBS_V4L2"]}, "includes": ["plugins/linux-v4l2"], "watch_inputs": ["plugins/linux-v4l2/build/linux-v4l2-nix.pch.h"], "header_libraries": ["Libv4l2::Libv4l2"], "definitions": ["HAVE_UDEV=1", "FIXTURE_DIR=\"${build}/test_v4l2_device_identity.d\"", "V4L2_DEV_DIR=\"${build}/test_v4l2_device_identity.d/dev\"", "V4L2_SYSFS_ROOT=\"${build}/test_v4l2_device_identity.d/sys\""]}, "test_vcam_format": {"sources": ["test/cmocka/test_vcam_format.c", "plugins/linux-v4l2/vcam-format.c.h"], "libraries": ["OBS::native-test-cmocka"], "requires": "HAVE_OBS_V4L2", "includes": ["plugins/linux-v4l2"], "watch_inputs": ["plugins/linux-v4l2/build/linux-v4l2-nix.pch.h"], "header_libraries": ["OBS::libobs", "Libv4l2::Libv4l2", "FFmpeg::avutil"]}, "test_v4l2_input_caps": {"sources": ["test/cmocka/test_v4l2_input_caps.c", "plugins/linux-v4l2/v4l2-input-caps.c.h"], "libraries": ["OBS::native-test-cmocka"], "requires": "HAVE_OBS_V4L2", "includes": ["plugins/linux-v4l2"], "watch_inputs": ["plugins/linux-v4l2/build/linux-v4l2-nix.pch.h"], "header_libraries": ["OBS::libobs", "Libv4l2::Libv4l2", "FFmpeg::avutil"]}, "test_windows_monitor_identity": {"sources": ["test/cmocka/test_windows_monitor_identity.c"], "includes": ["."], "watch_inputs": ["plugins/win-capture/monitor-identity.h"]}}, "tests": {"test_qsv_update": {"target": "test_qsv_update", "requires": {"all": ["OS_LINUX", "HAVE_QSV_TEST_SDK"]}, "command": ["${target}"], "timeout_seconds": 60}, "test_qsv_encode_bounds": {"target": "test_qsv_encode_bounds", "requires": {"all": ["OS_LINUX", "HAVE_QSV_TEST_SDK"]}, "command": ["${target}"], "timeout_seconds": 120}, "test_v4l2_device_identity": {"target": "test_v4l2_device_identity", "requires": {"all": ["OS_LINUX", "HAVE_OBS_V4L2"]}, "command": ["${target}"], "timeout_seconds": 60}, "test_vcam_format": {"target": "test_vcam_format", "requires": "HAVE_OBS_V4L2", "command": ["${target}"]}, "test_v4l2_input_caps": {"target": "test_v4l2_input_caps", "requires": "HAVE_OBS_V4L2", "command": ["${target}"]}, "test_windows_monitor_identity": {"target": "test_windows_monitor_identity", "command": ["${target}"]}}})
}
#[test]
fn canonical_headless_devices_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let expected = expected();
    for name in CASES {
        for section in ["targets", "tests"] {
            assert_eq!(
                raw[section][name], expected[section][name],
                "{section}/{name}"
            );
        }
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
        assert!(fragment.contains("test-graph.json"));
    }
}
#[test]
fn canonical_headless_devices_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.targets.retain(|n, _| CASES.contains(&n.as_str()));
    graph.tests.retain(|n, _| CASES.contains(&n.as_str()));
    assert_eq!(graph.targets.len(), 6);
    for os in ["OS_LINUX", "OS_MACOS", "OS_FREEBSD", "OS_WINDOWS"] {
        for owner in [false, true] {
            for sdk in [false, true] {
                let facts = [
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("HAVE_OBS_V4L2".into(), owner),
                    ("HAVE_QSV_TEST_SDK".into(), sdk),
                ]
                .into();
                let selected = graph.select(&facts).unwrap();
                let count = 1
                    + 2 * usize::from(owner)
                    + usize::from(owner && os == "OS_LINUX")
                    + 2 * usize::from(sdk && os == "OS_LINUX");
                assert_eq!(selected.targets.len(), count, "{os}/{owner}/{sdk}");
                assert_eq!(selected.tests.len(), count);
            }
        }
    }
}
