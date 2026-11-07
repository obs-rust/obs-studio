//! Build-system integration contracts; these compile and load actual C artifacts.
use obs_build::{NativeBuild, SharedLibrary};
use std::{fs, path::Path};

/// #1390: the portable virtual-microphone helpers need both native consumers.
#[test]
fn canonical_vmic_helpers_are_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let names = ["test_mac_vmic_clock", "test_mac_vmic_install"];
    for name in names {
        let target = graph.targets.get(name)
            .unwrap_or_else(|| panic!("canonical graph omits {name}"));
        assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
            if path == &format!("test/cmocka/{name}.c")));
        assert!(matches!(target.libraries.as_slice(), [ConditionalValue::Name(library)]
            if library == "OBS::native-test-cmocka"));
        assert_eq!(target.includes, ["plugins/mac-virtualcam/src/common"]);
        assert_eq!(target.watch_inputs, ["plugins/mac-virtualcam/src/common"]);
        assert!(target.header_libraries.is_empty());
        let test = &graph.tests[name];
        assert_eq!(test.target, name);
        assert_eq!(test.command, ["${target}"]);
        assert_eq!(test.suite, "core");
        assert_eq!(test.timeout_seconds, None);
        assert_eq!(test.skip_return_code, None);
        assert!(test.optional_prefixes.is_empty());
        assert_eq!(test.qt_runtime, None);
        let fragment = fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
    graph.targets.retain(|key, _| names.contains(&key.as_str()));
    graph.tests.retain(|key, _| names.contains(&key.as_str()));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        for name in names {
            assert!(selected.targets.contains_key(name), "{os}/{name}");
            assert!(selected.tests.contains_key(name), "{os}/{name}");
        }
    }
}

/// #1390: standalone caption compatibility must reach both native consumers.
#[test]
fn canonical_caption_unmappable_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let name = "test_caption_unmappable";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits test_caption_unmappable");
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_caption_unmappable.c"));
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("caption linkage must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::caption", "OBS::native-test-cmocka"]);
    assert_eq!(target.includes, ["deps/libcaption"]);
    assert!(target.header_libraries.is_empty());
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.suite, "core");
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    assert_eq!(test.qt_runtime, None);
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_caption_unmappable.cmake")).unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(selected.targets.contains_key(name), "{os}");
        assert!(selected.tests.contains_key(name), "{os}");
    }
}

/// #3716: both native consumers must retain video-output compatibility coverage.
#[test]
fn canonical_video_output_compatibility_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let cases = [
        ("test_video_output_formats", 180),
        ("test_video_reset_fps", 60),
    ];
    for (name, timeout) in cases {
        let target = graph.targets.get(name).unwrap_or_else(|| {
            panic!("canonical graph omits video-output compatibility coverage: {name}")
        });
        assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
            if path == &format!("test/cmocka/{name}.c")));
        let libraries: Vec<_> = target
            .libraries
            .iter()
            .map(|library| match library {
                ConditionalValue::Name(name) => name.as_str(),
                _ => panic!("video-output linkage must be unconditional"),
            })
            .collect();
        assert_eq!(
            libraries,
            ["OBS::libobs", "OBS::native-test-cmocka"],
            "{name}"
        );
        assert!(target.header_libraries.is_empty());
        assert_eq!(
            target.artifact_definitions["GRAPHICS_MODULE_PATH"],
            "OBS::libobs-opengl"
        );
        assert_eq!(target.watch_inputs, ["libobs/data"]);
        let test = &graph.tests[name];
        assert_eq!(test.target, name);
        assert_eq!(test.command, ["${target}"]);
        assert_eq!(test.suite, "graphics");
        assert_eq!(test.working_directory.as_deref(), Some("${runtime}"));
        assert_eq!(test.timeout_seconds, Some(timeout));
        assert_eq!(test.skip_return_code, None);
        assert_eq!(
            test.optional_prefixes,
            [vec!["xvfb-run".to_string(), "-a".to_string()]]
        );
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("), "{name}");
        assert!(!fragment.contains("add_test("), "{name}");
    }
    graph
        .targets
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    graph
        .tests
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for opengl in [true, false] {
            let selected = graph
                .select(
                    &[
                        ("OS_LINUX".into(), os == "OS_LINUX"),
                        ("OS_MACOS".into(), os == "OS_MACOS"),
                        ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                        ("HAVE_OBS_OPENGL".into(), opengl),
                    ]
                    .into(),
                )
                .unwrap();
            for (name, _) in cases {
                assert_eq!(
                    selected.targets.contains_key(name),
                    os == "OS_LINUX" && opengl,
                    "{os}/{opengl}/{name}"
                );
                assert_eq!(
                    selected.tests.contains_key(name),
                    os == "OS_LINUX" && opengl,
                    "{os}/{opengl}/{name}"
                );
            }
        }
    }
}

/// #3720: async cache overflow clocks must reach both native consumers.
#[test]
fn canonical_async_overflow_clock_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_async_overflow_clock";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits async overflow clock coverage");
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_async_overflow_clock.c"));
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("async overflow linkage must be unconditional"),
        })
        .collect();
    assert_eq!(
        libraries,
        ["OBS::libobs", "OBS::caption", "OBS::native-test-cmocka"]
    );
    assert!(target.header_libraries.is_empty());
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.suite, "core");
    assert_eq!(test.working_directory, None);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_async_overflow_clock.cmake"))
            .unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(
                &[
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("OS_MACOS".into(), os == "OS_MACOS"),
                    ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                ]
                .into(),
            )
            .unwrap();
        assert_eq!(
            selected.targets.contains_key(name),
            os == "OS_LINUX",
            "{os}"
        );
        assert_eq!(selected.tests.contains_key(name), os == "OS_LINUX", "{os}");
    }
}

/// #3718: fake-device graphics C suites must reach both native consumers.
#[test]
fn canonical_fake_device_graphics_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let cases = [
        ("test_texture_render", None),
        ("test_graphics_immediate_render", Some("address,undefined")),
    ];
    for (name, sanitizers) in cases {
        let target = graph.targets.get(name).unwrap_or_else(|| {
            panic!("canonical graph omits fake-device graphics coverage: {name}")
        });
        assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
            if path == &format!("test/cmocka/{name}.c")));
        let libraries: Vec<_> = target
            .libraries
            .iter()
            .map(|library| match library {
                ConditionalValue::Name(name) => name.as_str(),
                _ => panic!("fake-device graphics linkage must be unconditional"),
            })
            .collect();
        assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
        assert_eq!(target.sanitizers.as_deref(), sanitizers);
        assert!(target.artifact_definitions.is_empty());
        let test = &graph.tests[name];
        assert_eq!(test.target, name);
        assert_eq!(test.command, ["${target}"]);
        assert_eq!(test.suite, "core");
        assert_eq!(test.working_directory, None);
        assert_eq!(test.timeout_seconds, None);
        assert_eq!(test.skip_return_code, None);
        assert!(test.optional_prefixes.is_empty());
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("), "{name}");
        assert!(!fragment.contains("add_test("), "{name}");
    }
    graph
        .targets
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    graph
        .tests
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        for (name, _) in cases {
            assert!(selected.targets.contains_key(name), "{os}/{name}");
            assert!(selected.tests.contains_key(name), "{os}/{name}");
        }
    }
}

/// #3714: both native consumers must retain transition compatibility coverage.
#[test]
fn canonical_transition_compatibility_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let cases = [
        ("test_transition_recursive_destination", 180),
        ("test_transition_swap", 60),
        ("test_item_transition_empty_source", 120),
    ];
    for (name, timeout) in cases {
        let target = graph.targets.get(name).unwrap_or_else(|| {
            panic!("canonical graph omits transition compatibility coverage: {name}")
        });
        assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
            if path == &format!("test/cmocka/{name}.c")));
        let libraries: Vec<_> = target
            .libraries
            .iter()
            .map(|library| match library {
                ConditionalValue::Name(name) => name.as_str(),
                _ => panic!("transition linkage must be unconditional"),
            })
            .collect();
        let mut expected = vec!["OBS::libobs", "OBS::native-test-cmocka"];
        if name == "test_item_transition_empty_source" {
            expected.push("OBS::caption");
            assert_eq!(target.header_libraries, ["Uthash::Uthash"]);
        } else {
            assert!(target.header_libraries.is_empty());
        }
        assert_eq!(libraries, expected, "{name}");
        assert_eq!(
            target.artifact_definitions["GRAPHICS_MODULE_PATH"],
            "OBS::libobs-opengl"
        );
        assert_eq!(target.watch_inputs, ["libobs/data"]);
        let test = &graph.tests[name];
        assert_eq!(test.target, name);
        assert_eq!(test.command, ["${target}"]);
        assert_eq!(test.suite, "graphics");
        assert_eq!(test.working_directory.as_deref(), Some("${runtime}"));
        assert_eq!(test.timeout_seconds, Some(timeout));
        assert_eq!(test.skip_return_code, None);
        assert!(test.optional_prefixes.is_empty());
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("), "{name}");
        assert!(!fragment.contains("add_test("), "{name}");
    }
    graph
        .targets
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    graph
        .tests
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for opengl in [true, false] {
            let selected = graph
                .select(
                    &[
                        ("OS_LINUX".into(), os == "OS_LINUX"),
                        ("OS_MACOS".into(), os == "OS_MACOS"),
                        ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                        ("HAVE_OBS_OPENGL".into(), opengl),
                    ]
                    .into(),
                )
                .unwrap();
            for (name, _) in cases {
                assert_eq!(
                    selected.targets.contains_key(name),
                    os == "OS_LINUX" && opengl,
                    "{os}/{opengl}/{name}"
                );
                assert_eq!(
                    selected.tests.contains_key(name),
                    os == "OS_LINUX" && opengl,
                    "{os}/{opengl}/{name}"
                );
            }
        }
    }
}

/// #3712: group mutations and teardown must run through both native consumers.
#[test]
fn canonical_group_lifecycle_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let cases = [
        ("test_group_empty_bounds", true, Some(120)),
        ("test_group_active_refs", true, Some(120)),
        ("test_group_recursion", true, Some(180)),
        ("test_group_insert_empty", false, None),
        ("test_group_child_drift", true, Some(120)),
        ("test_group_teardown_uaf", false, None),
    ];
    for (name, graphics, timeout) in cases {
        assert_group_lifecycle_registration(&root, &graph, name, graphics, timeout);
    }
    graph
        .targets
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    graph
        .tests
        .retain(|name, _| cases.iter().any(|case| case.0 == name));
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        for opengl in [true, false] {
            let selected = graph
                .select(
                    &[
                        ("OS_LINUX".into(), os == "OS_LINUX"),
                        ("OS_MACOS".into(), os == "OS_MACOS"),
                        ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                        ("HAVE_OBS_OPENGL".into(), opengl),
                    ]
                    .into(),
                )
                .unwrap();
            for (name, graphics, _) in cases {
                let expected = if graphics {
                    os == "OS_LINUX" && opengl
                } else {
                    name == "test_group_teardown_uaf" || os != "OS_WINDOWS"
                };
                assert_eq!(
                    selected.targets.contains_key(name),
                    expected,
                    "{os}/{opengl}/{name}"
                );
                assert_eq!(
                    selected.tests.contains_key(name),
                    expected,
                    "{os}/{opengl}/{name}"
                );
                if expected && !graphics {
                    assert_eq!(
                        selected.tests[name].optional_prefixes.is_empty(),
                        os != "OS_LINUX"
                    );
                }
            }
        }
    }
}

fn assert_group_lifecycle_registration(
    root: &Path,
    graph: &obs_build::validation::graph::Graph,
    name: &str,
    graphics: bool,
    timeout: Option<u32>,
) {
    use obs_build::validation::graph::{ConditionalValue, Source};
    let target = graph.targets.get(name).unwrap_or_else(|| {
        panic!("canonical graph omits group lifecycle compatibility coverage: {name}")
    });
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == &format!("test/cmocka/{name}.c")));
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("group lifecycle linkage must be unconditional"),
        })
        .collect();
    let mut expected = vec!["OBS::libobs", "OBS::native-test-cmocka"];
    if name == "test_group_active_refs" {
        expected.push("OBS::caption");
    }
    if matches!(name, "test_group_empty_bounds" | "test_group_child_drift") {
        expected.push("m");
    }
    assert_eq!(libraries, expected, "{name}");
    if name == "test_group_active_refs" {
        assert_eq!(target.header_libraries, ["Uthash::Uthash"]);
    } else {
        assert!(target.header_libraries.is_empty());
    }
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, timeout, "{name}");
    assert_eq!(test.skip_return_code, None);
    assert_group_lifecycle_runtime(target, test, graphics);
    let fragment =
        fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
    assert!(!fragment.contains("add_executable("), "{name}");
    assert!(!fragment.contains("add_test("), "{name}");
}

fn assert_group_lifecycle_runtime(
    target: &obs_build::validation::graph::Target,
    test: &obs_build::validation::graph::Test,
    graphics: bool,
) {
    if graphics {
        assert_eq!(test.suite, "graphics");
        assert_eq!(
            target.artifact_definitions["GRAPHICS_MODULE_PATH"],
            "OBS::libobs-opengl"
        );
        assert_eq!(target.watch_inputs, ["libobs/data"]);
        assert_eq!(test.working_directory.as_deref(), Some("${runtime}"));
        assert!(test.optional_prefixes.is_empty());
    } else {
        assert_eq!(test.suite, "core");
        assert!(target.artifact_definitions.is_empty());
        assert!(target.watch_inputs.is_empty());
        assert_eq!(test.working_directory, None);
        assert_eq!(test.optional_prefixes, [vec!["xvfb-run", "-a"]]);
    }
}

/// #3707: keep the unchanged shutdown-after-failed-startup regression discoverable.
#[test]
fn canonical_shutdown_after_failed_startup_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_shutdown_after_failed_startup";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits shutdown after failed startup compatibility coverage");
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_shutdown_after_failed_startup.c"));
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("failed-startup shutdown linkage must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.working_directory.as_deref(), Some("${runtime}"));
    assert_eq!(test.timeout_seconds, Some(60));
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    let fragment = fs::read_to_string(
        root.join("test/cmocka/tests.d/test_shutdown_after_failed_startup.cmake"),
    )
    .unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for (os, expected) in [
        ("OS_LINUX", true),
        ("OS_MACOS", false),
        ("OS_WINDOWS", false),
    ] {
        let selected = graph
            .select(
                &[
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("OS_MACOS".into(), os == "OS_MACOS"),
                    ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                ]
                .into(),
            )
            .unwrap();
        assert_eq!(selected.targets.contains_key(name), expected, "{os}");
        assert_eq!(selected.tests.contains_key(name), expected, "{os}");
    }
}

/// #3702: retain all unchanged bounds placement alignment, crop and flip cases.
#[test]
fn canonical_scene_bounds_place_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Requirement, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_scene_bounds_place";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits scene bounds placement compatibility coverage");
    let sources: Vec<_> = target
        .sources
        .iter()
        .map(|source| match source {
            Source::Path(path) => path.as_str(),
            _ => panic!("bounds placement sources must be unconditional"),
        })
        .collect();
    assert_eq!(
        sources,
        [
            "test/cmocka/test_scene_bounds_place.c",
            "libobs/obs-scene-bounds-place.c.h",
            "libobs/obs-scene-bounds.c.h"
        ]
    );
    assert!(target.includes.is_empty());
    assert_eq!(target.watch_inputs, ["libobs/obs-scene-bounds.h"]);
    assert_eq!(target.sanitizers, None);
    let [
        ConditionalValue::Name(core),
        ConditionalValue::Name(cmocka),
        ConditionalValue::Conditional {
            name: library,
            requires,
        },
    ] = target.libraries.as_slice()
    else {
        panic!("bounds placement needs core, cmocka and conditional libm");
    };
    assert_eq!(core, "OBS::libobs");
    assert_eq!(cmocka, "OBS::native-test-cmocka");
    assert_eq!(library, "m");
    assert!(matches!(requires, Requirement::Not { not }
        if matches!(not.as_ref(), Requirement::Fact(fact) if fact == "MSVC")));
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_scene_bounds_place.cmake")).unwrap();
    assert!(!fragment.contains("add_executable(\n  test_scene_bounds_place\n"));
    assert!(fragment.contains("if(TARGET libobs_rust-static)"));
    assert!(
        fragment.contains("add_executable(test_scene_bounds_place_rust test_scene_bounds_place.c)")
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
}

/// #3705: retain the full scene collection crop-load integration suite.
#[test]
fn canonical_scene_item_crop_load_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("graphics");
    let name = "test_scene_item_crop_load";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits scene collection crop-load compatibility coverage");
    assert!(matches!(target.sources.as_slice(), [Source::Path(path)]
        if path == "test/cmocka/test_scene_item_crop_load.c"));
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("crop-load linkage must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    assert_eq!(
        target
            .artifact_definitions
            .get("GRAPHICS_MODULE_PATH")
            .map(String::as_str),
        Some("OBS::libobs-opengl")
    );
    assert_eq!(target.watch_inputs, ["libobs/data"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.working_directory.as_deref(), Some("${runtime}"));
    assert_eq!(test.timeout_seconds, Some(120));
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_scene_item_crop_load.cmake"))
            .unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for (os, graphics, expected) in [
        ("OS_LINUX", true, true),
        ("OS_LINUX", false, false),
        ("OS_MACOS", true, false),
        ("OS_WINDOWS", true, false),
    ] {
        let selected = graph
            .select(
                &[
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("OS_MACOS".into(), os == "OS_MACOS"),
                    ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                    ("HAVE_OBS_OPENGL".into(), graphics),
                ]
                .into(),
            )
            .unwrap();
        assert_eq!(
            selected.targets.contains_key(name),
            expected,
            "{os}, OpenGL={graphics}"
        );
        assert_eq!(
            selected.tests.contains_key(name),
            expected,
            "{os}, OpenGL={graphics}"
        );
    }
}

/// #3699: retain unchanged crop/size saturation cases and overflow sanitizers.
#[test]
fn canonical_scene_crop_steps_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Requirement, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_scene_crop_steps";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits scene crop arithmetic compatibility coverage");
    let sources: Vec<_> = target
        .sources
        .iter()
        .map(|source| match source {
            Source::Path(path) => path.as_str(),
            _ => panic!("scene crop sources must be unconditional"),
        })
        .collect();
    assert_eq!(
        sources,
        [
            "test/cmocka/test_scene_crop_steps.c",
            "libobs/obs-scene-crop.c.h"
        ]
    );
    assert_eq!(target.includes, ["libobs", "deps/libcaption"]);
    assert_eq!(
        target.sanitizers.as_deref(),
        Some("undefined,float-cast-overflow")
    );
    assert_eq!(target.libraries.len(), 5);
    for (library, expected) in target.libraries.iter().zip([
        "OBS::libobs",
        "OBS::caption",
        "Uthash::Uthash",
        "OBS::native-test-cmocka",
    ]) {
        assert!(matches!(library, ConditionalValue::Name(name) if name == expected));
    }
    let ConditionalValue::Conditional {
        name: library,
        requires,
    } = &target.libraries[4]
    else {
        panic!("libm must retain its non-MSVC guard");
    };
    assert_eq!(library, "m");
    assert!(matches!(requires, Requirement::Not { not }
        if matches!(not.as_ref(), Requirement::Fact(fact) if fact == "MSVC")));
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    assert!(
        !root
            .join("test/cmocka/tests.d/test_scene_crop_steps.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
}

/// #3694: retain all unchanged scene enumeration removal lifetime cases.
#[test]
fn canonical_scene_enum_items_removal_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_scene_enum_items_removal";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits scene enumeration removal lifetime coverage");
    assert_eq!(target.sources.len(), 1);
    assert!(matches!(&target.sources[0], Source::Path(path)
        if path == "test/cmocka/test_scene_enum_items_removal.c"));
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("scene enumeration libraries must be unconditional"),
        })
        .collect();
    assert_eq!(
        libraries,
        ["OBS::libobs", "OBS::native-test-cmocka", "Threads::Threads"]
    );
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert_eq!(test.optional_prefixes, [vec!["xvfb-run", "-a"]]);
    assert!(
        !root
            .join("test/cmocka/tests.d/test_scene_enum_items_removal.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(
                &[
                    ("OS_LINUX".into(), os == "OS_LINUX"),
                    ("OS_WINDOWS".into(), os == "OS_WINDOWS"),
                ]
                .into(),
            )
            .unwrap();
        assert_eq!(selected.targets.contains_key(name), os != "OS_WINDOWS");
        assert_eq!(selected.tests.contains_key(name), os != "OS_WINDOWS");
        if let Some(test) = selected.tests.get(name) {
            assert_eq!(!test.optional_prefixes.is_empty(), os == "OS_LINUX");
        }
    }
}

/// #3692: retain unchanged texture and cube mipmap data compatibility cases.
#[test]
fn canonical_texture_create_mip_data_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_texture_create_mip_data";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits texture mipmap data compatibility coverage");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_texture_create_mip_data.c")
    );
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("texture mipmap libraries must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    assert!(
        !root
            .join("test/cmocka/tests.d/test_texture_create_mip_data.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[("OS_LINUX".into(), os == "OS_LINUX"), (os.into(), true)].into())
            .unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
        assert!(selected.tests[name].optional_prefixes.is_empty());
    }
}

/// #3689: retain the five unchanged filter colour-space routing cases.
#[test]
fn canonical_filter_color_space_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_filter_color_space";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits filter colour-space routing compatibility coverage");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_filter_color_space.c")
    );
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("filter colour-space libraries must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    assert!(
        !root
            .join("test/cmocka/tests.d/test_filter_color_space.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[("OS_LINUX".into(), os == "OS_LINUX"), (os.into(), true)].into())
            .unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
        assert!(selected.tests[name].optional_prefixes.is_empty());
    }
}

