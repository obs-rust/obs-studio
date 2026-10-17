//! Native OBS module builds using the same engine as bundled libobs.
use crate::{NativeBuild, SharedLibrary, deps::shared_archive, libobs::add_package};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Module {
    pub library: PathBuf,
    pub data: PathBuf,
}

#[derive(Debug)]
pub struct NvencArtifacts {
    pub reference: Module,
    pub with_rust: Module,
    pub helper: PathBuf,
}

pub fn build_nvenc_pair(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    rust_archive: &Path,
    ffmpeg_ids: bool,
) -> Result<NvencArtifacts, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    pkg_config::Config::new()
        .cargo_metadata(false)
        .atleast_version("12")
        .probe("ffnvcodec")
        .map_err(|e| format!("NVENC requires ffnvcodec >=12: {e}"))?;
    let mut library = module(build, &root, core, "obs-nvenc")?;
    library.include_dirs.push(root.join("deps/glad/include"));
    crate::platform::configure_nvenc(&build.target, &root, &mut library)?;
    add_package(&mut library, &build.target, "ffnvcodec")?;
    library.include_dirs.push(root.join("shared/opts-parser"));
    library
        .dependencies
        .push(shared_archive(build, &root, "opts-parser")?);
    library
        .dependencies
        .push(crate::graphics::build_glad(build, &root)?);
    if ffmpeg_ids {
        library
            .definitions
            .push(("REGISTER_FFMPEG_IDS".into(), None));
    }
    // Both plugins share all objects except their CUDA driver loader.
    let mut common = library.clone();
    common.name = "obs-nvenc-common".into();
    library.precompiled_objects = build.objects(&common)?.2;
    library.sources = vec![root.join("plugins/obs-nvenc/build/obs-nvenc.cuda-load+.h.c")];
    let reference = Module {
        library: build.shared_library(&library)?,
        data: root.join("plugins/obs-nvenc/data"),
    };
    library.name = "obs-nvenc-with-rust".into();
    library.file_name = reference
        .library
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    library.sources = vec![root.join("plugins/obs-nvenc/build/obs-nvenc.cuda-load-rust+.h.c")];
    library
        .dependencies
        .push(rust_archive.canonicalize().map_err(|e| e.to_string())?);
    crate::platform::configure_nvenc_rust(&build.target, &mut library)?;
    let with_rust = Module {
        library: build.shared_library(&library)?,
        data: reference.data.clone(),
    };
    let mut helper = SharedLibrary {
        name: "obs-nvenc-test".into(),
        sources: vec![
            root.join("plugins/obs-nvenc/obs-nvenc-test/build/obs-nvenc-test.core+.hpp.cpp"),
        ],
        include_dirs: vec![root.clone()],
        ..SharedLibrary::default()
    };
    add_package(&mut helper, &build.target, "ffnvcodec")?;
    crate::platform::configure_nvenc_helper(&build.target, &mut helper)?;
    Ok(NvencArtifacts {
        reference,
        with_rust,
        helper: build.executable(&helper)?,
    })
}

#[derive(Clone, Debug)]
pub struct FiltersOptions {
    pub speexdsp: bool,
    pub rnnoise: bool,
}

impl Default for FiltersOptions {
    fn default() -> Self {
        Self {
            speexdsp: true,
            rnnoise: true,
        }
    }
}

pub fn build_transitions(build: &NativeBuild, root: &Path, core: &Path) -> Result<Module, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let library = module(build, &root, core, "obs-transitions")?;
    finish(build, &root, library)
}

pub fn build_filters(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    options: &FiltersOptions,
) -> Result<Module, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut library = module(build, &root, core, "obs-filters")?;
    if options.speexdsp {
        add_package(&mut library, &build.target, "speexdsp")?;
        library
            .definitions
            .push(("LIBSPEEXDSP_ENABLED".into(), None));
    }
    if options.rnnoise {
        if add_package(&mut library, &build.target, "rnnoise").is_err() {
            // Match OBS's Linux fallback when no system RNNoise is installed.
            let directory = root.join("plugins/obs-filters");
            let sources: Vec<String> = serde_json::from_slice(
                &std::fs::read(directory.join("build/rnnoise-sources.json"))
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let rnnoise = SharedLibrary {
                name: "obs-rnnoise".into(),
                sources: sources
                    .iter()
                    .map(|source| directory.join(source))
                    .collect(),
                include_dirs: vec![directory.join("rnnoise/include")],
                definitions: vec![("COMPILE_OPUS".into(), None)],
                compile_options: library.compile_options.clone(),
                ..SharedLibrary::default()
            };
            library.dependencies.push(build.static_library(&rnnoise)?);
            library.include_dirs.extend(rnnoise.include_dirs);
        }
        library
            .definitions
            .push(("LIBRNNOISE_ENABLED".into(), None));
    }
    if options.speexdsp || options.rnnoise {
        library
            .sources
            .push(root.join("plugins/obs-filters/build/obs-filters.noise-suppress+.h.c"));
        library
            .definitions
            .push(("HAS_NOISEREDUCTION".into(), None));
    }
    finish(build, &root, library)
}

