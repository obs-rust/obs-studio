use super::*;
use obs_build::validation::graph::Graph;

const CASES: [&str; 6] = [
    "test_plugin_getter_scroll",
    "test_plugin_getter_slideshow",
    "test_plugin_getter_slideshow_mk2",
    "test_plugin_getter_stinger",
    "test_plugin_getter_xshm",
    "test_obs_outputs_rtmp_host",
];

#[cfg(all(feature = "libobs-integration", target_os = "linux"))]
#[path = "plugin_getters_runtime.rs"]
mod runtime;

fn expected() -> serde_json::Value {
    serde_json::json!({"schema": 1, "targets": {"test_plugin_getter_scroll": {"sources": ["test/cmocka/test_plugin_getter_races.c"], "requires": {"all": ["OS_LINUX", "HAVE_PLUGIN_GETTER_OWNERS"]}, "definitions": ["GETTER_SCROLL", "_GNU_SOURCE"], "compile_options": ["-ffunction-sections", "-fdata-sections", "-UNDEBUG"], "link_options": ["-Wl,--gc-sections"], "includes": [".", "plugins/image-source", "plugins/obs-transitions", "plugins/linux-capture"], "libraries": ["OBS::libobs", "m", "OBS::glad", "X11::X11", "X11::XCB", "XCB::XCB", "XCB::XFIXES", "XCB::RANDR", "XCB::SHM", "XCB::XINERAMA", "XCB::COMPOSITE"], "watch_inputs": ["plugins/obs-filters/scroll-filter.c.h", "plugins/obs-filters/build/obs-filters.pch.h"]}, "test_plugin_getter_slideshow": {"sources": ["test/cmocka/test_plugin_getter_races.c", "plugins/image-source/build/image-source.core+.h.c", "plugins/image-source/build/image-source.posix+.h.c"], "requires": {"all": ["OS_LINUX", "HAVE_PLUGIN_GETTER_OWNERS"]}, "definitions": ["GETTER_SLIDESHOW", "_GNU_SOURCE"], "compile_options": ["-ffunction-sections", "-fdata-sections", "-UNDEBUG"], "link_options": ["-Wl,--gc-sections"], "includes": [".", "plugins/image-source", "plugins/obs-transitions", "plugins/linux-capture"], "libraries": ["OBS::libobs", "m", "OBS::glad", "X11::X11", "X11::XCB", "XCB::XCB", "XCB::XFIXES", "XCB::RANDR", "XCB::SHM", "XCB::XINERAMA", "XCB::COMPOSITE"], "watch_inputs": ["plugins/image-source/obs-slideshow.c.h", "plugins/image-source/build/image-source-slideshow.pch.h", "plugins/image-source/build/image-source.pch.h", "plugins/image-source/build/image-source-posix.pch.h"]}, "test_plugin_getter_slideshow_mk2": {"sources": ["test/cmocka/test_plugin_getter_races.c", "plugins/image-source/build/image-source.slideshow+.h.c", "plugins/image-source/build/image-source.posix+.h.c"], "requires": {"all": ["OS_LINUX", "HAVE_PLUGIN_GETTER_OWNERS"]}, "definitions": ["GETTER_SLIDESHOW_MK2", "_GNU_SOURCE"], "compile_options": ["-ffunction-sections", "-fdata-sections", "-UNDEBUG"], "link_options": ["-Wl,--gc-sections"], "includes": [".", "plugins/image-source", "plugins/obs-transitions", "plugins/linux-capture"], "libraries": ["OBS::libobs", "m", "OBS::glad", "X11::X11", "X11::XCB", "XCB::XCB", "XCB::XFIXES", "XCB::RANDR", "XCB::SHM", "XCB::XINERAMA", "XCB::COMPOSITE"], "watch_inputs": ["plugins/image-source/build/image-source.core+.h.c", "plugins/image-source/obs-slideshow-mk2.c.h", "plugins/image-source/build/image-source-slideshow.pch.h", "plugins/image-source/build/image-source.pch.h", "plugins/image-source/build/image-source-posix.pch.h"]}, "test_plugin_getter_stinger": {"sources": ["test/cmocka/test_plugin_getter_races.c"], "requires": {"all": ["OS_LINUX", "HAVE_PLUGIN_GETTER_OWNERS"]}, "definitions": ["GETTER_STINGER", "_GNU_SOURCE"], "compile_options": ["-ffunction-sections", "-fdata-sections", "-UNDEBUG"], "link_options": ["-Wl,--gc-sections"], "includes": [".", "plugins/image-source", "plugins/obs-transitions", "plugins/linux-capture"], "libraries": ["OBS::libobs", "m", "OBS::glad", "X11::X11", "X11::XCB", "XCB::XCB", "XCB::XFIXES", "XCB::RANDR", "XCB::SHM", "XCB::XINERAMA", "XCB::COMPOSITE"], "watch_inputs": ["plugins/obs-transitions/transition-stinger.c.h", "plugins/obs-transitions/build/obs-transitions.pch.h"]}, "test_plugin_getter_xshm": {"sources": ["test/cmocka/test_plugin_getter_races.c"], "requires": {"all": ["OS_LINUX", "HAVE_PLUGIN_GETTER_OWNERS"]}, "definitions": ["GETTER_XSHM", "_GNU_SOURCE"], "compile_options": ["-ffunction-sections", "-fdata-sections", "-UNDEBUG"], "link_options": ["-Wl,--gc-sections"], "includes": [".", "plugins/image-source", "plugins/obs-transitions", "plugins/linux-capture"], "libraries": ["OBS::libobs", "m", "OBS::glad", "X11::X11", "X11::XCB", "XCB::XCB", "XCB::XFIXES", "XCB::RANDR", "XCB::SHM", "XCB::XINERAMA", "XCB::COMPOSITE"], "watch_inputs": ["plugins/linux-capture/build/linux-capture.nix+.h.c", "plugins/linux-capture/build/linux-capture-nix.pch.h", "plugins/linux-capture/xshm-input.c.h"]}, "test_obs_outputs_rtmp_host": {"sources": ["test/cmocka/test_obs_outputs_rtmp_host.c"], "requires": {"not": "OS_WINDOWS"}, "includes": ["."], "libraries": ["OBS::libobs", "OBS::native-test-cmocka", {"name": "MbedTLS::mbedtls", "requires": "HAVE_MBEDTLS_TEST_SDK"}], "compile_options": [{"name": "-DTEST_HAVE_MBEDTLS", "requires": "HAVE_MBEDTLS_TEST_SDK"}], "watch_inputs": ["plugins/obs-outputs/rtmp-stream-host-posix.c.h", "plugins/obs-outputs/rtmp-send-result.h", "plugins/obs-outputs/build/obs-outputs-rtmp-host-posix.pch.h"]}}, "tests": {"test_plugin_getter_scroll": {"target": "test_plugin_getter_scroll", "command": ["${target}"], "timeout_seconds": 30}, "test_plugin_getter_slideshow": {"target": "test_plugin_getter_slideshow", "command": ["${target}"], "timeout_seconds": 30}, "test_plugin_getter_slideshow_mk2": {"target": "test_plugin_getter_slideshow_mk2", "command": ["${target}"], "timeout_seconds": 30}, "test_plugin_getter_stinger": {"target": "test_plugin_getter_stinger", "command": ["${target}"], "timeout_seconds": 30}, "test_plugin_getter_xshm": {"target": "test_plugin_getter_xshm", "command": ["${target}"], "timeout_seconds": 30}, "test_obs_outputs_rtmp_host": {"target": "test_obs_outputs_rtmp_host", "command": ["${target}"]}}})
}