/// #3687: retain the unchanged filter restore query while its target is absent.
#[test]
fn canonical_filter_size_no_target_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_filter_size_no_target";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits filter size without target compatibility coverage");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_filter_size_no_target.c")
    );
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("filter size libraries must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    assert!(
        !root
            .join("test/cmocka/tests.d/test_filter_size_no_target.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[("OS_LINUX".into(), os == "OS_LINUX"), (os.into(), true)].into())
            .unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
        assert!(selected.tests[name].optional_prefixes.is_empty());
    }
}

/// #3683: retain all eleven unchanged public hotkey pair latch cases.
#[test]
fn canonical_hotkey_pair_latch_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_hotkey_pair_latch";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits hotkey pair latch compatibility coverage");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_hotkey_pair_latch.c")
    );
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("hotkey pair latch libraries must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert_eq!(test.optional_prefixes, [["xvfb-run", "-a"]]);
    assert!(
        !root
            .join("test/cmocka/tests.d/test_hotkey_pair_latch.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[("OS_LINUX".into(), os == "OS_LINUX"), (os.into(), true)].into())
            .unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
        assert!(!selected.tests[name].optional_prefixes.is_empty());
    }
}

/// #3685: retain all five unchanged public effect name and index lookup cases.
#[test]
fn canonical_effect_lookups_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_effect_lookups";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits effect name and index lookup compatibility coverage");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_effect_lookups.c")
    );
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("effect lookup libraries must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    assert!(
        !root
            .join("test/cmocka/tests.d/test_effect_lookups.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[("OS_LINUX".into(), os == "OS_LINUX"), (os.into(), true)].into())
            .unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
        assert!(selected.tests[name].optional_prefixes.is_empty());
    }
}

/// #3681: retain all seven unchanged public output protocol enumeration cases.
#[test]
fn canonical_output_protocols_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_output_protocols";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits output protocol enumeration compatibility coverage");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_output_protocols.c")
    );
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("output protocol enumeration libraries must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert_eq!(
        test.optional_prefixes,
        [["xvfb-run", "-a", "-s", "-screen 0 1280x1024x24 -noreset"]]
    );
    assert!(
        !root
            .join("test/cmocka/tests.d/test_output_protocols.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[("OS_LINUX".into(), os == "OS_LINUX"), (os.into(), true)].into())
            .unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
        assert_eq!(
            selected.tests[name].optional_prefixes.is_empty(),
            os != "OS_LINUX"
        );
    }
}

/// #3679: retain the unchanged public encoder group NULL ownership regression.
#[test]
fn canonical_encoder_group_null_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_encoder_group_null";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits encoder group NULL compatibility coverage");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_encoder_group_null.c")
    );
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("encoder group NULL libraries must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert_eq!(
        test.optional_prefixes,
        [["xvfb-run", "-a", "-s", "-screen 0 1280x1024x24 -noreset"]]
    );
    assert!(
        !root
            .join("test/cmocka/tests.d/test_encoder_group_null.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[("OS_LINUX".into(), os == "OS_LINUX"), (os.into(), true)].into())
            .unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
        assert_eq!(
            selected.tests[name].optional_prefixes.is_empty(),
            os != "OS_LINUX"
        );
    }
}

/// #3677: both native consumers retain all four missing-files lifecycle cases.
#[test]
fn canonical_missing_files_list_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_missing_files_list";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits missing-files list compatibility coverage");
    assert_eq!(
        target
            .sources
            .iter()
            .map(|source| match source {
                Source::Path(path) => path.as_str(),
                _ => panic!("missing-files list sources must be unconditional"),
            })
            .collect::<Vec<_>>(),
        [
            "test/cmocka/test_missing_files_list.c",
            "frontend/utility/missing-files-list.c.h"
        ]
    );
    assert_eq!(target.includes, ["frontend/utility"]);
    assert_eq!(
        target
            .libraries
            .iter()
            .map(|library| match library {
                ConditionalValue::Name(name) => name.as_str(),
                _ => panic!("missing-files list libraries must be unconditional"),
            })
            .collect::<Vec<_>>(),
        ["OBS::libobs", "OBS::native-test-cmocka"]
    );
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert!(test.optional_prefixes.is_empty());
    assert!(
        !root
            .join("test/cmocka/tests.d/test_missing_files_list.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
}

/// #3673: retain all 15 unchanged public context-name cases.
#[test]
fn canonical_context_names_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_context_names";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits context-name compatibility coverage");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_context_names.c")
    );
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("context-name libraries must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert_eq!(
        test.optional_prefixes,
        [["xvfb-run", "-a", "-s", "-screen 0 1280x1024x24 -noreset"]]
    );
    assert!(
        !root
            .join("test/cmocka/tests.d/test_context_names.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[("OS_LINUX".into(), os == "OS_LINUX"), (os.into(), true)].into())
            .unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
        assert_eq!(
            selected.tests[name].optional_prefixes.is_empty(),
            os != "OS_LINUX"
        );
    }
}

/// #3671: retain all four unchanged public output protocol NULL cases.
#[test]
fn canonical_output_protocol_null_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_output_protocol_null";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits output protocol NULL compatibility coverage");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_output_protocol_null.c")
    );
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("output protocol NULL libraries must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert_eq!(test.skip_return_code, None);
    assert_eq!(
        test.optional_prefixes,
        [["xvfb-run", "-a", "-s", "-screen 0 1280x1024x24 -noreset"]]
    );
    assert!(
        !root
            .join("test/cmocka/tests.d/test_output_protocol_null.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph
            .select(&[("OS_LINUX".into(), os == "OS_LINUX"), (os.into(), true)].into())
            .unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
        assert_eq!(
            selected.tests[name].optional_prefixes.is_empty(),
            os != "OS_LINUX"
        );
    }
}

/// #3671: a disabled wrapper stays disabled even when its program exists.
#[test]
fn native_test_graph_conditional_prefixes_match_consumers() {
    use obs_build::validation::graph::Graph;
    let root = tempfile::tempdir().unwrap();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph = serde_json::json!({
        "schema": 1,
        "targets": {"answer": {"sources": ["answer.c"]}},
        "tests": {"answer": {
            "target": "answer",
            "optional_prefixes": [["cmake", "-E", "echo", "wrapped"]],
            "optional_prefixes_requires": "OS_LINUX"
        }}
    });
    fs::write(
        root.path().join("answer.c"),
        "int main(void) { return 0; }\n",
    )
    .unwrap();
    fs::write(root.path().join("graph.json"), graph.to_string()).unwrap();
    fs::write(
        root.path().join("CMakeLists.txt"),
        format!(
            "cmake_minimum_required(VERSION 3.28)\nproject(prefix-contract C)\nenable_testing()\ninclude(\"{}\")\nobs_native_test_graph(\"${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json\" \"${{CMAKE_CURRENT_SOURCE_DIR}}\")\n",
            repository.join("test/cmocka/native-test-graph.cmake").to_string_lossy().replace('\\', "/")
        ),
    ).unwrap();
    let graph = Graph::parse(&graph.to_string()).unwrap();
    for enabled in [true, false] {
        let selected = graph
            .select(&[("OS_LINUX".into(), enabled)].into())
            .unwrap();
        let target = root.path().join("answer");
        let command = selected.tests["answer"]
            .command(root.path(), root.path(), &target)
            .unwrap();
        if enabled {
            assert!(Path::new(command.get_program()).file_stem().unwrap() == "cmake");
        } else {
            assert_eq!(command.get_program(), target.as_os_str());
        }
        let output = root.path().join(if enabled { "wrapped" } else { "direct" });
        let configured = std::process::Command::new("cmake")
            .arg("-S")
            .arg(root.path())
            .arg("-B")
            .arg(&output)
            .arg(format!("-DOS_LINUX={}", if enabled { "ON" } else { "OFF" }))
            .output()
            .unwrap();
        assert!(
            configured.status.success(),
            "{}",
            String::from_utf8_lossy(&configured.stderr)
        );
        let built = std::process::Command::new("cmake")
            .arg("--build")
            .arg(&output)
            .arg("--config")
            .arg("Debug")
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let inventory = std::process::Command::new("ctest")
            .arg("--test-dir")
            .arg(&output)
            .args(["-C", "Debug", "--show-only=json-v1"])
            .output()
            .unwrap();
        assert!(inventory.status.success());
        let inventory: serde_json::Value = serde_json::from_slice(&inventory.stdout).unwrap();
        let arguments = inventory["tests"][0]["command"].as_array().unwrap();
        if enabled {
            assert_eq!(
                Path::new(arguments[0].as_str().unwrap())
                    .file_stem()
                    .unwrap(),
                "cmake"
            );
            assert_eq!(arguments[1], "-E");
            assert_eq!(arguments[3], "wrapped");
        } else {
            assert_eq!(arguments.len(), 1);
            assert!(arguments[0].as_str().unwrap().contains("answer"));
        }
    }
    assert!(
        graph
            .select(&Default::default())
            .unwrap_err()
            .contains("OS_LINUX")
    );
}

/// #3664: retain the unchanged bit-exact and locale configuration cases.
#[test]
fn canonical_config_hex_float_rounding_is_registered() {
    use obs_build::validation::graph::{Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_config_hex_float_rounding";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits hex-float configuration coverage");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], Source::Path(path) if path == "test/cmocka/test_config_hex_float_rounding.c")
    );
    let libraries: Vec<_> = target
        .libraries
        .iter()
        .map(|library| match library {
            obs_build::validation::graph::ConditionalValue::Name(name) => name.as_str(),
            _ => panic!("hex-float configuration libraries must be unconditional"),
        })
        .collect();
    assert_eq!(libraries, ["OBS::libobs", "OBS::native-test-cmocka"]);
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, Some(20));
    assert!(
        !root
            .join("test/cmocka/tests.d/test_config_hex_float_rounding.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
}

/// #1390: retain all five unchanged top-level window close cases.
#[test]
fn canonical_close_top_level_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_close_top_level")
        .expect("canonical graph omits test_close_top_level");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], obs_build::validation::graph::Source::Path(path) if path == "test/cmocka/test_close_top_level.cpp")
    );
    assert!(matches!(
        target.cpp_standard,
        Some(obs_build::CppStandard::Cpp17)
    ));
    assert_eq!(target.includes, ["frontend/utility"]);
    assert_eq!(
        target.watch_inputs,
        ["frontend/utility/close-top-level.hpp"]
    );
    let test = &graph.tests["test_close_top_level"];
    assert_eq!(test.qt_runtime.as_deref(), Some("Qt6::Core"));
    assert_eq!(test.timeout_seconds, Some(120));
    for linux in [false, true] {
        for widgets in [false, true] {
            let selected = graph
                .select(
                    &[
                        ("OS_LINUX".into(), linux),
                        ("MSVC".into(), !linux),
                        ("HAVE_QT6_CORE".into(), true),
                        ("HAVE_QT6_WIDGETS".into(), widgets),
                        ("HAVE_QT6_TEST".into(), false),
                    ]
                    .into(),
                )
                .unwrap();
            assert_eq!(selected.tests.contains_key("test_close_top_level"), widgets);
        }
    }
}

/// #3658: Cargo must retain CMake's unchanged Paste Filters compatibility cases.
#[test]
fn canonical_filter_clipboard_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let target = graph["targets"]
        .get("test_filter_clipboard")
        .expect("canonical graph omits test_filter_clipboard");
    assert_eq!(
        target["sources"],
        serde_json::json!([
            "test/cmocka/test_filter_clipboard.c",
            "frontend/utility/filter-clipboard.c.h"
        ])
    );
    assert_eq!(target["includes"], serde_json::json!(["frontend/utility"]));
    assert_eq!(
        target["watch_inputs"],
        serde_json::json!(["frontend/utility"])
    );
    let selected_graph = serde_json::json!({
        "schema": 1,
        "targets": {"test_filter_clipboard": target},
        "tests": {"test_filter_clipboard": graph["tests"]["test_filter_clipboard"]}
    });
    let parsed = obs_build::validation::graph::Graph::parse(&selected_graph.to_string()).unwrap();
    let selected = parsed.select(&Default::default()).unwrap();
    assert!(selected.targets.contains_key("test_filter_clipboard"));
    assert!(selected.tests.contains_key("test_filter_clipboard"));
    assert_eq!(
        target["libraries"],
        serde_json::json!(["OBS::libobs", "OBS::native-test-cmocka"])
    );
    assert_eq!(
        graph["tests"]["test_filter_clipboard"],
        serde_json::json!({"target": "test_filter_clipboard", "command": ["${target}"]})
    );
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_filter_clipboard.cmake")).unwrap();
    assert!(!fragment.contains("add_executable("));
    assert!(!fragment.contains("add_test("));
}

/// #1390: keep the original Linux replay-buffer estimate cases in the Qt graph.
#[test]
fn canonical_replay_buffer_estimate_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_replay_buffer_estimate")
        .expect("canonical graph omits test_replay_buffer_estimate");
    assert_eq!(
        target
            .sources
            .iter()
            .map(|source| match source {
                obs_build::validation::graph::Source::Path(path) => path.as_str(),
                obs_build::validation::graph::Source::Conditional { .. } =>
                    panic!("replay sources must be unconditional"),
            })
            .collect::<Vec<_>>(),
        [
            "test/cmocka/test_replay_buffer_estimate.cpp",
            "frontend/utility/replay-buffer-estimate.cpp.hpp",
        ]
    );
    assert_eq!(target.includes, ["frontend/utility"]);
    assert_eq!(
        target.watch_inputs,
        ["frontend/utility/replay-buffer-estimate.hpp"]
    );
    assert_eq!(
        target
            .libraries
            .iter()
            .map(|library| match library {
                obs_build::validation::graph::ConditionalValue::Name(name) => name.as_str(),
                obs_build::validation::graph::ConditionalValue::Conditional { .. } =>
                    panic!("replay libraries must be unconditional"),
            })
            .collect::<Vec<_>>(),
        ["Qt6::Widgets", "OBS::native-test-cmocka"]
    );
    let test = &graph.tests["test_replay_buffer_estimate"];
    assert_eq!(test.qt_runtime.as_deref(), Some("Qt6::Core"));
    for linux in [false, true] {
        for widgets in [false, true] {
            let facts = [
                ("OS_LINUX".into(), linux),
                ("MSVC".into(), !linux),
                ("HAVE_QT6_CORE".into(), true),
                ("HAVE_QT6_WIDGETS".into(), widgets),
                ("HAVE_QT6_TEST".into(), false),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(
                selected.tests.contains_key("test_replay_buffer_estimate"),
                linux && widgets
            );
        }
    }
}

/// #1390: retain the unchanged settings UI layout tests in the native graph.
#[test]
fn canonical_settings_track_rows_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_settings_track_rows")
        .expect("canonical graph omits test_settings_track_rows");
    assert_eq!(target.sources.len(), 1);
    assert_eq!(target.qt.uic, ["frontend/forms/OBSBasicSettings.ui"]);
    assert_eq!(target.includes_before, ["test/cmocka/settings-ui-stubs"]);
    assert_eq!(target.watch_inputs, ["test/cmocka/settings-ui-stubs"]);
    let test = &graph.tests["test_settings_track_rows"];
    assert_eq!(test.qt_runtime.as_deref(), Some("Qt6::Core"));
    assert_eq!(test.timeout_seconds, Some(120));
    for linux in [false, true] {
        for widgets in [false, true] {
            let facts = [
                ("OS_LINUX".into(), linux),
                ("MSVC".into(), !linux),
                ("HAVE_QT6_CORE".into(), true),
                ("HAVE_QT6_WIDGETS".into(), widgets),
                ("HAVE_QT6_TEST".into(), false),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(
                selected.tests.contains_key("test_settings_track_rows"),
                widgets
            );
        }
    }
}

#[test]
fn cargo_native_profile_uses_effective_optimization_and_symbols() {
    use obs_build::NativeProfile;
    for (optimization, symbols, expected) in [
        ("0", "true", NativeProfile::Debug),
        ("3", "false", NativeProfile::Release),
        ("2", "true", NativeProfile::RelWithDebInfo),
        ("s", "false", NativeProfile::MinSizeRel),
    ] {
        let actual = NativeProfile::from_cargo_lookup(|key| match key {
            "OPT_LEVEL" => Some(optimization.into()),
            "DEBUG" => Some(symbols.into()),
            // Deliberately misleading: effective settings must win.
            "PROFILE" => Some("debug".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(actual, expected);
    }
    for (key, value) in [
        ("OPT_LEVEL", "fast"),
        ("DEBUG", "yes"),
        ("OBS_SYS_NATIVE_PROFILE", "fast"),
        ("OBS_SYS_NATIVE_ASSERTIONS", "yes"),
    ] {
        let error = NativeProfile::from_cargo_lookup(|name| match name {
            name if name == key => Some(value.into()),
            "OPT_LEVEL" => Some("2".into()),
            "DEBUG" => Some("true".into()),
            _ => None,
        })
        .unwrap_err();
        assert!(error.contains(key), "{error}");
    }
    for (optimization, expected_level) in [
        ("0", obs_build::NativeOptimization::None),
        ("1", obs_build::NativeOptimization::One),
        ("2", obs_build::NativeOptimization::Two),
        ("3", obs_build::NativeOptimization::Three),
        ("s", obs_build::NativeOptimization::Size),
        ("z", obs_build::NativeOptimization::MinSize),
    ] {
        let actual = NativeProfile::from_cargo_lookup(|key| match key {
            "OPT_LEVEL" => Some(optimization.into()),
            "DEBUG" => Some("false".into()),
            "OBS_SYS_NATIVE_ASSERTIONS" => Some("1".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(
            actual,
            NativeProfile::Custom {
                optimization: expected_level,
                debug_info: false,
                assertions: true,
            }
        );
    }
    assert_eq!(
        NativeProfile::from_cargo_lookup(
            |key| (key == "OBS_SYS_NATIVE_PROFILE").then(|| "debug".into())
        )
        .unwrap(),
        NativeProfile::Debug
    );
}

/// #1390: missing canonical registrations must fail the routine, Qt-free gate.
#[test]
fn canonical_frontend_product_and_meter_tests_are_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph =
        obs_build::validation::graph::Graph::load(&root.join("test/cmocka/build/test-graph.json"))
            .unwrap();
    for name in ["test_product_name", "test_volume_meter_snapshot"] {
        assert!(
            graph.targets.contains_key(name),
            "missing canonical target {name}"
        );
        assert_eq!(graph.tests[name].target, name);
    }
}

/// #1390: run the original Qt-free frontend tests through both graph consumers.
/// Like the unchanged remux contract, this needs the native cmocka package.
#[test]
#[cfg(feature = "libobs-integration")]
#[expect(
    clippy::too_many_lines,
    reason = "one build-and-run block per graph consumer"
)]
fn canonical_frontend_product_and_meter_tests_run_unchanged() {
    use obs_build::validation::graph::{Dependency, Graph};
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let mut selected = serde_json::json!({"schema": 1, "targets": {}, "tests": {}});
    for name in ["test_product_name", "test_volume_meter_snapshot"] {
        for section in ["targets", "tests"] {
            selected[section][name] = raw[section]
                .get(name)
                .unwrap_or_else(|| panic!("canonical graph omits {section}/{name}"))
                .clone();
        }
    }
    let graph = Graph::parse(&selected.to_string()).unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("cargo-output"),
        profile: Default::default(),
    };
    let interfaces = [
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
        (
            "Threads::Threads".into(),
            Dependency {
                compile_options: vec!["-pthread".into()],
                link_options: vec!["-pthread".into()],
                ..Default::default()
            },
        ),
    ]
    .into();
    let facts = [("MSVC".into(), build.target.contains("msvc"))].into();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), 2);
    for (name, test) in &graph.tests {
        let result = test
            .command(&root, &build.output, &artifacts[&test.target])
            .unwrap()
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "Cargo {name}\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(
        directory.path().join("CMakeLists.txt"),
        format!(
            "cmake_minimum_required(VERSION 3.28)\nproject(frontend-contract C CXX)\nif(NOT DEFINED MSVC)\nset(MSVC FALSE)\nendif()\nenable_testing()\nfind_package(Threads REQUIRED)\nfind_package(PkgConfig REQUIRED)\npkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)\nadd_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)\ninclude(\"{source}/test/cmocka/native-test-graph.cmake\")\nobs_native_test_graph(\"${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json\" \"{source}\")\n"
        ),
    )
    .unwrap();
    let output = directory.path().join("cmake-output");
    for (program, arguments) in [
        (
            "cmake",
            vec![
                "-S",
                directory.path().to_str().unwrap(),
                "-B",
                output.to_str().unwrap(),
            ],
        ),
        (
            "cmake",
            vec!["--build", output.to_str().unwrap(), "--config", "Release"],
        ),
        (
            "ctest",
            vec![
                "--test-dir",
                output.to_str().unwrap(),
                "-C",
                "Release",
                "--output-on-failure",
                "--no-tests=error",
            ],
        ),
    ] {
        let mut command = std::process::Command::new(program);
        command.args(arguments);
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{program}\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let inventory = std::process::Command::new("ctest")
        .args([
            "--test-dir",
            output.to_str().unwrap(),
            "-C",
            "Release",
            "--show-only=json-v1",
        ])
        .output()
        .unwrap();
    assert!(inventory.status.success());
    let inventory: serde_json::Value = serde_json::from_slice(&inventory.stdout).unwrap();
    let names: std::collections::BTreeSet<_> = inventory["tests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|test| test["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, graph.tests.keys().map(String::as_str).collect());
}

/// #1390: preserve the optional Qt Test dependency of the unchanged Backtab cases.
#[test]
fn canonical_hotkey_backtab_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_hotkey_edit_backtab")
        .expect("canonical graph omits test_hotkey_edit_backtab");
    assert_eq!(
        target.watch_inputs,
        ["frontend/settings/OBSHotkeyCapture.hpp"]
    );
    assert_eq!(
        graph.tests["test_hotkey_edit_backtab"]
            .qt_runtime
            .as_deref(),
        Some("Qt6::Core")
    );
    for widgets in [false, true] {
        for qt_test in [false, true] {
            let facts = [
                ("OS_LINUX".into(), false),
                ("MSVC".into(), false),
                ("HAVE_QT6_CORE".into(), false),
                ("HAVE_QT6_WIDGETS".into(), widgets),
                ("HAVE_QT6_TEST".into(), qt_test),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(
                selected.tests.contains_key("test_hotkey_edit_backtab"),
                widgets && qt_test
            );
        }
    }
}

/// #1390: preserve the Linux-only Widgets/Test guard of the unchanged mnemonic cases.
#[test]
fn canonical_hotkey_mnemonic_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_hotkey_capture_mnemonic")
        .expect("canonical graph omits test_hotkey_capture_mnemonic");
    assert_eq!(
        target.watch_inputs,
        ["frontend/settings/OBSHotkeyCapture.hpp"]
    );
    assert_eq!(
        graph.tests["test_hotkey_capture_mnemonic"]
            .qt_runtime
            .as_deref(),
        Some("Qt6::Core")
    );
    assert_eq!(
        graph.tests["test_hotkey_capture_mnemonic"].resource_locks,
        ["timing"]
    );
    for linux in [false, true] {
        for widgets in [false, true] {
            for qt_test in [false, true] {
                let facts = [
                    ("OS_LINUX".into(), linux),
                    ("HAVE_QT6_CORE".into(), false),
                    ("HAVE_QT6_WIDGETS".into(), widgets),
                    ("HAVE_QT6_TEST".into(), qt_test),
                ]
                .into();
                let selected = graph.select(&facts).unwrap();
                assert_eq!(
                    selected.tests.contains_key("test_hotkey_capture_mnemonic"),
                    linux && widgets && qt_test
                );
            }
        }
    }
}

