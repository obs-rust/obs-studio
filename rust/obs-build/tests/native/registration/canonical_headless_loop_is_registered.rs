use super::*;
use obs_build::validation::graph::Graph;
const CASES: [&str; 6] = [
    "test_python_stdio",
    "test_pipewire_frame_pacing",
    "test_pipewire_connect_start",
    "test_pipewire_teardown_wait",
    "test_xshm_capture_pipeline",
    "test_mac_vmic_shm",
];
#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "support/headless_loop_runtime.rs"]
mod runtime;
fn expected() -> serde_json::Value {
    serde_json::json!({"schema": 1, "targets": {"test_python_stdio": {"sources": ["test/cmocka/test_python_stdio.c"], "libraries": ["OBS::libobs", "Python::Python", "OBS::native-test-cmocka"], "includes": ["shared/obs-scripting"], "requires": {"all": [{"any": ["OS_LINUX", "OS_FREEBSD", "OS_OPENBSD"]}, "HAVE_PYTHON_EMBED_TEST_SDK"]}, "watch_inputs": ["shared/obs-scripting/obs-scripting-python-startup.h"]}, "test_pipewire_frame_pacing": {"sources": ["test/cmocka/test_pipewire_frame_pacing.c", "plugins/linux-pipewire/frame-pacing.c.h"], "libraries": ["OBS::libobs", "OBS::native-test-cmocka"], "includes": [".", "plugins/linux-pipewire"], "requires": "HAVE_PIPEWIRE_HEADLESS_SDK", "header_libraries": ["Libdrm::Libdrm", "PipeWire::PipeWire", "gio::gio", "OBS::glad"], "watch_inputs": ["plugins/linux-pipewire/build/linux-pipewire-nix.pch.h"]}, "test_pipewire_connect_start": {"sources": ["test/cmocka/test_pipewire_connect_start.c", "plugins/linux-pipewire/formats.c.h", "plugins/linux-pipewire/frame-layout.c.h", "plugins/linux-pipewire/frame-pacing.c.h", "plugins/linux-pipewire/reconnect-policy.c.h"], "libraries": ["OBS::libobs", "OBS::glad", "PipeWire::PipeWire", "gio::gio", "OBS::native-test-glib", "OBS::native-test-cmocka"], "includes": [".", "plugins/linux-pipewire"], "requires": {"all": ["OS_LINUX", "HAVE_PIPEWIRE_HEADLESS_SDK"]}, "header_libraries": ["Libdrm::Libdrm"], "watch_inputs": ["plugins/linux-pipewire/build/linux-pipewire-nix.pch.h", "plugins/linux-pipewire/pipewire.c.h"], "link_options": ["-Wl,--wrap=pw_thread_loop_lock", "-Wl,--wrap=pw_thread_loop_unlock", "-Wl,--wrap=pw_thread_loop_start", "-Wl,--wrap=pw_thread_loop_stop", "-Wl,--wrap=pw_thread_loop_wait", "-Wl,--wrap=pw_context_connect_fd"]}, "test_pipewire_teardown_wait": {"sources": ["test/cmocka/test_pipewire_teardown_wait.c", "plugins/linux-pipewire/formats.c.h", "plugins/linux-pipewire/frame-layout.c.h", "plugins/linux-pipewire/frame-pacing.c.h", "plugins/linux-pipewire/reconnect-policy.c.h"], "libraries": ["OBS::libobs", "OBS::glad", "PipeWire::PipeWire", "gio::gio", "OBS::native-test-glib", "OBS::native-test-cmocka"], "includes": [".", "plugins/linux-pipewire"], "requires": {"all": ["OS_LINUX", "HAVE_PIPEWIRE_HEADLESS_SDK", "HAVE_PIPEWIRE_LOOP_TEST_SDK"]}, "header_libraries": ["Libdrm::Libdrm"], "watch_inputs": ["plugins/linux-pipewire/build/linux-pipewire-nix.pch.h", "plugins/linux-pipewire/pipewire.c.h"], "link_options": ["-Wl,--wrap=pw_thread_loop_lock", "-Wl,--wrap=pw_thread_loop_unlock", "-Wl,--wrap=pw_thread_loop_wait"]}, "test_xshm_capture_pipeline": {"sources": ["test/cmocka/test_xshm_capture_pipeline.c", "plugins/linux-capture/xcursor-xcb.c.h", "plugins/linux-capture/xhelpers.c.h"], "libraries": ["OBS::libobs", "OBS::glad", "X11::X11", "X11::XCB", "XCB::XCB", "XCB::XFIXES", "XCB::RANDR", "XCB::SHM", "XCB::XINERAMA", "XCB::COMPOSITE", "OBS::native-test-cmocka"], "includes": [".", "plugins/linux-capture"], "requires": {"all": ["OS_LINUX", "HAVE_LINUX_CAPTURE"]}, "watch_inputs": ["plugins/linux-capture/build/linux-capture-nix.pch.h", "plugins/linux-capture/xshm-input.c.h"]}, "test_mac_vmic_shm": {"sources": ["test/cmocka/test_mac_vmic_shm.c"], "libraries": ["OBS::native-test-cmocka"], "includes": ["plugins/mac-virtualcam/src/common"], "compile_options": [{"name": "/std:c11", "requires": "MSVC"}, {"name": "/experimental:c11atomics", "requires": "MSVC"}], "watch_inputs": ["plugins/mac-virtualcam/src/common/vmic-shm.h"]}}, "tests": {"test_python_stdio": {"target": "test_python_stdio"}, "test_pipewire_frame_pacing": {"target": "test_pipewire_frame_pacing"}, "test_pipewire_connect_start": {"target": "test_pipewire_connect_start", "timeout_seconds": 15}, "test_pipewire_teardown_wait": {"target": "test_pipewire_teardown_wait"}, "test_xshm_capture_pipeline": {"target": "test_xshm_capture_pipeline", "timeout_seconds": 10}, "test_mac_vmic_shm": {"target": "test_mac_vmic_shm"}}})
}
#[test]
fn canonical_headless_loop_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value = obs_build::validation::graph::Graph::read_value(
        &root.join("test/cmocka/build/test-graph.d"),
    )
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
        if name == "test_xshm_capture_pipeline" {
            assert!(!fragment.contains("NAME test_xshm_capture_pipeline"));
            assert!(!fragment.contains("test_xshm_capture_pipeline.c"));
        } else {
            assert!(!fragment.contains("add_executable("));
            assert!(!fragment.contains("add_test("));
        }
        assert!(fragment.contains("test-graph.d"));
    }
}
#[test]
fn canonical_headless_loop_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    graph.targets.retain(|n, _| CASES.contains(&n.as_str()));
    graph.tests.retain(|n, _| CASES.contains(&n.as_str()));
    assert_eq!(graph.targets.len(), 6);
    for os in [
        "OS_LINUX",
        "OS_FREEBSD",
        "OS_OPENBSD",
        "OS_WINDOWS",
        "OS_MACOS",
    ] {
        for sdk in [false, true] {
            for loop_sdk in [false, true] {
                let facts = [
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("OS_FREEBSD".into(), os == "OS_FREEBSD"),
                    ("OS_OPENBSD".into(), os == "OS_OPENBSD"),
                    ("MSVC".into(), os == "OS_WINDOWS"),
                    ("HAVE_LINUX_CAPTURE".into(), sdk),
                    ("HAVE_PIPEWIRE_HEADLESS_SDK".into(), sdk),
                    ("HAVE_PIPEWIRE_LOOP_TEST_SDK".into(), loop_sdk),
                    ("HAVE_PYTHON_EMBED_TEST_SDK".into(), sdk),
                ]
                .into();
                let selected = graph.select(&facts).unwrap();
                let count = 1
                    + usize::from(sdk)
                    + 2 * usize::from(sdk && os == "OS_LINUX")
                    + usize::from(sdk && loop_sdk && os == "OS_LINUX")
                    + usize::from(sdk && matches!(os, "OS_LINUX" | "OS_FREEBSD" | "OS_OPENBSD"));
                assert_eq!(selected.targets.len(), count, "{os}/{sdk}");
                assert_eq!(selected.tests.len(), count);
            }
        }
    }
}
