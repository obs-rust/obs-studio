//! Native target naming and linker conventions. Decisions use the requested
//! target triple, so a cross build never silently uses the build host's ABI.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

mod application_linux;
mod graphics_macos;
mod graphics_windows;
mod libobs_linux;
mod libobs_macos;
mod libobs_windows;
mod plugins_macos;
mod qt_sdk;
pub(crate) mod resources;
mod swift_macos;

pub fn qt_wrapper_inputs(target: &str, root: &Path) -> Result<Vec<String>, String> {
    let path = root.join("shared/qt/wrappers/source-sets.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let sets: std::collections::BTreeMap<String, Vec<String>> =
        serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let host = if target.contains("windows") {
        "windows"
    } else if target.contains("apple") {
        "macos"
    } else {
        "posix"
    };
    let mut inputs = Vec::new();
    for key in ["common", host] {
        for input in sets
            .get(key)
            .ok_or_else(|| format!("Qt wrappers source set missing: {key}"))?
        {
            inputs.push(format!("shared/qt/wrappers/{input}"));
        }
    }
    Ok(inputs)
}

pub fn application_sources(
    target: &str,
    udev: bool,
    service_updates: bool,
) -> Result<Vec<crate::plugins::SourceModule>, String> {
    if !target.contains("linux") {
        return Err(format!("application graph is not implemented for {target}"));
    }
    use crate::plugins::SourceModule;
    Ok(vec![
        SourceModule::Image,
        SourceModule::Text,
        SourceModule::PulseAudio,
        SourceModule::Alsa,
        SourceModule::V4l2 { udev },
        SourceModule::X11Capture,
        SourceModule::RtmpServices {
            updates: service_updates,
        },
        SourceModule::PipeWire,
    ])
}

pub fn validate_application_build(build: &crate::NativeBuild) -> Result<(), String> {
    application_sources(&build.target, true, true)?;
    if build.host != build.target {
        return Err("application cross-build dependency discovery is not implemented".into());
    }
    Ok(())
}

pub fn application_rust_archives(
    target: &str,
    directory: &Path,
) -> Result<crate::application::RustArchives, String> {
    application_sources(target, true, true)?;
    Ok(crate::application::RustArchives {
        core: directory.join("liblibobs_rust.a"),
        gpu_status: directory.join("libobs_frontend.a"),
        nvenc: directory.join("libobs_nvenc_plugin.a"),
    })
}

pub fn configure_application_archive_command(
    build: &crate::NativeBuild,
    command: &mut Command,
) -> Result<(), String> {
    validate_application_build(build)?;
    application_linux::configure_archive_command(build, command)
}

pub fn stage_application_runtime(
    build: &crate::NativeBuild,
    inputs: &crate::application::RuntimeInputs<'_>,
    destination: &Path,
) -> Result<crate::application::StagedRuntime, String> {
    if !build.target.contains("linux") {
        return Err(format!(
            "application runtime graph is not implemented for {}",
            build.target
        ));
    }
    application_linux::stage(inputs, destination)
}

pub fn stage_core_identity(target: &str, artifact: &Path) -> Result<(), String> {
    if target.contains("apple-darwin") {
        libobs_macos::stage_identity(artifact)?;
    }
    Ok(())
}

pub fn configure_core_features(
    target: &str,
    options: &crate::libobs::Options,
    header: &mut String,
) {
    if target.contains("linux") {
        for (enabled, name) in [
            (options.gio, "GIO_FOUND"),
            (options.pulseaudio, "PULSEAUDIO_FOUND"),
            (options.wayland, "ENABLE_WAYLAND"),
        ] {
            if enabled {
                header.push_str(&format!("#define {name}\n"));
            }
        }
    }
}

pub(crate) enum PchKind {
    Gnu,
    Clang,
    Msvc,
}

pub(crate) fn pch_kind(target: &str, compiler: &cc::Tool) -> Result<PchKind, String> {
    if target.contains("windows-msvc") && compiler.is_like_msvc() {
        return Ok(PchKind::Msvc);
    }
    if !target.contains("linux") && !target.contains("apple-darwin") {
        return Err(format!("compiled PCH for {target} is not implemented yet"));
    }
    if compiler.is_like_clang() && !compiler.is_like_msvc() {
        Ok(PchKind::Clang)
    } else if target.contains("linux") && compiler.is_like_gnu() {
        Ok(PchKind::Gnu)
    } else {
        Err("compiled PCH requires Linux GCC or a Clang driver".into())
    }
}

pub fn configure_libobs(
    build: &crate::NativeBuild,
    root: &Path,
    options: &crate::libobs::Options,
    sets: &std::collections::BTreeMap<String, Vec<String>>,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    let target = &build.target;
    if target.contains("linux") {
        libobs_linux::configure(target, root, options, sets, library)
    } else if target.contains("apple-darwin") {
        libobs_macos::configure(root, sets, library)
    } else if target.contains("windows-msvc") {
        libobs_windows::configure(build, root, sets, library)
    } else {
        Err(format!(
            "real libobs native graph for {target} is not implemented yet"
        ))
    }
}

pub fn configure_core_dependencies(
    target: &str,
    options: &crate::libobs::Options,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if target.contains("apple-darwin") {
        return libobs_macos::dependencies(options, library);
    }
    if target.contains("windows-msvc") {
        return libobs_windows::dependencies(options, library);
    }
    for package in [
        "libavformat",
        "libavcodec",
        "libavutil",
        "libswscale",
        "libswresample",
        "jansson",
        "zlib",
        "simde",
    ] {
        crate::libobs::add_package(library, target, package)?;
    }
    Ok(())
}

pub fn shared_filename(target: &str, name: &str) -> String {
    library_filename(target, name, crate::LibraryKind::Shared)
}

pub fn library_filename(target: &str, name: &str, kind: crate::LibraryKind) -> String {
    if target.contains("windows") {
        format!("{name}.dll")
    } else if kind == crate::LibraryKind::Module {
        format!("{name}.so")
    } else if target.contains("apple") {
        format!("lib{name}.dylib")
    } else {
        format!("lib{name}.so")
    }
}

pub fn configure_shared_link(
    target: &str,
    command: &mut Command,
    artifact: &Path,
    objects: &[PathBuf],
    dependencies: &[PathBuf],
    msvc: bool,
    kind: crate::LibraryKind,
) -> Result<(), String> {
    if target.contains("windows") {
        if msvc {
            command.arg("/LD").args(objects);
            command.args(dependencies.iter().map(|p| dependency_link_name(p, true)));
            command
                .arg("/link")
                .arg(format!("/OUT:{}", artifact.display()))
                .arg(format!(
                    "/IMPLIB:{}",
                    artifact.with_extension("lib").display()
                ));
        } else {
            command.arg("-shared").args(objects);
            command.args(dependencies.iter().map(|p| dependency_link_name(p, false)));
            command.arg("-o").arg(artifact).arg(format!(
                "-Wl,--out-implib,{}",
                artifact.with_extension("dll.a").display()
            ));
        }
    } else {
        command.arg(if target.contains("apple") {
            match kind {
                crate::LibraryKind::Shared => "-dynamiclib",
                crate::LibraryKind::Module => "-bundle",
            }
        } else {
            "-shared"
        });
        command
            .args(objects)
            .args(dependencies)
            .arg("-o")
            .arg(artifact);
        let filename = artifact
            .file_name()
            .ok_or("shared library has no filename")?
            .to_string_lossy();
        if target.contains("apple") {
            if kind == crate::LibraryKind::Shared {
                command.arg(format!("-Wl,-install_name,@rpath/{filename}"));
            }
            command.arg("-Wl,-rpath,@loader_path");
        } else {
            if kind == crate::LibraryKind::Shared {
                command.arg(format!("-Wl,-soname,{filename}"));
            }
            command.arg("-Wl,--no-undefined");
            command.arg("-Wl,-rpath,$ORIGIN");
            if target.contains("linux") {
                // Installed modules live in lib64/obs-modules/core. Resolve their
                // core and frontend dependencies from this runtime first.
                command.arg("-Wl,-rpath,$ORIGIN/../..");
            }
        }
        for dependency in dependencies {
            let parent = dependency
                .parent()
                .ok_or("dependency has no parent directory")?;
            command.arg(format!("-Wl,-rpath,{}", parent.display()));
        }
    }
    Ok(())
}

mod shared_library;
pub use qt_sdk::{
    qt_architecture, qt_binary_architectures, qt_check_target, qt_compiler_id,
    qt_configuration_usable, qt_framework_search, qt_imported_target, qt_link_artifact,
    qt_runtime_directory, qt_system_library, qt_tool,
};
pub use shared_library::package_link_options;

pub fn configure_libobs_variant(
    build: &crate::NativeBuild,
    root: &Path,
    options: &crate::libobs::Options,
    sets: &std::collections::BTreeMap<String, Vec<String>>,
    library: &mut crate::SharedLibrary,
    rust_archive: Option<&Path>,
) -> Result<(), String> {
    if build.target.contains("linux") {
        libobs_linux::configure_variant(
            root,
            options,
            sets,
            library,
            rust_archive,
            &build.output.join("config"),
        )
    } else if build.target.contains("apple-darwin") {
        libobs_macos::configure_variant(
            root,
            options,
            sets,
            library,
            rust_archive,
            &build.output.join("config"),
        )
    } else if build.target.contains("windows-msvc") {
        libobs_windows::configure_variant(root, options, sets, library, rust_archive)
    } else {
        Err(format!(
            "real libobs variant for {} is not implemented yet",
            build.target
        ))
    }
}

pub fn configure_module(target: &str, library: &mut crate::SharedLibrary) -> Result<(), String> {
    if target.contains("linux") {
        library.file_name = Some(format!("{}.so", library.name));
        if !library
            .c_compile_options
            .iter()
            .any(|option| option == "-std=gnu17")
        {
            library.c_compile_options.push("-std=gnu17".into());
        }
        for flag in ["-fno-strict-aliasing", "-fvisibility=hidden"] {
            if !library.compile_options.iter().any(|option| option == flag) {
                library.compile_options.push(flag.into());
            }
        }
        for flag in ["-lm", "-pthread"] {
            if !library.link_options.iter().any(|option| option == flag) {
                library.link_options.push(flag.into());
            }
        }
        Ok(())
    } else {
        Err(format!(
            "native OBS module graph for {target} is not implemented yet"
        ))
    }
}

pub fn build_macos_capture(
    build: &crate::NativeBuild,
    root: &Path,
    core: &Path,
    options: &crate::libobs::Options,
    module: crate::plugins::MacosCaptureModule,
) -> Result<crate::plugins::Module, String> {
    if build.target.contains("apple-darwin") {
        plugins_macos::capture(build, root, core, options, module)
    } else {
        Err(format!(
            "macOS capture requires an Apple target, got {}",
            build.target
        ))
    }
}

pub fn configure_d3d11(
    build: &crate::NativeBuild,
    root: &Path,
    options: &crate::libobs::Options,
    library: &mut crate::SharedLibrary,
    gpu_priority: i32,
) -> Result<(), String> {
    if build.target.contains("windows-msvc") {
        graphics_windows::d3d11(build, root, options, library, gpu_priority)
    } else {
        Err(format!(
            "native Direct3D 11 graph for {} is not implemented yet",
            build.target
        ))
    }
}

pub fn configure_opengl(
    build: &crate::NativeBuild,
    root: &Path,
    options: &crate::libobs::Options,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    let target = &build.target;
    if target.contains("windows-msvc") {
        return graphics_windows::opengl(build, root, options, library);
    }
    if target.contains("apple-darwin") {
        return graphics_macos::opengl(root, options, library);
    }
    if !target.contains("linux") {
        return Err(format!(
            "native OpenGL graph for {target} is not implemented yet"
        ));
    }
    configure_module(target, library)?;
    library.file_name = Some("libobs-opengl.so.33".into());
    crate::graphics::append_sources(root, "OPENGL_LINUX", library)?;
    for package in ["simde", "libdrm", "gl", "egl", "x11", "x11-xcb", "xcb"] {
        crate::libobs::add_package(library, target, package)?;
    }
    if options.wayland {
        crate::graphics::append_sources(root, "OPENGL_WAYLAND", library)?;
        for package in ["wayland-egl", "wayland-client"] {
            crate::libobs::add_package(library, target, package)?;
        }
    }
    library.link_options.push("-ldl".into());
    Ok(())
}

pub fn configure_glad(
    build: &crate::NativeBuild,
    root: &Path,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    let target = &build.target;
    if target.contains("linux") {
        configure_module(target, library)?;
        crate::graphics::append_sources(root, "GLAD_EGL", library)
    } else if target.contains("windows-msvc") {
        graphics_windows::glad(build, root, library)
    } else if target.contains("apple-darwin") {
        graphics_macos::glad(library);
        Ok(())
    } else {
        Err(format!(
            "native GL loader graph for {target} is not implemented yet"
        ))
    }
}

pub fn build_metal(
    build: &crate::NativeBuild,
    root: &Path,
    core: &Path,
    options: &crate::libobs::Options,
) -> Result<PathBuf, String> {
    if build.target.contains("apple-darwin") {
        graphics_macos::metal(build, root, core, options)
    } else {
        Err(format!(
            "native Metal graph for {} is not implemented yet",
            build.target
        ))
    }
}

pub fn build_swift_library(
    build: &crate::NativeBuild,
    library: &crate::SwiftLibrary,
) -> Result<PathBuf, String> {
    if build.target.contains("apple-darwin") {
        swift_macos::build(build, library)
    } else {
        Err(format!(
            "native Swift graph for {} is not implemented yet",
            build.target
        ))
    }
}

fn dependency_link_name(path: &Path, msvc: bool) -> PathBuf {
    let path = dunce::simplified(path);
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("dll"))
    {
        path.with_extension(if msvc { "lib" } else { "dll.a" })
    } else {
        // Static archives already name linker inputs; they are not DLLs.
        path.into()
    }
}

