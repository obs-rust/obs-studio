//! Build the unchanged native C tests through the Cargo native engine.
pub mod graph;
pub mod headless;
pub mod sdk_helpers;

use crate::{NativeBuild, SharedLibrary, libobs::add_package};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub struct Executable {
    pub name: String,
    pub path: PathBuf,
    pub runtime: Option<graph::Test>,
    /// Resolved link artifacts; used to verify core selection only when applicable.
    pub dependencies: Vec<PathBuf>,
}

impl Executable {
    pub fn command(&self, root: &Path, output: &Path) -> Result<std::process::Command, String> {
        self.command_in(root, output, output)
    }

    pub fn command_in(
        &self,
        root: &Path,
        output: &Path,
        runtime: &Path,
    ) -> Result<std::process::Command, String> {
        let path = dunce::canonicalize(&self.path)
            .map_err(|error| format!("native test executable {}: {error}", self.path.display()))?;
        match &self.runtime {
            Some(test) => test.command_in(root, output, &path, runtime),
            None => Ok(std::process::Command::new(path)),
        }
    }
}

pub struct Options {
    pub outputs: bool,
    pub filters: bool,
    pub pipewire: bool,
    /// Plugin availability is independent of libobs audio monitoring.
    pub pulseaudio_plugin: bool,
    pub hevc: bool,
    pub pulseaudio: bool,
    pub per_target_sanitizers: bool,
    /// Production staticlib for unchanged tests of hidden Rust shims.
    pub rust_archive: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            outputs: true,
            filters: true,
            pipewire: true,
            pulseaudio_plugin: true,
            hevc: true,
            pulseaudio: true,
            per_target_sanitizers: true,
            rust_archive: None,
        }
    }
}

