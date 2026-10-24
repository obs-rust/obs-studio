//! Opt-in integration against real native dependencies and libobs-rust.
use obs_build::{NativeBuild, frontend, graphics, libobs, plugins, scripting};
use std::{ffi::c_void, path::PathBuf};
#[path = "libobs/application.rs"]
mod application;
#[path = "libobs/macos_capture.rs"]
mod macos_capture;
#[path = "libobs/macos_graphics.rs"]
mod macos_graphics;
#[path = "libobs/native_tests.rs"]
mod native_tests;
#[path = "libobs/qt.rs"]
mod qt;
#[path = "libobs/render.rs"]
mod render;
#[path = "libobs/scripting.rs"]
mod script_tests;
#[path = "libobs/tools.rs"]
mod tools;
#[path = "libobs/vlc.rs"]
mod vlc_tests;
#[path = "libobs/webrtc.rs"]
mod webrtc_tests;
#[path = "libobs/windows.rs"]
mod windows_tests;

#[test]
fn c_and_rust_twins_preserve_public_data_lifecycle_and_module_apis() {
    if let Some(path) = std::env::var_os("OBS_BUILD_TEST_CORE") {
        let modules: Vec<(PathBuf, PathBuf)> =
            serde_json::from_str(&std::env::var("OBS_BUILD_TEST_MODULES").unwrap()).unwrap();
        let modules: Vec<_> = modules
            .into_iter()
            .map(|(library, data)| plugins::Module { library, data })
            .collect();
        exercise(
            &PathBuf::from(path),
            &modules,
            std::env::var_os("OBS_BUILD_DISABLE_HEVC").is_none(),
        );
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let archive = PathBuf::from(
        std::env::var_os("OBS_BUILD_RUST_ARCHIVE")
            .expect("build libobs-rust in release mode, then set OBS_BUILD_RUST_ARCHIVE"),
    );
    let output = tempfile::tempdir().unwrap();
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: std::env::var_os("OBS_BUILD_TEST_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(|| output.path().into()),
    };
    // Snapshot before native builds: another Cargo invocation can replace the
    // original integration executable while this long-running process lives.
    let bin = build.output.join("test-bin");
    std::fs::create_dir_all(&bin).unwrap();
    let runner = bin.join("obs-native-integration");
    std::fs::copy(std::env::current_exe().unwrap(), &runner).unwrap();
    let mut options = libobs::Options {
        pch: std::env::var_os("OBS_BUILD_PCH").is_some(),
        unity: std::env::var_os("OBS_BUILD_DISABLE_CORE_UNITY").is_none(),
        hevc: std::env::var_os("OBS_BUILD_DISABLE_HEVC").is_none(),
        core_modules: if enabled("OBS_BUILD_APP_GUI") {
            let mut application =
                obs_build::application::BuildOptions::for_target(&build.target).unwrap();
            // This driver enables frontend plugins explicitly below; its
            // baseline must not request modules absent from that variant.
            application.frontend_tools = false;
            application.websocket = None;
            application.nvenc = None;
            application.core_module_names()
        } else {
            vec![]
        },
        ..libobs::Options::default()
    };
    if enabled("OBS_BUILD_APP_GUI") && std::env::var_os("OBS_BUILD_BROWSER").is_some() {
        assert!(
            std::env::var_os("OBS_BUILD_WEBSOCKET").is_some(),
            "browser GUI validation requires websocket"
        );
        options.core_modules.push("obs-browser".into());
    }
    for (flag, name) in [
        ("OBS_BUILD_JACK", "linux-jack"),
        ("OBS_BUILD_SNDIO", "sndio"),
    ] {
        if enabled("OBS_BUILD_APP_GUI") && std::env::var_os(flag).is_some() {
            options.core_modules.push(name.into());
        }
    }
    if enabled("OBS_BUILD_APP_GUI") && std::env::var_os("OBS_BUILD_FDK").is_some() {
        options.core_modules.push("obs-libfdk".into());
    }
    if enabled("OBS_BUILD_APP_GUI") && std::env::var_os("OBS_BUILD_QSV").is_some() {
        options.core_modules.push("obs-qsv11".into());
    }
    if enabled("OBS_BUILD_APP_GUI") && std::env::var_os("OBS_BUILD_WEBRTC").is_some() {
        options.core_modules.push("obs-webrtc".into());
    }
    if enabled("OBS_BUILD_APP_GUI") && std::env::var_os("OBS_BUILD_NVENC").is_some() {
        options.core_modules.push("obs-nvenc".into());
    }
    if std::env::var_os("OBS_BUILD_FRONTEND_TOOLS").is_some() {
        assert!(
            std::env::var_os("OBS_BUILD_APP_GUI").is_some(),
            "frontend Tools test requires OBS_BUILD_APP_GUI=1"
        );
        options.core_modules.push("frontend-tools".into());
    }
    if std::env::var_os("OBS_BUILD_WEBSOCKET").is_some() {
        assert!(
            enabled("OBS_BUILD_APP_GUI"),
            "websocket test requires the GUI"
        );
        options.core_modules.push("obs-websocket".into());
    }
    if std::env::var_os("OBS_BUILD_VLC").is_some() {
        assert!(
            std::env::var_os("OBS_BUILD_VLC_C_TESTS").is_some() || enabled("OBS_BUILD_APP_GUI"),
            "VLC integration requires either C tests or the staged GUI"
        );
        if enabled("OBS_BUILD_APP_GUI") {
            assert!(
                std::env::var_os("OBS_BUILD_WEBSOCKET").is_some(),
                "VLC GUI integration requires the websocket client"
            );
            options.core_modules.push("vlc-video".into());
        }
    }
    if std::env::var_os("OBS_BUILD_FRONTEND_TOOLS").is_some()
        || std::env::var_os("OBS_BUILD_WEBSOCKET").is_some()
    {
        options.core_modules.push("native-frontend-probe".into());
    }
    let core_started = std::time::Instant::now();
    let libraries = libobs::build_pair(&build, &root, &options, &archive).unwrap();
    let core_wall_seconds = core_started.elapsed().as_secs_f64();
    std::fs::write(
        build.output.join("core-build-timing.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"unity":options.unity,"pch":options.pch,"hevc":options.hevc,
            "target":build.target,"wall_seconds":core_wall_seconds}),
        )
        .unwrap(),
    )
    .unwrap();
    assert_ne!(libraries.reference, libraries.with_rust);
    assert_eq!(exports(&libraries.reference), exports(&libraries.with_rust));
    // Exercise the normal bundled/native entry point too: the explicit pair
    // above must not be the only route that can produce a working hybrid.
    let production = NativeBuild {
        profile: Default::default(),
        host: build.host.clone(),
        target: build.target.clone(),
        output: build.output.join("production"),
    };
    let production_core = libobs::build_with_options(&production, &root, &options).unwrap();
    assert_eq!(exports(&production_core), exports(&libraries.with_rust));
    assert!(!production.output.join("obs-c-reference").exists());
    if std::env::var_os("OBS_BUILD_C_TESTS").is_some() {
        let executables = obs_build::validation::build_c_tests(
            &build,
            &root,
            &libraries.with_rust,
            &obs_build::validation::Options {
                hevc: options.hevc,
                pulseaudio: options.pulseaudio,
                per_target_sanitizers: std::env::var_os("OBS_BUILD_DISABLE_C_TEST_SANITIZERS")
                    .is_none(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            executables.len(),
            137 + usize::from(options.pulseaudio) - usize::from(!options.hevc)
        );
        for name in ["test_scene_group_transform", "test_shader_sampler_convert"] {
            assert!(executables.iter().any(|test| test.name == name));
        }
        assert_eq!(
            executables
                .iter()
                .any(|test| test.name == "test_pulse_monitor_samples"),
            options.pulseaudio
        );
        native_tests::exercise(
            &build,
            &root,
            &libraries.reference,
            &libraries.with_rust,
            &executables,
            "core",
        );
        std::fs::copy(
            build.output.join("core-c-tests.json"),
            build.output.join("c-tests.json"),
        )
        .unwrap();
    }
    let nvenc = if std::env::var_os("OBS_BUILD_NVENC").is_some() {
        let archive = std::env::var_os("OBS_BUILD_NVENC_RUST_ARCHIVE")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("target/release/libobs_nvenc_plugin.a"));
        let nvenc = plugins::build_nvenc_pair(
            &build,
            &root,
            &libraries.with_rust,
            &archive,
            std::env::var_os("OBS_BUILD_DISABLE_NVENC_FFMPEG_IDS").is_none(),
        )
        .unwrap();
        assert_eq!(
            exports(&nvenc.reference.library),
            exports(&nvenc.with_rust.library)
        );
        let output = std::process::Command::new(&nvenc.helper).output().unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("nvenc_supported=true"));
        std::fs::write(build.output.join("nvenc-capabilities.ini"), &output.stdout).unwrap();
        if let Some(reference) = std::env::var_os("OBS_BUILD_NVENC_REFERENCE") {
            assert_eq!(
                exports(&nvenc.reference.library),
                exports(&PathBuf::from(reference))
            );
        }
        Some(nvenc)
    } else {
        None
    };
    let filters = plugins::FiltersOptions {
        speexdsp: std::env::var_os("OBS_BUILD_DISABLE_SPEEXDSP").is_none(),
        rnnoise: std::env::var_os("OBS_BUILD_DISABLE_RNNOISE").is_none(),
    };
    let tools_enabled = std::env::var_os("OBS_BUILD_FRONTEND_TOOLS").is_some();
    let scripting_enabled = (std::env::var_os("OBS_BUILD_SCRIPTING").is_some() || tools_enabled)
        && std::env::var_os("OBS_BUILD_DISABLE_SCRIPTING").is_none();
    let scripting_api = (scripting_enabled
        && (std::env::var_os("OBS_BUILD_SCRIPTING_FRONTEND").is_some() || tools_enabled))
        .then(|| frontend::build_api(&build, &root, &libraries.with_rust).unwrap());
    let script_artifacts = if scripting_enabled {
        let artifacts = scripting::build(
            &build,
            &root,
            &libraries.with_rust,
            scripting_api.as_deref(),
            &scripting::Options {
                lua: std::env::var_os("OBS_BUILD_DISABLE_LUA").is_none(),
                python: std::env::var_os("OBS_BUILD_DISABLE_PYTHON").is_none(),
                ..scripting::Options::default()
            },
        )
        .unwrap();
        script_tests::exercise(
            &build,
            &root,
            &artifacts,
            &[&libraries.reference, &libraries.with_rust],
            scripting_api.is_some(),
        );
        Some(artifacts)
    } else {
        None
    };
    let mut modules = vec![
        plugins::build_transitions(&build, &root, &libraries.with_rust).unwrap(),
        plugins::build_filters(&build, &root, &libraries.with_rust, &filters).unwrap(),
    ];
    for (flag, source) in [
        ("OBS_BUILD_JACK", plugins::SourceModule::Jack),
        ("OBS_BUILD_SNDIO", plugins::SourceModule::Sndio),
    ] {
        if std::env::var_os(flag).is_some() {
            modules
                .push(plugins::build_source(&build, &root, &libraries.with_rust, source).unwrap());
        }
    }
    if std::env::var_os("OBS_BUILD_FDK").is_some() {
        modules.push(plugins::build_fdk(&build, &root, &libraries.with_rust).unwrap());
    }
    let qsv_enabled = std::env::var_os("OBS_BUILD_QSV").is_some();
    if std::env::var_os("OBS_BUILD_WEBRTC").is_some() {
        let mut module = plugins::build_webrtc(
            &build,
            &root,
            &libraries.with_rust,
            &plugins::DataChannelSdk {
                include_dir: std::env::var_os("OBS_DATACHANNEL_INCLUDE_DIR")
                    .map(PathBuf::from)
                    .expect("set OBS_DATACHANNEL_INCLUDE_DIR"),
                library: std::env::var_os("OBS_DATACHANNEL_LIBRARY")
                    .map(PathBuf::from)
                    .expect("set OBS_DATACHANNEL_LIBRARY"),
                media: true,
                websocket: std::env::var_os("OBS_DATACHANNEL_DISABLE_WEBSOCKET").is_none(),
            },
            &plugins::WebRtcOptions {
                hevc: options.hevc,
                unity: std::env::var_os("OBS_BUILD_DISABLE_WEBRTC_UNITY").is_none(),
                pch: std::env::var_os("OBS_BUILD_WEBRTC_PCH").is_some(),
            },
        )
        .unwrap();
        if let Some(reference) = std::env::var_os("OBS_BUILD_WEBRTC_REFERENCE") {
            let reference = PathBuf::from(reference).canonicalize().unwrap();
            assert_eq!(exports(&module.library), exports(&reference));
            if std::env::var_os("OBS_BUILD_WEBRTC_USE_REFERENCE").is_some() {
                module.library = reference;
                println!("WebRTC runtime uses original CMake module");
            }
        } else {
            assert!(
                std::env::var_os("OBS_BUILD_WEBRTC_USE_REFERENCE").is_none(),
                "set OBS_BUILD_WEBRTC_REFERENCE before selecting its runtime"
            );
        }
        modules.push(module);
    }
    if qsv_enabled {
        let module = plugins::build_qsv(
            &build,
            &root,
            &libraries.with_rust,
            &plugins::QsvOptions {
                hevc: options.hevc,
                unity: std::env::var_os("OBS_BUILD_DISABLE_QSV_UNITY").is_none(),
                pch: std::env::var_os("OBS_BUILD_QSV_PCH").is_some(),
            },
        )
        .unwrap();
        if let Some(reference) = std::env::var_os("OBS_BUILD_QSV_REFERENCE") {
            assert_eq!(exports(&module.library), exports(&PathBuf::from(reference)));
        }
        modules.push(module);
    }
    if std::env::var_os("OBS_BUILD_VLC").is_some() {
        let vlc = plugins::build_source(
            &build,
            &root,
            &libraries.with_rust,
            plugins::SourceModule::Vlc,
        )
        .unwrap();
        let dynamic = std::process::Command::new("readelf")
            .arg("-d")
            .arg(&vlc.library)
            .output()
            .unwrap();
        assert!(dynamic.status.success());
        assert!(
            !String::from_utf8_lossy(&dynamic.stdout).contains("libvlc.so"),
            "VLC must remain an optional runtime dependency"
        );
        modules.push(vlc);
    }
    if enabled("OBS_BUILD_SOURCES") {
        for source in obs_build::application::source_modules(
            &build.target,
            std::env::var_os("OBS_BUILD_DISABLE_UDEV").is_none(),
            false, // Offline smoke tests must not start the update service.
        )
        .unwrap()
        .into_iter()
        .filter(|source| !matches!(source, plugins::SourceModule::PipeWire))
        {
            modules
                .push(plugins::build_source(&build, &root, &libraries.with_rust, source).unwrap());
        }
    }
    if enabled("OBS_BUILD_PIPEWIRE") {
        modules.push(
            plugins::build_source(
                &build,
                &root,
                &libraries.with_rust,
                plugins::SourceModule::PipeWire,
            )
            .unwrap(),
        );
    }
    let browser_enabled = std::env::var_os("OBS_BUILD_BROWSER").is_some();
    let frontend =
        (enabled("OBS_BUILD_QT") || enabled("OBS_BUILD_APP") || browser_enabled).then(|| {
            qt::exercise(&build, &root);
            modules.push(plugins::build_vst(&build, &root, &libraries.with_rust).unwrap());
            scripting_api.clone().unwrap_or_else(|| {
                frontend::build_api(&build, &root, &libraries.with_rust).unwrap()
            })
        });
    let frontend_tools = tools_enabled.then(|| {
        plugins::build_frontend_tools(
            &build,
            &root,
            &libraries.with_rust,
            frontend
                .as_deref()
                .expect("frontend Tools test requires OBS_BUILD_APP_GUI=1"),
            script_artifacts.as_ref(),
        )
        .unwrap()
    });
    let websocket = std::env::var_os("OBS_BUILD_WEBSOCKET").map(|_| {
        plugins::build_websocket(
            &build,
            &root,
            &libraries.with_rust,
            frontend
                .as_deref()
                .expect("websocket requires the frontend API"),
            &plugins::WebsocketOptions {
                unity: std::env::var_os("OBS_BUILD_DISABLE_WEBSOCKET_UNITY").is_none(),
            },
        )
        .unwrap()
    });
    let browser = browser_enabled.then(|| {
        let artifacts = plugins::build_browser(
            &build,
            &root,
            &libraries.with_rust,
            frontend.as_ref().unwrap(),
            &plugins::BrowserOptions {
                cef_root: PathBuf::from(
                    std::env::var_os("OBS_CEF_ROOT").expect("set OBS_CEF_ROOT to the CEF SDK"),
                ),
                panels: std::env::var_os("OBS_BUILD_DISABLE_BROWSER_PANELS").is_none(),
                unity: std::env::var_os("OBS_BUILD_DISABLE_BROWSER_UNITY").is_none(),
            },
        )
        .unwrap();
        assert!(artifacts.helper.is_file());
        assert!(artifacts.cef_root.join("include/cef_version.h").is_file());
        let symbols = exports(&artifacts.module.library);
        for symbol in ["obs_browser_create_qcef", "obs_browser_qcef_version_export"] {
            assert_eq!(
                symbols.contains(symbol),
                std::env::var_os("OBS_BUILD_DISABLE_BROWSER_PANELS").is_none()
            );
        }
        for symbol in [
            "obs_module_load",
            "obs_module_set_pointer",
            "obs_module_ver",
        ] {
            assert!(
                symbols.contains(symbol),
                "missing browser entry point {symbol}"
            );
        }
        for path in [&artifacts.helper, &artifacts.module.library] {
            let deps = std::process::Command::new("readelf")
                .args(["-d"])
                .arg(path)
                .output()
                .unwrap();
            assert!(deps.status.success());
            let deps = String::from_utf8_lossy(&deps.stdout);
            assert!(deps.contains("libcef.so"));
        }
        let result = std::process::Command::new(&artifacts.helper)
            .output()
            .unwrap();
        // CEF documents -1 when no subprocess type was requested. Linux main
        // exposes that as 255; executing it also checks runtime library loading.
        assert_eq!(
            result.status.code(),
            Some(255),
            "CEF helper: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        artifacts
    });
    if let Some(browser) = &browser {
        std::fs::write(build.output.join("browser-build.json"), serde_json::to_vec_pretty(&serde_json::json!({"module":browser.module.library,"helper":browser.helper,"cef_root":browser.cef_root})).unwrap()).unwrap();
    }
    if let (Some(frontend), Some(reference)) =
        (&frontend, std::env::var_os("OBS_BUILD_FRONTEND_REFERENCE"))
    {
        assert_eq!(exports(frontend), exports(&PathBuf::from(reference)));
    }
    let mut application = None;
    if enabled("OBS_BUILD_APP") {
        let gpu_archive = PathBuf::from(
            std::env::var_os("OBS_BUILD_GPU_ARCHIVE")
                .expect("build obs-frontend in release mode first"),
        );
        let executable = frontend::build_application_with_options(
            &build,
            &root,
            &libraries.with_rust,
            frontend.as_ref().unwrap(),
            &gpu_archive,
            &frontend::ApplicationOptions {
                browser_panels: browser_enabled
                    && std::env::var_os("OBS_BUILD_DISABLE_BROWSER_PANELS").is_none(),
            },
        )
        .unwrap();
        let output = std::process::Command::new(&executable)
            .arg("--version")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("33.0"));
        println!(
            "OBS application version: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        );
        application = Some(executable);
    }
    let mut ffmpeg = None;
    if enabled("OBS_BUILD_RECORD") {
        modules.push(plugins::build_x264(&build, &root, &libraries.with_rust).unwrap());
        modules
            .push(plugins::build_outputs(&build, &root, &libraries.with_rust, &options).unwrap());
        let artifacts = plugins::build_ffmpeg(
            &build,
            &root,
            &libraries.with_rust,
            &options,
            &plugins::FfmpegOptions::default(),
        )
        .unwrap();
        modules.push(plugins::Module {
            library: artifacts.module.library.clone(),
            data: artifacts.module.data.clone(),
        });
        ffmpeg = Some(artifacts);
    }
    // Mirror the app layout: the module launches its helper beside the process.
    if ffmpeg.is_some() || nvenc.is_some() {
        let bin = runner.parent().unwrap();
        if let Some(ffmpeg) = &ffmpeg {
            let helper = ffmpeg.deploy_muxer(bin).unwrap();
            assert!(helper.is_file());
        }
        if let Some(nvenc) = &nvenc {
            std::fs::copy(&nvenc.helper, bin.join(nvenc.helper.file_name().unwrap())).unwrap();
        }
    }
    let wants_graphics = enabled("OBS_BUILD_RENDER")
        || enabled("OBS_BUILD_RECORD")
        || enabled("OBS_BUILD_SOURCES")
        || enabled("OBS_BUILD_PIPEWIRE")
        || nvenc.is_some()
        || qsv_enabled
        || captures_audio()
        || std::env::var_os("OBS_BUILD_VLC_C_TESTS").is_some()
        || std::env::var_os("OBS_BUILD_GRAPHICS_C_TESTS").is_some();
    let renderer = wants_graphics
        .then(|| graphics::build_opengl(&build, &root, &libraries.with_rust, &options).unwrap());
    let probe = renderer
        .as_ref()
        .map(|_| render::build(&build, &root, &libraries.with_rust));
    if std::env::var_os("OBS_BUILD_GRAPHICS_C_TESTS").is_some() {
        let tests = obs_build::validation::build_graphics_tests(
            &build,
            &root,
            &libraries.with_rust,
            renderer.as_ref().unwrap(),
        )
        .unwrap();
        assert_eq!(tests.len(), 12);
        native_tests::exercise(
            &build,
            &root,
            &libraries.reference,
            &libraries.with_rust,
            &tests,
            "graphics",
        );
    }
    if std::env::var_os("OBS_BUILD_VLC_C_TESTS").is_some() {
        let vlc = modules
            .iter()
            .find(|module| module.library.file_name() == Some(std::ffi::OsStr::new("vlc-video.so")))
            .expect("VLC compatibility tests require OBS_BUILD_VLC=1");
        vlc_tests::exercise(
            &build,
            &root,
            &libraries.reference,
            &libraries.with_rust,
            vlc,
            renderer.as_ref().unwrap(),
        );
    }
    if enabled("OBS_BUILD_APP_GUI") {
        let gui_probe = (frontend_tools.is_some() || websocket.is_some()).then(|| {
            tools::build(
                &build,
                &root,
                &libraries.with_rust,
                frontend.as_ref().unwrap(),
            )
        });
        application::exercise(
            application::Inputs {
                root: &root,
                build: &build,
                executable: application.as_ref().unwrap(),
                api: frontend.as_ref().unwrap(),
                renderer: renderer.as_ref().unwrap(),
                ffmpeg: ffmpeg.as_ref().unwrap(),
                nvenc_helper: nvenc.as_ref().map(|nvenc| nvenc.helper.as_path()),
                modules: &modules,
                frontend_tools: frontend_tools.as_ref(),
                websocket: websocket.as_ref(),
                browser: browser.as_ref(),
                script_artifacts: if tools_enabled {
                    script_artifacts.as_ref()
                } else {
                    None
                },
                gui_probe: gui_probe.as_ref(),
            },
            &[
                application::Variant {
                    core: &libraries.reference,
                    nvenc: nvenc.as_ref().map(|nvenc| &nvenc.reference),
                },
                application::Variant {
                    core: &libraries.with_rust,
                    nvenc: nvenc.as_ref().map(|nvenc| &nvenc.with_rust),
                },
            ],
        )
        .unwrap();
    }
    // libobs twins intentionally share one SONAME. Loading both in one process
    // can retain a prior plugin's dependency and bind it to the wrong OBS state.
    // Run each variant in the same fresh-process isolation as the C tests.
    let modules: Vec<_> = modules
        .iter()
        .map(|module| (&module.library, &module.data))
        .collect();
    let mov_recording = std::env::var_os("OBS_BUILD_MOV_RECORD").is_some();
    if mov_recording {
        assert!(enabled("OBS_BUILD_RECORD") && ffmpeg.is_some());
    }
    let outputs: &[(&str, &str)] = if mov_recording {
        &[
            ("mp4_output", "mp4"),
            ("ffmpeg_muxer", "mkv"),
            ("mov_output", "mov"),
        ]
    } else if ffmpeg.is_some() {
        &[("mp4_output", "mp4"), ("ffmpeg_muxer", "mkv")]
    } else {
        &[("mp4_output", "mp4")]
    };
    let mut qsv_registrations = Vec::<Vec<String>>::new();
    for (path, (output_id, extension)) in [&libraries.reference, &libraries.with_rust]
        .into_iter()
        .flat_map(|path| outputs.iter().map(move |output| (path, output)))
    {
        let mut selected = modules.clone();
        if let Some(nvenc) = &nvenc {
            let module = if path == &libraries.with_rust {
                &nvenc.with_rust
            } else {
                &nvenc.reference
            };
            selected.push((&module.library, &module.data));
        }
        let serialized = serde_json::to_string(&selected).unwrap();
        let mut command = std::process::Command::new(&runner);
        if *output_id != "mp4_output" {
            command.env_remove("OBS_BUILD_WHIP_ENDPOINT");
        }
        command.env("OBS_BUILD_TEST_OUTPUT_ID", output_id);
        if let Some(frontend) = &frontend {
            command.env("OBS_BUILD_TEST_FRONTEND_API", frontend);
        }
        // The helper is a fresh process; select the same twin as its parent.
        let mut search = vec![path.parent().unwrap().to_owned()];
        search.extend(std::env::split_paths(
            &std::env::var_os("LD_LIBRARY_PATH").unwrap_or_default(),
        ));
        command.env("LD_LIBRARY_PATH", std::env::join_paths(search).unwrap());
        if let Some(renderer) = &renderer {
            command.env("OBS_BUILD_TEST_RENDERER", renderer);
            command.env("OBS_BUILD_TEST_PROBE", probe.as_ref().unwrap());
        }
        let recording = enabled("OBS_BUILD_RECORD").then(|| {
            let variant = path
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();
            build
                .output
                .join(format!("recording-{variant}.{extension}"))
        });
        if let Some(recording) = &recording {
            command.env("OBS_BUILD_TEST_RECORDING", recording);
        }
        let run = command
            .args([
                "--exact",
                concat!(
                    module_path!(),
                    "::c_and_rust_twins_preserve_public_data_lifecycle_and_module_apis"
                )
                .split_once("::")
                .expect("module path contains the crate prefix")
                .1,
                "--nocapture",
            ])
            .env("OBS_BUILD_TEST_CORE", path)
            .env("OBS_BUILD_TEST_MODULES", &serialized)
            .output()
            .unwrap();
        for line in String::from_utf8_lossy(&run.stdout).lines().filter(|line| {
            line.contains("Loading up OpenGL")
                || line.contains("cc::Build graphics device")
                || line.contains("cc::Build recording")
                || line.contains("cc::Build color source")
                || line.contains("cc::Build audio encoder")
                || line.contains("cc::Build JACK capture")
                || line.contains("cc::Build sndio capture")
                || line.contains("cc::Build QSV encoders")
                || line.contains("cc::Build WHIP")
        }) {
            println!("{}: {line}", path.display());
        }
        assert!(
            run.status.success(),
            "{}\n{}\n{}",
            run.status,
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        );
        assert!(
            String::from_utf8_lossy(&run.stdout).contains("1 passed"),
            "native core child did not execute exactly one compatibility test"
        );
        if qsv_enabled {
            let stdout = String::from_utf8_lossy(&run.stdout);
            let encoders: Vec<String> = serde_json::from_str(
                stdout
                    .lines()
                    .find_map(|line| {
                        line.split_once("cc::Build QSV encoders: ")
                            .map(|(_, ids)| ids)
                    })
                    .expect("QSV capability inventory was not exercised"),
            )
            .unwrap();
            if let Some(reference) = std::env::var_os("OBS_BUILD_QSV_REFERENCE") {
                let reference = PathBuf::from(reference);
                let mut replaced = false;
                for (binary, _) in &mut selected {
                    if binary.file_name() == Some(std::ffi::OsStr::new("obs-qsv11.so")) {
                        *binary = &reference;
                        replaced = true;
                    }
                }
                assert!(replaced, "QSV reference did not replace the built module");
                let mut oracle = std::process::Command::new(command.get_program());
                oracle.args(command.get_args());
                for (key, value) in command.get_envs() {
                    if let Some(value) = value {
                        oracle.env(key, value);
                    } else {
                        oracle.env_remove(key);
                    }
                }
                let oracle = oracle
                    .env(
                        "OBS_BUILD_TEST_MODULES",
                        serde_json::to_string(&selected).unwrap(),
                    )
                    .env_remove("OBS_BUILD_TEST_RECORDING")
                    .output()
                    .unwrap();
                assert!(
                    oracle.status.success(),
                    "{}\n{}",
                    String::from_utf8_lossy(&oracle.stdout),
                    String::from_utf8_lossy(&oracle.stderr)
                );
                let oracle_stdout = String::from_utf8_lossy(&oracle.stdout);
                let reference_ids: Vec<String> = serde_json::from_str(
                    oracle_stdout
                        .lines()
                        .find_map(|line| {
                            line.split_once("cc::Build QSV encoders: ")
                                .map(|(_, ids)| ids)
                        })
                        .expect("reference QSV inventory was not exercised"),
                )
                .unwrap();
                assert_eq!(
                    encoders, reference_ids,
                    "QSV registrations differ from CMake"
                );
                println!("QSV reference runtime matched for {}", path.display());
            }
            if let Some(first) = qsv_registrations.first() {
                assert_eq!(
                    &encoders, first,
                    "QSV registrations differ across core variants"
                );
            }
            qsv_registrations.push(encoders);
            std::fs::write(
                build.output.join("qsv-registrations.json"),
                serde_json::to_vec_pretty(&qsv_registrations).unwrap(),
            )
            .unwrap();
        }
        if std::env::var_os("OBS_BUILD_FDK").is_some() && recording.is_some() {
            assert!(
                String::from_utf8_lossy(&run.stdout)
                    .contains("cc::Build audio encoder: libfdk_aac")
            );
        }
        if std::env::var_os("OBS_BUILD_JACK_CAPTURE").is_some() {
            assert!(String::from_utf8_lossy(&run.stdout).contains("cc::Build JACK capture:"));
        }
        if std::env::var_os("OBS_BUILD_SNDIO_CAPTURE_DEVICE").is_some() {
            assert!(String::from_utf8_lossy(&run.stdout).contains("cc::Build sndio capture:"));
        }
        if let Some(recording) = recording {
            verify_recording(&recording);
        }
    }
}