pub fn executable_filename(target: &str, name: &str) -> String {
    if target.contains("windows") {
        format!("{name}.exe")
    } else {
        name.into()
    }
}

pub fn configure_executable_link(
    target: &str,
    command: &mut Command,
    artifact: &Path,
    objects: &[PathBuf],
    dependencies: &[PathBuf],
    msvc: bool,
) -> Result<(), String> {
    command.args(objects);
    if target.contains("windows") {
        command.args(
            dependencies
                .iter()
                .map(|path| dependency_link_name(path, msvc)),
        );
        if msvc {
            command
                .arg("/link")
                .arg(format!("/OUT:{}", artifact.display()));
        } else {
            command.arg("-o").arg(artifact);
        }
    } else {
        command.args(dependencies).arg("-o").arg(artifact);
        command.arg(if target.contains("apple") {
            "-Wl,-rpath,@executable_path"
        } else {
            "-Wl,-rpath,$ORIGIN"
        });
        if target.contains("linux") {
            // Development and installed application layouts use bin/ + lib64/.
            // This must precede the fallback paths to build artifacts.
            command.arg("-Wl,-rpath,$ORIGIN/../lib64");
        }
        for dependency in dependencies {
            let parent = dependency
                .parent()
                .ok_or("dependency has no parent directory")?;
            command.arg(format!("-Wl,-rpath,{}", parent.display()));
        }
    }
    Ok(())
}

