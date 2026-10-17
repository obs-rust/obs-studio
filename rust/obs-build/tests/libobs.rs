//! Opt-in integration against real native dependencies and libobs-rust.
use obs_build::{NativeBuild, frontend, graphics, libobs, plugins};
use std::{ffi::c_void, path::PathBuf};
#[path = "libobs/application.rs"]
mod application;
#[path = "libobs/qt.rs"]
mod qt;
#[path = "libobs/render.rs"]
mod render;

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
        host: env!("OBS_BUILD_HOST").into(),
        target: env!("OBS_BUILD_TARGET").into(),
        output: std::env::var_os("OBS_BUILD_TEST_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(|| output.path().into()),
    };
    let mut options = libobs::Options {
        hevc: std::env::var_os("OBS_BUILD_DISABLE_HEVC").is_none(),
        core_modules: if enabled("OBS_BUILD_APP_GUI") {
            [
                "obs-transitions",
                "obs-filters",
                "image-source",
                "text-freetype2",
                "linux-pulseaudio",
                "linux-alsa",
                "linux-v4l2",
                "linux-capture",
                "rtmp-services",
                "linux-pipewire",
                "obs-vst",
                "obs-x264",
                "obs-outputs",
                "obs-ffmpeg",
            ]
            .into_iter()
            .map(String::from)
            .collect()
        } else {
            vec![]
        },
        ..libobs::Options::default()
    };
    if enabled("OBS_BUILD_APP_GUI") && std::env::var_os("OBS_BUILD_NVENC").is_some() {
        options.core_modules.push("obs-nvenc".into());
    }
    let libraries = libobs::build_pair(&build, &root, &options, &archive).unwrap();
    assert_ne!(libraries.reference, libraries.with_rust);
    assert_eq!(exports(&libraries.reference), exports(&libraries.with_rust));
    let nvenc = if std::env::var_os("OBS_BUILD_NVENC").is_some() {
        let archive = std::env::var_os("OBS_BUILD_NVENC_RUST_ARCHIVE")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("target/release/libobs_nvenc_plugin.a"));
        let nvenc = plugins::build_nvenc_pair(
            &build,
            &root,
            &libraries.reference,
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
    let mut modules = vec![
        plugins::build_transitions(&build, &root, &libraries.reference).unwrap(),
        plugins::build_filters(&build, &root, &libraries.reference, &filters).unwrap(),
    ];
    if enabled("OBS_BUILD_SOURCES") {
        for source in [
            plugins::SourceModule::Image,
            plugins::SourceModule::Text,
            plugins::SourceModule::PulseAudio,
            plugins::SourceModule::Alsa,
            plugins::SourceModule::V4l2 {
                udev: std::env::var_os("OBS_BUILD_DISABLE_UDEV").is_none(),
            },
            plugins::SourceModule::X11Capture,
            // Offline smoke tests must not start the update service.
            plugins::SourceModule::RtmpServices { updates: false },
        ] {
            modules
                .push(plugins::build_source(&build, &root, &libraries.reference, source).unwrap());
        }
    }
    if enabled("OBS_BUILD_PIPEWIRE") {
        modules.push(
            plugins::build_source(
                &build,
                &root,
                &libraries.reference,
                plugins::SourceModule::PipeWire,
            )
            .unwrap(),
        );
    }
    let frontend = (enabled("OBS_BUILD_QT") || enabled("OBS_BUILD_APP")).then(|| {
        qt::exercise(&build, &root);
        modules.push(plugins::build_vst(&build, &root, &libraries.reference).unwrap());
        frontend::build_api(&build, &root, &libraries.reference).unwrap()
    });
    if let (Some(frontend), Some(reference)) =
        (&frontend, std::env::var_os("OBS_BUILD_FRONTEND_REFERENCE"))
    {
        assert_eq!(exports(frontend), exports(&PathBuf::from(reference)));
    }
    let mut application = None;
    if enabled("OBS_BUILD_APP") {
        let gpu_archive = PathBuf::from(
            std::env::var_os("OBS_BUILD_GPU_ARCHIVE")
                .expect("build obs-gpu-status-frontend in release mode first"),
        );
        let executable = frontend::build_application(
            &build,
            &root,
            &libraries.reference,
            frontend.as_ref().unwrap(),
            &gpu_archive,
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
        modules.push(plugins::build_x264(&build, &root, &libraries.reference).unwrap());
        modules
            .push(plugins::build_outputs(&build, &root, &libraries.reference, &options).unwrap());
        let artifacts = plugins::build_ffmpeg(
            &build,
            &root,
            &libraries.reference,
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
    let runner = if ffmpeg.is_some() || nvenc.is_some() {
        let bin = build.output.join("test-bin");
        std::fs::create_dir_all(&bin).unwrap();
        if let Some(ffmpeg) = &ffmpeg {
            let helper = ffmpeg.deploy_muxer(&bin).unwrap();
            assert!(helper.is_file());
        }
        if let Some(nvenc) = &nvenc {
            std::fs::copy(&nvenc.helper, bin.join(nvenc.helper.file_name().unwrap())).unwrap();
        }
        let runner = bin.join("obs-native-integration");
        std::fs::copy(std::env::current_exe().unwrap(), &runner).unwrap();
        runner
    } else {
        std::env::current_exe().unwrap()
    };
    let wants_graphics = enabled("OBS_BUILD_RENDER")
        || enabled("OBS_BUILD_RECORD")
        || enabled("OBS_BUILD_SOURCES")
        || enabled("OBS_BUILD_PIPEWIRE")
        || nvenc.is_some();
    let renderer = wants_graphics
        .then(|| graphics::build_opengl(&build, &root, &libraries.reference, &options).unwrap());
    let probe = renderer
        .as_ref()
        .map(|_| render::build(&build, &root, &libraries.reference));
    if enabled("OBS_BUILD_APP_GUI") {
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
    let outputs: &[(&str, &str)] = if ffmpeg.is_some() {
        &[("mp4_output", "mp4"), ("ffmpeg_muxer", "mkv")]
    } else {
        &[("mp4_output", "mp4")]
    };
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
                "c_and_rust_twins_preserve_public_data_lifecycle_and_module_apis",
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
                    enabled("OBS_BUILD_RECORD")
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
        if std::env::var_os("OBS_BUILD_NVENC").is_some() {
            expected_types.extend([
                (b"obs_enum_encoder_types".as_slice(), "obs_nvenc_h264_soft"),
                (b"obs_enum_encoder_types".as_slice(), "obs_nvenc_h264_tex"),
                (b"obs_enum_encoder_types".as_slice(), "ffmpeg_nvenc"),
            ]);
        }
        if enabled("OBS_BUILD_RECORD") {
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
            "stream=codec_name,width,height,nb_read_frames:format=duration",
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