fn exercise(path: &std::path::Path, modules: &[plugins::Module], hevc: bool) {
    let module_config = tempfile::tempdir().unwrap();
    let module_config_path =
        std::ffi::CString::new(module_config.path().to_str().unwrap()).unwrap();
    // SAFETY: these signatures come from the unchanged obs.h/obs-data.h.
    // Symbols are called only while their library is loaded, data handles
    // are released, and each library owns its complete startup/shutdown.
    unsafe {
        let library = libloading::Library::new(path).unwrap();
        for symbol in [
            b"obs_hevc_keyframe".as_slice(),
            b"obs_parse_hevc_packet".as_slice(),
            b"obs_parse_hevc_packet_priority".as_slice(),
            b"obs_extract_hevc_headers".as_slice(),
        ] {
            assert_eq!(library.get::<unsafe extern "C" fn()>(symbol).is_ok(), hevc);
        }
        let frontend = std::env::var_os("OBS_BUILD_TEST_FRONTEND_API")
            .map(|path| libloading::Library::new(path).unwrap());
        if let Some(frontend) = &frontend {
            assert!(!frontend
                .get::<unsafe extern "C" fn() -> bool>(b"obs_frontend_recording_active")
                .unwrap()());
            assert!(
                frontend
                    .get::<unsafe extern "C" fn() -> *mut c_void>(b"obs_frontend_get_main_window")
                    .unwrap()()
                .is_null()
            );
        }
        let probe = std::env::var_os("OBS_BUILD_TEST_PROBE")
            .map(|path| libloading::Library::new(path).unwrap());
        if let Some(probe) = &probe {
            let open = probe
                .get::<unsafe extern "C" fn() -> bool>(b"obs_fixture_open_display")
                .unwrap();
            assert!(open(), "isolated X11 display could not be opened");
        }
        let startup = library
            .get::<unsafe extern "C" fn(
                *const std::ffi::c_char,
                *const std::ffi::c_char,
                *mut c_void,
            ) -> bool>(b"obs_startup")
            .unwrap();
        let shutdown = library
            .get::<unsafe extern "C" fn()>(b"obs_shutdown")
            .unwrap();
        let create = library
            .get::<unsafe extern "C" fn() -> *mut c_void>(b"obs_data_create")
            .unwrap();
        let set = library
            .get::<unsafe extern "C" fn(*mut c_void, *const std::ffi::c_char, i64)>(
                b"obs_data_set_int",
            )
            .unwrap();
        let get = library
            .get::<unsafe extern "C" fn(*mut c_void, *const std::ffi::c_char) -> i64>(
                b"obs_data_get_int",
            )
            .unwrap();
        let release = library
            .get::<unsafe extern "C" fn(*mut c_void)>(b"obs_data_release")
            .unwrap();
        assert!(startup(
            c"en-US".as_ptr(),
            module_config_path.as_ptr(),
            std::ptr::null_mut()
        ));
        if let Some(probe) = &probe {
            let renderer =
                std::ffi::CString::new(std::env::var("OBS_BUILD_TEST_RENDERER").unwrap()).unwrap();
            let data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../libobs/data")
                .canonicalize()
                .unwrap();
            let data_path = std::ffi::CString::new(format!("{}/", data_path.display())).unwrap();
            let initialize =
                probe
                    .get::<unsafe extern "C" fn(
                        *const std::ffi::c_char,
                        *const std::ffi::c_char,
                        bool,
                    ) -> i32>(b"obs_fixture_initialize")
                    .unwrap();
            assert_eq!(
                initialize(
                    renderer.as_ptr(),
                    data_path.as_ptr(),
                    enabled("OBS_BUILD_RECORD") || captures_audio()
                ),
                0
            );
        }
        let open = library
            .get::<unsafe extern "C" fn(
                *mut *mut c_void,
                *const std::ffi::c_char,
                *const std::ffi::c_char,
            ) -> i32>(b"obs_open_module")
            .unwrap();
        let init = library
            .get::<unsafe extern "C" fn(*mut c_void) -> bool>(b"obs_init_module")
            .unwrap();
        for module in modules {
            let binary = std::ffi::CString::new(module.library.to_str().unwrap()).unwrap();
            let data_path = std::ffi::CString::new(module.data.to_str().unwrap()).unwrap();
            let mut handle = std::ptr::null_mut();
            assert_eq!(open(&mut handle, binary.as_ptr(), data_path.as_ptr()), 0);
            assert!(init(handle));
        }
        let mut expected_types = vec![
            (b"obs_enum_transition_types".as_slice(), "fade_transition"),
            (b"obs_enum_filter_types".as_slice(), "noise_suppress_filter"),
            (b"obs_enum_filter_types".as_slice(), "gain_filter"),
        ];
        for (flag, id) in [
            ("OBS_BUILD_JACK", "jack_output_capture"),
            ("OBS_BUILD_SNDIO", "sndio_output_capture"),
        ] {
            if std::env::var_os(flag).is_some() {
                expected_types.push((b"obs_enum_input_types".as_slice(), id));
            }
        }
        if std::env::var_os("OBS_BUILD_FDK").is_some() {
            expected_types.push((b"obs_enum_encoder_types".as_slice(), "libfdk_aac"));
        }
        if std::env::var_os("OBS_BUILD_NVENC").is_some() {
            expected_types.extend([
                (b"obs_enum_encoder_types".as_slice(), "obs_nvenc_h264_soft"),
                (b"obs_enum_encoder_types".as_slice(), "obs_nvenc_h264_tex"),
                (b"obs_enum_encoder_types".as_slice(), "ffmpeg_nvenc"),
            ]);
        }
        if enabled("OBS_BUILD_RECORD") {
            let codecs = library
                .get::<unsafe extern "C" fn(*const std::ffi::c_char) -> *const std::ffi::c_char>(
                    b"obs_get_output_supported_video_codecs",
                )
                .unwrap();
            for id in [c"mp4_output", c"mov_output"] {
                let supported = codecs(id.as_ptr());
                assert!(!supported.is_null());
                let supported = std::ffi::CStr::from_ptr(supported).to_str().unwrap();
                assert_eq!(
                    supported.split(';').any(|codec| codec == "hevc"),
                    hevc,
                    "output HEVC advertisement does not match the build option"
                );
            }
            expected_types.extend([
                (b"obs_enum_encoder_types".as_slice(), "obs_x264"),
                (b"obs_enum_encoder_types".as_slice(), "ffmpeg_aac"),
                (b"obs_enum_output_types".as_slice(), "mp4_output"),
                (b"obs_enum_output_types".as_slice(), "ffmpeg_muxer"),
            ]);
            let maps = std::fs::read_to_string("/proc/self/maps").unwrap();
            let srt: std::collections::BTreeSet<_> = maps
                .lines()
                .filter_map(|line| line.split_whitespace().last())
                .filter(|path| path.contains("/libsrt.so"))
                .collect();
            assert_eq!(
                srt.len(),
                1,
                "exactly one SRT shared library must be loaded"
            );
        }
        if enabled("OBS_BUILD_SOURCES") {
            expected_types.extend([
                (b"obs_enum_input_types".as_slice(), "image_source"),
                (b"obs_enum_input_types".as_slice(), "color_source"),
                (b"obs_enum_input_types".as_slice(), "slideshow"),
                (b"obs_enum_input_types".as_slice(), "text_ft2_source"),
                (b"obs_enum_input_types".as_slice(), "pulse_input_capture"),
                (b"obs_enum_input_types".as_slice(), "pulse_output_capture"),
                (b"obs_enum_input_types".as_slice(), "alsa_input_capture"),
                (b"obs_enum_input_types".as_slice(), "v4l2_input"),
                (b"obs_enum_input_types".as_slice(), "xshm_input"),
                (b"obs_enum_input_types".as_slice(), "xcomposite_input"),
                (b"obs_enum_output_types".as_slice(), "virtualcam_output"),
                (b"obs_enum_service_types".as_slice(), "rtmp_common"),
                (b"obs_enum_service_types".as_slice(), "rtmp_custom"),
            ]);
        }
        if enabled("OBS_BUILD_QT") {
            expected_types.push((b"obs_enum_filter_types".as_slice(), "vst_filter"));
        }
        if std::env::var_os("OBS_BUILD_WEBRTC").is_some() {
            for (symbol, expected) in [
                (
                    b"obs_get_output_supported_video_codecs".as_slice(),
                    if hevc { "h264;hevc;av1" } else { "h264;av1" },
                ),
                (b"obs_get_output_supported_audio_codecs".as_slice(), "opus"),
            ] {
                let codecs = library
                    .get::<unsafe extern "C" fn(*const std::ffi::c_char) -> *const std::ffi::c_char>(symbol)
                    .unwrap();
                let supported = codecs(c"whip_output".as_ptr());
                assert!(!supported.is_null());
                assert_eq!(
                    std::ffi::CStr::from_ptr(supported).to_str().unwrap(),
                    expected
                );
            }
            expected_types.extend([
                (b"obs_enum_output_types".as_slice(), "whip_output"),
                (b"obs_enum_service_types".as_slice(), "whip_custom"),
            ]);
        }
        for (symbol, expected) in expected_types {
            let enumerate = library
                .get::<unsafe extern "C" fn(usize, *mut *const std::ffi::c_char) -> bool>(symbol)
                .unwrap();
            let mut found = false;
            for index in 0..1024 {
                let mut name = std::ptr::null();
                if !enumerate(index, &mut name) {
                    break;
                }
                assert!(!name.is_null());
                found |= std::ffi::CStr::from_ptr(name).to_str().unwrap() == expected;
            }
            let enabled = expected != "noise_suppress_filter"
                || std::env::var_os("OBS_BUILD_DISABLE_SPEEXDSP").is_none()
                || std::env::var_os("OBS_BUILD_DISABLE_RNNOISE").is_none();
            let enabled = enabled
                && (expected != "ffmpeg_nvenc"
                    || std::env::var_os("OBS_BUILD_DISABLE_NVENC_FFMPEG_IDS").is_none());
            assert_eq!(found, enabled, "unexpected registration of {expected}");
        }
        if std::env::var_os("OBS_BUILD_QSV").is_some() {
            let enumerate = library
                .get::<unsafe extern "C" fn(usize, *mut *const std::ffi::c_char) -> bool>(
                    b"obs_enum_encoder_types",
                )
                .unwrap();
            let mut ids = Vec::new();
            for index in 0..1024 {
                let mut id = std::ptr::null();
                if !enumerate(index, &mut id) {
                    break;
                }
                let id = std::ffi::CStr::from_ptr(id).to_str().unwrap();
                if id.starts_with("obs_qsv11") {
                    assert!(hevc || !id.contains("hevc"));
                    ids.push(id.to_owned());
                }
            }
            ids.sort();
            println!(
                "cc::Build QSV encoders: {}",
                serde_json::to_string(&ids).unwrap()
            );
        }
        if let Some(probe) = &probe {
            let render = probe
                .get::<unsafe extern "C" fn(
                    *const std::ffi::c_char,
                    *const std::ffi::c_char,
                ) -> i32>(b"obs_fixture_render")
                .unwrap();
            let recording = std::env::var("OBS_BUILD_TEST_RECORDING")
                .ok()
                .map(|path| std::ffi::CString::new(path).unwrap());
            let output_id = std::ffi::CString::new(
                std::env::var("OBS_BUILD_TEST_OUTPUT_ID").unwrap_or_else(|_| "mp4_output".into()),
            )
            .unwrap();
            assert_eq!(
                render(
                    recording
                        .as_ref()
                        .map_or(std::ptr::null(), |path| path.as_ptr()),
                    output_id.as_ptr()
                ),
                0,
                "graphics/frame rendering failed"
            );
            if std::env::var_os("OBS_BUILD_JACK_CAPTURE").is_some() {
                assert!(std::env::var_os("OBS_BUILD_JACK").is_some());
                assert_eq!(
                    probe
                        .get::<unsafe extern "C" fn() -> i32>(b"obs_fixture_capture_jack")
                        .unwrap()(),
                    0,
                    "JACK source did not receive the isolated writer's stereo samples"
                );
            }
            if let Ok(device) = std::env::var("OBS_BUILD_SNDIO_CAPTURE_DEVICE") {
                assert!(std::env::var_os("OBS_BUILD_SNDIO").is_some());
                let device = std::ffi::CString::new(device).unwrap();
                assert_eq!(
                    probe
                        .get::<unsafe extern "C" fn(*const std::ffi::c_char) -> i32>(
                            b"obs_fixture_capture_sndio",
                        )
                        .unwrap()(device.as_ptr()),
                    0,
                    "sndio source did not receive the isolated writer's stereo samples"
                );
            }
        }
        let data = create();
        assert!(!data.is_null());
        set(data, c"value".as_ptr(), -9223372036854775807);
        assert_eq!(get(data, c"value".as_ptr()), -9223372036854775807);
        release(data);
        shutdown();
        if let Some(probe) = &probe {
            probe
                .get::<unsafe extern "C" fn()>(b"obs_fixture_close_display")
                .unwrap()();
        }
    }
}