fn module(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    name: &str,
) -> Result<SharedLibrary, String> {
    let mut library = SharedLibrary {
        name: name.into(),
        sources: vec![root.join(format!("plugins/{name}/build/{name}.core+.h.c"))],
        include_dirs: vec![
            root.into(),
            root.join("libobs"),
            root.join("plugins").join(name),
            build.output.join("config"),
        ],
        dependencies: vec![core.canonicalize().map_err(|e| e.to_string())?],
        ..SharedLibrary::default()
    };
    crate::platform::configure_module(&build.target, &mut library)?;
    // libobs public graphics headers expose SIMDe vector types.
    add_package(&mut library, &build.target, "simde")?;
    Ok(library)
}

fn finish(build: &NativeBuild, root: &Path, library: SharedLibrary) -> Result<Module, String> {
    Ok(Module {
        data: root.join("plugins").join(&library.name).join("data"),
        library: build.shared_library(&library)?,
    })
}

#[derive(Clone, Debug)]
pub struct FfmpegOptions {
    pub logging: bool,
    pub mpegts: bool,
    pub nvenc: bool,
    pub mux_debug: bool,
}

impl Default for FfmpegOptions {
    fn default() -> Self {
        Self {
            logging: false,
            mpegts: true,
            nvenc: false,
            mux_debug: false,
        }
    }
}

#[derive(Debug)]
pub struct FfmpegArtifacts {
    pub module: Module,
    pub muxer: PathBuf,
}

impl FfmpegArtifacts {
    /// Deploy the mux helper beside the executable that loads the module.
    pub fn deploy_muxer(&self, bin_dir: &Path) -> Result<PathBuf, String> {
        let name = self
            .muxer
            .file_name()
            .ok_or("FFmpeg helper has no filename")?;
        std::fs::create_dir_all(bin_dir)
            .map_err(|e| format!("create {}: {e}", bin_dir.display()))?;
        let destination = bin_dir.join(name);
        let source = self.muxer.canonicalize().map_err(|e| e.to_string())?;
        if destination.canonicalize().ok().as_ref() == Some(&source) {
            return Ok(destination);
        }
        std::fs::copy(&self.muxer, &destination).map_err(|e| {
            format!(
                "deploy {} to {}: {e}",
                self.muxer.display(),
                destination.display()
            )
        })?;
        Ok(destination)
    }
}

pub fn build_x264(build: &NativeBuild, root: &Path, core: &Path) -> Result<Module, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut library = module(build, &root, core, "obs-x264")?;
    library.include_dirs.push(root.join("shared/opts-parser"));
    library
        .dependencies
        .push(shared_archive(build, &root, "opts-parser")?);
    add_package(&mut library, &build.target, "x264")?;
    finish(build, &root, library)
}

pub fn build_outputs(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    options: &crate::libobs::Options,
) -> Result<Module, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut library = module(build, &root, core, "obs-outputs")?;
    for group in ["av1", "mp4-mux", "rtmp", "net-if"] {
        library.sources.push(root.join(format!(
            "plugins/obs-outputs/build/obs-outputs.{group}+.h.c"
        )));
    }
    if options.hevc {
        library
            .sources
            .push(root.join("plugins/obs-outputs/build/obs-outputs.hevc+.h.c"));
        library.definitions.push(("ENABLE_HEVC".into(), None));
    }
    for source in [
        "amf", "cencode", "hashswf", "log", "md5", "parseurl", "rtmp",
    ] {
        library
            .sources
            .push(root.join(format!("plugins/obs-outputs/librtmp/{source}.c")));
    }
    for name in ["opts-parser", "happy-eyeballs", "bpm"] {
        library.include_dirs.push(root.join("shared").join(name));
        library
            .dependencies
            .push(shared_archive(build, &root, name)?);
    }
    library
        .dependencies
        .push(crate::libobs::build_caption(build, &root)?);
    library
        .definitions
        .extend(["USE_MBEDTLS", "CRYPTO"].map(|name| (name.into(), None)));
    for package in ["mbedtls", "mbedx509", "mbedcrypto", "zlib", "jansson"] {
        add_package(&mut library, &build.target, package)?;
    }
    crate::platform::configure_outputs(&build.target, &root, &mut library)?;
    finish(build, &root, library)
}

