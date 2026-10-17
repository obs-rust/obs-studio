//! Native target naming and linker conventions. Decisions use the requested
//! target triple, so a cross build never silently uses the build host's ABI.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

mod libobs_linux;

pub fn configure_libobs(
    target: &str,
    root: &Path,
    options: &crate::libobs::Options,
    sets: &std::collections::BTreeMap<String, Vec<String>>,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if target.contains("linux") {
        libobs_linux::configure(target, root, options, sets, library)
    } else {
        Err(format!(
            "real libobs native graph for {target} is not implemented yet"
        ))
    }
}

pub fn shared_filename(target: &str, name: &str) -> String {
    if target.contains("windows") {
        format!("{name}.dll")
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
            "-dynamiclib"
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
            command.arg(format!("-Wl,-install_name,@rpath/{filename}"));
            command.arg("-Wl,-rpath,@loader_path");
        } else {
            command.arg(format!("-Wl,-soname,{filename}"));
            command.arg("-Wl,--no-undefined");
            command.arg("-Wl,-rpath,$ORIGIN");
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

/// Prefer a shared dependency even when a .pc file lists static paths first.
fn package_shared_library_name(target: &str, name: &str) -> Option<String> {
    if target.contains("linux") {
        Some(format!("lib{name}.so"))
    } else if target.contains("apple") {
        Some(format!("lib{name}.dylib"))
    } else {
        // Windows requires import-library handling rather than linking the DLL.
        None
    }
}

pub fn package_shared_library(target: &str, paths: &[PathBuf], name: &str) -> Option<PathBuf> {
    let filename = package_shared_library_name(target, name)?;
    paths
        .iter()
        .map(|path| path.join(&filename))
        .find(|path| path.is_file())
}

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

pub fn configure_opengl(
    target: &str,
    root: &Path,
    options: &crate::libobs::Options,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if !target.contains("linux") {
        return Err(format!(
            "native OpenGL graph for {target} is not implemented yet"
        ));
    }
    configure_module(target, library)?;
    library.file_name = Some("libobs-opengl.so.33".into());
    library
        .sources
        .push(root.join("libobs-opengl/build/opengl.nix+.h.c"));
    for package in ["simde", "libdrm", "gl", "egl", "x11", "x11-xcb", "xcb"] {
        crate::libobs::add_package(library, target, package)?;
    }
    if options.wayland {
        library
            .sources
            .push(root.join("libobs-opengl/build/opengl.nix-wayland+.h.c"));
        for package in ["wayland-egl", "wayland-client"] {
            crate::libobs::add_package(library, target, package)?;
        }
    }
    library.link_options.push("-ldl".into());
    Ok(())
}

pub fn configure_glad(
    target: &str,
    root: &Path,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if target.contains("linux") {
        library.sources.push(root.join("deps/glad/src/glad_egl.c"));
        Ok(())
    } else {
        Err(format!(
            "native GL loader graph for {target} is not implemented yet"
        ))
    }
}

fn dependency_link_name(path: &Path, msvc: bool) -> PathBuf {
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
    configure_module(target, library)?;
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
        SourceModule::PipeWire => &["libpipewire-0.3", "gio-unix-2.0", "libdrm"],
        SourceModule::Image => &[],
        SourceModule::Text => &["freetype2", "fontconfig"],
        SourceModule::PulseAudio => &["libpulse"],
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

pub fn configure_nvenc(
    target: &str,
    root: &Path,
    library: &mut crate::SharedLibrary,
) -> Result<(), String> {
    if !target.contains("linux") {
        return Err("NVENC native graph is currently Linux-only".into());
    }
    for group in ["caps-cache-linux", "linux", "caps-cache", "override"] {
        library
            .sources
            .push(root.join(format!("plugins/obs-nvenc/build/obs-nvenc.{group}+.h.c")));
    }
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
    let inputs: Vec<String> =
        serde_json::from_value(manifest["linux"].clone()).map_err(|e| e.to_string())?;
    library
        .sources
        .extend(inputs.iter().map(|path| root.join(path)));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_dependency_wins_over_an_earlier_static_archive() {
        let dir = tempfile::tempdir().unwrap();
        let archive_dir = dir.path().join("static");
        let shared_dir = dir.path().join("shared");
        std::fs::create_dir_all(&archive_dir).unwrap();
        std::fs::create_dir_all(&shared_dir).unwrap();
        std::fs::write(archive_dir.join("libz.a"), []).unwrap();
        let shared = shared_dir.join("libz.so");
        std::fs::write(&shared, []).unwrap();
        let paths = [archive_dir, shared_dir];
        assert_eq!(
            package_shared_library("x86_64-unknown-linux-gnu", &paths, "z"),
            Some(shared)
        );
        assert_eq!(
            package_shared_library("x86_64-unknown-linux-gnu", &paths, "missing"),
            None
        );
    }
}