pub fn configure_caption(target: &str, library: &mut crate::SharedLibrary) -> Result<(), String> {
    if target.contains("windows-msvc") {
        libobs_windows::defaults(library);
        return Ok(());
    }
    if target.contains("apple-darwin") {
        library.c_compile_options.push("-std=gnu17".into());
        library.compile_options.push("-fvisibility=hidden".into());
    } else {
        configure_module(target, library)?;
    }
    library.compile_options.push("-fopenmp-simd".into());
    Ok(())
}

pub fn configure_shared_dependency(
    target: &str,
    root: &Path,
    name: &str,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    configure_module(target, library)?;
    if name == "happy-eyeballs" {
        library
            .include_dirs
            .push(root.join("shared/happy-eyeballs/hosts/posix"));
    }
    Ok(())
}

pub fn configure_outputs(
    target: &str,
    root: &Path,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if !target.contains("linux") {
        return Err(format!(
            "native outputs graph for {target} is not implemented yet"
        ));
    }
    library
        .include_dirs
        .push(root.join("shared/happy-eyeballs/hosts/posix"));
    Ok(())
}

pub fn configure_ffmpeg(
    target: &str,
    root: &Path,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if !target.contains("linux") {
        return Err(format!(
            "native FFmpeg graph for {target} is not implemented yet"
        ));
    }
    for group in ["vaapi", "vaapi-device-select", "vaapi-probe-policy"] {
        library
            .sources
            .push(root.join(format!("plugins/obs-ffmpeg/build/obs-ffmpeg.{group}+.h.c")));
    }
    for package in ["libva", "libva-drm", "libpci", "libdrm"] {
        crate::libobs::add_package(library, target, package)?;
    }
    Ok(())
}