/// #1390: preserve the original Linux/Core-only volume-meter compatibility test.
#[test]
fn canonical_volume_meter_ticker_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_volume_meter_ticker")
        .expect("canonical graph omits test_volume_meter_ticker");
    assert_eq!(target.sources.len(), 1);
    assert_eq!(target.includes, ["frontend/components"]);
    assert_eq!(
        target.watch_inputs,
        ["frontend/components/VolumeMeterTicker.hpp"]
    );
    let test = &graph.tests["test_volume_meter_ticker"];
    assert_eq!(test.qt_runtime.as_deref(), Some("Qt6::Core"));
    assert_eq!(test.resource_locks, ["timing"]);
    for linux in [false, true] {
        for core in [false, true] {
            let facts = [
                ("OS_LINUX".into(), linux),
                ("MSVC".into(), false),
                ("HAVE_QT6_CORE".into(), core),
                ("HAVE_QT6_WIDGETS".into(), false),
                ("HAVE_QT6_TEST".into(), false),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(
                selected.tests.contains_key("test_volume_meter_ticker"),
                linux && core
            );
        }
    }
}

#[test]
fn canonical_undo_stack_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_undo_stack_enable")
        .expect("canonical graph omits test_undo_stack_enable");
    assert_eq!(target.qt.moc, ["frontend/utility/undo_stack.hpp"]);
    assert_eq!(target.includes_before, ["test/cmocka/undo-stack-stubs"]);
    assert_eq!(target.watch_inputs, ["test/cmocka/undo-stack-stubs"]);
    let test = &graph.tests["test_undo_stack_enable"];
    assert_eq!(test.timeout_seconds, Some(120));
    assert_eq!(test.qt_runtime.as_deref(), Some("Qt6::Core"));
    for linux in [false, true] {
        for widgets in [false, true] {
            let facts = [
                ("OS_LINUX".into(), linux),
                ("HAVE_QT6_CORE".into(), false),
                ("HAVE_QT6_WIDGETS".into(), widgets),
                ("HAVE_QT6_TEST".into(), false),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(
                selected.tests.contains_key("test_undo_stack_enable"),
                linux && widgets
            );
        }
    }
}

#[test]
fn canonical_frontend_utility_tests_are_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph =
        obs_build::validation::graph::Graph::load(&root.join("test/cmocka/build/test-graph.json"))
            .unwrap();
    for name in [
        "test_stream_service_select",
        "test_saved_transition",
        "test_record_disk_space",
        "test_multitrack_ladder",
        "test_multitrack_encoder_override",
    ] {
        assert!(
            graph.targets.contains_key(name),
            "missing canonical target {name}"
        );
        assert_eq!(graph.tests[name].target, name);
        assert_eq!(graph.targets[name].watch_inputs, ["frontend/utility"]);
    }
}

/// #1390: run unchanged Qt-free frontend utility tests through both graph consumers.
/// Like the unchanged remux contract, this needs the native cmocka package.
#[test]
#[cfg(feature = "libobs-integration")]
#[expect(
    clippy::too_many_lines,
    reason = "one build-and-run block per graph consumer"
)]
fn canonical_caption_and_frontend_utility_tests_run_unchanged() {
    use obs_build::validation::graph::{Dependency, Graph};
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let mut selected = serde_json::json!({"schema": 1, "targets": {}, "tests": {}});
    for name in [
        "test_caption_unmappable",
        "test_stream_service_select",
        "test_saved_transition",
        "test_record_disk_space",
        "test_multitrack_ladder",
        "test_multitrack_encoder_override",
    ] {
        for section in ["targets", "tests"] {
            selected[section][name] = raw[section]
                .get(name)
                .unwrap_or_else(|| panic!("canonical graph omits {section}/{name}"))
                .clone();
        }
    }
    let graph = Graph::parse(&selected.to_string()).unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("cargo-output"),
        profile: Default::default(),
    };
    let interfaces = [(
        "OBS::native-test-cmocka".into(),
        Dependency::package(&build.target, "cmocka").unwrap(),
    )]
    .into_iter()
    .chain([(
        "OBS::caption".into(),
        Dependency {
            artifacts: vec![obs_build::libobs::build_caption(&build, &root).unwrap()],
            include_dirs: vec![root.join("deps/libcaption")],
            ..Default::default()
        },
    )])
    .collect();
    let facts = [("MSVC".into(), build.target.contains("msvc"))].into();
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), 6);
    for (name, test) in &graph.tests {
        let result = test
            .command(&root, &build.output, &artifacts[&test.target])
            .unwrap()
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "Cargo {name}\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(
        directory.path().join("CMakeLists.txt"),
        format!(
            "cmake_minimum_required(VERSION 3.28)\nproject(frontend-utility-contract C)\nif(NOT DEFINED MSVC)\nset(MSVC FALSE)\nendif()\nenable_testing()\nfind_package(PkgConfig REQUIRED)\npkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)\nadd_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)\nadd_subdirectory(\"{source}/deps/libcaption\" caption)\ninclude(\"{source}/test/cmocka/native-test-graph.cmake\")\nobs_native_test_graph(\"${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json\" \"{source}\")\n"
        ),
    )
    .unwrap();
    let output = directory.path().join("cmake-output");
    for (program, arguments) in [
        (
            "cmake",
            vec![
                "-S",
                directory.path().to_str().unwrap(),
                "-B",
                output.to_str().unwrap(),
            ],
        ),
        (
            "cmake",
            vec![
                "--build",
                output.to_str().unwrap(),
                "--config",
                "Release",
                "--parallel",
                "2",
            ],
        ),
        (
            "ctest",
            vec![
                "--test-dir",
                output.to_str().unwrap(),
                "-C",
                "Release",
                "--output-on-failure",
                "--no-tests=error",
            ],
        ),
    ] {
        let mut command = std::process::Command::new(program);
        command.args(arguments);
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{program}\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let inventory = std::process::Command::new("ctest")
        .args([
            "--test-dir",
            output.to_str().unwrap(),
            "-C",
            "Release",
            "--show-only=json-v1",
        ])
        .output()
        .unwrap();
    assert!(inventory.status.success());
    let inventory: serde_json::Value = serde_json::from_slice(&inventory.stdout).unwrap();
    let names: std::collections::BTreeSet<_> = inventory["tests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|test| test["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, graph.tests.keys().map(String::as_str).collect());
}

#[test]
fn native_test_graph_rejects_colliding_qt_ui_outputs() {
    let root = tempfile::tempdir().unwrap();
    let graph = serde_json::json!({"schema": 1, "targets": {
        "first": {"sources": ["first.cpp"], "requires": false, "qt": {"uic": ["first/answer.ui"]}},
        "second": {"sources": ["second.cpp"], "requires": false, "qt": {"uic": ["second/answer.ui"]}}
    }, "tests": {"first": {"target": "first"}}});
    assert!(
        obs_build::validation::graph::Graph::parse(&graph.to_string())
            .unwrap_err()
            .contains("Qt UI output")
    );
    fs::write(root.path().join("graph.json"), graph.to_string()).unwrap();
    let consumer =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/cmocka/native-test-graph.cmake");
    fs::write(root.path().join("CMakeLists.txt"), format!("cmake_minimum_required(VERSION 3.28)\nproject(collision NONE)\ninclude(\"{}\")\nobs_native_test_graph(\"${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json\" \"${{CMAKE_CURRENT_SOURCE_DIR}}\")\n", consumer.to_string_lossy().replace('\\', "/"))).unwrap();
    let result = std::process::Command::new("cmake")
        .args([
            "-S",
            root.path().to_str().unwrap(),
            "-B",
            root.path().join("output").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Qt UI output"));
}

#[test]
#[cfg(feature = "libobs-integration")]
#[expect(
    clippy::too_many_lines,
    reason = "one build-and-run block per graph consumer with evaluated inventory"
)]
fn canonical_remux_queue_runs_unchanged() {
    use obs_build::validation::{self, graph::Dependency};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = tempfile::tempdir().unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("cargo-output"),
        profile: Default::default(),
    };
    let mut interfaces: std::collections::BTreeMap<String, Dependency> = [
        (
            "Qt6::Widgets".into(),
            Dependency::package(&build.target, "Qt6Widgets").unwrap(),
        ),
        (
            "Qt6::Core".into(),
            Dependency::package(&build.target, "Qt6Core").unwrap(),
        ),
        (
            "OBS::native-test-cmocka".into(),
            Dependency::package(&build.target, "cmocka").unwrap(),
        ),
    ]
    .into();
    // The published native SDKs may omit Qt Test, just as the original CMake
    // target is optional. A present but broken module must still fail resolution.
    let qt_test_available = if let Some(prefix) = obs_build::qt::Inputs::from_env().target_prefix {
        prefix
            .join("lib/cmake/Qt6Test/Qt6TestConfig.cmake")
            .try_exists()
            .unwrap()
    } else {
        let probe = std::process::Command::new("pkg-config")
            .args(["--variable=pcfiledir", "Qt6Test"])
            .output()
            .unwrap();
        probe.status.success() && !probe.stdout.is_empty()
    };
    if qt_test_available {
        interfaces.insert(
            "Qt6::Test".into(),
            Dependency::package(&build.target, "Qt6Test").unwrap(),
        );
    } else {
        println!(
            "Backtab not selected: Qt Test is absent from the selected SDK; original optional guard retained"
        );
    }
    let graph: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    // Compare the evaluated inventory, including optional Qt Test selection.
    let linux = build.target.contains("linux");
    let mut expected = obs_build::validation::graph::Graph::parse(&graph.to_string()).unwrap();
    expected.retain_suite("qt");
    let expected = expected
        .select(
            &[
                ("OS_LINUX".into(), linux),
                ("MSVC".into(), build.target.contains("msvc")),
                ("HAVE_QT6_CORE".into(), true),
                ("HAVE_QT6_WIDGETS".into(), true),
                ("HAVE_QT6_TEST".into(), qt_test_available),
            ]
            .into(),
        )
        .unwrap();
    assert!(
        graph["targets"].get("test_hotkey_edit_backtab").is_some(),
        "canonical graph omits test_hotkey_edit_backtab"
    );
    let tests = validation::build_qt_tests(&build, &root, &interfaces).unwrap();
    assert_eq!(
        tests
            .iter()
            .map(|test| test.name.as_str())
            .collect::<std::collections::BTreeSet<_>>(),
        expected.tests.keys().copied().collect()
    );
    assert_eq!(
        tests
            .iter()
            .any(|test| test.name == "test_hotkey_edit_backtab"),
        qt_test_available
    );
    assert_eq!(
        tests
            .iter()
            .any(|test| test.name == "test_undo_stack_enable"),
        linux
    );
    assert_eq!(
        tests
            .iter()
            .any(|test| test.name == "test_hotkey_capture_mnemonic"),
        linux && qt_test_available
    );
    assert!(
        tests
            .iter()
            .any(|test| test.name == "test_frontend_api_call")
    );
    assert_eq!(
        tests
            .iter()
            .any(|test| test.name == "test_volume_meter_ticker"),
        linux
    );
    assert_eq!(
        tests
            .iter()
            .any(|test| test.name == "test_replay_buffer_estimate"),
        linux
    );
    for test in tests {
        let result = test
            .command(&root, &build.output)
            .unwrap()
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let selected = serde_json::json!({"schema": 1,
    "targets": {
        "test_remux_queue_model": graph["targets"]["test_remux_queue_model"],
        "test_hotkey_edit_backtab": graph["targets"]["test_hotkey_edit_backtab"],
        "test_hotkey_capture_mnemonic": graph["targets"].get("test_hotkey_capture_mnemonic").expect("canonical graph omits test_hotkey_capture_mnemonic"),
        "test_undo_stack_enable": graph["targets"].get("test_undo_stack_enable").expect("canonical graph omits test_undo_stack_enable"),
        "test_volume_meter_ticker": graph["targets"].get("test_volume_meter_ticker").expect("canonical graph omits test_volume_meter_ticker"),
        "test_dock_proportions": graph["targets"].get("test_dock_proportions").expect("canonical graph omits test_dock_proportions"),
        "test_close_top_level": graph["targets"].get("test_close_top_level").expect("canonical graph omits test_close_top_level"),
        "test_settings_track_rows": graph["targets"].get("test_settings_track_rows").expect("canonical graph omits test_settings_track_rows"),
        "test_replay_buffer_estimate": graph["targets"].get("test_replay_buffer_estimate").expect("canonical graph omits test_replay_buffer_estimate"),
        "test_frontend_api_call": graph["targets"].get("test_frontend_api_call")
            .expect("canonical graph omits test_frontend_api_call")
    },
    "tests": {
        "test_remux_queue_model": graph["tests"]["test_remux_queue_model"],
        "test_hotkey_edit_backtab": graph["tests"]["test_hotkey_edit_backtab"],
        "test_hotkey_capture_mnemonic": graph["tests"]["test_hotkey_capture_mnemonic"],
        "test_undo_stack_enable": graph["tests"]["test_undo_stack_enable"],
        "test_volume_meter_ticker": graph["tests"]["test_volume_meter_ticker"],
        "test_dock_proportions": graph["tests"]["test_dock_proportions"],
        "test_close_top_level": graph["tests"]["test_close_top_level"],
        "test_settings_track_rows": graph["tests"]["test_settings_track_rows"],
        "test_replay_buffer_estimate": graph["tests"]["test_replay_buffer_estimate"],
        "test_frontend_api_call": graph["tests"]["test_frontend_api_call"]
    }});
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    let root = dunce::canonicalize(root)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(directory.path().join("CMakeLists.txt"), format!(
        "cmake_minimum_required(VERSION 3.28)\nproject(remux CXX)\nenable_testing()\nfind_package(Qt6 REQUIRED COMPONENTS Widgets)\nfind_package(Qt6 QUIET COMPONENTS Test)\nfind_package(Threads REQUIRED)\nfind_package(PkgConfig REQUIRED)\npkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)\nadd_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)\nset(HAVE_QT6_WIDGETS TRUE)\nset(HAVE_QT6_CORE TRUE)\nif(NOT DEFINED MSVC)\nset(MSVC FALSE)\nendif()\nset(OS_LINUX FALSE)\nif(CMAKE_SYSTEM_NAME STREQUAL \"Linux\")\nset(OS_LINUX TRUE)\nendif()\nset(HAVE_QT6_TEST FALSE)\nif(TARGET Qt6::Test)\nset(HAVE_QT6_TEST TRUE)\nendif()\ninclude(\"{root}/test/cmocka/native-test-graph.cmake\")\nobs_native_test_graph(\"${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json\" \"{root}\")\n"
    )).unwrap();
    let output = directory.path().join("cmake-output");
    for (program, arguments) in [
        (
            "cmake",
            vec![
                "-S",
                directory.path().to_str().unwrap(),
                "-B",
                output.to_str().unwrap(),
            ],
        ),
        (
            "cmake",
            vec![
                "--build",
                output.to_str().unwrap(),
                "--config",
                "Release",
                "--parallel",
                "2",
            ],
        ),
        (
            "ctest",
            vec![
                "--test-dir",
                output.to_str().unwrap(),
                "-C",
                "Release",
                "--output-on-failure",
            ],
        ),
    ] {
        let result = std::process::Command::new(program)
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let inventory = std::process::Command::new("ctest")
        .args([
            "--test-dir",
            output.to_str().unwrap(),
            "-C",
            "Release",
            "--show-only=json-v1",
        ])
        .output()
        .unwrap();
    assert!(inventory.status.success());
    let inventory: serde_json::Value = serde_json::from_slice(&inventory.stdout).unwrap();
    let tests = inventory["tests"].as_array().unwrap();
    let names: std::collections::BTreeSet<_> = tests
        .iter()
        .map(|test| test["name"].as_str().unwrap())
        .collect();
    let mut expected_names: std::collections::BTreeSet<_> = [
        "test_remux_queue_model",
        "test_close_top_level",
        "test_frontend_api_call",
        "test_settings_track_rows",
    ]
    .into();
    if qt_test_available {
        expected_names.insert("test_hotkey_edit_backtab");
    }
    if linux {
        expected_names.insert("test_undo_stack_enable");
        if qt_test_available {
            expected_names.insert("test_hotkey_capture_mnemonic");
            expected_names.insert("test_dock_proportions");
        }
        expected_names.insert("test_volume_meter_ticker");
        expected_names.insert("test_replay_buffer_estimate");
    }
    assert_eq!(names, expected_names);
    if linux && qt_test_available {
        assert_dock_proportions_properties(tests);
        let mnemonic = tests
            .iter()
            .find(|test| test["name"] == "test_hotkey_capture_mnemonic")
            .unwrap();
        assert!(
            mnemonic["properties"]
                .as_array()
                .unwrap()
                .iter()
                .any(|property| property["name"] == "RESOURCE_LOCK"
                    && property["value"] == serde_json::json!(["timing"]))
        );
    }
    assert_ticker_ctest_properties(tests, linux);
    if linux {
        let undo = tests
            .iter()
            .find(|test| test["name"] == "test_undo_stack_enable")
            .unwrap();
        assert!(
            undo["properties"]
                .as_array()
                .unwrap()
                .iter()
                .any(|property| property["name"] == "TIMEOUT"
                    && property["value"].as_f64() == Some(120.0))
        );
    }
    assert_settings_track_rows_ctest_properties(tests);
    assert_close_top_level_ctest_properties(tests);
    let frontend_api = tests
        .iter()
        .find(|test| test["name"] == "test_frontend_api_call")
        .unwrap();
    assert!(
        frontend_api["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|property| property["name"] == "TIMEOUT"
                && property["value"].as_f64() == Some(60.0))
    );
}

#[cfg(feature = "libobs-integration")]
fn assert_close_top_level_ctest_properties(tests: &[serde_json::Value]) {
    let close = tests
        .iter()
        .find(|test| test["name"] == "test_close_top_level")
        .unwrap();
    assert!(
        close["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|property| property["name"] == "TIMEOUT"
                && property["value"].as_f64() == Some(120.0))
    );
}

#[cfg(feature = "libobs-integration")]
fn assert_ticker_ctest_properties(tests: &[serde_json::Value], linux: bool) {
    if linux {
        let ticker = tests
            .iter()
            .find(|test| test["name"] == "test_volume_meter_ticker")
            .unwrap();
        assert!(
            ticker["properties"]
                .as_array()
                .unwrap()
                .iter()
                .any(|property| property["name"] == "RESOURCE_LOCK"
                    && property["value"] == serde_json::json!(["timing"]))
        );
    }
}

#[cfg(feature = "libobs-integration")]
fn assert_settings_track_rows_ctest_properties(tests: &[serde_json::Value]) {
    let settings = tests
        .iter()
        .find(|test| test["name"] == "test_settings_track_rows")
        .unwrap();
    assert!(
        settings["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|property| property["name"] == "TIMEOUT"
                && property["value"].as_f64() == Some(120.0))
    );
}

/// #1390: the QtCore-only frontend API helper must remain available without Widgets.
#[test]
fn canonical_frontend_api_call_is_registered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph =
        obs_build::validation::graph::Graph::load(&root.join("test/cmocka/build/test-graph.json"))
            .unwrap();
    let target = graph
        .targets
        .get("test_frontend_api_call")
        .expect("canonical graph omits test_frontend_api_call");
    assert_eq!(target.sources.len(), 1);
    assert_eq!(
        target.watch_inputs,
        ["frontend/utility/frontend-api-call.hpp"]
    );
    let test = &graph.tests["test_frontend_api_call"];
    assert_eq!(test.target, "test_frontend_api_call");
    assert_eq!(test.suite, "qt");
    assert_eq!(test.timeout_seconds, Some(60));
    assert_eq!(test.qt_runtime.as_deref(), Some("Qt6::Core"));
    let mut qt =
        obs_build::validation::graph::Graph::load(&root.join("test/cmocka/build/test-graph.json"))
            .unwrap();
    qt.retain_suite("qt");
    for have_core in [false, true] {
        let facts = [
            ("OS_LINUX".into(), false),
            ("MSVC".into(), false),
            ("HAVE_QT6_CORE".into(), have_core),
            ("HAVE_QT6_WIDGETS".into(), false),
            ("HAVE_QT6_TEST".into(), false),
        ]
        .into();
        let selected = qt.select(&facts).unwrap();
        assert_eq!(
            selected.tests.contains_key("test_frontend_api_call"),
            have_core
        );
        for widgets_test in ["test_remux_queue_model", "test_profile_import"] {
            assert!(!selected.tests.contains_key(widgets_test));
        }
    }
}

