use super::*;
use obs_build::validation::graph::{Dependency, Graph};
use std::process::Command;

fn checked(command: &mut Command) -> std::process::Output {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{command:?}: {error}"));
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn canonical_display_live_runs_unchanged() {
    assert!(std::env::var_os("DISPLAY").is_some_and(|value| !value.is_empty()));
    let root = dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("test/cmocka/build/test-graph.json")).unwrap())
            .unwrap();
    let mut selected = expected();
    for section in ["targets", "tests"] {
        for (name, record) in selected[section].as_object_mut().unwrap() {
            *record = raw[section][name].clone();
        }
    }
    let graph = Graph::parse(&selected.to_string()).unwrap();
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: directory.path().join("cargo-output"),
        profile: Default::default(),
    };
    let core = built_core(&build, &root);
    let core_path = core.dependency.artifacts[0].clone();
    let renderer =
        obs_build::graphics::build_opengl(&build, &root, &core_path, &Default::default()).unwrap();
    assert_core_binding(&renderer, &core_path);
    let mut interfaces = interfaces(&build, core.dependency, renderer.clone());
    add_media(&build, &root, &mut interfaces);
    let transitions = obs_build::plugins::build_transitions(&build, &root, &core_path).unwrap();
    assert_core_binding(&transitions.library, &core_path);
    interfaces.insert(
        "obs-transitions".into(),
        Dependency {
            artifacts: vec![transitions.library.clone()],
            ..Default::default()
        },
    );
    add_capture(&build, &root, &mut interfaces);
    let facts = [
        ("OS_LINUX".into(), true),
        ("OS_WINDOWS".into(), false),
        ("OS_MACOS".into(), false),
        ("HAVE_LINUX_CAPTURE".into(), true),
        ("HAVE_OBS_PIPEWIRE".into(), true),
        ("HAVE_TRANSITIONS_TEST_OWNER".into(), true),
        ("HAVE_MEDIA_PLAYBACK_TEST_OWNER".into(), true),
        ("HAVE_OBS_OPENGL".into(), true),
        ("HAVE_X11_DISPLAY".into(), true),
    ]
    .into();
    let runtime = directory.path().join("runtime");
    for data in ["share/obs", "RelWithDebInfo/share/obs"] {
        fs::create_dir_all(runtime.join(data)).unwrap();
        checked(
            Command::new("cp")
                .arg("-R")
                .arg(root.join("libobs/data"))
                .arg(runtime.join(data).join("libobs")),
        );
    }
    let artifacts = graph.compile(&build, &root, &facts, &interfaces).unwrap();
    assert_eq!(artifacts.len(), CASES.len());
    for name in CASES {
        assert_core_binding(&artifacts[name], &core_path);
    }
    for (name, spec) in &graph.tests {
        let result = checked(
            spec.command_in(&root, &build.output, &artifacts[&spec.target], &runtime)
                .unwrap()
                .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
        );
        assert_cases(&result, &spec.target);
        eprintln!(
            "Cargo {name}: {}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fs::write(directory.path().join("graph.json"), selected.to_string()).unwrap();
    write_cmake(
        directory.path(),
        &root,
        &runtime,
        &core.cmake,
        &renderer,
        &transitions.library,
    );
    replay_cmake(directory.path(), &core_path);
}

fn interfaces(
    build: &NativeBuild,
    core: Dependency,
    renderer: std::path::PathBuf,
) -> std::collections::BTreeMap<String, Dependency> {
    let mut interfaces = std::collections::BTreeMap::from([
        ("OBS::libobs".into(), core),
        (
            "OBS::libobs-opengl".into(),
            Dependency {
                artifacts: vec![renderer],
                ..Default::default()
            },
        ),
        ("OBS::caption".into(), Dependency::default()),
        ("Uthash::Uthash".into(), Dependency::default()),
    ]);
    for (name, package) in [
        ("OBS::native-test-cmocka", "cmocka"),
        ("FFmpeg::avcodec", "libavcodec"),
        ("FFmpeg::avformat", "libavformat"),
        ("FFmpeg::avutil", "libavutil"),
        ("FFmpeg::swscale", "libswscale"),
        ("FFmpeg::swresample", "libswresample"),
    ] {
        interfaces.insert(
            name.into(),
            Dependency::package(&build.target, package).unwrap(),
        );
    }
    interfaces.insert(
        "m".into(),
        Dependency {
            link_options: vec!["-lm".into()],
            ..Default::default()
        },
    );
    interfaces
}

fn write_cmake(
    directory: &Path,
    root: &Path,
    runtime: &Path,
    core: &str,
    renderer: &Path,
    transitions: &Path,
) {
    let source = root.to_string_lossy().replace('\\', "/");
    fs::write(directory.join("CMakeLists.txt"), format!(r#"cmake_minimum_required(VERSION 3.28)
project(display-live-contract C CXX)
set(CMAKE_C_VISIBILITY_PRESET hidden)
enable_testing()
option(OS_LINUX "Original Linux guard" ON)
option(OS_WINDOWS "Windows dependency guard" OFF)
option(OS_MACOS "macOS source guard" OFF)
option(HAVE_OBS_OPENGL "Original renderer guard" ON)
option(HAVE_LINUX_CAPTURE "Original capture owner guard" ON)
option(HAVE_OBS_PIPEWIRE "Original PipeWire owner guard" ON)
option(HAVE_TRANSITIONS_TEST_OWNER "Original transition owner guard" ON)
option(WITH_MEDIA "Original media playback owner guard" ON)
option(DECLARE_XVFB_RUNNER "Root CMake declares its Xvfb runner" ON)
if(DECLARE_XVFB_RUNNER)
  find_program(OBS_XVFB_RUN_REAL xvfb-run REQUIRED)
  configure_file("{source}/test/cmocka/xvfb-run-or-display.sh.in" "${{CMAKE_CURRENT_BINARY_DIR}}/xvfb-run-or-display.sh" @ONLY)
  set(XVFB_RUN "${{CMAKE_CURRENT_BINARY_DIR}}/xvfb-run-or-display.sh")
  file(CHMOD "${{XVFB_RUN}}" PERMISSIONS OWNER_READ OWNER_WRITE OWNER_EXECUTE GROUP_READ GROUP_EXECUTE WORLD_READ WORLD_EXECUTE)
endif()
set(OBS_OUTPUT_DIR "{runtime}")
find_package(PkgConfig REQUIRED)
pkg_check_modules(CMOCKA REQUIRED IMPORTED_TARGET cmocka)
{core}
# built_core deliberately hides consumer symbols for ordinary probes. The real
# core target has no such interface policy; these interposition fixtures need
# their original default visibility, selected by the graph's compiler option.
set_property(TARGET OBS::libobs PROPERTY INTERFACE_COMPILE_OPTIONS "")
add_library(OBS::native-test-cmocka ALIAS PkgConfig::CMOCKA)
foreach(component avcodec avformat avutil swscale swresample)
  pkg_check_modules(${{component}} REQUIRED IMPORTED_TARGET lib${{component}})
  add_library(FFmpeg::${{component}} ALIAS PkgConfig::${{component}})
endforeach()
add_library(OBS::caption INTERFACE IMPORTED)
add_library(Uthash::Uthash INTERFACE IMPORTED)
list(APPEND CMAKE_MODULE_PATH "{source}/cmake/finders")
if(WITH_MEDIA)
  find_package(FFmpeg 8.0 REQUIRED COMPONENTS avcodec avdevice avutil avformat swscale swresample)
  add_subdirectory("{source}/shared/media-playback" media-playback)
  target_include_directories(media-playback INTERFACE "{source}" "{source}/libobs")
endif()
set(HAVE_MEDIA_PLAYBACK_TEST_OWNER FALSE)
if(TARGET OBS::media-playback)
  set(HAVE_MEDIA_PLAYBACK_TEST_OWNER TRUE)
endif()
add_library(obs-transitions MODULE IMPORTED)
set_property(TARGET obs-transitions PROPERTY IMPORTED_LOCATION "{transitions}")
add_library(obsglad STATIC "{source}/deps/glad/src/glad.c")
set_property(TARGET obsglad PROPERTY POSITION_INDEPENDENT_CODE ON)
target_include_directories(obsglad SYSTEM PUBLIC "{source}/deps/glad/include")
add_library(OBS::glad ALIAS obsglad)
foreach(pair IN ITEMS "X11::X11=x11" "X11::XCB=x11-xcb" "XCB::XCB=xcb" "XCB::XFIXES=xcb-xfixes" "XCB::RANDR=xcb-randr" "XCB::SHM=xcb-shm" "XCB::XINERAMA=xcb-xinerama" "XCB::COMPOSITE=xcb-composite" "PipeWire::PipeWire=libpipewire-0.3" "gio::gio=gio-2.0" "PkgConfig::_portal_test_glib=gobject-2.0")
  string(REPLACE "=" ";" fields "${{pair}}")
  list(GET fields 0 name)
  list(GET fields 1 package)
  string(MAKE_C_IDENTIFIER "${{package}}" prefix)
  pkg_check_modules(${{prefix}} REQUIRED IMPORTED_TARGET "${{package}}")
  add_library(${{name}} ALIAS PkgConfig::${{prefix}})
endforeach()
if(HAVE_OBS_OPENGL)
  add_library(OBS::libobs-opengl MODULE IMPORTED)
  set_property(TARGET OBS::libobs-opengl PROPERTY IMPORTED_LOCATION "{renderer}")
endif()
include("{source}/test/cmocka/native-test-graph.cmake")
obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{source}")
"#, runtime=runtime.display(), renderer=renderer.display(), transitions=transitions.display())).unwrap();
}

fn replay_cmake(directory: &Path, core: &Path) {
    let all: Vec<String> = expected()["tests"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    for (mode, options, expected_names) in [
        ("enabled", vec![], all.clone()),
        ("windows", vec!["-DOS_LINUX=OFF", "-DOS_WINDOWS=ON"], vec![]),
        ("macos", vec!["-DOS_LINUX=OFF", "-DOS_MACOS=ON"], vec![]),
        ("no-display", vec!["-DDECLARE_XVFB_RUNNER=OFF"], vec![]),
        ("declared-runner", vec![], all.clone()),
        (
            "no-renderer",
            vec!["-DHAVE_OBS_OPENGL=OFF"],
            vec!["test_screencast_portal_teardown".into()],
        ),
        (
            "no-media",
            vec!["-DWITH_MEDIA=OFF"],
            vec![
                "obs_runtime_bench_smoke".into(),
                "test_screencast_portal_teardown".into(),
                "test_xshm_capture_live".into(),
            ],
        ),
        (
            "no-capture",
            vec!["-DHAVE_LINUX_CAPTURE=OFF"],
            all.iter()
                .filter(|n| n.as_str() != "test_xshm_capture_live")
                .cloned()
                .collect(),
        ),
        (
            "no-pipewire",
            vec!["-DHAVE_OBS_PIPEWIRE=OFF"],
            all.iter()
                .filter(|n| n.as_str() != "test_screencast_portal_teardown")
                .cloned()
                .collect(),
        ),
        (
            "no-transitions",
            vec!["-DHAVE_TRANSITIONS_TEST_OWNER=OFF"],
            all.iter()
                .filter(|n| n.as_str() != "test_stinger_media_duration")
                .cloned()
                .collect(),
        ),
    ] {
        let output = directory.join(mode);
        let mut command = Command::new("cmake");
        command
            .arg("-S")
            .arg(directory)
            .arg("-B")
            .arg(&output)
            .arg("-DCMAKE_BUILD_TYPE=RelWithDebInfo")
            .args(options);
        if mode == "no-display" || mode == "declared-runner" {
            command.env_remove("DISPLAY");
        }
        checked(&mut command);
        if mode == "enabled" {
            checked(
                Command::new("cmake")
                    .arg("--build")
                    .arg(&output)
                    .args(["--parallel", "2"]),
            );
            for name in CASES {
                assert_core_binding(&output.join(name), core);
            }
            for name in &all {
                let result = checked(
                    Command::new("ctest")
                        .arg("--test-dir")
                        .arg(&output)
                        .args(["--output-on-failure", "--no-tests=error", "-V", "-R"])
                        .arg(format!("^{name}$"))
                        .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT"),
                );
                let fixture = expected()["tests"][name]["target"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                assert_cases(&result, &fixture);
                assert!(
                    String::from_utf8_lossy(&result.stdout)
                        .contains("100% tests passed, 0 tests failed out of 1")
                );
                eprintln!("CMake {name}: {}", String::from_utf8_lossy(&result.stdout));
            }
        }
        check_inventory(&output, &expected_names, mode == "enabled");
    }
}

fn check_inventory(output: &Path, names: &[String], built: bool) {
    let result = checked(
        Command::new("ctest")
            .arg("--test-dir")
            .arg(output)
            .arg("--show-only=json-v1"),
    );
    let inventory: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let tests = inventory["tests"].as_array().unwrap();
    assert_eq!(tests.len(), names.len(), "{}", output.display());
    let expected = expected();
    for test in tests {
        let name = test["name"].as_str().unwrap();
        assert!(names.iter().any(|expected| expected == name));
        let record = &expected["tests"][name];
        let properties = test["properties"].as_array().unwrap();
        let property = |name: &str| {
            properties
                .iter()
                .find(|p| p["name"] == name)
                .map(|p| &p["value"])
        };
        assert_eq!(
            property("TIMEOUT").unwrap().as_f64(),
            record["timeout_seconds"].as_f64()
        );
        assert_eq!(
            property("SKIP_RETURN_CODE").and_then(serde_json::Value::as_f64),
            record["skip_return_code"].as_f64()
        );
        let actual_locks = property("RESOURCE_LOCK")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        let wanted_locks = record
            .get("resource_locks")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        assert_eq!(actual_locks, wanted_locks);
        if built {
            let command = test["command"].as_array().unwrap();
            let target = record["target"].as_str().unwrap();
            let executable = output.join(target).to_string_lossy().into_owned();
            let index = command
                .iter()
                .position(|value| value.as_str() == Some(&executable))
                .unwrap();
            if target == "obs_runtime_bench" {
                assert_eq!(
                    &command[index + 1..],
                    &record["command"].as_array().unwrap()[1..]
                );
            }
            if record.get("optional_prefixes").is_some() {
                assert!(index > 0);
            } else {
                assert_eq!(index, 0);
            }
            let cwd = property("WORKING_DIRECTORY").unwrap().as_str().unwrap();
            let wanted = if record.get("working_directory").is_some() {
                output.parent().unwrap().join("runtime/RelWithDebInfo")
            } else {
                output.to_path_buf()
            };
            assert_eq!(
                dunce::canonicalize(cwd).unwrap(),
                dunce::canonicalize(wanted).unwrap()
            );
        }
    }
}

fn assert_cases(output: &std::process::Output, name: &str) {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !super::super::canonical_display_startup_is_registered::has_skipped_cases(&text),
        "{text}"
    );
    if name == "obs_runtime_bench" {
        let json_line = text
            .lines()
            .find_map(|line| line.find("{\"seconds\":").map(|index| &line[index..]))
            .expect("runtime benchmark JSON");
        let metrics: serde_json::Value = serde_json::from_str(json_line).unwrap();
        assert!(metrics["frames"].as_u64().unwrap() > 0, "{text}");
        assert!(
            metrics["video_callbacks_per_second"].as_f64().unwrap() > 0.0,
            "{text}"
        );
        assert!(
            metrics["audio_ticks_per_second"].as_f64().unwrap() > 0.0,
            "{text}"
        );
        return;
    }
    let expected = match name {
        "test_media_cover_art_monitor" => vec![2],
        "test_media_sync_offset_restart" => vec![6],
        "test_screencast_portal_teardown" | "test_xshm_capture_live" => vec![1],
        "test_stinger_media_duration" => vec![5],
        _ => panic!("unexpected fixture {name}"),
    };
    let actual: Vec<usize> = text
        .lines()
        .filter_map(|line| {
            line.split_once("[  PASSED  ] ")
                .map(|(_, summary)| summary.split_whitespace().next().unwrap().parse().unwrap())
        })
        .collect();
    assert_eq!(actual, expected, "{text}");
}

fn assert_core_binding(executable: &Path, core: &Path) {
    let output = checked(Command::new("ldd").arg(executable));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(core.to_str().unwrap()),
        "{} must load {}: {stdout}",
        executable.display(),
        core.display()
    );
    eprintln!("Loader {}: {}", executable.display(), core.display());
}
fn add_media(
    build: &NativeBuild,
    root: &Path,
    interfaces: &mut std::collections::BTreeMap<String, Dependency>,
) {
    let mut library = SharedLibrary {
        name: "headless-media-playback".into(),
        sources: ["cache", "core"]
            .iter()
            .map(|group| {
                root.join(format!(
                    "shared/media-playback/build/media-playback.{group}+.h.c"
                ))
            })
            .collect(),
        include_dirs: vec![
            root.into(),
            root.join("libobs"),
            root.join("shared/media-playback"),
        ],
        ..Default::default()
    };
    library
        .include_dirs
        .extend(interfaces["OBS::libobs"].include_dirs.clone());
    for (group, header) in [
        ("cache", "media-playback-cache.pch.h"),
        ("core", "media-playback.pch.h"),
    ] {
        library
            .precompiled_headers
            .push(obs_build::PrecompiledHeader {
                header: root.join("shared/media-playback/build").join(header),
                sources: vec![root.join(format!(
                    "shared/media-playback/build/media-playback.{group}+.h.c"
                ))],
            });
    }
    for package in [
        "simde",
        "libavcodec",
        "libavdevice",
        "libavutil",
        "libavformat",
    ] {
        let dependency = Dependency::package(&build.target, package).unwrap();
        library.include_dirs.extend(dependency.include_dirs);
        library.compile_options.extend(dependency.compile_options);
        library.link_options.extend(dependency.link_options);
    }
    let artifact = build.static_library(&library).unwrap();
    interfaces.insert(
        "OBS::media-playback".into(),
        Dependency {
            artifacts: std::iter::once(artifact)
                .chain(interfaces["OBS::libobs"].artifacts.clone())
                .collect(),
            include_dirs: library.include_dirs,
            compile_options: library.compile_options,
            link_options: library.link_options,
            ..Default::default()
        },
    );
}

fn add_capture(
    build: &NativeBuild,
    root: &Path,
    interfaces: &mut std::collections::BTreeMap<String, Dependency>,
) {
    let glad = build
        .static_library(&SharedLibrary {
            name: "display-live-glad".into(),
            sources: vec![root.join("deps/glad/src/glad.c")],
            include_dirs: vec![root.join("deps/glad/include")],
            ..Default::default()
        })
        .unwrap();
    interfaces.insert(
        "OBS::glad".into(),
        Dependency {
            artifacts: vec![glad],
            include_dirs: vec![root.join("deps/glad/include")],
            ..Default::default()
        },
    );
    for (name, package) in [
        ("X11::X11", "x11"),
        ("X11::XCB", "x11-xcb"),
        ("XCB::XCB", "xcb"),
        ("XCB::XFIXES", "xcb-xfixes"),
        ("XCB::RANDR", "xcb-randr"),
        ("XCB::SHM", "xcb-shm"),
        ("XCB::XINERAMA", "xcb-xinerama"),
        ("XCB::COMPOSITE", "xcb-composite"),
        ("PipeWire::PipeWire", "libpipewire-0.3"),
        ("gio::gio", "gio-2.0"),
        ("PkgConfig::_portal_test_glib", "gobject-2.0"),
    ] {
        interfaces.insert(
            name.into(),
            Dependency::package(&build.target, package).unwrap(),
        );
    }
}