fn exports(path: &std::path::Path) -> std::collections::BTreeSet<String> {
    // This opt-in integration currently exercises the Linux native graph.
    let result = std::process::Command::new("nm")
        .args(["-D", "--defined-only"])
        .arg(path)
        .output()
        .expect("GNU nm is required for the Linux libobs export contract");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| line.split_whitespace().last())
        .map(|name| name.split('@').next().unwrap().to_owned())
        .collect()
}

fn verify_recording(path: &std::path::Path) {
    let result = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_frames",
            "-show_entries",
            "stream=codec_name,profile,sample_rate,channels,width,height,nb_read_frames:format=duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .expect("ffprobe is required for recording integration");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let streams = document["streams"].as_array().unwrap();
    let video = streams
        .iter()
        .find(|stream| stream["codec_name"] == "h264")
        .expect("missing H.264 track");
    let (width, height) = if std::env::var_os("OBS_BUILD_NVENC_ENCODE").is_some() {
        (640, 360)
    } else {
        (64, 64)
    };
    assert_eq!(video["width"], width);
    assert_eq!(video["height"], height);
    assert!(
        video["nb_read_frames"]
            .as_str()
            .unwrap()
            .parse::<u32>()
            .unwrap()
            >= 10
    );
    let audio = streams
        .iter()
        .find(|stream| stream["codec_name"] == "aac")
        .expect("missing AAC track");
    if std::env::var_os("OBS_BUILD_FDK").is_some() {
        assert_eq!(audio["profile"], "LC");
        assert_eq!(audio["sample_rate"], "48000");
        assert_eq!(audio["channels"], 2);
    }
    assert!(
        audio["nb_read_frames"]
            .as_str()
            .unwrap()
            .parse::<u32>()
            .unwrap()
            > 0
    );
    assert!(
        document["format"]["duration"]
            .as_str()
            .unwrap()
            .parse::<f64>()
            .unwrap()
            >= 0.2
    );
    println!("validated recording: {}", path.display());
}