#[test]
fn native_test_graph_preserves_remux_queue_requirements() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let target = &graph["targets"]["test_remux_queue_model"];
    assert!(target.is_object(), "missing canonical remux queue target");
    assert_eq!(
        target["sources"],
        serde_json::json!([
            "test/cmocka/test_remux_queue_model.cpp",
            "frontend/utility/RemuxQueueModel.cpp.hpp"
        ])
    );
    assert_eq!(
        target["qt"]["moc"],
        serde_json::json!(["frontend/utility/RemuxQueueModel.hpp"])
    );
    assert_eq!(target["cpp_standard"], "17");
    assert_eq!(target["requires"], "HAVE_QT6_WIDGETS");
    assert_eq!(
        target["includes_before"],
        serde_json::json!(["test/cmocka/remux-queue-stubs"])
    );
    assert_eq!(
        target["libraries"],
        serde_json::json!(["Qt6::Widgets", "OBS::native-test-cmocka"])
    );
    assert_eq!(
        target["watch_inputs"],
        serde_json::json!(["test/cmocka/remux-queue-stubs"])
    );
    let test = &graph["tests"]["test_remux_queue_model"];
    assert_eq!(test["target"], "test_remux_queue_model");
    assert_eq!(test["suite"], "qt");
    assert_eq!(test["timeout_seconds"], 120);
    assert_eq!(test["qt_runtime"], "Qt6::Core");
    let fragment =
        fs::read_to_string(root.join("test/cmocka/tests.d/test_remux_queue_model.cmake")).unwrap();
    assert!(!fragment.contains("add_executable("));
}