pub fn configure_qsv(
    target: &str,
    root: &Path,
    options: &crate::plugins::QsvOptions,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if !target.contains("linux") {
        return Err(format!(
            "QSV native graph for {target} is not implemented yet"
        ));
    }
    if !target.starts_with("x86_64-") {
        return Err(format!("QSV requires an x86_64 target, received {target}"));
    }
    pkg_config::Config::new()
        .cargo_metadata(false)
        .atleast_version("2.9")
        .probe("vpl")
        .map_err(|e| e.to_string())?;
    for package in ["vpl", "libva", "libva-drm"] {
        crate::libobs::add_package(library, target, package)?;
    }
    for group in ["linux+.hpp.cpp", "linux-c+.h.c", "core+.hpp.cpp", "c+.h.c"] {
        library
            .sources
            .push(root.join(format!("plugins/obs-qsv11/build/obs-qsv11.{group}")));
    }
    library.cpp_compile_options.push("-std=gnu++17".into());
    library
        .cpp_compile_options
        .push("-fvisibility-inlines-hidden".into());
    library.compile_options.push("-Wno-error=switch".into());
    library.definitions.push((
        "OBS_QSV_HOST_DEFS".into(),
        Some("\"plugins/obs-qsv11/bits/linux_defs.h\"".into()),
    ));
    if options.hevc {
        library.definitions.push(("ENABLE_HEVC".into(), None));
    }
    Ok(())
}

pub fn configure_webrtc_tests(
    target: &str,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if !target.contains("linux") {
        return Err(format!(
            "standalone WHIP test graph for {target} is not implemented yet"
        ));
    }
    library.c_compile_options.push("-std=gnu17".into());
    library.cpp_compile_options.push("-std=gnu++17".into());
    Ok(())
}

pub fn configure_webrtc(
    target: &str,
    root: &Path,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if !target.contains("linux") {
        return Err(format!(
            "WebRTC native graph for {target} is not implemented yet"
        ));
    }
    for group in ["posix", "core"] {
        library.sources.push(root.join(format!(
            "plugins/obs-webrtc/build/obs-webrtc.{group}+.hpp.cpp"
        )));
    }
    library
        .cpp_compile_options
        .extend(["-std=gnu++17".into(), "-fvisibility-inlines-hidden".into()]);
    Ok(())
}