/// Refs #1390: preserve six unchanged plugin getter/RTMP host tests and their original contracts.
#[test]
fn canonical_plugin_getters_is_registered() {
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
        let fragment = fs::read_to_string(root.join(format!(
            "test/cmocka/tests.d/{}.cmake",
            if name.starts_with("test_plugin_getter_") {
                "test_plugin_getter_races"
            } else {
                name
            }
        )))
        .unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
        assert!(fragment.contains("test-graph.json"));
    }
}

#[test]
fn canonical_plugin_getters_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph
        .targets
        .retain(|name, _| CASES.contains(&name.as_str()));
    graph.tests.retain(|name, _| CASES.contains(&name.as_str()));
    assert_eq!(graph.targets.len(), CASES.len());
    for os in ["OS_LINUX", "OS_MACOS", "OS_FREEBSD", "OS_WINDOWS"] {
        for owner in [false, true] {
            for tls in [false, true] {
                let facts = [
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                    ("HAVE_PLUGIN_GETTER_OWNERS".into(), owner),
                    ("HAVE_MBEDTLS_TEST_SDK".into(), tls),
                ]
                .into();
                let selected = graph.select(&facts).unwrap();
                let expected =
                    usize::from(owner && os == "OS_LINUX") * 5 + usize::from(os != "OS_WINDOWS");
                assert_eq!(selected.targets.len(), expected, "{os}/{owner}/{tls}");
                assert_eq!(selected.tests.len(), expected, "{os}/{owner}/{tls}");
            }
        }
    }
}