#[test]
fn native_test_graph_prepends_runtime_paths_in_order() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::write(root.join("answer.c"), "int main(void) { return 0; }\n").unwrap();
    let graph = serde_json::json!({
        "schema": 1, "targets": {"answer": {"sources": ["answer.c"]}},
        "tests": {"paths": {"target": "answer", "command": ["cmake", "-E", "environment"],
            "environment": {"OBS_GRAPH_PATHS": "retained"},
            "environment_path_prepend": {"OBS_GRAPH_PATHS": ["first directory", "second directory"]}}}
    });
    let parsed = obs_build::validation::graph::Graph::parse(&graph.to_string()).unwrap();
    let expected =
        std::env::join_paths(["first directory", "second directory", "retained"]).unwrap();
    let mut command = parsed.tests["paths"]
        .command(root, root, &root.join("answer"))
        .unwrap();
    let result = command.output().unwrap();
    assert!(result.status.success());
    let assignment = format!("OBS_GRAPH_PATHS={}", expected.to_str().unwrap());
    assert!(
        String::from_utf8_lossy(&result.stdout)
            .lines()
            .any(|line| line == assignment)
    );
    fs::write(root.join("graph.json"), graph.to_string()).unwrap();
    let consumer =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/cmocka/native-test-graph.cmake");
    fs::write(root.join("CMakeLists.txt"), format!(
        "cmake_minimum_required(VERSION 3.28)\nproject(paths C)\nenable_testing()\ninclude(\"{}\")\nobs_native_test_graph(\"${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json\" \"${{CMAKE_CURRENT_SOURCE_DIR}}\")\n", consumer.to_string_lossy().replace('\\', "/")
    )).unwrap();
    let output = root.join("cmake-output");
    let result = std::process::Command::new("cmake")
        .args(["-S", root.to_str().unwrap(), "-B", output.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = std::process::Command::new("ctest")
        .args(["--test-dir", output.to_str().unwrap(), "-V"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(
        String::from_utf8_lossy(&result.stdout)
            .lines()
            .any(|line| line.ends_with(&format!("1: {assignment}")))
    );
}

#[test]
#[cfg(feature = "libobs-integration")]
#[expect(
    clippy::too_many_lines,
    reason = "one block per generated CMake target"
)]
fn native_test_graph_generates_qt_metaobjects() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::write(root.join("answer.hpp"), "#pragma once\n#include <QObject>\nclass Answer : public QObject {\n Q_OBJECT\n public: using QObject::QObject; Q_INVOKABLE int value() const { return 42; } };\n").unwrap();
    fs::write(
        root.join("twin.cpp"),
        "#include \"answer.hpp\"\nint main() { Answer answer; return answer.value() != 42; }\n",
    )
    .unwrap();
    fs::write(root.join("answer.ui"), r#"<?xml version="1.0"?><ui version="4.0"><class>AnswerForm</class><widget class="QWidget" name="AnswerForm"><property name="windowTitle"><string>Generated answer</string></property></widget><resources/><connections/></ui>"#).unwrap();
    fs::write(
        root.join("ui_answer.h"),
        "#error stale source-tree generated header selected\n",
    )
    .unwrap();
    fs::write(root.join("payload.txt"), "embedded answer 42").unwrap();
    fs::write(
        root.join("answer.qrc"),
        "<RCC><qresource prefix=\"/proof\"><file>payload.txt</file></qresource></RCC>",
    )
    .unwrap();
    fs::write(root.join("answer.cpp"), "#include \"answer.hpp\"\n#include <QMetaObject>\n#include <QApplication>\n#include <QFile>\n#include <ui_answer.h>\n#include <cstring>\nint main(int argc, char **argv) { QApplication app(argc, argv); QWidget widget; Ui::AnswerForm ui; ui.setupUi(&widget); if (widget.windowTitle() != \"Generated answer\") return 2; QFile file(\":/proof/payload.txt\"); if (!file.open(QIODevice::ReadOnly) || file.readAll() != \"embedded answer 42\") return 3; Answer answer; int value = 0; if (!QMetaObject::invokeMethod(&answer, \"value\", Qt::DirectConnection, Q_RETURN_ARG(int, value))) return 1; return value != 42 || std::strcmp(answer.metaObject()->className(), \"Answer\"); }\n").unwrap();
    let graph = serde_json::json!({
        "schema": 1,
        "targets": {"twin": {
            "sources": ["twin.cpp"], "cpp_standard": "17", "definitions": ["TWIN_METAOBJECT=1"],
            "qt": {"moc": ["answer.hpp"]}, "libraries": ["Qt6::Core"]
        }, "answer": {
            "sources": ["answer.cpp"], "cpp_standard": "17",
            "qt": {"moc": ["answer.hpp"], "uic": ["answer.ui"], "rcc": ["answer.qrc"]},
            "libraries": ["Qt6::Widgets"]
        }},
        "tests": {"answer": {"target": "answer", "environment": {"QT_QPA_PLATFORM": "minimal"}}, "twin": {"target": "twin"}}
    });
    let parsed = obs_build::validation::graph::Graph::parse(&graph.to_string()).unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: root.join("cargo-output"),
        profile: Default::default(),
    };
    let interfaces = [
        (
            "Qt6::Core".into(),
            obs_build::validation::graph::Dependency::package(&build.target, "Qt6Core").unwrap(),
        ),
        (
            "Qt6::Widgets".into(),
            obs_build::validation::graph::Dependency::package(&build.target, "Qt6Widgets").unwrap(),
        ),
    ]
    .into();
    let artifacts = parsed
        .compile(&build, root, &Default::default(), &interfaces)
        .unwrap();
    let mut answer = std::process::Command::new(&artifacts["answer"]);
    // minimal, not offscreen: the published Windows SDK ships no qoffscreen.
    answer.env("QT_QPA_PLATFORM", "minimal");
    let output = bounded(answer);
    assert!(
        output.status.success(),
        "{:?}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(root.join("graph.json"), graph.to_string()).unwrap();
    let consumer =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/cmocka/native-test-graph.cmake");
    fs::write(root.join("CMakeLists.txt"), format!(
        "cmake_minimum_required(VERSION 3.28)\nproject(metaobjects CXX)\nenable_testing()\nfind_package(Qt6 REQUIRED COMPONENTS Widgets)\ninclude(\"{}\")\nobs_native_test_graph(\"${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json\" \"${{CMAKE_CURRENT_SOURCE_DIR}}\")\nget_target_property(metaobjects answer AUTOMOC)\nif(NOT metaobjects)\nmessage(FATAL_ERROR \"Original per-target AUTOMOC must be preserved\")\nendif()\n", consumer.to_string_lossy().replace('\\', "/")
    )).unwrap();
    let output = root.join("cmake-output");
    for (program, arguments) in [
        (
            "cmake",
            vec!["-S", root.to_str().unwrap(), "-B", output.to_str().unwrap()],
        ),
        (
            "cmake",
            vec!["--build", output.to_str().unwrap(), "--config", "Release"],
        ),
        (
            "ctest",
            vec![
                "--test-dir",
                output.to_str().unwrap(),
                "-C",
                "Release",
                "--output-on-failure",
                "--timeout",
                "60",
            ],
        ),
    ] {
        let result = std::process::Command::new(program)
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn native_test_graph_preserves_include_precedence() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::create_dir_all(root.join("stubs")).unwrap();
    fs::create_dir_all(root.join("sources")).unwrap();
    fs::write(
        root.join("selected.h"),
        "#error repository header won over stub\n",
    )
    .unwrap();
    fs::write(root.join("stubs/selected.h"), "#define ANSWER 42\n").unwrap();
    fs::write(
        root.join("sources/answer.c"),
        "#include <selected.h>\nint main(void) { return ANSWER != 42; }\n",
    )
    .unwrap();
    let graph = serde_json::json!({
        "schema": 1,
        "targets": {"answer": {
            "sources": ["sources/answer.c"],
            "includes_before": ["stubs"]
        }},
        "tests": {"answer": {"target": "answer"}}
    });
    let parsed = obs_build::validation::graph::Graph::parse(&graph.to_string()).unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: root.join("cargo-output"),
        profile: Default::default(),
    };
    let artifacts = parsed
        .compile(&build, root, &Default::default(), &Default::default())
        .unwrap();
    assert!(
        std::process::Command::new(&artifacts["answer"])
            .status()
            .unwrap()
            .success()
    );
    fs::write(root.join("graph.json"), graph.to_string()).unwrap();
    let consumer =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/cmocka/native-test-graph.cmake");
    fs::write(root.join("CMakeLists.txt"), format!(
        "cmake_minimum_required(VERSION 3.28)\nproject(precedence C)\ninclude_directories(\"${{CMAKE_CURRENT_SOURCE_DIR}}\")\nenable_testing()\ninclude(\"{}\")\nobs_native_test_graph(\"${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json\" \"${{CMAKE_CURRENT_SOURCE_DIR}}\")\n",
        consumer.to_string_lossy().replace('\\', "/")
    )).unwrap();
    let output = root.join("cmake-output");
    for (program, arguments) in [
        (
            "cmake",
            vec!["-S", root.to_str().unwrap(), "-B", output.to_str().unwrap()],
        ),
        (
            "cmake",
            vec!["--build", output.to_str().unwrap(), "--config", "Release"],
        ),
        (
            "ctest",
            vec![
                "--test-dir",
                output.to_str().unwrap(),
                "-C",
                "Release",
                "--output-on-failure",
            ],
        ),
    ] {
        let result = std::process::Command::new(program)
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn native_test_graph_preserves_cpp_standard_without_changing_c_compilation() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::write(
        root.join("answer.c"),
        "#ifdef __cplusplus\n#error C source compiled as C++\n#endif\ntypedef char c_integer_width[(sizeof(int) >= 2) ? 1 : -1];\nint c_answer(void) { return 42; }\n",
    )
    .unwrap();
    fs::write(
        root.join("answer.cpp"),
        "#include <bit>\n#if defined(__GNUC__) && !defined(__STRICT_ANSI__)\n#error ISO C++ mode required\n#endif\nextern \"C\" int c_answer(void);\nconsteval unsigned answer_bits() { return std::bit_width(42u); }\nstatic_assert(answer_bits() == 6);\nint main() { return c_answer() != 42; }\n",
    )
    .unwrap();
    let graph = serde_json::json!({
        "schema": 1,
        "targets": {"answer": {"sources": ["answer.c", "answer.cpp"], "cpp_standard": "20"}},
        "tests": {"answer": {"target": "answer"}}
    });
    let parsed = obs_build::validation::graph::Graph::parse(&graph.to_string()).unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: root.join("cargo-output"),
        profile: Default::default(),
    };
    let artifacts = parsed
        .compile(&build, root, &Default::default(), &Default::default())
        .unwrap();
    assert!(
        std::process::Command::new(&artifacts["answer"])
            .status()
            .unwrap()
            .success()
    );
    fs::write(root.join("graph.json"), graph.to_string()).unwrap();
    let consumer =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/cmocka/native-test-graph.cmake");
    fs::write(
        root.join("CMakeLists.txt"),
        format!(
            "cmake_minimum_required(VERSION 3.28)\nproject(standards C CXX)\nenable_testing()\nset(CMAKE_CXX_STANDARD 11)\ninclude(\"{}\")\nobs_native_test_graph(\"${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json\" \"${{CMAKE_CURRENT_SOURCE_DIR}}\")\nget_target_property(extensions answer CXX_EXTENSIONS)\nif(extensions MATCHES \"NOTFOUND$\" OR extensions)\nmessage(FATAL_ERROR \"C++ extensions must be disabled\")\nendif()\n",
            consumer.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    let output = root.join("cmake-output");
    for (program, arguments) in [
        (
            "cmake",
            vec!["-S", root.to_str().unwrap(), "-B", output.to_str().unwrap()],
        ),
        (
            "cmake",
            vec!["--build", output.to_str().unwrap(), "--config", "Release"],
        ),
        (
            "ctest",
            vec![
                "--test-dir",
                output.to_str().unwrap(),
                "-C",
                "Release",
                "--output-on-failure",
            ],
        ),
    ] {
        let result = std::process::Command::new(program)
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    for value in [
        serde_json::json!(17),
        serde_json::json!(""),
        serde_json::json!("99"),
        serde_json::json!(true),
    ] {
        let mut invalid = graph.clone();
        invalid["targets"]["answer"]["cpp_standard"] = value;
        assert!(obs_build::validation::graph::Graph::parse(&invalid.to_string()).is_err());
        fs::write(root.join("graph.json"), invalid.to_string()).unwrap();
        let result = std::process::Command::new("cmake")
            .args(["-S", root.to_str().unwrap(), "-B", output.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("cpp_standard"));
    }
}

#[test]
fn native_test_graph_preserves_cpp_helper_build_requirements() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    for (name, implementation) in [
        ("test_disk_space_monitor", Some("disk-space-monitor")),
        (
            "test_scene_collection_migration",
            Some("scene-collection-migration"),
        ),
        ("test_crash_sentinel_dir", Some("crash-sentinel-dir")),
        ("test_settings_migration", None),
        ("test_output_start_log", None),
        ("test_unclean_launch", None),
    ] {
        let target = &raw["targets"][name];
        assert!(target.is_object(), "missing canonical {name}");
        let mut sources = vec![format!("test/cmocka/{name}.cpp")];
        if let Some(implementation) = implementation {
            sources.push(format!("frontend/utility/{implementation}.cpp.hpp"));
        }
        assert_eq!(target["sources"], serde_json::json!(sources));
        assert_eq!(target["cpp_standard"], "17");
        assert_eq!(target["includes"], serde_json::json!(["frontend/utility"]));
        let mut watched = vec!["frontend/utility".to_owned()];
        if let Some(implementation) = implementation {
            watched.push(format!(
                "frontend/build/frontend-utility-{implementation}.pch.hpp"
            ));
        }
        assert_eq!(target["watch_inputs"], serde_json::json!(watched));
        let mut libraries = vec!["OBS::libobs"];
        if name == "test_disk_space_monitor" {
            libraries.push("Threads::Threads");
        }
        libraries.push("OBS::native-test-cmocka");
        assert_eq!(target["libraries"], serde_json::json!(libraries));
        assert_eq!(raw["tests"][name]["target"], name);
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "scenario table: one block per ported step"
)]
fn native_test_graph_cmake_builds_and_runs_selected_contract() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let output = root.join("output");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let consumer = repository.join("test/cmocka/native-test-graph.cmake");
    fs::create_dir_all(root.join("dependency-include")).unwrap();
    fs::create_dir_all(root.join("header-only-include")).unwrap();
    fs::write(
        root.join("header-only-include/headers-only.h"),
        "#define HEADER_ONLY_ANSWER 17\n",
    )
    .unwrap();
    fs::write(
        root.join("dependency-include/dependency.h"),
        "#define DEPENDENCY_ANSWER 42\n",
    )
    .unwrap();
    fs::write(
        root.join("answer.c"),
        r#"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "dependency.h"
#include "headers-only.h"
#ifdef HEADER_POLICY_LEAKED
#error Header imports must not inherit compile definitions or link policy
#endif
int selected_answer(void);
int main(int argc, char **argv) {
    const char *value = getenv("OBS_GRAPH_VALUE");
    const char *wrapped = getenv("OBS_WRAPPER_VALUE");
    if (!wrapped || strcmp(wrapped, "wrapped")) return 5;
    if (COMPILER_OPTION_REQUIRED != 23 || HEADER_ONLY_ANSWER != 17 || selected_answer() != 42 || EXPECTED != 42 || DEPENDENCY_ANSWER != 42 || DEPENDENCY_REQUIRED != 5 || argc != 2 || strcmp(argv[1], "two words") ||
        !value || strcmp(value, "first;second")) return 1;
    FILE *module = fopen(ARTIFACT_PATH, "rb");
    if (!module) return 4;
    fclose(module);
    FILE *input = fopen(SOURCE_HEADER, "r");
    if (!input) return 3;
    fclose(input);
    FILE *file = fopen("marker.txt", "w");
    if (!file) return 2;
    return fclose(file);
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("selected.c"),
        "int selected_answer(void) { return 42; }\n",
    )
    .unwrap();
    fs::write(
        root.join("fixture-module.c"),
        "int fixture_module(void) { return 7; }\n",
    )
    .unwrap();
    let graph = serde_json::json!({
        "schema": 1,
        "targets": {
            "answer": {"sources": ["answer.c", {"path": "selected.c", "requires": "GRAPH_FEATURE"}, {"path": "missing-disabled.c", "requires": "GRAPH_DISABLED"}], "definitions": ["EXPECTED=42", "SOURCE_HEADER=\"${source}/dependency-include/dependency.h\""],
                "libraries": ["fixture-dependency", {"name": "disabled-library", "requires": "GRAPH_DISABLED"}],
                "header_libraries": ["fixture-headers"],
                "artifact_definitions": {"ARTIFACT_PATH": "fixture-module"},
                "watch_inputs": ["dependency-include", "fixture-module.c"],
                "compile_options": [{"name": "-DCOMPILER_OPTION_REQUIRED=23", "requires": "GRAPH_FEATURE"}, {"name": "-invalid-disabled-compiler-option", "requires": "GRAPH_DISABLED"}],
                "requires": {"all": ["GRAPH_FEATURE", {"not": "GRAPH_DISABLED"}]}},
            "disabled": {"sources": ["does-not-exist.c"], "requires": "GRAPH_DISABLED"}
        },
        "tests": {
            "answer-case": {"target": "answer", "suite": "fixture", "optional_prefixes": [["cmake", "-E", "env", "OBS_WRAPPER_VALUE=outer"], ["obs-missing-wrapper-fixture-1332", "${source}", "${build}", "${runtime}", "${target}"], ["cmake", "-E", "env", "OBS_WRAPPER_VALUE=wrapped"]], "command": ["${target}", "two words"],
                "environment": {"OBS_GRAPH_VALUE": "first;second"},
                "working_directory": "${runtime}", "timeout_seconds": 17,
                "skip_return_code": 77, "resource_locks": ["fixture-lock"]},
            "disabled-case": {"target": "disabled"}
        }
    });
    fs::write(root.join("graph.json"), graph.to_string()).unwrap();
    fs::write(root.join("CMakeLists.txt"), format!(
        "cmake_minimum_required(VERSION 3.28)\nproject(graph-fixture C)\nenable_testing()\nset(OBS_OUTPUT_DIR \"${{CMAKE_CURRENT_BINARY_DIR}}/runtime root\")\nfile(MAKE_DIRECTORY \"${{OBS_OUTPUT_DIR}}\" \"${{OBS_OUTPUT_DIR}}/Release\")\nadd_library(fixture-module MODULE fixture-module.c)\nset_target_properties(fixture-module PROPERTIES LIBRARY_OUTPUT_DIRECTORY \"${{CMAKE_CURRENT_BINARY_DIR}}/module output\")\nset(GRAPH_FEATURE ON)\nset(GRAPH_DISABLED OFF)\nadd_library(fixture-dependency INTERFACE)\ntarget_include_directories(fixture-dependency INTERFACE \"${{CMAKE_CURRENT_SOURCE_DIR}}/dependency-include\")\ntarget_compile_definitions(fixture-dependency INTERFACE DEPENDENCY_REQUIRED=5)\nadd_library(fixture-headers INTERFACE)\ntarget_include_directories(fixture-headers INTERFACE \"${{CMAKE_CURRENT_SOURCE_DIR}}/header-only-include\")\ntarget_compile_definitions(fixture-headers INTERFACE HEADER_POLICY_LEAKED=1)\ntarget_compile_options(fixture-headers INTERFACE -invalid-header-policy)\ntarget_link_libraries(fixture-headers INTERFACE missing-header-link-library)\ninclude(\"{}\")\nobs_native_test_graph(\"${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json\" \"${{CMAKE_CURRENT_SOURCE_DIR}}\")\n",
        consumer.to_string_lossy().replace('\\', "/")
    )).unwrap();
    let mut project = fs::read_to_string(root.join("CMakeLists.txt")).unwrap();
    project.push_str(
        r#"
get_property(graph_inputs DIRECTORY PROPERTY CMAKE_CONFIGURE_DEPENDS)
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/watched-inputs.txt" "${graph_inputs}")
"#,
    );
    fs::write(root.join("CMakeLists.txt"), project).unwrap();
    for arguments in [
        vec![
            "-S".into(),
            root.to_string_lossy().into_owned(),
            "-B".into(),
            output.to_string_lossy().into_owned(),
            "-DCMAKE_BUILD_TYPE=Release".into(),
        ],
        vec![
            "--build".into(),
            output.to_string_lossy().into_owned(),
            "--config".into(),
            "Release".into(),
        ],
    ] {
        let result = std::process::Command::new("cmake")
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let registered_inputs = fs::read_to_string(output.join("watched-inputs.txt")).unwrap();
    for input in ["dependency-include", "fixture-module.c"] {
        assert!(registered_inputs.contains(&root.join(input).to_string_lossy().replace('\\', "/")));
    }
    let inventory = std::process::Command::new("ctest")
        .args([
            "--test-dir",
            output.to_str().unwrap(),
            "-C",
            "Release",
            "--show-only=json-v1",
        ])
        .output()
        .unwrap();
    assert!(inventory.status.success());
    let inventory: serde_json::Value = serde_json::from_slice(&inventory.stdout).unwrap();
    let tests = inventory["tests"].as_array().unwrap();
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0]["name"], "answer-case");
    for (name, value) in [
        ("TIMEOUT", serde_json::json!(17.0)),
        ("SKIP_RETURN_CODE", serde_json::json!(77)),
        ("RESOURCE_LOCK", serde_json::json!(["fixture-lock"])),
        ("LABELS", serde_json::json!(["native-fixture"])),
        (
            "WORKING_DIRECTORY",
            serde_json::json!(
                output
                    .join("runtime root/Release")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        ),
    ] {
        assert!(
            tests[0]["properties"]
                .as_array()
                .unwrap()
                .iter()
                .any(|property| property["name"] == name && property["value"] == value),
            "missing {name}: {tests:?}"
        );
    }
    let result = std::process::Command::new("ctest")
        .args([
            "--test-dir",
            output.to_str().unwrap(),
            "-C",
            "Release",
            "--output-on-failure",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.join("runtime root/Release/marker.txt").is_file());
    let rust_graph = obs_build::validation::graph::Graph::parse(&graph.to_string()).unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: root.join("cargo-output"),
        profile: Default::default(),
    };
    let facts = std::collections::BTreeMap::from([
        ("GRAPH_FEATURE".into(), true),
        ("GRAPH_DISABLED".into(), false),
    ]);
    let module_build = NativeBuild {
        output: build.output.join("module output"),
        ..build.clone()
    };
    let module = module_build
        .shared_library(&SharedLibrary {
            name: "fixture-module".into(),
            sources: vec![root.join("fixture-module.c")],
            ..Default::default()
        })
        .unwrap();
    let dependencies = std::collections::BTreeMap::from([
        (
            "fixture-module".into(),
            obs_build::validation::graph::Dependency {
                artifacts: vec![module],
                ..Default::default()
            },
        ),
        (
            "fixture-dependency".into(),
            obs_build::validation::graph::Dependency {
                include_dirs: vec![root.join("dependency-include")],
                definitions: vec![("DEPENDENCY_REQUIRED".into(), Some("5".into()))],
                ..Default::default()
            },
        ),
        (
            "fixture-headers".into(),
            obs_build::validation::graph::Dependency {
                include_dirs: vec![root.join("header-only-include")],
                artifacts: vec![root.join("missing-header-library.a")],
                definitions: vec![("HEADER_POLICY_LEAKED".into(), Some("1".into()))],
                compile_options: vec!["-invalid-header-policy".into()],
                link_options: vec!["-invalid-header-policy".into()],
                ..Default::default()
            },
        ),
    ]);
    let artifacts = rust_graph
        .compile(&build, root, &facts, &dependencies)
        .unwrap();
    assert_eq!(artifacts.len(), 1);
    let selected = rust_graph.select(&facts).unwrap();
    assert_eq!(
        selected.tests.keys().copied().collect::<Vec<_>>(),
        vec![tests[0]["name"].as_str().unwrap()]
    );
    let executable = obs_build::validation::Executable {
        name: "answer-case".into(),
        path: artifacts["answer"].clone(),
        runtime: Some(selected.tests["answer-case"].clone()),
        dependencies: rust_graph.targets["answer"]
            .dependency_artifacts(&facts, &dependencies)
            .unwrap(),
    };
    assert!(
        executable.dependencies.is_empty(),
        "header-only imports must not create runtime dependencies"
    );
    let mut with_artifact = dependencies.clone();
    let imported = root.join("selected-imported-library.so");
    with_artifact
        .get_mut("fixture-dependency")
        .unwrap()
        .artifacts
        .push(imported.clone());
    assert_eq!(
        rust_graph.targets["answer"]
            .dependency_artifacts(&facts, &with_artifact)
            .unwrap(),
        vec![imported]
    );
    let runtime = build.output.join("runtime root");
    fs::create_dir_all(&runtime).unwrap();
    let mut command = executable
        .command_in(root, &build.output, &runtime)
        .unwrap();
    assert_eq!(command.get_current_dir(), Some(runtime.as_path()));
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(runtime.join("marker.txt").is_file());
    for (pointer, value, message) in [
        ("/schema", serde_json::json!(2), "schema"),
        (
            "/tests/answer-case/optional_prefixes",
            serde_json::json!([[]]),
            "prefix",
        ),
        (
            "/tests/answer-case/optional_prefixes",
            serde_json::json!([[""]]),
            "prefix",
        ),
        (
            "/tests/answer-case/optional_prefixes",
            serde_json::json!([["${target}"]]),
            "prefix",
        ),
        (
            "/tests/answer-case/optional_prefixes",
            serde_json::json!([["obs-missing-wrapper-fixture-1332", "${unknown}"]]),
            "unknown",
        ),
        (
            "/targets/answer/watch_inputs",
            serde_json::json!([""]),
            "watch input",
        ),
        (
            "/targets/answer/watch_inputs",
            serde_json::json!(["../outside"]),
            "watch input",
        ),
        (
            "/targets/answer/watch_inputs",
            serde_json::json!(["/outside"]),
            "watch input",
        ),
        (
            "/targets/answer/watch_inputs",
            serde_json::json!(["C:/outside"]),
            "watch input",
        ),
        (
            "/targets/answer/artifact_definitions",
            serde_json::json!({"ARTIFACT_PATH":"absent-artifact"}),
            "absent-artifact",
        ),
        (
            "/targets/answer/artifact_definitions",
            serde_json::json!({"BAD-NAME":"fixture-module"}),
            "artifact",
        ),
        (
            "/targets/answer/artifact_definitions",
            serde_json::json!({"ARTIFACT_PATH":"fixture-headers"}),
            "artifact",
        ),
        (
            "/targets/answer/definitions",
            serde_json::json!(["BAD=${runtime}"]),
            "compile definition",
        ),
        ("/tests/answer-case/suite", serde_json::json!(42), "suite"),
        (
            "/targets/answer/compile_options",
            serde_json::json!([{ "name": "-DUNKNOWN", "requires": "UNKNOWN_COMPILER_FEATURE" }]),
            "UNKNOWN_COMPILER_FEATURE",
        ),
        (
            "/targets/answer/header_libraries",
            serde_json::json!(["absent-header-library"]),
            "absent-header-library",
        ),
        (
            "/targets/answer/sources",
            serde_json::json!([{"path": "selected.c", "requires": "UNKNOWN_SOURCE_FEATURE"}]),
            "UNKNOWN_SOURCE_FEATURE",
        ),
        (
            "/targets/answer/sources",
            serde_json::json!([{"path": "selected.c", "requires": false}]),
            "sources",
        ),
        (
            "/targets/answer/sources",
            serde_json::json!([{"path": 42, "requires": true}]),
            "source path",
        ),
        (
            "/targets/answer/definitions",
            serde_json::json!(["SELF=${target}"]),
            "compile definition",
        ),
        (
            "/targets/answer/requires",
            serde_json::json!("UNKNOWN_FEATURE"),
            "UNKNOWN_FEATURE",
        ),
        (
            "/tests/answer-case/target",
            serde_json::json!("absent"),
            "absent",
        ),
        (
            "/tests/answer-case/command",
            serde_json::json!([]),
            "command",
        ),
        (
            "/tests/answer-case/command",
            serde_json::json!(["${unknown}"]),
            "unknown",
        ),
    ] {
        let mut invalid = graph.clone();
        *invalid.pointer_mut(pointer).unwrap() = value;
        let rust_result = obs_build::validation::graph::Graph::parse(&invalid.to_string())
            .and_then(|graph| graph.compile(&build, root, &facts, &dependencies));
        assert!(
            rust_result.is_err(),
            "Cargo accepted invalid graph at {pointer}"
        );
        fs::write(root.join("graph.json"), invalid.to_string()).unwrap();
        let result = std::process::Command::new("cmake")
            .args(["-S", root.to_str().unwrap(), "-B", output.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            !result.status.success(),
            "CMake accepted invalid graph at {pointer}"
        );
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(message),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let mut invalid = graph;
    invalid["targets"]["answer"]["sorces"] = serde_json::json!(["typo.c"]);
    fs::write(root.join("graph.json"), invalid.to_string()).unwrap();
    let result = std::process::Command::new("cmake")
        .args(["-S", root.to_str().unwrap(), "-B", output.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("sorces"));
}

#[test]
fn native_test_graph_checks_selection_references_and_commands() {
    use obs_build::validation::graph::Graph;
    use std::collections::BTreeMap;
    let fixture = serde_json::json!({
        "schema": 1,
        "targets": {
            "portable": {"sources": ["test/cmocka/portable.c"]},
            "pulse": {"sources": ["test/cmocka/pulse.c"],
                "requires": {"all": ["OS_LINUX", "ENABLE_PULSEAUDIO"]}}
        },
        "tests": {
            "portable-case": {"target": "portable"},
            "pulse-case": {"target": "pulse", "command": [
                "sh", "${source}/test/cmocka/pulse.sh", "${target}"],
                "timeout_seconds": 60, "skip_return_code": 77,
                "environment": {"LP_NUM_THREADS": "1"}}
        }
    });
    let graph = Graph::parse(&fixture.to_string()).unwrap();
    let mut flags = BTreeMap::from([
        ("OS_LINUX".into(), true),
        ("ENABLE_PULSEAUDIO".into(), false),
    ]);
    let selected = graph.select(&flags).unwrap();
    assert_eq!(selected.targets.len(), 1);
    assert_eq!(selected.tests.len(), 1);
    assert!(selected.tests.contains_key("portable-case"));
    assert_eq!(selected.tests["portable-case"].timeout_seconds, None);
    flags.insert("ENABLE_PULSEAUDIO".into(), true);
    let selected = graph.select(&flags).unwrap();
    assert_eq!(selected.targets.len(), 2);
    let pulse = &selected.tests["pulse-case"];
    assert_eq!(pulse.skip_return_code, Some(77));
    assert_eq!(pulse.timeout_seconds, Some(60));
    assert_eq!(pulse.environment["LP_NUM_THREADS"], "1");
    assert_eq!(pulse.command[2], "${target}");
    flags.remove("ENABLE_PULSEAUDIO");
    assert!(
        graph
            .select(&flags)
            .unwrap_err()
            .contains("ENABLE_PULSEAUDIO")
    );
    for (pointer, value, message) in [
        ("/schema", serde_json::json!(2), "schema"),
        (
            "/tests/portable-case/target",
            serde_json::json!("absent"),
            "absent",
        ),
        (
            "/tests/pulse-case/timeout_seconds",
            serde_json::json!(-1),
            "u32",
        ),
        (
            "/tests/pulse-case/command",
            serde_json::json!([]),
            "command",
        ),
        (
            "/tests/pulse-case/command",
            serde_json::json!(["${unknown}"]),
            "unknown",
        ),
        ("/targets/pulse/sources", serde_json::json!([]), "sources"),
    ] {
        let mut invalid = fixture.clone();
        *invalid.pointer_mut(pointer).unwrap() = value;
        assert!(
            Graph::parse(&invalid.to_string())
                .unwrap_err()
                .contains(message)
        );
    }
    let mut unbounded = fixture.clone();
    unbounded["tests"]["pulse-case"]["timeout_seconds"] = serde_json::json!(0);
    assert_eq!(
        Graph::parse(&unbounded.to_string()).unwrap().tests["pulse-case"].timeout_seconds,
        Some(0)
    );
    let mut invalid = fixture;
    invalid["targets"]["portable"]["definitions"] = serde_json::json!(["SELF=${target}"]);
    assert!(
        Graph::parse(&invalid.to_string())
            .unwrap_err()
            .contains("compile definition")
    );
    invalid["targets"]["portable"]["definitions"] = serde_json::json!([]);
    invalid["targets"]["pulse"]["sorces"] = serde_json::json!(["typo.c"]);
    assert!(
        Graph::parse(&invalid.to_string())
            .unwrap_err()
            .contains("sorces")
    );
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "scenario table: one block per ported step"
)]
#[expect(
    clippy::too_many_lines,
    reason = "scenario table: one block per ported step"
)]
fn native_test_graph_replaces_legacy_utility_definitions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph =
        obs_build::validation::graph::Graph::load(&root.join("test/cmocka/build/test-graph.json"))
            .unwrap();
    assert!(
        !root
            .join("rust/obs-build/manifests/c-tests-linux.json")
            .exists(),
        "independent Linux test inventory remains"
    );
    for name in [
        "test_menu_text",
        "test_profile_remove",
        "test_tbar_position",
        "test_media_time",
        "test_studio_mode_exit",
        "test_studio_mode_swap",
        "test_profile_activate",
        "test_output_deactivate_state",
        "test_cf_preprocessor",
        "test_output_null_id",
        "test_output_failed_start_encoders",
        "test_output_rescale",
        "test_output_start_twice",
        "test_texture_encoder_raw_format",
        "test_audio_restart_resampler_tail",
        "test_obs_enum_outputs_removal",
        "test_output_caption_queue",
        "test_output_caption_text",
        "test_scene_bounds_fit",
        "test_scene_coords",
        "test_encoder_fallback",
        "test_scene_list_remove",
        "test_output_filename_format",
        "test_output_delay_e2e",
        "test_encoder_video_reset",
        "test_scene_private_canvas",
        "test_source_profiler",
        "test_scene_rotated_scale_filter",
        "test_scene_duplicate_scale_ref",
        "test_scene_dup_group_children",
        "test_output_caption_codec",
        "test_output_delay_restart",
        "test_output_interleave_e2e",
        "test_output_interleave_restart",
        "test_audio_sync_offset_change",
        "test_audio_capture_mix_race",
        "test_audio_io_inlines",
        "test_audio_math",
        "test_bounds",
        "test_calldata",
        "test_decl",
        "test_dstr_zero_len",
        "test_audio_controls",
        "test_audio_clock_drift",
        "test_obs_data",
        "test_obs_properties",
        "test_obs_properties_leaks",
        "test_missing_files",
        "test_proc",
        "test_signal",
        "test_format_conversion",
        "test_math_extra",
        "test_matrix3",
        "test_matrix4",
        "test_plane",
        "test_graphics_alias_ordinary",
        "test_graphics_alias_rest",
        "test_config_double_window",
        "test_config_set_double_format",
        "test_hotkey_name_map",
        "test_hotkey_bindings_codec",
        "test_hotkey_pair_steps",
        "test_hotkey_press_steps",
        "test_hotkey_translations",
        "test_shader_param_names",
        "test_utf8_insize0",
        "test_whip_rtp_timestamp",
        "test_whip_host_os_name",
        "test_group_hide_transition_audio",
        "test_vlc_drain_gate",
        "test_vlc_host_path",
        "test_vlc_channel_order",
        "test_vlc_audio_track",
        "test_vlc_audio_pacing",
        "test_vlc_playlist_gap",
        "test_amf",
        "test_amf3_classname",
        "test_amf3_terminator",
        "test_mp4_mux_multichannel",
        "test_mp4_split_time",
        "test_null_output_stop_thread",
        "test_rtmp_dbr_user_bitrate",
        "test_rtmp_send_thread_leak",
        "test_rtmp_error",
        "test_rtmp_chunk_size",
        "test_amf_decode",
        "test_flv_mux",
        "test_scene_audio_action_steps",
        "test_serializer",
        "test_darray",
        "test_bitstream",
        "test_os_path",
        "test_config_save",
        "test_file_serializer_safe",
        "test_quick_write",
        "test_hevc",
        "test_deque",
        "test_platform_time",
        "test_cube_lut",
        "test_pipewire_reconnect",
        "test_pulse_reconnect",
        "test_pulse_default_id",
        "test_scene_blend",
        "test_output_delay",
        "test_output_interleave",
        "test_context_name_helpers",
        "test_video_output_plan",
        "test_audio_buffering_subsample",
        "test_portal_shortcut_ids",
        "test_portal_shortcut_sets",
        "test_hotkey_load",
        "test_svt_av1",
        "test_avc",
        "test_formatted_filename",
        "test_utf8",
        "test_dstr_regressions",
        "test_dstr_libc",
        "test_threading",
        "test_video_stats_race",
        "test_task",
        "test_quat",
        "test_axisang",
        "test_vec2",
        "test_vec3",
        "test_vec4",
    ] {
        assert!(graph.targets.contains_key(name), "missing canonical {name}");
    }
    let facts = std::collections::BTreeMap::from([
        ("OS_LINUX".into(), false),
        ("ENABLE_PULSEAUDIO".into(), false),
        ("MSVC".into(), true),
        ("ENABLE_HEVC".into(), false),
        ("OS_FREEBSD".into(), false),
        ("OS_WINDOWS".into(), true),
        ("OS_MACOS".into(), false),
        ("HAVE_OBS_OPENGL".into(), false),
        ("HAVE_VLC_MODULE".into(), false),
        ("HAVE_OBS_OUTPUTS".into(), false),
        ("HAVE_OBS_FILTERS".into(), false),
        ("HAVE_OBS_PIPEWIRE".into(), false),
        ("HAVE_OBS_PULSEAUDIO".into(), false),
        ("HAVE_OBS_FFMPEG".into(), false),
        ("HAVE_OBS_QSV11".into(), false),
        ("HAVE_QT6_CORE".into(), false),
        ("HAVE_QT6_WIDGETS".into(), false),
        ("HAVE_QT6_TEST".into(), false),
    ]);
    // No suite's size is pinned: every new cmocka test lands in one, and two
    // open PRs that each add one would both bump the same number (#1908,
    // #1997). The partitions must instead cover the whole graph between them.
    let (mut covered_tests, mut covered_targets) = (0, 0);
    for suite in ["core", "graphics", "vlc", "webrtc", "qt"] {
        let mut partition = obs_build::validation::graph::Graph::load(
            &root.join("test/cmocka/build/test-graph.json"),
        )
        .unwrap();
        partition.retain_suite(suite);
        assert!(!partition.tests.is_empty(), "{suite} has no tests");
        assert!(partition.tests.values().all(|test| test.suite == suite));
        assert_eq!(
            partition.targets.len(),
            partition.tests.len(),
            "{suite} targets"
        );
        covered_tests += partition.tests.len();
        covered_targets += partition.targets.len();
        if suite != "core" {
            assert!(
                partition
                    .targets
                    .values()
                    .all(|target| !target.watch_inputs.is_empty())
            );
        }
    }
    assert_eq!(
        covered_tests,
        graph.tests.len(),
        "tests outside every suite"
    );
    assert_eq!(
        covered_targets,
        graph.targets.len(),
        "targets outside every suite"
    );
    let selected = graph.select(&facts).unwrap();
    for name in [
        "test_audio_controls",
        "test_missing_files",
        "test_hotkey_bindings_codec",
        "test_hotkey_translations",
    ] {
        assert_eq!(
            graph.tests[name].optional_prefixes,
            vec![vec!["xvfb-run", "-a"]]
        );
    }
    assert_eq!(
        graph.tests["test_audio_clock_drift"].timeout_seconds,
        Some(300)
    );
    assert_eq!(
        graph.tests["test_config_double_window"].timeout_seconds,
        Some(20)
    );
    assert_eq!(graph.tests["test_svt_av1"].timeout_seconds, Some(60));
    assert_eq!(graph.tests["test_svt_av1"].skip_return_code, Some(77));
    for name in ["test_context_name_helpers", "test_video_output_plan"] {
        assert_eq!(
            graph.targets[name].sanitizers.as_deref(),
            Some("address,undefined")
        );
    }
    assert_eq!(
        graph.tests["test_dstr_regressions"].timeout_seconds,
        Some(60)
    );
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    assert!(
        raw["targets"]["test_avc"]["includes"]
            .as_array()
            .is_some_and(|includes| includes.contains(&serde_json::json!("deps/libcaption"))),
        "AVC's private implementation needs the caption headers from its group PCH"
    );
    for name in ["test_threading", "test_video_stats_race", "test_task"] {
        let libraries = raw["targets"][name]["libraries"].as_array().unwrap();
        assert!(libraries.contains(&serde_json::json!({
            "name": "Threads::Threads", "requires": {"not": "MSVC"}
        })));
    }
    assert_eq!(
        raw["targets"]["test_video_stats_race"]["libraries"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(selected.tests.contains_key("test_serializer"));
    for name in ["test_portal_shortcut_ids", "test_portal_shortcut_sets"] {
        assert!(!selected.tests.contains_key(name));
    }
    let mut freebsd_facts = facts.clone();
    freebsd_facts.insert("OS_FREEBSD".into(), true);
    freebsd_facts.insert("MSVC".into(), false);
    freebsd_facts.insert("OS_WINDOWS".into(), false);
    let freebsd = graph.select(&freebsd_facts).unwrap();
    for name in ["test_portal_shortcut_ids", "test_portal_shortcut_sets"] {
        assert!(freebsd.tests.contains_key(name));
    }
    let plugin_tests = [
        "test_cube_lut",
        "test_pipewire_reconnect",
        "test_pulse_reconnect",
        "test_pulse_default_id",
    ];
    for name in plugin_tests {
        assert!(!selected.tests.contains_key(name));
    }
    let mut plugins_enabled = facts.clone();
    for (fact, value) in [
        ("OS_LINUX", true),
        ("OS_WINDOWS", false),
        ("MSVC", false),
        ("HAVE_OBS_FILTERS", true),
        ("HAVE_OBS_PIPEWIRE", true),
        ("HAVE_OBS_PULSEAUDIO", true),
    ] {
        plugins_enabled.insert(fact.into(), value);
    }
    let enabled = graph.select(&plugins_enabled).unwrap();
    for name in plugin_tests {
        assert!(enabled.tests.contains_key(name));
    }
    assert!(
        !enabled.tests.contains_key("test_pulse_monitor_samples"),
        "plugin availability must not enable core PulseAudio monitoring"
    );
    assert_eq!(
        raw["targets"]["test_pulse_default_id"]["header_libraries"],
        serde_json::json!(["PulseAudio::PulseAudio"])
    );
    let output_tests = [
        "test_rtmp_error",
        "test_rtmp_chunk_size",
        "test_amf_decode",
        "test_flv_mux",
    ];
    for name in output_tests {
        assert!(!selected.tests.contains_key(name));
    }
    let mut with_outputs = facts.clone();
    with_outputs.insert("HAVE_OBS_OUTPUTS".into(), true);
    let output_selection = graph.select(&with_outputs).unwrap();
    for name in output_tests {
        assert!(output_selection.tests.contains_key(name));
    }
    for name in [
        "test_amf",
        "test_amf3_classname",
        "test_amf3_terminator",
        "test_mp4_mux_multichannel",
        "test_mp4_split_time",
    ] {
        assert!(output_selection.tests.contains_key(name));
        assert!(!selected.tests.contains_key(name));
    }
    for name in [
        "test_null_output_stop_thread",
        "test_rtmp_dbr_user_bitrate",
        "test_rtmp_send_thread_leak",
    ] {
        assert!(!output_selection.tests.contains_key(name));
        let mut linux_outputs = with_outputs.clone();
        linux_outputs.insert("OS_LINUX".into(), true);
        linux_outputs.insert("OS_WINDOWS".into(), false);
        assert!(
            graph
                .select(&linux_outputs)
                .unwrap()
                .tests
                .contains_key(name)
        );
    }
    assert_eq!(
        raw["targets"]["test_amf"]["libraries"],
        serde_json::json!(["OBS::native-test-cmocka", {"name":"ws2_32", "requires":"OS_WINDOWS"}])
    );
    for name in ["test_amf3_classname", "test_amf3_terminator"] {
        assert_eq!(graph.targets[name].sanitizers.as_deref(), Some("address"));
    }
    assert_eq!(
        raw["targets"]["test_amf_decode"]["compile_options"][2],
        serde_json::json!({"name": "-fsigned-char", "requires": {"not": "MSVC"}})
    );
    assert_eq!(
        graph.targets["test_scene_audio_action_steps"]
            .sanitizers
            .as_deref(),
        Some("address,undefined")
    );
    assert_eq!(
        raw["targets"]["test_output_protocol_list_steps"]["sources"],
        serde_json::json!([
            "test/cmocka/test_output_protocol_list_steps.c",
            "libobs/obs-output-protocols.c.h"
        ])
    );
    assert_eq!(
        raw["targets"]["test_output_protocol_list_steps"]["libraries"],
        raw["targets"]["test_scene_audio_action_steps"]["libraries"]
    );
    assert_eq!(
        graph.targets["test_output_protocol_list_steps"]
            .sanitizers
            .as_deref(),
        Some("address,undefined")
    );
    assert!(selected.tests.contains_key("test_avc"));
    assert!(!selected.tests.contains_key("test_formatted_filename"));
    assert!(!selected.tests.contains_key("test_utf8"));
    assert!(!selected.tests.contains_key("test_hevc"));
    assert!(!selected.tests.contains_key("test_config_save"));
    assert!(!selected.tests.contains_key("test_pulse_monitor_samples"));
}

#[test]
fn native_test_graph_preserves_sources_and_runtime_requirements() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("test/cmocka/build/test-graph.json");
    let graph: serde_json::Value = serde_json::from_slice(
        &fs::read(&path).expect("CMake and Cargo need a shared native test graph"),
    )
    .unwrap();
    assert_eq!(graph["schema"], 1);
    let targets = graph["targets"].as_object().unwrap();
    let tests = graph["tests"].as_object().unwrap();
    for name in [
        "test_scene_group_transform",
        "test_shader_sampler_convert",
        "test_pulse_monitor_samples",
    ] {
        let sources = targets[name]["sources"].as_array().unwrap();
        assert!(sources.contains(&serde_json::json!(format!("test/cmocka/{name}.c"))));
        assert_eq!(tests[name]["target"], name);
    }
    assert!(
        targets["test_scene_group_transform"]["sources"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("libobs/obs-scene-group-transform.c.h"))
    );
    let pulse = &tests["test_pulse_monitor_samples"];
    assert_eq!(
        pulse["command"],
        serde_json::json!([
            "uv",
            "run",
            "--script",
            "${source}/test/cmocka/test_pulse_monitor_device.py",
            "${target}"
        ])
    );
    assert_eq!(pulse["timeout_seconds"], 60);
    assert_eq!(pulse["skip_return_code"], 77);
    assert_eq!(
        targets["test_pulse_monitor_samples"]["requires"],
        serde_json::json!({"all": ["OS_LINUX", "ENABLE_PULSEAUDIO"]})
    );
}

fn target(name: &str, source: &Path, dependencies: Vec<std::path::PathBuf>) -> SharedLibrary {
    SharedLibrary {
        name: name.into(),
        sources: vec![source.into()],
        include_dirs: vec![],
        dependencies,
        unity: true,
        ..SharedLibrary::default()
    }
}

unsafe fn answer(library: &Path, symbol: &[u8], dependencies: &[&Path]) -> i32 {
    // SAFETY: the fixture below defines this symbol with the C ABI and no args;
    // its owning library stays loaded for the entire call.
    unsafe {
        // Load dependencies explicitly as well: Windows does not search sibling
        // target directories for DLLs. Keep their handles alive through the call.
        let _dependencies: Vec<_> = dependencies
            .iter()
            .map(|path| libloading::Library::new(path).unwrap())
            .collect();
        let lib = libloading::Library::new(library).unwrap();
        let function = lib.get::<unsafe extern "C" fn() -> i32>(symbol).unwrap();
        function()
    }
}

#[test]
fn native_profile_switch_rebuilds_assertion_policy() {
    if native_answer_probe() {
        return;
    }
    let selector = format!(
        "{}::native_profile_switch_rebuilds_assertion_policy",
        module_path!().split_once("::").unwrap().1
    );
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("profile.c");
    fs::write(
        &source,
        format!(
            "{EXPORT} int obs_fixture_profile(void) {{\n#ifdef NDEBUG\nreturn 1;\n#else\nreturn 0;\n#endif\n}}\n"
        ),
    )
    .unwrap();
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("artifacts"),
    };
    use obs_build::NativeProfile::{Debug, MinSizeRel, RelWithDebInfo, Release};
    for (profile, expected) in [
        (Debug, 0),
        (Release, 1),
        (RelWithDebInfo, 1),
        (MinSizeRel, 1),
        (
            obs_build::NativeProfile::from_cargo_lookup(|key| match key {
                "OPT_LEVEL" => Some("3".into()),
                "DEBUG" => Some("true".into()),
                "OBS_SYS_NATIVE_ASSERTIONS" => Some("1".into()),
                _ => None,
            })
            .unwrap(),
            0,
        ),
        (
            obs_build::NativeProfile::from_cargo_lookup(|key| match key {
                "OPT_LEVEL" => Some("0".into()),
                "DEBUG" => Some("false".into()),
                "OBS_SYS_NATIVE_ASSERTIONS" => Some("0".into()),
                _ => None,
            })
            .unwrap(),
            1,
        ),
        (Debug, 0),
    ] {
        let artifact = build
            .with_profile(profile)
            .shared_library(&target("fixture-profile", &source, vec![]))
            .unwrap();
        assert_native_answer_fresh(&artifact, "obs_fixture_profile", expected, &selector);
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "scenario table: one block per ported step"
)]
fn production_runtime_stages_selected_artifacts_and_rejects_stale_outputs() {
    use obs_build::{application, plugins};
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("source");
    let artifacts = temporary.path().join("artifacts");
    fs::create_dir_all(root.join("libobs/data")).unwrap();
    fs::create_dir_all(root.join("frontend/data")).unwrap();
    fs::create_dir_all(artifacts.join("module-data/locale")).unwrap();
    fs::write(root.join("libobs/data/default.effect"), "shader").unwrap();
    fs::write(root.join("frontend/data/theme.qss"), "theme").unwrap();
    fs::write(root.join("AUTHORS"), "authors").unwrap();
    for name in [
        "obs-studio-rust",
        "libobs.so",
        "libobs-frontend-api.so",
        "libobs-opengl.so",
        "fixture-source.so",
        "obs-ffmpeg.so",
        "obs-ffmpeg-mux",
    ] {
        fs::write(artifacts.join(name), name).unwrap();
    }
    fs::write(artifacts.join("module-data/locale/en-US.ini"), "translated").unwrap();
    let module = plugins::Module {
        library: artifacts.join("fixture-source.so"),
        data: artifacts.join("module-data"),
    };
    let ffmpeg = plugins::FfmpegArtifacts {
        module: plugins::Module {
            library: artifacts.join("obs-ffmpeg.so"),
            data: artifacts.join("module-data"),
        },
        muxer: artifacts.join("obs-ffmpeg-mux"),
    };
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: "x86_64-unknown-linux-gnu".into(),
        output: artifacts.clone(),
        profile: Default::default(),
    };
    let executable = artifacts.join("obs-studio-rust");
    let core = artifacts.join("libobs.so");
    let api = artifacts.join("libobs-frontend-api.so");
    let renderer = artifacts.join("libobs-opengl.so");
    let inputs = application::RuntimeInputs {
        root: &root,
        executable: &executable,
        core: &core,
        api: &api,
        renderer: &renderer,
        ffmpeg: &ffmpeg,
        nvenc_helper: None,
        modules: &[&module, &ffmpeg.module],
        browser: None,
        scripting: None,
    };
    let destination = temporary.path().join("runtime");
    let runtime = application::stage_runtime(&build, &inputs, &destination).unwrap();
    assert_eq!(
        fs::read_to_string(&runtime.executable).unwrap(),
        "obs-studio-rust"
    );
    assert_eq!(runtime.modules, ["fixture-source", "obs-ffmpeg"]);
    for (path, expected) in [
        ("lib64/libobs.so", "libobs.so"),
        ("bin/obs-ffmpeg-mux", "obs-ffmpeg-mux"),
        (
            "lib64/obs-modules/core/fixture-source.so",
            "fixture-source.so",
        ),
        (
            "share/obs/obs-modules/core/fixture-source/locale/en-US.ini",
            "translated",
        ),
        ("share/obs/libobs/default.effect", "shader"),
        ("share/obs/obs-studio/theme.qss", "theme"),
        ("share/obs/obs-studio/authors", "authors"),
    ] {
        assert_eq!(
            fs::read_to_string(destination.join(path)).unwrap(),
            expected
        );
    }
    fs::write(destination.join("lib64/obs-modules/core/stale.so"), "stale").unwrap();
    let error = application::stage_runtime(&build, &inputs, &destination)
        .err()
        .unwrap();
    assert!(error.contains("empty"), "{error}");
    assert_eq!(
        fs::read_to_string(destination.join("lib64/obs-modules/core/stale.so")).unwrap(),
        "stale"
    );
    let unsupported = NativeBuild {
        target: "x86_64-pc-windows-msvc".into(),
        ..build.clone()
    };
    let rejected = temporary.path().join("unsupported");
    assert!(application::stage_runtime(&unsupported, &inputs, &rejected).is_err());
    assert!(!rejected.exists());

    let nvenc_library = artifacts.join("obs-nvenc.so");
    let nvenc_helper = artifacts.join("obs-nvenc-test");
    fs::write(&nvenc_library, "selected Rust NVENC module").unwrap();
    fs::write(&nvenc_helper, "selected capability helper").unwrap();
    let nvenc_data = module.data.clone();
    let production = application::Artifacts {
        core,
        api,
        renderer,
        executable,
        modules: vec![module],
        ffmpeg,
        scripting: None,
        browser: None,
        nvenc: Some(plugins::NvencProductionArtifacts {
            module: plugins::Module {
                library: nvenc_library,
                data: nvenc_data,
            },
            helper: nvenc_helper,
        }),
    };
    let destination = temporary.path().join("production-runtime");
    let runtime = production.stage(&build, &root, &destination).unwrap();
    assert_eq!(
        runtime.modules,
        ["fixture-source", "obs-ffmpeg", "obs-nvenc"]
    );
    assert_eq!(
        fs::read_to_string(destination.join("bin/obs-nvenc-test")).unwrap(),
        "selected capability helper"
    );
    assert_eq!(
        fs::read_to_string(destination.join("lib64/obs-modules/core/obs-nvenc.so")).unwrap(),
        "selected Rust NVENC module"
    );
}

#[test]
fn production_nvenc_defaults_and_explicit_selection_match_cmake() {
    use obs_build::{application::BuildOptions, cli};
    let options = BuildOptions::for_target("x86_64-unknown-linux-gnu").unwrap();
    assert!(
        options
            .core_module_names()
            .iter()
            .any(|name| name == "obs-nvenc"),
        "the production graph must include CMake's default-enabled NVENC module"
    );
    assert!(options.nvenc.as_ref().unwrap().ffmpeg_ids);
    for arguments in [
        vec![
            "build",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--without-nvenc",
        ],
        vec![
            "build",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--without-nvenc-ffmpeg-ids",
        ],
    ] {
        let cli::Invocation::Build(request) = cli::parse(arguments.clone()).unwrap() else {
            panic!("expected a production build request");
        };
        let enabled = !arguments.contains(&"--without-nvenc");
        assert_eq!(request.options.nvenc.is_some(), enabled);
        if let Some(nvenc) = &request.options.nvenc {
            assert_eq!(
                nvenc.ffmpeg_ids,
                !arguments.contains(&"--without-nvenc-ffmpeg-ids")
            );
        }
        assert_eq!(
            request
                .options
                .core_module_names()
                .iter()
                .any(|name| name == "obs-nvenc"),
            enabled
        );
    }
}

#[test]
fn bundled_frontend_plugins_follow_application_feature_selection() {
    let mut options =
        obs_build::application::BuildOptions::for_target("x86_64-unknown-linux-gnu").unwrap();
    for (tools, websocket) in [(true, true), (true, false), (false, true), (false, false)] {
        options.frontend_tools = tools;
        options.websocket = websocket.then(Default::default);
        let names = options.core_module_names();
        assert_eq!(names.iter().any(|name| name == "frontend-tools"), tools);
        assert_eq!(names.iter().any(|name| name == "obs-websocket"), websocket);
    }
}

#[test]
fn production_archive_command_uses_the_native_rust_cache_safeguard() {
    if !obs_build::BUILD_TARGET.contains("linux") {
        return;
    }
    if let Some(output) = std::env::var_os("OBS_NATIVE_CACHE_PROBE_OUTPUT") {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut arguments = vec![
            std::ffi::OsString::from("build"),
            "--root".into(),
            root.into_os_string(),
            "--output".into(),
            output,
        ];
        if std::env::var_os("OBS_NATIVE_CACHE_WITHOUT_NVENC").is_some() {
            arguments.push("--without-nvenc".into());
        }
        let result = obs_build::cli::run(arguments);
        assert!(result.unwrap_err().contains("Rust archive build failed"));
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let soldr = directory.path().join("soldr");
    fs::write(&soldr, "#!/bin/sh\nprintf '%s\\n' \"${SOLDR_RUSTC_WRAPPER:-unset}\" \"$@\" > \"$OBS_NATIVE_CACHE_LOG\"\nexit 19\n").unwrap();
    assert!(
        std::process::Command::new("chmod")
            .arg("+x")
            .arg(&soldr)
            .status()
            .unwrap()
            .success()
    );
    let marker = directory.path().join("wrapper");
    let path = std::env::join_paths(
        std::iter::once(directory.path().to_path_buf())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let selector = format!(
        "{}::production_archive_command_uses_the_native_rust_cache_safeguard",
        module_path!().split_once("::").unwrap().1
    );
    for nvenc in [true, false] {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", &selector, "--nocapture"])
            .env("PATH", &path)
            .env_remove("SOLDR_RUSTC_WRAPPER")
            .env("OBS_NATIVE_CACHE_LOG", &marker)
            .env(
                "OBS_NATIVE_CACHE_PROBE_OUTPUT",
                directory.path().join("output"),
            );
        if nvenc {
            command.env_remove("OBS_NATIVE_CACHE_WITHOUT_NVENC");
        } else {
            command.env("OBS_NATIVE_CACHE_WITHOUT_NVENC", "1");
        }
        assert!(command.status().unwrap().success());
        let invocation = fs::read_to_string(&marker).unwrap();
        let mut lines = invocation.lines();
        assert_eq!(lines.next(), Some("none"));
        let arguments: Vec<_> = lines.collect();
        assert_eq!(
            arguments
                .windows(2)
                .any(|pair| pair == ["-p", "obs-nvenc-plugin"]),
            nvenc
        );
    }
}

#[test]
fn production_command_selects_explicit_profile_target_and_features() {
    use obs_build::{NativeProfile, cli};
    let invocation = cli::parse([
        "build",
        "--root",
        "source tree",
        "--output",
        "build output",
        "--target",
        "x86_64-unknown-linux-gnu",
        "--profile",
        "release",
        "--separate-core",
        "--core-pch",
        "--disable-service-updates",
        "--without-websocket",
        "--without-scripting",
        "--with-vlc",
    ])
    .unwrap();
    let cli::Invocation::Build(request) = invocation else {
        panic!("build request was lost")
    };
    assert_eq!(request.root, std::path::PathBuf::from("source tree"));
    assert_eq!(request.output, std::path::PathBuf::from("build output"));
    assert_eq!(request.target, "x86_64-unknown-linux-gnu");
    assert_eq!(request.profile, NativeProfile::Release);
    assert!(!request.options.core.unity);
    assert!(request.options.core.pch);
    assert!(request.options.websocket.is_none());
    assert!(request.options.scripting.is_none());
    assert!(
        request
            .options
            .sources
            .iter()
            .any(|source| matches!(source, obs_build::plugins::SourceModule::Vlc))
    );
    assert!(request.options.sources.iter().any(|source| matches!(
        source,
        obs_build::plugins::SourceModule::RtmpServices { updates: false }
    )));
    for arguments in [
        vec!["build", "--profile", "fast"],
        vec!["build", "--target", "x86_64-pc-windows-msvc"],
        vec!["build", "--unrecognized"],
        vec!["build", "--output"],
        vec!["build", "--profile", "debug", "--profile", "release"],
    ] {
        assert!(cli::parse(arguments).is_err());
    }
    assert!(matches!(
        cli::parse(["--help"]).unwrap(),
        cli::Invocation::Help
    ));
}

#[test]
fn swift_bridge_preserves_native_linkage_header_flags_and_source_rebuilds() {
    if native_answer_probe() {
        return;
    }
    if !obs_build::BUILD_TARGET.contains("apple-darwin") {
        return;
    }
    let selector = format!(
        "{}::swift_bridge_preserves_native_linkage_header_flags_and_source_rebuilds",
        module_path!().split_once("::").unwrap().1
    );
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: root.join("artifacts"),
    };
    let base = root.join("base.c");
    fs::write(&base, "int base_value(void) { return 1; }\n").unwrap();
    let native = build
        .shared_library(&target("fixture-swift-base", &base, vec![]))
        .unwrap();
    let header = root.join("value.h");
    fs::write(&header, "#define HEADER_VALUE 38\n").unwrap();
    let bridge = root.join("bridge.h");
    fs::write(&bridge, "#include \"value.h\"\nstatic const int bridge_value = HEADER_VALUE;\nstatic const int build_flag = BUILD_FLAG;\nint base_value(void);\n").unwrap();
    let source = root.join("fixture.swift");
    let text = "@_cdecl(\"obs_fixture_swift\")\npublic func obs_fixture_swift() -> Int32 { return base_value() + bridge_value + build_flag }\n@_cdecl(\"obs_fixture_swift_assertions\")\npublic func obs_fixture_swift_assertions() -> Int32 { return _isDebugAssertConfiguration() ? 1 : 0 }\n";
    fs::write(&source, text).unwrap();
    let mut library = obs_build::SwiftLibrary {
        name: "fixture-swift".into(),
        module_name: "NativeSwiftFixture".into(),
        file_name: "libfixture-swift.dylib".into(),
        sources: vec![source.clone()],
        bridging_header: bridge,
        input_files: vec![header.clone()],
        include_dirs: vec![root.into()],
        definitions: vec![("BUILD_FLAG".into(), Some("3".into()))],
        dependencies: vec![native],
        deployment_target: "13.0".into(),
        compile_options: vec!["-swift-version".into(), "6".into()],
        link_options: vec![],
    };
    let artifact = build.swift_shared_library(&library).unwrap();
    assert!(
        artifact
            .parent()
            .unwrap()
            .join("NativeSwiftFixture.swiftmodule")
            .is_file()
    );
    assert!(
        artifact
            .parent()
            .unwrap()
            .join("NativeSwiftFixture-Swift.h")
            .is_file()
    );
    assert_native_answer_fresh(&artifact, "obs_fixture_swift", 42, &selector);
    fs::write(&header, "#define HEADER_VALUE 48\n").unwrap();
    let artifact = build.swift_shared_library(&library).unwrap();
    assert_native_answer_fresh(&artifact, "obs_fixture_swift", 52, &selector);
    library.definitions[0].1 = Some("5".into());
    let artifact = build.swift_shared_library(&library).unwrap();
    assert_native_answer_fresh(&artifact, "obs_fixture_swift", 54, &selector);
    fs::write(&source, text.replace("+ build_flag", "+ build_flag + 2")).unwrap();
    let artifact = build.swift_shared_library(&library).unwrap();
    assert_native_answer_fresh(&artifact, "obs_fixture_swift", 56, &selector);
    fs::write(&base, "int base_value(void) { return 3; }\n").unwrap();
    build
        .shared_library(&target("fixture-swift-base", &base, vec![]))
        .unwrap();
    // The unchanged Swift dylib must observe the rebuilt shared dependency.
    assert_native_answer_fresh(&artifact, "obs_fixture_swift", 58, &selector);
    for (optimization, assertions, expected) in [("3", "1", 1), ("0", "0", 0)] {
        let profile = obs_build::NativeProfile::from_cargo_lookup(|key| match key {
            "OPT_LEVEL" => Some(optimization.into()),
            "DEBUG" => Some("true".into()),
            "OBS_SYS_NATIVE_ASSERTIONS" => Some(assertions.into()),
            _ => None,
        })
        .unwrap();
        let artifact = build
            .with_profile(profile)
            .swift_shared_library(&library)
            .unwrap();
        assert_native_answer_fresh(
            &artifact,
            "obs_fixture_swift_assertions",
            expected,
            &selector,
        );
    }
}

#[test]
fn native_plugin_links_shared_core_and_observes_rebuilt_dependency() {
    let dir = tempfile::tempdir().unwrap();
    let core_source = dir.path().join("core.c");
    let plugin_source = dir.path().join("plugin.c");
    fs::write(
        &core_source,
        "__declspec_placeholder__ int obs_fixture_answer(void) { return 42; }"
            .replace("__declspec_placeholder__", EXPORT),
    )
    .unwrap();
    fs::write(&plugin_source, format!("extern int obs_fixture_answer(void); {EXPORT} int obs_fixture_plugin(void) {{ return obs_fixture_answer() + 1; }}")).unwrap();
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: dir.path().join("artifacts"),
    };
    let core = build
        .shared_library(&target("fixture-core", &core_source, vec![]))
        .unwrap();
    let mut plugin_target = target("fixture-plugin", &plugin_source, vec![core.clone()]);
    plugin_target.kind = obs_build::LibraryKind::Module;
    let plugin = build.shared_library(&plugin_target).unwrap();
    if obs_build::BUILD_TARGET.contains("apple-darwin") {
        let bytes = fs::read(&plugin).unwrap();
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 8);
        let bytes = fs::read(&core).unwrap();
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 6);
    }
    // SAFETY: both sources above define the expected no-argument C functions.
    assert_eq!(
        unsafe { answer(&plugin, b"obs_fixture_plugin", &[&core]) },
        43
    );
    fs::write(
        &core_source,
        format!("{EXPORT} int obs_fixture_answer(void) {{ return 84; }}"),
    )
    .unwrap();
    build
        .shared_library(&target("fixture-core", &core_source, vec![]))
        .unwrap();
    // SAFETY: the rebuilt core preserves the function ABI; it changes its result.
    assert_eq!(
        unsafe { answer(&plugin, b"obs_fixture_plugin", &[&core]) },
        85
    );
}