fn load_suite(root: &Path, suite: &str) -> Result<graph::Graph, String> {
    let path = root.join("test/cmocka/build/test-graph.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let mut graph = graph::Graph::load(&path)?;
    graph.retain_suite(suite);
    Ok(graph)
}

fn compile_suite(
    build: &NativeBuild,
    root: &Path,
    graph: &graph::Graph,
    facts: &BTreeMap<String, bool>,
    interfaces: &BTreeMap<String, graph::Dependency>,
) -> Result<Vec<Executable>, String> {
    let mut facts = facts.clone();
    facts.insert("HAVE_LIBOBS".into(), interfaces.contains_key("OBS::libobs"));
    facts.insert(
        "HAVE_HAPPY_EYEBALLS".into(),
        interfaces.contains_key("OBS::happy-eyeballs"),
    );
    let facts = &facts;
    let artifacts = graph.compile(build, root, facts, interfaces)?;
    graph
        .select(facts)?
        .tests
        .into_iter()
        .map(|(name, test)| {
            Ok(Executable {
                name: name.into(),
                path: artifacts
                    .get(&test.target)
                    .ok_or("selected native test artifact missing")?
                    .clone(),
                runtime: Some(test.resolve_runtime(interfaces)?),
                dependencies: graph.targets[&test.target]
                    .dependency_artifacts(facts, interfaces)?,
            })
        })
        .collect()
}

/// Build Qt tests using caller-resolved SDK compile/link and runtime interfaces.
pub fn build_qt_tests(
    build: &NativeBuild,
    root: &Path,
    interfaces: &BTreeMap<String, graph::Dependency>,
) -> Result<Vec<Executable>, String> {
    let root = dunce::canonicalize(root).map_err(|error| error.to_string())?;
    let graph = load_suite(&root, "qt")?;
    let mut facts = crate::platform::native_test_facts(&build.target, false, false);
    facts.extend([
        ("HAVE_QT6_CORE".into(), interfaces.contains_key("Qt6::Core")),
        (
            "HAVE_QT6_WIDGETS".into(),
            interfaces.contains_key("Qt6::Widgets"),
        ),
        ("HAVE_QT6_TEST".into(), interfaces.contains_key("Qt6::Test")),
    ]);
    let mut interfaces = interfaces.clone();
    let thread_options = crate::platform::native_test_thread_options(&build.target);
    interfaces
        .entry("Threads::Threads".into())
        .or_insert_with(|| graph::Dependency {
            compile_options: thread_options.clone(),
            link_options: thread_options,
            ..Default::default()
        });
    compile_suite(build, &root, &graph, &facts, &interfaces)
}

/// Build standalone WHIP tests from the shared graph, with no core or SDK.
pub fn build_webrtc_tests(build: &NativeBuild, root: &Path) -> Result<Vec<Executable>, String> {
    let root = dunce::canonicalize(root).map_err(|error| error.to_string())?;
    let graph = load_suite(&root, "webrtc")?;
    let mut cmocka = SharedLibrary::default();
    add_package(&mut cmocka, &build.target, "cmocka")?;
    crate::platform::configure_webrtc_tests(&build.target, &mut cmocka)?;
    let interfaces = BTreeMap::from([
        ("OBS::native-test-cmocka".into(), cmocka.into()),
        (
            "m".into(),
            graph::Dependency {
                link_options: crate::platform::native_test_math_options(&build.target),
                ..Default::default()
            },
        ),
    ]);
    compile_suite(
        build,
        &root,
        &graph,
        &crate::platform::native_test_facts(&build.target, false, false),
        &interfaces,
    )
}

fn runtime_interfaces(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    graphics: &Path,
) -> Result<BTreeMap<String, graph::Dependency>, String> {
    let mut core_interface = SharedLibrary {
        include_dirs: vec![
            root.join("libobs"),
            root.join("deps/libcaption"),
            build.output.join("config"),
        ],
        dependencies: vec![core.into()],
        ..Default::default()
    };
    crate::platform::configure_c_tests(build, &mut core_interface, None, core)?;
    add_package(&mut core_interface, &build.target, "simde")?;
    let mut cmocka = SharedLibrary::default();
    add_package(&mut cmocka, &build.target, "cmocka")?;
    Ok(BTreeMap::from([
        ("OBS::libobs".into(), core_interface.into()),
        (
            "SIMDe::SIMDe".into(),
            graph::Dependency::package(&build.target, "simde")?,
        ),
        ("OBS::native-test-cmocka".into(), cmocka.into()),
        // Graphics compatibility tests can inspect obs-internal.h, whose uthash
        // dependency is header-only and uses the core build's system search.
        ("Uthash::Uthash".into(), graph::Dependency::default()),
        (
            "OBS::caption".into(),
            graph::Dependency {
                artifacts: vec![crate::libobs::build_caption(build, root)?],
                include_dirs: vec![root.join("deps/libcaption")],
                ..Default::default()
            },
        ),
        (
            "OBS::libobs-opengl".into(),
            graph::Dependency {
                artifacts: vec![graphics.into()],
                ..Default::default()
            },
        ),
        (
            "m".into(),
            graph::Dependency {
                link_options: crate::platform::native_test_math_options(&build.target),
                ..Default::default()
            },
        ),
    ]))
}

pub fn build_graphics_tests(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    graphics: &Path,
) -> Result<Vec<Executable>, String> {
    let root = dunce::canonicalize(root).map_err(|error| error.to_string())?;
    let graph = load_suite(&root, "graphics")?;
    let interfaces = runtime_interfaces(build, &root, core, graphics)?;
    let mut facts = crate::platform::native_test_facts(&build.target, false, false);
    facts.extend([
        ("HAVE_OBS_OPENGL".into(), true),
        ("HAVE_SLIDESHOW_TEST_OWNER".into(), false),
        // This graphics-only builder has no optional plugin, X11 or Qt interfaces.
        // Focused replays supply those owners and their capabilities explicitly.
        ("HAVE_OBS_FILTERS".into(), false),
        ("HAVE_RTMP_SERVICES_MODULE".into(), false),
        ("HAVE_STARTUP_X11".into(), false),
        ("HAVE_QT6_WIDGETS".into(), false),
    ]);
    compile_suite(build, &root, &graph, &facts, &interfaces)
}

pub fn build_vlc_tests(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    vlc: &crate::plugins::Module,
    graphics: &Path,
) -> Result<Vec<Executable>, String> {
    let root = dunce::canonicalize(root).map_err(|error| error.to_string())?;
    let graph = load_suite(&root, "vlc")?;
    let mut interfaces = runtime_interfaces(build, &root, core, graphics)?;
    interfaces.insert(
        "vlc-video".into(),
        graph::Dependency {
            artifacts: vec![vlc.library.clone()],
            ..Default::default()
        },
    );
    for component in ["avcodec", "avformat", "avutil"] {
        let mut dependency = SharedLibrary::default();
        add_package(&mut dependency, &build.target, &format!("lib{component}"))?;
        interfaces.insert(format!("FFmpeg::{component}"), dependency.into());
    }
    let mut facts = crate::platform::native_test_facts(&build.target, false, false);
    facts.extend([
        ("HAVE_OBS_OPENGL".into(), true),
        ("HAVE_SLIDESHOW_TEST_OWNER".into(), false),
        ("HAVE_VLC_MODULE".into(), true),
    ]);
    compile_suite(build, &root, &graph, &facts, &interfaces)
}

#[expect(clippy::too_many_lines, reason = "one block per generated test target")]
pub fn build_c_tests(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    options: &Options,
) -> Result<Vec<Executable>, String> {
    let root = dunce::canonicalize(root).map_err(|e| e.to_string())?;
    let mut graph = load_suite(&root, "core")?;
    if !options.per_target_sanitizers {
        for target in graph.targets.values_mut() {
            target.sanitizers = None;
        }
    }
    let mut facts =
        crate::platform::native_test_facts(&build.target, options.pulseaudio, options.hevc);
    facts.extend([
        ("HAVE_OBS_OUTPUTS".into(), options.outputs),
        (
            "C_COMPILER_CLANG".into(),
            crate::platform::native_test_compiler_is_clang(build),
        ),
        ("HAVE_OBS_FILTERS".into(), options.filters),
        ("HAVE_OBS_PIPEWIRE".into(), options.pipewire),
        ("HAVE_OBS_PULSEAUDIO".into(), options.pulseaudio_plugin),
        // This compatibility stage does not build these plugin owners.
        // Focused graph replays supply each owner and its SDK explicitly.
        ("HAVE_OBS_ALSA".into(), false),
        ("HAVE_OBS_FFMPEG".into(), false),
        ("HAVE_OBS_QSV11".into(), false),
        ("HAVE_OBS_OPENGL".into(), false),
        ("HAVE_MEDIA_PLAYBACK_TEST_OWNER".into(), false),
        ("HAVE_SLIDESHOW_TEST_OWNER".into(), false),
        ("HAVE_STARTUP_GLAD".into(), false),
        ("HAVE_STARTUP_X11".into(), false),
        ("HAVE_FONTCONFIG_TEST_OWNER".into(), false),
        ("HAVE_FRONTEND_WARN_TEST_OWNER".into(), false),
        ("HAVE_FRONTEND_PLAIN_TEST_OWNER".into(), false),
        ("HAVE_X264_TEST_OWNER".into(), false),
        ("ENABLE_CORE_MODULES".into(), false),
        ("HAVE_PREFLIGHT_TEST_ARCHIVE".into(), false),
        ("HAVE_SCRIPTING_DEFS_TEST_SDK".into(), false),
    ]);
    let mut core_interface = SharedLibrary {
        include_dirs: vec![
            root.join("libobs"),
            root.join("deps/libcaption"),
            build.output.join("config"),
        ],
        dependencies: vec![core.into()],
        ..Default::default()
    };
    if options.hevc {
        core_interface
            .definitions
            .push(("ENABLE_HEVC".into(), None));
    }
    add_package(&mut core_interface, &build.target, "simde")?;
    headers_only(&mut core_interface, "jansson")?;
    crate::platform::configure_c_tests(build, &mut core_interface, None, core)?;
    let mut cmocka_interface = SharedLibrary::default();
    add_package(&mut cmocka_interface, &build.target, "cmocka")?;
    let thread_options = crate::platform::native_test_thread_options(&build.target);
    let mut interfaces = BTreeMap::from([
        (
            "Threads::Threads".into(),
            graph::Dependency {
                compile_options: thread_options.clone(),
                link_options: thread_options,
                ..Default::default()
            },
        ),
        ("OBS::libobs".into(), core_interface.into()),
        (
            "SIMDe::SIMDe".into(),
            graph::Dependency::package(&build.target, "simde")?,
        ),
        ("OBS::native-test-cmocka".into(), cmocka_interface.into()),
        (
            "m".into(),
            graph::Dependency {
                link_options: crate::platform::native_test_math_options(&build.target),
                ..Default::default()
            },
        ),
    ]);
    facts.insert(
        "HAVE_LIBOBS_RUST_STATIC".into(),
        options.rust_archive.is_some(),
    );
    if let Some(archive) = &options.rust_archive {
        interfaces.insert(
            "libobs_rust-static".into(),
            graph::Dependency::rust_static(&build.target, archive)?,
        );
    }
    interfaces.insert(
        "OBS::headless-characterization-headers".into(),
        headless::prepare_headers(build, &root)?,
    );
    let caption = crate::libobs::build_caption(build, &root)?;
    interfaces.insert(
        "OBS::caption".into(),
        graph::Dependency {
            artifacts: vec![caption.clone()],
            include_dirs: vec![root.join("deps/libcaption")],
            ..Default::default()
        },
    );
    for component in ["avcodec", "avformat", "avutil"] {
        let mut dependency = SharedLibrary::default();
        add_package(&mut dependency, &build.target, &format!("lib{component}"))?;
        interfaces.insert(format!("FFmpeg::{component}"), dependency.into());
    }
    let mut zlib = SharedLibrary::default();
    add_package(&mut zlib, &build.target, "zlib")?;
    interfaces.insert("ZLIB::ZLIB".into(), zlib.into());
    // This Linux-only compatibility builder inherits the compiler's system
    // header search (including CFLAGS/CPATH). Uthash is header-only and already
    // required by the built core; it contributes no artifact or link flags.
    interfaces.insert("Uthash::Uthash".into(), graph::Dependency::default());
    if options.pipewire {
        let mut dependency = SharedLibrary::default();
        add_package(&mut dependency, &build.target, "libpipewire-0.3")?;
        interfaces.insert("PipeWire::PipeWire".into(), dependency.into());
    }
    if options.pulseaudio || options.pulseaudio_plugin {
        let mut dependency = SharedLibrary::default();
        add_package(&mut dependency, &build.target, "libpulse")?;
        interfaces.insert("PulseAudio::PulseAudio".into(), dependency.into());
    }
    configure_headless_plugin_sdks(build, &mut facts, &interfaces);
    configure_transport_test_sdks(build, &root, &mut facts, &mut interfaces)?;
    if options.outputs {
        let mut happy = graph::Dependency {
            artifacts: vec![crate::deps::shared_archive(build, &root, "happy-eyeballs")?],
            include_dirs: crate::platform::native_test_happy_includes(&build.target, &root),
            ..Default::default()
        };
        happy.artifacts.push(core.into());
        interfaces.insert("OBS::happy-eyeballs".into(), happy);
        let mut tls = SharedLibrary::default();
        for package in ["mbedtls", "mbedx509", "mbedcrypto"] {
            add_package(&mut tls, &build.target, package)?;
        }
        interfaces.insert("MbedTLS::mbedtls".into(), tls.into());
        for name in ["opts-parser", "bpm"] {
            let mut artifacts = vec![
                crate::deps::shared_archive(build, &root, name)?,
                core.into(),
            ];
            if name == "bpm" {
                artifacts.push(caption.clone());
            }
            interfaces.insert(
                format!("OBS::{name}"),
                graph::Dependency {
                    artifacts,
                    include_dirs: vec![root.join("shared").join(name)],
                    ..Default::default()
                },
            );
        }
        let mut jansson = SharedLibrary::default();
        add_package(&mut jansson, &build.target, "jansson")?;
        interfaces.insert("jansson::jansson".into(), jansson.into());
        for name in ["crypt32", "iphlpapi", "winmm", "ws2_32"] {
            interfaces.insert(
                name.into(),
                graph::Dependency {
                    link_options: crate::platform::native_test_system_library(&build.target, name),
                    ..Default::default()
                },
            );
        }
        for name in ["Foundation", "Security"] {
            interfaces.insert(
                format!("OBS::native-test-{}", name.to_lowercase()),
                graph::Dependency {
                    link_options: crate::platform::native_test_framework_options(
                        &build.target,
                        name,
                    ),
                    ..Default::default()
                },
            );
        }
    }
    let result = compile_suite(build, &root, &graph, &facts, &interfaces)?;
    for directory in ["libobs", "test/cmocka", "deps/libcaption"] {
        println!("cargo:rerun-if-changed={}", root.join(directory).display());
    }
    Ok(result)
}

fn headers_only(library: &mut SharedLibrary, package: &str) -> Result<(), String> {
    let headers = pkg_config::Config::new()
        .cargo_metadata(false)
        .probe(package)
        .map_err(|e| e.to_string())?;
    library.include_dirs.extend(headers.include_paths);
    let mut definitions: Vec<_> = headers.defines.into_iter().collect();
    definitions.sort();
    library.definitions.extend(definitions);
    Ok(())
}

// Match optional CMake tests.d discovery within each original host/owner guard.
// FreeType platform defaults apply to Linux/BSD; PulseAudio/PipeWire to Linux.
fn configure_headless_plugin_sdks(
    build: &NativeBuild,
    facts: &mut BTreeMap<String, bool>,
    interfaces: &BTreeMap<String, graph::Dependency>,
) {
    let text_host = facts["OS_LINUX"] || facts["OS_FREEBSD"] || facts["OS_OPENBSD"];
    let text_sdk = text_host
        && ["freetype2", "fontconfig"]
            .iter()
            .all(|package| graph::Dependency::package(&build.target, package).is_ok());
    facts.insert("HAVE_FREETYPE_PLATFORM_SDK".into(), text_sdk);
    // This builder does not build linux-v4l2; keep its original owner guard off.
    if !facts["OS_LINUX"] {
        return;
    }
    facts.insert(
        "HAVE_PULSE_DEFAULT_RETRY_SDK".into(),
        facts["HAVE_OBS_PULSEAUDIO"] && interfaces.contains_key("PulseAudio::PulseAudio"),
    );
    let layout = facts["HAVE_OBS_PIPEWIRE"]
        && interfaces.contains_key("PipeWire::PipeWire")
        && ["libdrm", "gio-2.0"]
            .iter()
            .all(|package| graph::Dependency::package(&build.target, package).is_ok());
    facts.insert("HAVE_PIPEWIRE_FRAME_LAYOUT_SDK".into(), layout);
}

// Preserve optional discovery; this Linux compatibility stage has no file-updater
// or FFmpeg plugin owner. Focused replays supply those original owner facts.
fn configure_transport_test_sdks(
    build: &NativeBuild,
    root: &Path,
    facts: &mut BTreeMap<String, bool>,
    interfaces: &mut BTreeMap<String, graph::Dependency>,
) -> Result<(), String> {
    interfaces.insert(
        "OBS::ffmpeg-net-host".into(),
        graph::Dependency {
            include_dirs: vec![root.join("plugins/obs-ffmpeg/hosts/posix")],
            ..Default::default()
        },
    );
    for (fact, packages) in [
        (
            "HAVE_LIBRIST_TEST_SDK",
            vec![("Librist::Librist", "librist")],
        ),
        ("HAVE_CURL_TEST_SDK", vec![("CURL::libcurl", "libcurl")]),
    ] {
        let dependencies = packages
            .into_iter()
            .map(|(name, package)| {
                graph::Dependency::package(&build.target, package)
                    .map(|dependency| (name.into(), dependency))
            })
            .collect::<Result<Vec<_>, _>>();
        facts.insert(fact.into(), dependencies.is_ok());
        if let Ok(dependencies) = dependencies {
            interfaces.extend(dependencies);
        }
    }
    // Unlike RIST/curl, the original core+avformat guard makes avdevice
    // REQUIRED. A missing SDK is an error, never a silently omitted test.
    for (name, package) in [
        ("FFmpeg::avdevice", "libavdevice"),
        ("FFmpeg::swscale", "libswscale"),
        ("FFmpeg::swresample", "libswresample"),
    ] {
        interfaces.insert(
            name.into(),
            graph::Dependency::package(&build.target, package)?,
        );
    }
    facts.insert("HAVE_MEDIA_PACKET_TEST_SDK".into(), true);
    Ok(())
}