fn enabled(name: &str) -> bool {
    std::env::var_os(name).is_some() || std::env::var_os("OBS_BUILD_APP_GUI").is_some()
}

fn captures_audio() -> bool {
    std::env::var_os("OBS_BUILD_JACK_CAPTURE").is_some()
        || std::env::var_os("OBS_BUILD_SNDIO_CAPTURE_DEVICE").is_some()
}

#[test]
fn macos_core_builds_from_the_canonical_manifest() {
    let Some(dependencies) = std::env::var_os("OBS_BUILD_MACOS_DEPENDENCIES") else {
        return; // Explicit SDK-dependent integration, including cross compilation.
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = PathBuf::from(
        std::env::var_os("OBS_BUILD_MACOS_OUTPUT").expect("set OBS_BUILD_MACOS_OUTPUT"),
    );
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: std::env::var("OBS_BUILD_MACOS_TARGET")
            .unwrap_or_else(|_| obs_build::BUILD_TARGET.into()),
        output,
    };
    assert!(build.target.contains("apple-darwin"));
    let options = libobs::Options {
        native_dependencies: Some(dependencies.into()),
        hevc: std::env::var_os("OBS_BUILD_DISABLE_HEVC").is_none(),
        pulseaudio: false,
        gio: false,
        wayland: false,
        unity: std::env::var_os("OBS_BUILD_MACOS_SEPARATE").is_none(),
        pch: std::env::var_os("OBS_BUILD_MACOS_PCH").is_some(),
        ..libobs::Options::default()
    };
    let library = if let Some(archive) = std::env::var_os("OBS_BUILD_MACOS_RUST_ARCHIVE") {
        let pair = libobs::build_pair(&build, &root, &options, &PathBuf::from(archive)).unwrap();
        let exports = |path: &std::path::Path| {
            let result = std::process::Command::new(
                std::env::var_os("OBS_BUILD_MACOS_NM").unwrap_or_else(|| "nm".into()),
            )
            .args(["-gUj"])
            .arg(path)
            .output()
            .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            String::from_utf8(result.stdout)
                .unwrap()
                .lines()
                .map(str::to_owned)
                .collect::<std::collections::BTreeSet<_>>()
        };
        let reference = exports(&pair.reference);
        let hybrid = exports(&pair.with_rust);
        assert!(!reference.is_empty());
        assert_eq!(reference, hybrid);
        if let Some(original) = std::env::var_os("OBS_BUILD_MACOS_REFERENCE") {
            assert_eq!(
                reference,
                exports(&PathBuf::from(original)),
                "Cargo must preserve the actual CMake reference exports"
            );
        }
        std::fs::write(
            build.output.join("macos-symbols.json"),
            serde_json::to_vec_pretty(&reference).unwrap(),
        )
        .unwrap();
        pair.with_rust
    } else {
        libobs::build_c_reference_with_options(&build, &root, &options).unwrap()
    };
    assert!(library.is_file());
    let framework = library.parent().unwrap().join("libobs.framework");
    let bytes = std::fs::read(&library).unwrap();
    assert_eq!(
        bytes,
        std::fs::read(framework.join("Versions/A/libobs")).unwrap()
    );
    assert_eq!(bytes, std::fs::read(framework.join("libobs")).unwrap());
    if std::env::var_os("OBS_BUILD_MACOS_CAPTURE").is_some() {
        use obs_build::plugins::{MacosCaptureModule, build_macos_capture};
        let mut symbols_by_module = std::collections::BTreeMap::new();
        let probe = macos_capture::build_probe(&build, &root, &options, &library);
        let mut paths = String::new();
        for module in [
            MacosCaptureModule::ScreenAudio,
            MacosCaptureModule::Camera,
            MacosCaptureModule::LegacyCamera,
        ] {
            let artifact = build_macos_capture(&build, &root, &library, &options, module).unwrap();
            assert!(artifact.library.is_file());
            assert_macos_module_image(&artifact.library);
            assert!(artifact.data.join("locale/en-US.ini").is_file());
            let symbols = macos_graphics_exports(&artifact.library);
            for required in [
                "_obs_module_load",
                "_obs_module_ver",
                "_obs_module_set_pointer",
                "_obs_module_description",
            ] {
                assert!(
                    symbols.contains(required),
                    "{} lacks {required}",
                    module.name()
                );
            }
            if let Some(reference) = std::env::var_os(format!(
                "OBS_BUILD_MACOS_{}_REFERENCE",
                module.name().replace('-', "_").to_uppercase()
            )) {
                assert_macos_module_image(std::path::Path::new(&reference));
                assert_eq!(
                    symbols,
                    macos_graphics_exports(std::path::Path::new(&reference)),
                    "{} exports differ from CMake",
                    module.name()
                );
            }
            symbols_by_module.insert(module.name(), symbols);
            for path in [&artifact.library, &artifact.data, &probe] {
                assert!(
                    !path.to_str().unwrap().contains(['\t', '\n', '\r']),
                    "capture probe manifest needs single-line paths"
                );
            }
            paths.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                module.name(),
                artifact.library.display(),
                artifact.data.display(),
                probe.display()
            ));
        }
        std::fs::write(build.output.join("macos-capture-paths.txt"), paths).unwrap();
        std::fs::write(
            build.output.join("macos-capture-symbols.json"),
            serde_json::to_vec_pretty(&symbols_by_module).unwrap(),
        )
        .unwrap();
    }
    if let Ok(mode) = std::env::var("OBS_BUILD_MACOS_GRAPHICS") {
        assert!(
            matches!(mode.as_str(), "1" | "opengl"),
            "select 1 (all renderers) or opengl (explicit cross-build subset)"
        );
        let mut renderers = vec![(
            "OPENGL",
            graphics::build_opengl(&build, &root, &library, &options).unwrap(),
        )];
        if mode == "1" {
            renderers.push((
                "METAL",
                graphics::build_metal(&build, &root, &library, &options).unwrap(),
            ));
        }
        let mut symbols_by_renderer = std::collections::BTreeMap::new();
        for (name, renderer) in &renderers {
            let symbols = macos_graphics_exports(renderer);
            for required in [
                "_device_get_name",
                "_device_get_type",
                "_device_create",
                "_device_destroy",
            ] {
                assert!(symbols.contains(required), "{name} lacks {required}");
            }
            if let Some(reference) = std::env::var_os(format!("OBS_BUILD_MACOS_{name}_REFERENCE")) {
                assert_eq!(
                    symbols,
                    macos_graphics_exports(std::path::Path::new(&reference)),
                    "{name} exports differ from CMake"
                );
            }
            symbols_by_renderer.insert(name.to_string(), symbols);
        }
        let probe = macos_graphics::build_probe(&build, &root, &options, &library);
        let mut paths = String::new();
        for (name, renderer) in &renderers {
            for path in [renderer, &probe] {
                assert!(
                    !path.to_str().unwrap().contains(['\t', '\n', '\r']),
                    "native probe manifest needs single-line paths"
                );
            }
            paths.push_str(&format!(
                "{name}\t{}\t{}\n",
                renderer.display(),
                probe.display()
            ));
        }
        std::fs::write(build.output.join("macos-graphics-paths.txt"), paths).unwrap();
        std::fs::write(
            build.output.join("macos-graphics-symbols.json"),
            serde_json::to_vec_pretty(&symbols_by_renderer).unwrap(),
        )
        .unwrap();
    }
    std::fs::write(
        build.output.join("macos-core-path.txt"),
        library.to_str().unwrap(),
    )
    .unwrap();
}

fn assert_macos_module_image(path: &std::path::Path) {
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(
        &bytes[..4],
        &[0xcf, 0xfa, 0xed, 0xfe],
        "not a 64-bit Mach-O image"
    );
    assert_eq!(
        u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
        8,
        "{} must be an MH_BUNDLE module, as in CMake",
        path.display()
    );
}

fn macos_graphics_exports(path: &std::path::Path) -> std::collections::BTreeSet<String> {
    let result = std::process::Command::new(
        std::env::var_os("OBS_BUILD_MACOS_NM").unwrap_or_else(|| "nm".into()),
    )
    .args(["-gUj"])
    .arg(path)
    .output()
    .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}