#[test]
fn unity_toggle_preserves_exports_and_explicit_generated_source_wins() {
    let dir = tempfile::tempdir().unwrap();
    let source_dir = dir.path().join("source");
    let generated_dir = dir.path().join("generated");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        source_dir.join("manifest.c"),
        "#error stale source tree manifest selected\n",
    )
    .unwrap();
    let generated = generated_dir.join("manifest.c");
    fs::write(
        &generated,
        format!("{EXPORT} int obs_fixture_count(void) {{ return 17; }}"),
    )
    .unwrap();
    let extra = source_dir.join("extra.c");
    fs::write(&extra, "int obs_fixture_other(void) { return 23; }").unwrap();
    for unity in [false, true] {
        let build = NativeBuild {
            profile: Default::default(),
            host: obs_build::BUILD_HOST.into(),
            target: obs_build::BUILD_TARGET.into(),
            output: dir.path().join(format!("output-{unity}")),
        };
        let mut library = target("fixture-manifest", &generated, vec![]);
        library.sources.push(extra.clone());
        library.include_dirs.push(source_dir.clone());
        library.unity = unity;
        let artifact = build.shared_library(&library).unwrap();
        // SAFETY: the generated fixture defines this no-argument C function.
        assert_eq!(unsafe { answer(&artifact, b"obs_fixture_count", &[]) }, 17);
    }
}