pub fn configure_source(
    target: &str,
    root: &Path,
    source: crate::plugins::SourceModule,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    use crate::{libobs::add_package, plugins::SourceModule};
    if !target.contains("linux") {
        return Err(format!(
            "source module graph for {target} is not implemented yet"
        ));
    }
    let groups: &[&str] = match source {
        SourceModule::Jack | SourceModule::Sndio => &["nix"],
        SourceModule::Vlc => &["core", "posix"],
        SourceModule::PipeWire => &["nix"],
        SourceModule::Image => &["core", "slideshow", "posix"],
        SourceModule::Text => &["core", "nix"],
        SourceModule::PulseAudio | SourceModule::Alsa | SourceModule::X11Capture => &["nix"],
        SourceModule::V4l2 { udev: true } => &["nix", "nix-output", "nix-udev"],
        SourceModule::V4l2 { udev: false } => &["nix", "nix-output"],
        SourceModule::RtmpServices { .. } => &["core"],
    };
    library.sources.extend(
        groups
            .iter()
            .map(|group| root.join(format!("plugins/{0}/build/{0}.{group}+.h.c", source.name()))),
    );
    let packages: &[&str] = match source {
        SourceModule::Vlc => &[],
        SourceModule::PipeWire => &["libpipewire-0.3", "gio-unix-2.0", "libdrm"],
        SourceModule::Image => &[],
        SourceModule::Text => &["freetype2", "fontconfig"],
        SourceModule::PulseAudio => &["libpulse"],
        SourceModule::Jack => &["jack"],
        SourceModule::Sndio => &["sndio"],
        SourceModule::Alsa => &["alsa"],
        SourceModule::V4l2 { .. } => &["libv4l2", "libavcodec", "libavformat", "libavutil"],
        SourceModule::X11Capture => &[
            "x11",
            "x11-xcb",
            "xcb",
            "xcb-xfixes",
            "xcb-randr",
            "xcb-shm",
            "xcb-xinerama",
            "xcb-composite",
        ],
        SourceModule::RtmpServices { .. } => &["libcurl", "jansson"],
    };
    for package in packages {
        add_package(library, target, package)?;
    }
    match source {
        SourceModule::Sndio => {
            library.compile_options.push("-Wno-error=vla".into());
        }
        SourceModule::Vlc => {
            // Upstream loads libVLC at runtime and gracefully disables this
            // source when it is absent. Consume headers without a link edge.
            let headers = pkg_config::Config::new()
                .cargo_metadata(false)
                .probe("libvlc")
                .ok();
            for variable in ["OBS_VLC_INCLUDE_DIR", "VLC_PATH"] {
                println!("cargo:rerun-if-env-changed={variable}");
            }
            let mut includes = Vec::new();
            if let Some(path) = std::env::var_os("OBS_VLC_INCLUDE_DIR") {
                includes.push(std::path::PathBuf::from(path));
            }
            if let Some(path) = std::env::var_os("VLC_PATH") {
                includes.push(std::path::PathBuf::from(path).join("include"));
            }
            if let Some(headers) = headers {
                includes.extend(headers.include_paths);
                let mut definitions: Vec<_> = headers.defines.into_iter().collect();
                definitions.sort();
                library.definitions.extend(definitions);
            }
            includes.extend(["/usr/include".into(), "/usr/local/include".into()]);
            if !includes
                .iter()
                .any(|path| path.join("vlc/libvlc.h").is_file())
            {
                return Err("VLC headers not found; set OBS_VLC_INCLUDE_DIR or VLC_PATH".into());
            }
            library.include_dirs.extend(includes);
        }
        SourceModule::PipeWire => {
            pkg_config::Config::new()
                .cargo_metadata(false)
                .atleast_version("2.76")
                .probe("gio-2.0")
                .map_err(|e| e.to_string())?;
            pkg_config::Config::new()
                .cargo_metadata(false)
                .atleast_version("0.3.33")
                .probe("libpipewire-0.3")
                .map_err(|e| e.to_string())?;
            if pkg_config::Config::new()
                .cargo_metadata(false)
                .atleast_version("0.3.60")
                .probe("libpipewire-0.3")
                .is_ok()
            {
                library
                    .sources
                    .push(root.join("plugins/linux-pipewire/build/linux-pipewire.nix-camera+.h.c"));
            }
            library.include_dirs.push(root.join("deps/glad/include"));
        }
        SourceModule::V4l2 { udev: true } => {
            add_package(library, target, "libudev")?;
            library.definitions.push(("HAVE_UDEV".into(), None));
        }
        SourceModule::X11Capture => {
            library.include_dirs.push(root.join("deps/glad/include"));
        }
        SourceModule::RtmpServices { updates } => {
            library
                .sources
                .push(root.join("shared/file-updater/build/file-updater.core+.h.c"));
            library.include_dirs.push(root.join("shared/file-updater"));
            library.definitions.push((
                "SERVICES_URL".into(),
                Some("\"https://obsproject.com/obs2_update/rtmp-services\"".into()),
            ));
            if updates {
                library
                    .definitions
                    .push(("ENABLE_SERVICE_UPDATES".into(), None));
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn configure_c_tests(
    build: &crate::NativeBuild,
    library: &mut crate::SharedLibrary,
    sanitizer: Option<&str>,
    core: &Path,
) -> Result<(), String> {
    if !build.target.contains("linux") {
        return Err(format!(
            "C test graph for {} is not implemented yet",
            build.target
        ));
    }
    library.compile_options.extend([
        "-std=gnu17".into(),
        "-pthread".into(),
        "-Wno-double-promotion".into(),
        "-Wno-cast-qual".into(),
        "-Wno-format-nonliteral".into(),
        "-Wno-shadow".into(),
        "-fno-strict-aliasing".into(),
        "-fvisibility=hidden".into(),
        "-fopenmp-simd".into(),
    ]);
    library
        .definitions
        .push(("SIMDE_ENABLE_OPENMP".into(), None));
    library.link_options.extend([
        "-pthread".into(),
        "-lm".into(),
        "-Wl,--export-dynamic".into(),
    ]);
    // Standalone C characterizations (UTF-8, for example) may import no core
    // symbol. Keep their declared libobs dependency so validation can verify
    // the selected backend instead of silently dropping it under --as-needed.
    library.link_options.extend([
        "-Wl,--no-as-needed".into(),
        core.display().to_string(),
        "-Wl,--as-needed".into(),
    ]);
    configure_test_sanitizers(build, library, sanitizer)
}

pub fn native_test_facts(
    target: &str,
    pulseaudio: bool,
    hevc: bool,
) -> std::collections::BTreeMap<String, bool> {
    std::collections::BTreeMap::from([
        ("OS_LINUX".into(), target.contains("linux")),
        ("OS_FREEBSD".into(), target.contains("freebsd")),
        ("OS_WINDOWS".into(), target.contains("windows")),
        ("OS_MACOS".into(), target.contains("apple")),
        ("OS_OPENBSD".into(), target.contains("openbsd")),
        ("ENABLE_PULSEAUDIO".into(), pulseaudio),
        ("ENABLE_HEVC".into(), hevc),
        ("MSVC".into(), target.contains("msvc")),
    ])
}

/// Match the native thread interface: Darwin uses libSystem, while the other
/// non-MSVC toolchains need the driver flag during both compilation and linking.
pub fn native_test_thread_options(target: &str) -> Vec<String> {
    if target.contains("msvc") || target.contains("apple") {
        vec![]
    } else {
        vec!["-pthread".into()]
    }
}

pub fn native_test_math_options(target: &str) -> Vec<String> {
    if target.contains("msvc") {
        vec![]
    } else {
        vec!["-lm".into()]
    }
}

pub fn configure_test_sanitizers(
    build: &crate::NativeBuild,
    library: &mut crate::SharedLibrary,
    sanitizer: Option<&str>,
) -> Result<(), String> {
    let compiler = build.compiler(library, &build.output, false).get_compiler();
    // CMake's per-target sanitizer policy excludes MSVC, including clang-cl.
    if compiler.is_like_msvc() {
        return Ok(());
    }
    let mut already_sanitized = compiler
        .args()
        .iter()
        .any(|arg| arg.to_string_lossy().contains("-fsanitize="));
    for variable in [
        "CXXFLAGS".into(),
        "HOST_CXXFLAGS".into(),
        "TARGET_CXXFLAGS".into(),
        format!("CXXFLAGS_{}", build.target),
        format!("CXXFLAGS_{}", build.target.replace('-', "_")),
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
        already_sanitized |= std::env::var_os(variable)
            .is_some_and(|flags| flags.to_string_lossy().contains("-fsanitize="));
    }
    if let Some(sanitizer) = sanitizer.filter(|_| !already_sanitized) {
        let flag = format!("-fsanitize={sanitizer}");
        library
            .compile_options
            .extend([flag.clone(), "-fno-omit-frame-pointer".into()]);
        if sanitizer.split(',').any(|name| name == "undefined") {
            library
                .compile_options
                .push("-fno-sanitize-recover=undefined".into());
        }
        library.link_options.push(flag);
    }
    Ok(())
}

pub fn configure_cpp17(target: &str, library: &mut crate::SharedLibrary) {
    let standard = if target.contains("msvc") {
        "/std:c++17"
    } else {
        "-std=gnu++17"
    };
    if !library
        .cpp_compile_options
        .iter()
        .any(|option| option == standard)
    {
        library.cpp_compile_options.push(standard.into());
    }
}

pub fn configure_scripting(target: &str, library: &mut crate::SharedLibrary) -> Result<(), String> {
    if !target.contains("linux") {
        return Err("scripting native graph is currently Linux-only".into());
    }
    configure_cpp_module(target, library)?;
    library.file_name = None;
    library.c_compile_options.push("-Wno-cast-qual".into());
    Ok(())
}

pub fn configure_frontend_tools(
    target: &str,
    root: &Path,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    configure_cpp_module(target, library)?;
    // configure_cpp_module currently admits only Linux.
    for group in [
        "frontend-tools.linux+.hpp.cpp",
        "frontend-tools.host-posix+.h.c",
    ] {
        library
            .sources
            .push(root.join("plugins/frontend-tools/build").join(group));
    }
    crate::libobs::add_package(library, target, "x11")?;
    Ok(())
}

pub fn python_library_name(target: &str, libraries: &[String]) -> Result<String, String> {
    if !target.contains("linux") {
        return Err("Python library discovery is currently Linux-only".into());
    }
    let library = libraries
        .iter()
        .find(|name| name.starts_with("python"))
        .ok_or("Python pkg-config lacks a Python library")?;
    Ok(format!("lib{library}.so"))
}

pub fn scripting_module_name(target: &str, language: &str) -> Result<String, String> {
    if !target.contains("linux") {
        return Err("scripting binding naming is currently Linux-only".into());
    }
    match language {
        "lua" => Ok("obslua.so".into()),
        "python" => Ok("_obspython.so".into()),
        _ => Err(format!("unknown scripting language {language}")),
    }
}

pub fn configure_nvenc(
    target: &str,
    root: &Path,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if !target.contains("linux") {
        return Err("NVENC native graph is currently Linux-only".into());
    }
    library.sources = crate::plugins::nvenc_sources(root, "common")?;
    library
        .sources
        .extend(crate::plugins::nvenc_sources(root, "linux")?);
    Ok(())
}

pub fn configure_nvenc_rust(
    target: &str,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if !target.contains("linux") {
        return Err("NVENC Rust loader native linking is currently Linux-only".into());
    }
    library.link_options.extend([
        "-Wl,--exclude-libs,ALL".into(),
        "-ldl".into(),
        "-lpthread".into(),
        "-lm".into(),
    ]);
    Ok(())
}

pub fn configure_nvenc_helper(
    target: &str,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if !target.contains("linux") {
        return Err("NVENC capability helper native linking is currently Linux-only".into());
    }
    configure_cpp17(target, library);
    library
        .link_options
        .extend(["-pthread".into(), "-ldl".into()]);
    Ok(())
}

pub fn configure_cpp_module(
    target: &str,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    configure_module(target, library)?;
    configure_cpp17(target, library);
    library
        .cpp_compile_options
        .push("-fvisibility-inlines-hidden".into());
    Ok(())
}

pub fn configure_frontend_api(
    target: &str,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    configure_cpp_module(target, library)?;
    library.file_name = Some("libobs-frontend-api.so.30".into());
    Ok(())
}

pub fn configure_vst(
    target: &str,
    root: &Path,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    configure_cpp_module(target, library)?;
    library
        .sources
        .push(root.join("plugins/obs-vst/build/obs-vst.linux+.hpp.cpp"));
    library
        .include_dirs
        .push(root.join("plugins/obs-vst/hosts/linux"));
    crate::libobs::add_package(library, target, "xcb")?;
    Ok(())
}

pub fn configure_application(
    target: &str,
    root: &Path,
    manifest: &serde_json::Value,
    config: &Path,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if !target.contains("linux") {
        return Err(format!(
            "frontend application graph for {target} is not implemented yet"
        ));
    }
    configure_cpp_module(target, library)?;
    library.file_name = None;
    let sets: Vec<String> =
        serde_json::from_value(manifest["linux_sets"].clone()).map_err(|e| e.to_string())?;
    let inputs = crate::frontend::source_set_inputs(root, &sets)?;
    library.sources.extend(
        inputs
            .iter()
            .filter(|path| path.ends_with(".cpp") || path.ends_with(".c"))
            .map(|path| root.join(path)),
    );
    for package in ["Qt6DBus", "libpci", "x11", "x11-xcb", "xcb"] {
        crate::libobs::add_package(library, target, package)?;
    }
    library.definitions.extend([
        ("OBS_INSTALL_PREFIX".into(), Some("\"/usr/local\"".into())),
        ("DL_OPENGL".into(), Some("\"libobs-opengl.so.33\"".into())),
        ("DL_D3D11".into(), Some("\"\"".into())),
        ("DL_METAL".into(), Some("\"\"".into())),
    ]);
    library
        .link_options
        .extend(["-lgcc_s", "-lutil", "-lrt", "-ldl"].map(String::from));
    let mut paths =
        std::fs::read_to_string(root.join("frontend/plugin-manager/LoaderPaths_Linux.hpp.in"))
            .map_err(|e| e.to_string())?;
    for (name, value) in [
        ("OBS_PLATFORM_INSTALL_PATH", "/usr/local/"),
        ("OBS_PLATFORM_LIBRARY_PATH", "lib64/"),
        ("OBS_PLATFORM_DATA_PATH", "share/"),
        (
            "OBS_PLATFORM_PLUGIN_MODULE_PATH",
            "/usr/local/lib64/obs-modules/plugins",
        ),
        (
            "OBS_PLATFORM_PLUGIN_DATA_PATH",
            "/usr/local/share/obs/obs-modules/plugins",
        ),
    ] {
        paths = paths.replace(&format!("@{name}@"), value);
    }
    let output = config.join("LoaderPaths_Linux.hpp");
    if std::fs::read_to_string(&output).ok().as_deref() != Some(&paths) {
        std::fs::write(output, paths).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn configure_browser(
    target: &str,
    root: &Path,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    configure_cpp_module(target, library)?;
    library
        .cpp_compile_options
        .retain(|flag| !flag.starts_with("-std="));
    library.cpp_compile_options.push("-std=c++20".into());
    library.sources.extend([
        root.join("plugins/obs-browser/drm-format.cpp"),
        root.join("plugins/obs-browser/deps/ip-string-posix.cpp"),
    ]);
    for package in ["x11", "libdrm"] {
        crate::libobs::add_package(library, target, package)?;
    }
    library.link_options.push("-ldl".into());
    Ok(())
}

pub fn configure_browser_helper(
    target: &str,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if !target.contains("linux") {
        return Err(format!("browser helper for {target} is not implemented"));
    }
    library.cpp_compile_options.push("-std=c++20".into());
    library
        .link_options
        .extend(["-pthread".into(), "-ldl".into()]);
    Ok(())
}

pub fn configure_websocket(target: &str, library: &mut crate::SharedLibrary) -> Result<(), String> {
    configure_cpp_module(target, library)?;
    fn include(variable: &str, header: &str) -> Result<PathBuf, String> {
        println!("cargo:rerun-if-env-changed={variable}");
        let candidates = match std::env::var_os(variable) {
            Some(path) => vec![PathBuf::from(path)],
            None => vec![
                PathBuf::from("/usr/include"),
                PathBuf::from("/usr/local/include"),
                PathBuf::from("/usr/include/qrcodegen"),
                PathBuf::from("/usr/local/include/qrcodegen"),
            ],
        };
        candidates
            .into_iter()
            .find(|path| path.join(header).is_file())
            .ok_or_else(|| {
                format!("obs-websocket requires {header}; set {variable} to its include directory")
            })
    }
    let websocket = include("OBS_WEBSOCKETPP_INCLUDE_DIR", "websocketpp/version.hpp")?;
    let version_path = websocket.join("websocketpp/version.hpp");
    println!("cargo:rerun-if-changed={}", version_path.display());
    let header = std::fs::read_to_string(&version_path).map_err(|e| e.to_string())?;
    let version = |name: &str| -> Result<u32, String> {
        header
            .lines()
            .find(|line| line.contains(name))
            .and_then(|line| line.split('=').nth(1))
            .and_then(|value| value.trim().trim_end_matches(';').parse().ok())
            .ok_or_else(|| format!("cannot read WebSocket++ {name}"))
    };
    if (version("major_version")?, version("minor_version")?) < (0, 8) {
        return Err("obs-websocket requires WebSocket++ >=0.8".into());
    }
    library.include_dirs.push(websocket);
    library
        .include_dirs
        .push(include("OBS_QRCODEGEN_INCLUDE_DIR", "qrcodegen.hpp")?);
    println!("cargo:rerun-if-env-changed=OBS_QRCODEGEN_LIBRARY");
    if let Some(path) = std::env::var_os("OBS_QRCODEGEN_LIBRARY") {
        library.dependencies.push(
            PathBuf::from(path)
                .canonicalize()
                .map_err(|e| e.to_string())?,
        );
    } else {
        library.link_options.push("-lqrcodegencpp".into());
    }
    Ok(())
}

pub fn native_test_compiler_is_clang(build: &crate::NativeBuild) -> bool {
    let compiler = build
        .compiler(&crate::SharedLibrary::default(), &build.output, false)
        .get_compiler();
    compiler.is_like_clang() || compiler.is_like_clang_cl()
}

pub fn native_test_happy_includes(target: &str, root: &Path) -> Vec<PathBuf> {
    let host = if target.contains("windows") {
        "windows"
    } else {
        "posix"
    };
    vec![
        root.join("shared/happy-eyeballs"),
        root.join(format!("shared/happy-eyeballs/hosts/{host}")),
    ]
}

pub fn native_test_system_library(target: &str, name: &str) -> Vec<String> {
    vec![if target.contains("msvc") {
        format!("{name}.lib")
    } else {
        format!("-l{name}")
    }]
}

pub fn native_test_framework_options(target: &str, name: &str) -> Vec<String> {
    if target.contains("apple") {
        vec!["-framework".into(), name.into()]
    } else {
        vec![]
    }
}

cfg_select! {
    unix => {
        fn native_program_wrapper(program: &str, executable: &Path, root: &Path, output: &Path) -> Result<PathBuf, String> {
            use std::os::unix::fs::PermissionsExt;
            let template = root.join("test/cmocka/xvfb-run-or-display.sh.in");
            if program != "xvfb-run" || !template.is_file() { return Ok(executable.into()); }
            let executable = executable.to_str().ok_or("non-UTF8 native test display program")?;
            let quoted = executable.replace('\\', r"\\").replace('"', r#"\""#).replace('$', r"\$").replace('`', r"\`");
            let content = std::fs::read_to_string(template).map_err(|error| error.to_string())?.replace("@OBS_XVFB_RUN_REAL@", &quoted);
            let script = output.join("test-programs/xvfb-run-or-display.sh");
            std::fs::create_dir_all(script.parent().ok_or("native test wrapper has no parent")?).map_err(|error| error.to_string())?;
            crate::frontend::write(&script, &content)?;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).map_err(|error| error.to_string())?;
            dunce::canonicalize(&script).map_err(|error| error.to_string())
        }
        fn native_program_suffixes() -> Vec<String> { vec![String::new()] }
        fn native_program_executable(path: &Path) -> bool {
            use std::os::unix::fs::PermissionsExt;
            path.metadata().is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        }
    }
    windows => {
        fn native_program_wrapper(_: &str, executable: &Path, _: &Path, _: &Path) -> Result<PathBuf, String> { Ok(executable.into()) }
        fn native_program_suffixes() -> Vec<String> {
            let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
            let mut suffixes = vec![String::new()];
            suffixes.extend(extensions.split(';').filter(|extension| !extension.is_empty()).map(str::to_owned));
            suffixes
        }
        fn native_program_executable(path: &Path) -> bool { path.is_file() }
    }
    _ => {
        fn native_program_wrapper(_: &str, executable: &Path, _: &Path, _: &Path) -> Result<PathBuf, String> { Ok(executable.into()) }
        fn native_program_suffixes() -> Vec<String> { vec![String::new()] }
        fn native_program_executable(path: &Path) -> bool { path.is_file() }
    }
}

/// Resolve an optional test command on the execution host without a shell.
pub fn native_test_program(program: &str) -> Option<PathBuf> {
    let suffixes = native_program_suffixes();
    let path = Path::new(program);
    let candidates = if path.components().count() > 1 || path.is_absolute() {
        vec![path.to_owned()]
    } else {
        std::env::split_paths(&std::env::var_os("PATH")?)
            .map(|directory| directory.join(program))
            .collect()
    };
    for candidate in candidates {
        for suffix in &suffixes {
            let mut path = candidate.as_os_str().to_owned();
            path.push(suffix);
            let path = PathBuf::from(path);
            if native_program_executable(&path) {
                // Keep symlinks: resolving a busybox-style executable changes argv[0].
                return Some(if path.is_absolute() {
                    path
                } else {
                    std::env::current_dir().ok()?.join(path)
                });
            }
        }
    }
    None
}

pub fn native_test_command_program(
    program: &str,
    root: &Path,
    output: &Path,
) -> Result<Option<PathBuf>, String> {
    native_test_program(program)
        .map(|path| native_program_wrapper(program, &path, root, output))
        .transpose()
}