pub fn build_ffmpeg(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    options: &crate::libobs::Options,
    features: &FfmpegOptions,
) -> Result<FfmpegArtifacts, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut library = module(build, &root, core, "obs-ffmpeg")?;
    for group in ["mux", "output", "source", "video-encoders"] {
        library
            .sources
            .push(root.join(format!("plugins/obs-ffmpeg/build/obs-ffmpeg.{group}+.h.c")));
    }
    for (enabled, group, definition) in [
        (features.logging, "logging", "ENABLE_FFMPEG_LOGGING"),
        (features.nvenc, "nvenc", "ENABLE_FFMPEG_NVENC"),
        (features.mpegts, "mpegts", "NEW_MPEGTS_OUTPUT"),
    ] {
        if enabled {
            library
                .sources
                .push(root.join(format!("plugins/obs-ffmpeg/build/obs-ffmpeg.{group}+.h.c")));
            library.definitions.push((definition.into(), None));
        }
    }
    if options.hevc {
        library.definitions.push(("ENABLE_HEVC".into(), None));
    }
    for group in ["cache", "core"] {
        library.sources.push(root.join(format!(
            "shared/media-playback/build/media-playback.{group}+.h.c"
        )));
    }
    library.include_dirs.extend([
        root.join("shared/media-playback"),
        root.join("shared/opts-parser"),
    ]);
    library
        .dependencies
        .push(shared_archive(build, &root, "opts-parser")?);
    for package in [
        "libavcodec",
        "libavfilter",
        "libavformat",
        "libavdevice",
        "libavutil",
        "libswscale",
        "libswresample",
    ] {
        add_package(&mut library, &build.target, package)?;
    }
    if features.mpegts {
        for package in ["librist", "srt"] {
            add_package(&mut library, &build.target, package)?;
        }
    }
    crate::platform::configure_ffmpeg(&build.target, &root, &mut library)?;
    let module = finish(build, &root, library)?;
    let mut mux = SharedLibrary {
        name: "obs-ffmpeg-mux".into(),
        sources: vec![root.join("plugins/obs-ffmpeg/ffmpeg-mux/build/ffmpeg-mux.core+.h.c")],
        include_dirs: vec![
            root.clone(),
            root.join("libobs"),
            root.join("plugins/obs-ffmpeg/ffmpeg-mux"),
            build.output.join("config"),
        ],
        dependencies: vec![core.into()],
        ..SharedLibrary::default()
    };
    crate::platform::configure_module(&build.target, &mut mux)?;
    // Executable naming, not the MODULE filename selected by configure_module.
    mux.file_name = None;
    if features.mux_debug {
        mux.definitions
            .push(("ENABLE_FFMPEG_MUX_DEBUG".into(), None));
    }
    for package in ["simde", "libavcodec", "libavutil", "libavformat"] {
        add_package(&mut mux, &build.target, package)?;
    }
    Ok(FfmpegArtifacts {
        module,
        muxer: build.executable(&mux)?,
    })
}

/// Built-in source and service modules with independent optional features.
#[derive(Clone, Copy, Debug)]
pub enum SourceModule {
    Image,
    Text,
    PulseAudio,
    PipeWire,
    Alsa,
    V4l2 { udev: bool },
    X11Capture,
    RtmpServices { updates: bool },
}

impl SourceModule {
    pub fn name(self) -> &'static str {
        match self {
            Self::Image => "image-source",
            Self::Text => "text-freetype2",
            Self::PulseAudio => "linux-pulseaudio",
            Self::PipeWire => "linux-pipewire",
            Self::Alsa => "linux-alsa",
            Self::V4l2 { .. } => "linux-v4l2",
            Self::X11Capture => "linux-capture",
            Self::RtmpServices { .. } => "rtmp-services",
        }
    }
}

pub fn build_source(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    source: SourceModule,
) -> Result<Module, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut library = module(build, &root, core, source.name())?;
    library.sources.clear();
    crate::platform::configure_source(&build.target, &root, source, &mut library)?;
    if matches!(source, SourceModule::X11Capture | SourceModule::PipeWire) {
        library
            .dependencies
            .push(crate::graphics::build_glad(build, &root)?);
    }
    finish(build, &root, library)
}

pub fn build_vst(build: &NativeBuild, root: &Path, core: &Path) -> Result<Module, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut library = module(build, &root, core, "obs-vst")?;
    library.sources = vec![root.join("plugins/obs-vst/build/obs-vst.core+.hpp.cpp")];
    library
        .include_dirs
        .push(root.join("plugins/obs-vst/vst_header"));
    crate::platform::configure_vst(&build.target, &root, &mut library)?;
    add_package(&mut library, &build.target, "Qt6Widgets")?;
    crate::qt::moc(
        &mut library,
        &root.join("plugins/obs-vst/headers/VSTPlugin.h"),
        &build.output.join("obs-vst/qt/moc_VSTPlugin.cpp"),
    )?;
    finish(build, &root, library)
}