#[test]
fn executable_runs_against_its_shared_core_dependency() {
    let dir = tempfile::tempdir().unwrap();
    let core_source = dir.path().join("core.c");
    let executable_source = dir.path().join("main.c");
    let helper_source = dir.path().join("helper.c");
    fs::write(
        &helper_source,
        "int fixture_private_value(void) { return 42; }",
    )
    .unwrap();
    fs::write(
        &core_source,
        format!("extern int fixture_private_value(void); {EXPORT} int fixture_value(void) {{ return fixture_private_value(); }}"),
    )
    .unwrap();
    fs::write(
        &executable_source,
        "extern int fixture_value(void); int main(void) { return fixture_value() == 42 ? 0 : 7; }",
    )
    .unwrap();
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: dir.path().join("artifacts"),
    };
    let helper = build
        .static_library(&target("fixture-helper", &helper_source, vec![]))
        .unwrap();
    let core = build
        .shared_library(&target("fixture-core", &core_source, vec![helper]))
        .unwrap();
    let executable = build
        .executable(&target(
            "fixture-app",
            &executable_source,
            vec![core.clone()],
        ))
        .unwrap();
    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let search = std::env::join_paths(
        std::iter::once(core.parent().unwrap().to_path_buf())
            .chain(std::env::split_paths(&old_path)),
    )
    .unwrap();
    assert!(
        std::process::Command::new(executable)
            .env("PATH", search)
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn staged_executable_prefers_runtime_core_over_build_outputs() {
    if !obs_build::BUILD_TARGET.contains("linux") {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("core.c");
    let main = directory.path().join("main.c");
    fs::write(
        &main,
        "extern int fixture_value(void); int main(void) { return fixture_value() == 42 ? 0 : 7; }",
    )
    .unwrap();
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("original"),
    };
    fs::write(&source, "int fixture_value(void) { return 1; }").unwrap();
    let original = build
        .shared_library(&target("fixture-core", &source, vec![]))
        .unwrap();
    let executable = build
        .executable(&target("fixture-app", &main, vec![original.clone()]))
        .unwrap();
    fs::write(&source, "int fixture_value(void) { return 42; }").unwrap();
    let runtime_build = NativeBuild {
        output: directory.path().join("replacement"),
        ..build.clone()
    };
    let replacement = runtime_build
        .shared_library(&target("fixture-core", &source, vec![]))
        .unwrap();
    let runtime = directory.path().join("runtime");
    fs::create_dir_all(runtime.join("bin")).unwrap();
    fs::create_dir_all(runtime.join("lib64")).unwrap();
    let staged = runtime.join("bin/fixture-app");
    fs::copy(&executable, &staged).unwrap();
    fs::copy(
        &replacement,
        runtime.join("lib64").join(replacement.file_name().unwrap()),
    )
    .unwrap();
    let status = std::process::Command::new(&staged)
        .env_remove("LD_LIBRARY_PATH")
        .status()
        .unwrap();
    assert!(
        status.success(),
        "staged executable loaded the original core: {status}"
    );
    // A separately loaded module must resolve its own dependency from the
    // staged lib64 directory, without the executable preloading that core.
    let plugin_source = directory.path().join("plugin.c");
    fs::write(
        &plugin_source,
        "extern int fixture_value(void); int plugin_value(void) { return fixture_value(); }",
    )
    .unwrap();
    let plugin = build
        .shared_library(&target("fixture-plugin", &plugin_source, vec![original]))
        .unwrap();
    let modules = runtime.join("lib64/obs-modules/core");
    fs::create_dir_all(&modules).unwrap();
    let staged_plugin = modules.join(plugin.file_name().unwrap());
    fs::copy(plugin, &staged_plugin).unwrap();
    let loader_source = directory.path().join("loader.c");
    fs::write(&loader_source, "#include <dlfcn.h>\nint main(int argc, char **argv) { if (argc != 2) return 2; void *lib = dlopen(argv[1], RTLD_NOW); if (!lib) return 3; int (*value)(void) = dlsym(lib, \"plugin_value\"); return value && value() == 42 ? 0 : 7; }").unwrap();
    let mut loader = target("fixture-loader", &loader_source, vec![]);
    loader.link_options.push("-ldl".into());
    let loader = build.executable(&loader).unwrap();
    let output = std::process::Command::new(loader)
        .arg(staged_plugin)
        .env_remove("LD_LIBRARY_PATH")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "staged module loaded the original core: {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

const EXPORT: &str = "\n#if defined(_WIN32)\n__declspec(dllexport)\n#endif\n";

#[test]
fn windows_resource_payload_tracks_headers_definitions_and_unity() {
    let requested = std::env::var("OBS_NATIVE_WINDOWS_TARGET")
        .unwrap_or_else(|_| obs_build::BUILD_TARGET.into());
    if !requested.contains("windows-msvc") {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let source = root.join("resource-probe.c");
    let script = root.join("payload.rc");
    let header = root.join("resource-values.h");
    fs::write(
        &source,
        r#"
#include <windows.h>
#include "resource-values.h"
__declspec(dllexport) int obs_fixture_resource(void) {
    HMODULE owner = NULL;
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
            GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            (LPCWSTR)&obs_fixture_resource, &owner)) return -1;
    HRSRC resource = FindResourceW(owner, MAKEINTRESOURCEW(17), MAKEINTRESOURCEW(10));
    if (!resource) return -2;
    HGLOBAL loaded = LoadResource(owner, resource);
    const WORD *value = loaded ? LockResource(loaded) : NULL;
    return value ? value[0] + value[1] : -3;
}
int main(void) { return obs_fixture_resource() == HEADER_VALUE + RESOURCE_VALUE ? 0 : 7; }
"#,
    )
    .unwrap();
    fs::write(
        &script,
        "#include \"resource-values.h\"\n17 RCDATA { HEADER_VALUE, RESOURCE_VALUE }\n",
    )
    .unwrap();
    for unity in [false, true] {
        fs::write(&header, "#define HEADER_VALUE 2\n").unwrap();
        let build = NativeBuild {
            profile: Default::default(),
            host: obs_build::BUILD_HOST.into(),
            target: requested.clone(),
            output: root.join(format!("resources-{unity}")),
        };
        let mut library = target("fixture-resources", &source, vec![]);
        library.unity = unity;
        library.resource_files.push(script.clone());
        library.resource_input_files.push(header.clone());
        library.include_dirs.push(root.into());
        library
            .definitions
            .push(("RESOURCE_VALUE".into(), Some("40".into())));
        // RC must receive resource options, not C-only compiler options.
        library.c_compile_options.push("/std:c17".into());
        library.resource_compile_options.push("/c65001".into());
        let artifact = build.shared_library(&library).unwrap();
        assert_windows_resource(&artifact, 42, [2, 40], &requested);
        fs::write(&header, "#define HEADER_VALUE 10\n").unwrap();
        let artifact = build.shared_library(&library).unwrap();
        assert_windows_resource(&artifact, 50, [10, 40], &requested);
        library.definitions[0].1 = Some("50".into());
        let artifact = build.shared_library(&library).unwrap();
        assert_windows_resource(&artifact, 60, [10, 50], &requested);
        library.name = "fixture-resource-app".into();
        let executable = build.executable(&library).unwrap();
        assert_windows_resource(&executable, 60, [10, 50], &requested);
        assert!(
            build
                .static_library(&library)
                .unwrap_err()
                .contains("resources")
        );
    }
}

fn assert_windows_resource(library: &Path, expected: i32, words: [u16; 2], target: &str) {
    if target == obs_build::BUILD_TARGET {
        if library
            .extension()
            .is_some_and(|extension| extension == "exe")
        {
            assert!(
                std::process::Command::new(library)
                    .status()
                    .unwrap()
                    .success()
            );
            return;
        }
        // SAFETY: the fixture exports this no-argument C ABI function, and the
        // resource lookup does not retain an extra reference to the DLL.
        assert_eq!(
            unsafe { answer(library, b"obs_fixture_resource", &[]) },
            expected
        );
    } else {
        // Cross compilation proves the PE payload; native CI also calls the
        // exported lookup above against the loaded DLL.
        let result = std::process::Command::new(
            std::env::var_os("OBS_NATIVE_WINDOWS_READOBJ").unwrap_or_else(|| "llvm-readobj".into()),
        )
        .arg("--coff-resources")
        .arg(library)
        .output()
        .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let output = String::from_utf8(result.stdout).unwrap();
        let payload = words
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .map(|byte| format!("{byte:02X}"))
            .collect::<String>();
        assert!(output.contains("Total Number of Resources: 1"), "{output}");
        assert!(output.contains("Name: (ID 17)"), "{output}");
        assert!(output.contains("DataSize: 4"), "{output}");
        assert!(
            output
                .lines()
                .any(|line| line.split_whitespace().nth(1) == Some(&payload)),
            "{output}"
        );
    }
}

#[test]
fn msvc_pch_links_its_object_and_preserves_language_header_and_flag_changes() {
    let requested = std::env::var("OBS_NATIVE_WINDOWS_TARGET")
        .unwrap_or_else(|_| obs_build::BUILD_TARGET.into());
    if !requested.contains("windows-msvc") {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let header = root.join("common.h");
    let c = root.join("part.c");
    let cpp = root.join("part.cpp");
    let wrapper = root.join("archive-probe.cpp");
    fs::write(&wrapper, "extern \"C\" int obs_fixture_msvc_pch(void);\nextern \"C\" __declspec(dllexport) int obs_fixture_archive_probe(void) { return obs_fixture_msvc_pch(); }\n").unwrap();
    let write_header = |value| {
        fs::write(
            &header,
            format!(
                r#"
#pragma once
#define HEADER_VALUE {value}
#ifdef __cplusplus
#include <vector>
#ifdef FIXTURE_C
#error C flags leaked into C++ PCH
#endif
__declspec(selectany) int fixture_cpp_storage = HEADER_VALUE;
#else
#include <stdint.h>
#ifdef FIXTURE_CPP
#error C++ flags leaked into C PCH
#endif
__declspec(selectany) int fixture_c_storage = HEADER_VALUE;
#endif
"#
            ),
        )
        .unwrap()
    };
    fs::write(&c, "#include \"common.h\"\nint fixture_pch_c(void) { return fixture_c_storage + FIXTURE_C; }\n").unwrap();
    fs::write(&cpp, "#include \"common.h\"\nextern \"C\" int fixture_pch_c(void);\nextern \"C\" __declspec(dllexport) int obs_fixture_msvc_pch(void) { std::vector<int> values{fixture_cpp_storage, FIXTURE_CPP}; return fixture_pch_c() + values[0] + values[1]; }\n").unwrap();
    for unity in [false, true] {
        write_header(17);
        let build = NativeBuild {
            profile: Default::default(),
            host: obs_build::BUILD_HOST.into(),
            target: requested.clone(),
            output: root.join(format!("msvc-pch-{unity}")),
        };
        let mut library = target("fixture-msvc-pch", &c, vec![]);
        library.sources.push(cpp.clone());
        library.include_dirs.push(root.into());
        library.unity = unity;
        library.c_compile_options = vec!["/std:c17".into(), "/DFIXTURE_C=3".into()];
        library.cpp_compile_options = vec!["/std:c++17".into(), "/DFIXTURE_CPP=5".into()];
        library
            .precompiled_headers
            .push(obs_build::PrecompiledHeader {
                header: header.clone(),
                sources: vec![c.clone(), cpp.clone()],
            });
        for (value, c_flag, cpp_flag, expected) in [
            (17, 3, 5, 42),
            (34, 3, 5, 76),
            (34, 6, 5, 79),
            (34, 6, 10, 84),
        ] {
            write_header(value);
            library.c_compile_options[1] = format!("/DFIXTURE_C={c_flag}");
            library.cpp_compile_options[1] = format!("/DFIXTURE_CPP={cpp_flag}");
            let artifact = build.shared_library(&library).unwrap();
            assert_windows_function(&artifact, "obs_fixture_msvc_pch", expected, &requested);
        }
        let archive = build.static_library(&library).unwrap();
        let artifact = build
            .shared_library(&target("fixture-archive-pch", &wrapper, vec![archive]))
            .unwrap();
        assert_windows_function(&artifact, "obs_fixture_archive_probe", 84, &requested);
        let pch: Vec<_> = fs::read_dir(build.output.join("pch"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(
            pch.len(),
            4,
            "PCH identity must distinguish C/C++ and their changed flags"
        );
        for directory in pch {
            assert!(directory.join("prefix.pch").metadata().unwrap().len() > 0);
            assert!(
                directory.join("prefix.obj").is_file(),
                "MSVC PCH creation also emits a required object"
            );
        }
    }
}

fn assert_windows_function(library: &Path, symbol: &str, expected: i32, target: &str) {
    if target == obs_build::BUILD_TARGET {
        // SAFETY: both owning fixtures define no-argument C ABI functions;
        // answer keeps each library loaded throughout the call.
        assert_eq!(unsafe { answer(library, symbol.as_bytes(), &[]) }, expected);
    } else {
        let result = std::process::Command::new(
            std::env::var_os("OBS_NATIVE_WINDOWS_READOBJ").unwrap_or_else(|| "llvm-readobj".into()),
        )
        .arg("--coff-exports")
        .arg(library)
        .output()
        .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let name = format!("Name: {symbol}");
        assert!(
            String::from_utf8_lossy(&result.stdout)
                .lines()
                .any(|line| line.trim() == name)
        );
    }
}

#[test]
fn pch_preserves_mixed_languages_unity_and_header_flag_invalidation() {
    if !obs_build::BUILD_TARGET.contains("linux") && !obs_build::BUILD_TARGET.contains("apple") {
        eprintln!("compiled PCH integration covers Linux GCC/Clang and Apple Clang");
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let header = root.join("common.h");
    let transitive = root.join("value.h");
    let write_header = |value| {
        fs::write(&header, format!("#pragma once\n#include \"value.h\"\n#define HEADER_VALUE {value}\n#ifdef __cplusplus\n#include <vector>\n#else\n#include <stdint.h>\n#endif\n")).unwrap()
    };
    let c = root.join("part.c");
    let cpp = root.join("part.cpp");
    fs::write(&c, "#include \"common.h\"\n#ifdef FIXTURE_CPP\n#error C++ flags leaked into C\n#endif\nint fixture_c(void) { return HEADER_VALUE + FIXTURE_C + TRANSITIVE_VALUE; }\n").unwrap();
    fs::write(&cpp, "#include \"common.h\"\n#ifdef FIXTURE_C\n#error C flags leaked into C++\n#endif\nextern \"C\" int fixture_c(void);\nextern \"C\" int obs_fixture_pch(void) { std::vector<int> values{HEADER_VALUE + TRANSITIVE_VALUE, FIXTURE_CPP}; return fixture_c() + values[0] + values[1]; }\n").unwrap();
    for unity in [false, true] {
        write_header(17);
        fs::write(&transitive, "#define TRANSITIVE_VALUE 0\n").unwrap();
        let build = NativeBuild {
            profile: Default::default(),
            host: obs_build::BUILD_HOST.into(),
            target: obs_build::BUILD_TARGET.into(),
            output: root.join(format!("output-{unity}")),
        };
        let mut library = target("fixture-pch", &c, vec![]);
        library.sources.push(cpp.clone());
        library.include_dirs.push(root.into());
        library.unity = unity;
        library.c_compile_options.push("-DFIXTURE_C=3".into());
        library.cpp_compile_options.push("-DFIXTURE_CPP=5".into());
        library
            .precompiled_headers
            .push(obs_build::PrecompiledHeader {
                header: header.clone(),
                sources: vec![c.clone(), cpp.clone()],
            });
        let artifact = build.shared_library(&library).unwrap();
        // SAFETY: the fixture defines this no-argument C ABI function.
        assert_eq!(unsafe { answer(&artifact, b"obs_fixture_pch", &[]) }, 42);
        write_header(37);
        let artifact = build.shared_library(&library).unwrap();
        // SAFETY: the edited header preserves the function ABI.
        assert_eq!(unsafe { answer(&artifact, b"obs_fixture_pch", &[]) }, 82);
        fs::write(&transitive, "#define TRANSITIVE_VALUE 11\n").unwrap();
        let artifact = build.shared_library(&library).unwrap();
        // SAFETY: the edited transitive header preserves the function ABI.
        assert_eq!(unsafe { answer(&artifact, b"obs_fixture_pch", &[]) }, 104);
        library.c_compile_options = vec!["-DFIXTURE_C=7".into()];
        let artifact = build.shared_library(&library).unwrap();
        // SAFETY: changing the compiler macro preserves the function ABI.
        assert_eq!(unsafe { answer(&artifact, b"obs_fixture_pch", &[]) }, 108);
    }
}

#[test]
fn explicit_groups_compile_as_individual_sources_and_observe_member_edits() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::create_dir(root.join("build")).unwrap();
    fs::write(root.join("build/fixture.pch.h"), "#define VALUE 17\n").unwrap();
    let first = root.join("first.c.h");
    fs::write(
        &first,
        "#include \"build/fixture.pch.h\"\nint first(void) { return VALUE; }\n",
    )
    .unwrap();
    fs::write(root.join("second.c.h"), format!("#include \"build/fixture.pch.h\"\nextern int first(void); {EXPORT} int obs_fixture_separate(void) {{ return first() + VALUE; }}\n")).unwrap();
    let group = root.join("build/fixture+.h.c");
    fs::write(&group, "/* explicit group */\n#include \"build/fixture.pch.h\"\n#include \"first.c.h\"\n#include \"second.c.h\"\n").unwrap();
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: root.join("artifacts"),
    };
    let mut library = target("fixture-separate", &group, vec![]);
    library.unity = false;
    library.include_dirs.push(root.into());
    library.sources = build
        .separate_group_sources(root, &library.sources)
        .unwrap();
    assert_eq!(library.sources.len(), 2);
    assert!(!library.sources.contains(&group));
    let artifact = build.shared_library(&library).unwrap();
    // SAFETY: the fixture defines this no-argument C function.
    assert_eq!(
        unsafe { answer(&artifact, b"obs_fixture_separate", &[]) },
        34
    );
    fs::write(
        &first,
        "#include \"build/fixture.pch.h\"\nint first(void) { return VALUE + 1; }\n",
    )
    .unwrap();
    fs::write(root.join("build/fixture.pch.h"), "#define VALUE 21\n").unwrap();
    let artifact = build.shared_library(&library).unwrap();
    // SAFETY: the rebuilt fixture preserves its function ABI.
    assert_eq!(
        unsafe { answer(&artifact, b"obs_fixture_separate", &[]) },
        43
    );
}

#[test]
fn individual_compilation_rejects_conditional_group_selection() {
    let directory = tempfile::tempdir().unwrap();
    let group = directory.path().join("conditional+.h.c");
    fs::write(
        &group,
        "#if ENABLE_OPTION\n#include \"optional.c.h\"\n#endif\n",
    )
    .unwrap();
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("artifacts"),
    };
    let error = build
        .separate_group_sources(directory.path(), &[group])
        .unwrap_err();
    assert!(error.contains("non-include group line"), "{error}");
}

#[test]
fn deploying_mux_helper_in_place_preserves_its_contents() {
    let directory = tempfile::tempdir().unwrap();
    let helper = directory.path().join("obs-ffmpeg-mux");
    std::fs::write(&helper, b"existing executable").unwrap();
    let artifacts = obs_build::plugins::FfmpegArtifacts {
        module: obs_build::plugins::Module {
            library: std::path::PathBuf::new(),
            data: std::path::PathBuf::new(),
        },
        muxer: helper.clone(),
    };
    assert_eq!(artifacts.deploy_muxer(directory.path()).unwrap(), helper);
    assert_eq!(std::fs::read(helper).unwrap(), b"existing executable");
}

#[test]
fn mixed_c_and_cpp_use_independent_flags_and_the_cpp_linker() {
    let directory = tempfile::tempdir().unwrap();
    let c_source = directory.path().join("helper.c");
    let cpp_source = directory.path().join("main.cpp");
    std::fs::write(&c_source, "#if !defined(C_ONLY) || defined(CPP_ONLY)\n#error wrong C flags\n#endif\nint helper(void) { return 42; }\n").unwrap();
    std::fs::write(&cpp_source, "#include <vector>\n#if !defined(CPP_ONLY) || defined(C_ONLY)\n#error wrong C++ flags\n#endif\nextern \"C\" int helper(void);\nint main() { std::vector<int> values{helper()}; return values.at(0) == 42 ? 0 : 1; }\n").unwrap();
    let target = SharedLibrary {
        name: "mixed-native".into(),
        sources: vec![c_source, cpp_source],
        c_compile_options: vec!["-DC_ONLY".into()],
        cpp_compile_options: vec!["-DCPP_ONLY".into()],
        ..SharedLibrary::default()
    };
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("out"),
    };
    let artifact = build.executable(&target).unwrap();
    assert!(
        std::process::Command::new(artifact)
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn pch_batches_preserve_constructor_order_with_interleaved_headers() {
    if !obs_build::BUILD_TARGET.contains("linux") {
        eprintln!("compiled PCH integration currently covers Linux GCC and Clang");
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let a = root.join("a-header.h");
    let z = root.join("z-header.h");
    for header in [&a, &z] {
        fs::write(header, "#pragma once\nextern int fixture_order;\n").unwrap();
    }
    let mut sources = Vec::new();
    for index in 1..=3 {
        let path = root.join(format!("part-{index}.cpp"));
        let header = if index == 2 {
            "a-header.h"
        } else {
            "z-header.h"
        };
        fs::write(&path, format!("#include \"{header}\"\nstruct Init{index} {{ Init{index}() {{ fixture_order = fixture_order * 10 + {index}; }} }};\nstatic Init{index} init{index};\n")).unwrap();
        sources.push(path);
    }
    let entry = root.join("entry.cpp");
    fs::write(&entry, "int fixture_order = 0;\nextern \"C\" int obs_fixture_order(void) { return fixture_order; }\n").unwrap();
    for unity in [false, true] {
        let build = NativeBuild {
            profile: Default::default(),
            host: obs_build::BUILD_HOST.into(),
            target: obs_build::BUILD_TARGET.into(),
            output: root.join(format!("artifacts-{unity}")),
        };
        let mut library = target("fixture-order", &entry, vec![]);
        library.sources = sources.clone();
        library.sources.push(entry.clone());
        library.include_dirs.push(root.into());
        library.unity = unity;
        library.precompiled_headers = vec![
            obs_build::PrecompiledHeader {
                header: a.clone(),
                sources: vec![sources[1].clone()],
            },
            obs_build::PrecompiledHeader {
                header: z.clone(),
                sources: vec![sources[0].clone(), sources[2].clone()],
            },
        ];
        let artifact = build.shared_library(&library).unwrap();
        // SAFETY: the fixture defines this no-argument C ABI function.
        assert_eq!(unsafe { answer(&artifact, b"obs_fixture_order", &[]) }, 123);
    }
}

#[test]
fn production_hybrid_requires_a_real_rust_archive_before_native_compilation() {
    let directory = tempfile::tempdir().unwrap();
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("build"),
    };
    let error = obs_build::libobs::build_hybrid_with_options(
        &build,
        directory.path(),
        &obs_build::libobs::Options::default(),
        &directory.path().join("missing-rust.a"),
    )
    .unwrap_err();
    assert!(error.contains("Rust archive does not exist"), "{error}");
    assert!(!build.output.exists(), "must not silently start a C build");
}

#[test]
fn objective_groups_expand_to_individual_language_correct_sources() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: root.join("artifacts"),
    };
    for (suffix, implementation, extension) in
        [("+.h.m", ".m.h", "m"), ("+.hpp.mm", ".mm.hpp", "mm")]
    {
        let member = format!("member{implementation}");
        fs::write(root.join(&member), "/* fixture */\n").unwrap();
        let group = root.join(format!("fixture{suffix}"));
        fs::write(&group, format!("#include \"{member}\"\n")).unwrap();
        let sources = build
            .separate_group_sources(root, std::slice::from_ref(&group))
            .unwrap();
        assert_eq!(sources.len(), 1);
        assert_ne!(
            sources[0], group,
            "unity-off must expand Objective-C groups"
        );
        assert_eq!(sources[0].extension().unwrap(), extension);
        assert!(fs::read_to_string(&sources[0]).unwrap().contains(&member));
    }
}

#[test]
fn mixed_objective_languages_compile_load_and_rebuild_with_unity_and_pch() {
    if native_answer_probe() {
        return;
    }
    if !obs_build::BUILD_TARGET.contains("linux") && !obs_build::BUILD_TARGET.contains("apple") {
        return; // The fixture requires an Objective-C-capable Clang toolchain.
    }
    let selector = format!(
        "{}::mixed_objective_languages_compile_load_and_rebuild_with_unity_and_pch",
        module_path!().split_once("::").unwrap().1
    );
    if std::env::var_os("OBS_BUILD_OBJC_CHILD").is_none() {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &selector, "--nocapture"])
            .env("OBS_BUILD_OBJC_CHILD", "1")
            .env("CC", "clang")
            .env("CXX", "clang++")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let header = root.join("fixture.pch.h");
    let write_header = |value| {
        fs::write(&header, format!("#define VALUE {value}\n#if defined(__OBJC__)\n#if !__has_feature(objc_arc)\n#error ARC flag missing\n#endif\n#ifdef __cplusplus\n#if !defined(OBJCPP_ONLY) || defined(OBJC_ONLY)\n#error Objective-C++ flags missing or contaminated\n#endif\n#else\n#if !defined(OBJC_ONLY) || defined(OBJCPP_ONLY)\n#error Objective-C flags missing or contaminated\n#endif\n#endif\n#else\n#if defined(OBJC_ONLY) || defined(OBJCPP_ONLY)\n#error Objective-C flags leaked to C or C++\n#endif\n#endif\n")).unwrap();
    };
    write_header(1);
    let c = root.join("part.c");
    let cpp = root.join("part.cpp");
    let objc = root.join("part.m");
    let objcpp = root.join("part.mm");
    fs::write(&c, "#include \"fixture.pch.h\"\n#if defined(__OBJC__) || defined(__cplusplus)\n#error wrong C language\n#endif\nint fixture_c(void) { return VALUE; }\n").unwrap();
    fs::write(&objc, "#include \"fixture.pch.h\"\n#if !defined(__OBJC__) || defined(__cplusplus)\n#error wrong Objective-C language\n#endif\nint fixture_objc(void) { return 4 * VALUE * OBJC_ONLY * (@encode(int)[0] == 'i'); }\n").unwrap();
    fs::write(&objcpp, "#include \"fixture.pch.h\"\n#if !defined(__OBJC__) || !defined(__cplusplus)\n#error wrong Objective-C++ language\n#endif\ntemplate<int N> constexpr int value() { return N; }\nextern \"C\" int fixture_objcpp(void) { return value<8>() * VALUE * OBJCPP_ONLY * (@encode(int)[0] == 'i'); }\n").unwrap();
    fs::write(&cpp, "#include \"fixture.pch.h\"\n#if defined(__OBJC__) || !defined(__cplusplus)\n#error wrong C++ language\n#endif\nextern \"C\" int fixture_c(void);\nextern \"C\" int fixture_objc(void);\nextern \"C\" int fixture_objcpp(void);\nextern \"C\" int obs_fixture_languages(void) { return fixture_c() + fixture_objc() + fixture_objcpp() + 2 * VALUE; }\n").unwrap();
    for unity in [false, true] {
        for pch in [false, true] {
            write_header(1);
            let build = NativeBuild {
                profile: Default::default(),
                host: obs_build::BUILD_HOST.into(),
                target: obs_build::BUILD_TARGET.into(),
                output: root.join(format!("output-{unity}-{pch}")),
            };
            let mut library = target("fixture-languages", &c, vec![]);
            library
                .sources
                .extend([objc.clone(), cpp.clone(), objcpp.clone()]);
            library.include_dirs.push(root.into());
            library.unity = unity;
            library.cpp_compile_options.push("-fno-exceptions".into());
            // Select a modern runtime for ARC parsing on Linux too. The fixture
            // emits no Objective-C objects or runtime calls and needs no SDK.
            library.objc_compile_options = vec![
                "-fobjc-arc".into(),
                "-fobjc-runtime=macosx-10.12".into(),
                "-DOBJC_ONLY=1".into(),
            ];
            library.objcpp_compile_options = vec![
                "-fobjc-arc".into(),
                "-fobjc-runtime=macosx-10.12".into(),
                "-DOBJCPP_ONLY=1".into(),
            ];
            if pch {
                library
                    .precompiled_headers
                    .push(obs_build::PrecompiledHeader {
                        header: header.clone(),
                        sources: library.sources.clone(),
                    });
            }
            let artifact = build.shared_library(&library).unwrap();
            assert_native_answer_fresh(&artifact, "obs_fixture_languages", 15, &selector);
            write_header(2);
            let artifact = build.shared_library(&library).unwrap();
            assert_native_answer_fresh(&artifact, "obs_fixture_languages", 30, &selector);
            library.objc_compile_options[2] = "-DOBJC_ONLY=2".into();
            let artifact = build.shared_library(&library).unwrap();
            assert_native_answer_fresh(&artifact, "obs_fixture_languages", 38, &selector);
            library.objcpp_compile_options[2] = "-DOBJCPP_ONLY=2".into();
            let artifact = build.shared_library(&library).unwrap();
            assert_native_answer_fresh(&artifact, "obs_fixture_languages", 54, &selector);
        }
    }
}

#[test]
fn arc_and_manual_groups_keep_source_flags_through_expansion_and_pch() {
    if native_answer_probe() {
        return;
    }
    if !obs_build::BUILD_TARGET.contains("linux") && !obs_build::BUILD_TARGET.contains("apple") {
        return;
    }
    let selector = format!(
        "{}::arc_and_manual_groups_keep_source_flags_through_expansion_and_pch",
        module_path!().split_once("::").unwrap().1
    );
    if std::env::var_os("OBS_BUILD_SOURCE_FLAGS_CHILD").is_none() {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &selector, "--nocapture"])
            .env("OBS_BUILD_SOURCE_FLAGS_CHILD", "1")
            .env("CC", "clang")
            .env("CXX", "clang++")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let header = root.join("fixture.pch.h");
    fs::write(&header, "#pragma once\n#ifndef SOURCE_VALUE\n#error source flags missing from PCH\n#endif\n#define HEADER_ARC __has_feature(objc_arc)\n").unwrap();
    let arc_member = root.join("arc.m.h");
    let manual_member = root.join("manual.m.h");
    fs::write(&arc_member, "#include \"fixture.pch.h\"\n#if !HEADER_ARC\n#error ARC disabled for ARC member\n#endif\nint fixture_arc(void) { return SOURCE_VALUE; }\n").unwrap();
    fs::write(&manual_member, "#include \"fixture.pch.h\"\n#if HEADER_ARC\n#error ARC enabled for manual member\n#endif\nint fixture_arc(void);\nint obs_fixture_source_flags(void) { return SOURCE_VALUE + fixture_arc(); }\n").unwrap();
    let arc = root.join("arc+.h.m");
    let manual = root.join("manual+.h.m");
    fs::write(&arc, "#include \"fixture.pch.h\"\n#include \"arc.m.h\"\n").unwrap();
    fs::write(
        &manual,
        "#include \"fixture.pch.h\"\n#include \"manual.m.h\"\n",
    )
    .unwrap();
    for separate in [false, true] {
        for pch in [false, true] {
            let build = NativeBuild {
                profile: Default::default(),
                host: obs_build::BUILD_HOST.into(),
                target: obs_build::BUILD_TARGET.into(),
                output: root.join(format!("output-{separate}-{pch}")),
            };
            let mut library = target("fixture-source-flags", &arc, vec![]);
            library.sources.push(manual.clone());
            library.include_dirs.push(root.into());
            library.objc_compile_options =
                vec!["-fobjc-runtime=macosx-10.12".into(), "-fno-objc-arc".into()];
            library.source_compile_options = vec![
                obs_build::SourceCompileOptions {
                    sources: vec![arc.clone()],
                    options: vec!["-fobjc-arc".into(), "-DSOURCE_VALUE=17".into()],
                },
                obs_build::SourceCompileOptions {
                    sources: vec![manual.clone()],
                    options: vec!["-DSOURCE_VALUE=25".into()],
                },
            ];
            if pch {
                library
                    .precompiled_headers
                    .push(obs_build::PrecompiledHeader {
                        header: header.clone(),
                        sources: library.sources.clone(),
                    });
            }
            if separate {
                build.separate_target_sources(root, &mut library).unwrap();
            }
            let artifact = build.shared_library(&library).unwrap();
            assert_native_answer_fresh(&artifact, "obs_fixture_source_flags", 42, &selector);
            library.source_compile_options[0].options[1] = "-DSOURCE_VALUE=35".into();
            let artifact = build.shared_library(&library).unwrap();
            assert_native_answer_fresh(&artifact, "obs_fixture_source_flags", 60, &selector);
        }
    }
}

#[test]
fn native_source_options_reject_ambiguous_or_orphan_assignments() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.c");
    let other = dir.path().join("other.c");
    fs::write(&source, "int fixture(void) { return 1; }\n").unwrap();
    fs::write(&other, "int other(void) { return 2; }\n").unwrap();
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: dir.path().join("artifacts"),
    };
    let mut library = target("fixture-options", &source, vec![]);
    let entry = obs_build::SourceCompileOptions {
        sources: vec![source.clone()],
        options: vec!["-DVALUE=1".into()],
    };
    library.source_compile_options = vec![entry.clone(), entry];
    assert!(
        build
            .shared_library(&library)
            .unwrap_err()
            .contains("duplicate compiler options")
    );
    library.source_compile_options = vec![obs_build::SourceCompileOptions {
        sources: vec![other],
        options: vec![],
    }];
    assert!(
        build
            .shared_library(&library)
            .unwrap_err()
            .contains("outside this target")
    );
    assert!(
        build
            .separate_target_sources(dir.path(), &mut library)
            .unwrap_err()
            .contains("annotation outside target")
    );
    assert_eq!(
        library.sources,
        vec![source],
        "an invalid expansion must not mutate the target"
    );
}

// dyld retains Objective-C dylibs after dlclose. Probe rebuilt images in fresh
// processes so compiler/header invalidation is observed independently of that
// loader lifetime policy. The probe reuses its owning test in this category.
fn native_answer_probe() -> bool {
    let Some(library) = std::env::var_os("OBS_NATIVE_ANSWER_LIBRARY") else {
        return false;
    };
    let symbol = std::env::var("OBS_NATIVE_ANSWER_SYMBOL").unwrap();
    let expected: i32 = std::env::var("OBS_NATIVE_ANSWER_EXPECTED")
        .unwrap()
        .parse()
        .unwrap();
    // SAFETY: these self-built fixtures define a no-argument C ABI function;
    // answer keeps the library alive throughout the call.
    assert_eq!(
        unsafe { answer(Path::new(&library), symbol.as_bytes(), &[]) },
        expected
    );
    true
}

fn assert_native_answer_fresh(library: &Path, symbol: &str, expected: i32, selector: &str) {
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", selector, "--nocapture"])
        .env("OBS_NATIVE_ANSWER_LIBRARY", library)
        .env("OBS_NATIVE_ANSWER_SYMBOL", symbol)
        .env("OBS_NATIVE_ANSWER_EXPECTED", expected.to_string())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
}

/// Run a Qt test program for at most a minute: one that cannot start can
/// block on a dialog instead of exiting (Windows, #1878). Its output is kept.
#[cfg(feature = "libobs-integration")]
fn bounded(mut command: std::process::Command) -> std::process::Output {
    use std::process::Stdio;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() > deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!(
                "{command:?} did not exit in 60 s\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    child.wait_with_output().unwrap()
}

/// #3675: both native consumers must run the unchanged broadcast-settings cases.
#[test]
fn canonical_youtube_broadcast_settings_is_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("core");
    let name = "test_youtube_broadcast_settings";
    let target = graph
        .targets
        .get(name)
        .expect("canonical graph omits YouTube broadcast settings");
    assert_eq!(
        target
            .sources
            .iter()
            .map(|source| match source {
                Source::Path(path) => path.as_str(),
                _ => panic!("broadcast settings sources must be unconditional"),
            })
            .collect::<Vec<_>>(),
        [
            "test/cmocka/test_youtube_broadcast_settings.c",
            "frontend/utility/youtube-broadcast-settings.c.h"
        ]
    );
    assert_eq!(target.includes, ["frontend/utility"]);
    assert_eq!(
        target
            .libraries
            .iter()
            .map(|library| match library {
                ConditionalValue::Name(name) => name.as_str(),
                _ => panic!("broadcast settings libraries must be unconditional"),
            })
            .collect::<Vec<_>>(),
        ["OBS::libobs", "OBS::native-test-cmocka"]
    );
    let test = &graph.tests[name];
    assert_eq!(test.target, name);
    assert_eq!(test.command, ["${target}"]);
    assert_eq!(test.timeout_seconds, None);
    assert!(
        !root
            .join("test/cmocka/tests.d/test_youtube_broadcast_settings.cmake")
            .exists()
    );
    graph.targets.retain(|key, _| key == name);
    graph.tests.retain(|key, _| key == name);
    for os in ["OS_LINUX", "OS_MACOS", "OS_WINDOWS"] {
        let selected = graph.select(&[(os.into(), true)].into()).unwrap();
        assert!(selected.targets.contains_key(name));
        assert!(selected.tests.contains_key(name));
    }
}

/// #1390: the original nine dock cases need Linux, Widgets and Qt Test.
#[test]
fn canonical_dock_proportions_is_registered() {
    use obs_build::validation::graph::Graph;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    graph.retain_suite("qt");
    let target = graph
        .targets
        .get("test_dock_proportions")
        .expect("canonical graph omits test_dock_proportions");
    assert_eq!(target.sources.len(), 1);
    assert!(
        matches!(&target.sources[0], obs_build::validation::graph::Source::Path(path) if path == "test/cmocka/test_dock_proportions.cpp")
    );
    assert_eq!(target.includes, ["frontend/utility"]);
    assert_eq!(
        target
            .libraries
            .iter()
            .map(|library| match library {
                obs_build::validation::graph::Library::Name(name) => name.as_str(),
                _ => panic!("dock libraries must be unconditional"),
            })
            .collect::<Vec<_>>(),
        ["Qt6::Widgets", "Qt6::Test", "OBS::native-test-cmocka"]
    );
    assert_eq!(
        target.watch_inputs,
        ["frontend/utility/dock-proportions.hpp"]
    );
    let test = &graph.tests["test_dock_proportions"];
    assert_eq!(test.timeout_seconds, Some(120));
    assert_eq!(test.resource_locks, ["timing"]);
    assert_eq!(test.qt_runtime.as_deref(), Some("Qt6::Core"));
    for linux in [false, true] {
        for widgets in [false, true] {
            for qt_test in [false, true] {
                let facts = [
                    ("OS_LINUX".into(), linux),
                    ("HAVE_QT6_CORE".into(), false),
                    ("HAVE_QT6_WIDGETS".into(), widgets),
                    ("HAVE_QT6_TEST".into(), qt_test),
                ]
                .into();
                let selected = graph.select(&facts).unwrap();
                assert_eq!(
                    selected.tests.contains_key("test_dock_proportions"),
                    linux && widgets && qt_test
                );
            }
        }
    }
}

#[cfg(feature = "libobs-integration")]
fn assert_dock_proportions_properties(tests: &[serde_json::Value]) {
    let dock = tests
        .iter()
        .find(|test| test["name"] == "test_dock_proportions")
        .unwrap();
    let properties = dock["properties"].as_array().unwrap();
    assert!(
        properties
            .iter()
            .any(|property| property["name"] == "TIMEOUT"
                && property["value"].as_f64() == Some(120.0))
    );
    assert!(
        properties
            .iter()
            .any(|property| property["name"] == "RESOURCE_LOCK"
                && property["value"] == serde_json::json!(["timing"]))
    );
}
