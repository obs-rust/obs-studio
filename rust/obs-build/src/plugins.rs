//! Native OBS module builds using the same engine as bundled libobs.
use crate::{NativeBuild, SharedLibrary, deps::shared_archive, libobs::add_package};
use std::path::{Path, PathBuf};
mod browser;
mod frontend_tools;
mod websocket;
pub use browser::{
    Artifacts as BrowserArtifacts, Options as BrowserOptions, build as build_browser,
};
pub use frontend_tools::build as build_frontend_tools;
pub use websocket::{Options as WebsocketOptions, build as build_websocket};

#[derive(Debug)]
pub struct Module {
    pub library: PathBuf,
    pub data: PathBuf,
}

/// Native macOS display/audio and camera capture modules.
#[derive(Clone, Copy, Debug)]
pub enum MacosCaptureModule {
    ScreenAudio,
    Camera,
    LegacyCamera,
}

impl MacosCaptureModule {
    pub fn name(self) -> &'static str {
        match self {
            Self::ScreenAudio => "mac-capture",
            Self::Camera => "mac-avcapture",
            Self::LegacyCamera => "mac-avcapture-legacy",
        }
    }
}

pub fn build_macos_capture(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    options: &crate::libobs::Options,
    module: MacosCaptureModule,
) -> Result<Module, String> {
    crate::platform::build_macos_capture(build, root, core, options, module)
}

#[derive(Debug)]
pub struct NvencArtifacts {
    pub reference: Module,
    pub with_rust: Module,
    pub helper: PathBuf,
}

#[derive(Clone, Debug)]
pub struct NvencOptions {
    pub ffmpeg_ids: bool,
}

impl Default for NvencOptions {
    fn default() -> Self {
        Self { ffmpeg_ids: true }
    }
}

#[derive(Debug)]
pub struct NvencProductionArtifacts {
    pub module: Module,
    pub helper: PathBuf,
}

pub(crate) fn nvenc_sources(root: &Path, key: &str) -> Result<Vec<PathBuf>, String> {
    let manifest = root.join("plugins/obs-nvenc/build/source-sets.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let sets: std::collections::BTreeMap<String, Vec<String>> =
        serde_json::from_slice(&std::fs::read(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    Ok(sets
        .get(key)
        .ok_or_else(|| format!("NVENC source set missing: {key}"))?
        .iter()
        .map(|source| root.join("plugins/obs-nvenc").join(source))
        .collect())
}

fn nvenc_library(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    options: &NvencOptions,
) -> Result<SharedLibrary, String> {
    let mut library = module(build, root, core, "obs-nvenc")?;
    library.include_dirs.push(root.join("deps/glad/include"));
    crate::platform::configure_nvenc(&build.target, root, &mut library)?;
    let package = pkg_config::Config::new()
        .cargo_metadata(false)
        .atleast_version("12")
        .probe("ffnvcodec")
        .map_err(|e| format!("NVENC requires ffnvcodec >=12: {e}"))?;
    library.include_dirs.extend(package.include_paths);
    library.include_dirs.push(root.join("shared/opts-parser"));
    library
        .dependencies
        .push(shared_archive(build, root, "opts-parser")?);
    library
        .dependencies
        .push(crate::graphics::build_glad(build, root)?);
    if options.ffmpeg_ids {
        library
            .definitions
            .push(("REGISTER_FFMPEG_IDS".into(), None));
    }
    Ok(library)
}

fn nvenc_rust_module(
    build: &NativeBuild,
    root: &Path,
    rust_archive: &Path,
    mut library: SharedLibrary,
) -> Result<Module, String> {
    library.sources.extend(nvenc_sources(root, "rust")?);
    library
        .dependencies
        .push(rust_archive.canonicalize().map_err(|e| e.to_string())?);
    crate::platform::configure_nvenc_rust(&build.target, &mut library)?;
    Ok(Module {
        library: build.shared_library(&library)?,
        data: root.join("plugins/obs-nvenc/data"),
    })
}

fn nvenc_helper(build: &NativeBuild, root: &Path) -> Result<PathBuf, String> {
    let mut helper = SharedLibrary {
        name: "obs-nvenc-test".into(),
        sources: nvenc_sources(root, "helper")?,
        include_dirs: vec![root.into()],
        ..SharedLibrary::default()
    };
    add_package(&mut helper, &build.target, "ffnvcodec")?;
    crate::platform::configure_nvenc_helper(&build.target, &mut helper)?;
    build.executable(&helper)
}

/// Production builds only the completed Rust loader, never its C reference.
pub fn build_nvenc(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    rust_archive: &Path,
    options: &NvencOptions,
) -> Result<NvencProductionArtifacts, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if !rust_archive.is_file() {
        return Err(format!(
            "Rust archive does not exist: {}",
            rust_archive.display()
        ));
    }
    let library = nvenc_library(build, &root, core, options)?;
    Ok(NvencProductionArtifacts {
        module: nvenc_rust_module(build, &root, rust_archive, library)?,
        helper: nvenc_helper(build, &root)?,
    })
}

pub fn build_nvenc_pair(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    rust_archive: &Path,
    ffmpeg_ids: bool,
) -> Result<NvencArtifacts, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut library = nvenc_library(build, &root, core, &NvencOptions { ffmpeg_ids })?;
    // Both plugins share all objects except their CUDA driver loader.
    let mut common = library.clone();
    common.name = "obs-nvenc-common".into();
    library.precompiled_objects = build.objects(&common)?.2;
    library.sources = nvenc_sources(&root, "reference")?;
    let reference = Module {
        library: build.shared_library(&library)?,
        data: root.join("plugins/obs-nvenc/data"),
    };
    library.name = "obs-nvenc-with-rust".into();
    library.file_name = reference
        .library
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    library.sources.clear();
    let with_rust = nvenc_rust_module(build, &root, rust_archive, library)?;
    Ok(NvencArtifacts {
        reference,
        with_rust,
        helper: nvenc_helper(build, &root)?,
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

/// Build the optional original FDK AAC encoder module.
pub fn build_fdk(build: &NativeBuild, root: &Path, core: &Path) -> Result<Module, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut library = module(build, &root, core, "obs-libfdk")?;
    add_package(&mut library, &build.target, "fdk-aac")?;
    finish(build, &root, library)
}

#[derive(Clone, Debug)]
pub struct DataChannelSdk {
    pub include_dir: PathBuf,
    pub library: PathBuf,
    pub media: bool,
    pub websocket: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct WebRtcOptions {
    pub hevc: bool,
    pub unity: bool,
    pub pch: bool,
}

impl Default for WebRtcOptions {
    fn default() -> Self {
        Self {
            hevc: true,
            unity: true,
            pch: false,
        }
    }
}

/// Build the original WebRTC WHIP output and service module.
pub fn build_webrtc(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    sdk: &DataChannelSdk,
    options: &WebRtcOptions,
) -> Result<Module, String> {
    if !sdk.media {
        return Err("OBS WebRTC requires libdatachannel media support".into());
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let include = sdk.include_dir.canonicalize().map_err(|e| e.to_string())?;
    let version = std::fs::read_to_string(include.join("rtc/version.h"))
        .map_err(|e| format!("read libdatachannel version: {e}"))?;
    let mut numbers = Vec::new();
    for field in [
        "RTC_VERSION_MAJOR",
        "RTC_VERSION_MINOR",
        "RTC_VERSION_PATCH",
    ] {
        let number = version
            .lines()
            .find_map(|line| {
                let mut words = line.split_whitespace();
                (words.next() == Some("#define") && words.next() == Some(field))
                    .then(|| words.next().and_then(|word| word.parse::<u32>().ok()))
                    .flatten()
            })
            .ok_or_else(|| format!("invalid libdatachannel version field {field}"))?;
        numbers.push(number);
    }
    if (numbers[0], numbers[1], numbers[2]) < (0, 20, 0) {
        return Err("OBS WebRTC requires libdatachannel >= 0.20".into());
    }
    let mut library = module(build, &root, core, "obs-webrtc")?;
    library.sources.clear();
    library.include_dirs.push(include);
    library
        .dependencies
        .push(sdk.library.canonicalize().map_err(|e| e.to_string())?);
    for (name, enabled) in [
        ("RTC_ENABLE_MEDIA", sdk.media),
        ("RTC_ENABLE_WEBSOCKET", sdk.websocket),
    ] {
        library
            .definitions
            .push((name.into(), Some(u8::from(enabled).to_string())));
    }
    add_package(&mut library, &build.target, "libcurl")?;
    if options.hevc {
        library.definitions.push(("ENABLE_HEVC".into(), None));
    }
    crate::platform::configure_webrtc(&build.target, &root, &mut library)?;
    if !options.unity {
        library.sources = build.separate_group_sources(&root, &library.sources)?;
    }
    if options.pch {
        library.precompiled_headers = crate::groups::headers(&root, &library.sources)?;
    }
    println!("cargo:rerun-if-changed={}", sdk.include_dir.display());
    println!("cargo:rerun-if-changed={}", sdk.library.display());
    finish(build, &root, library)
}

#[derive(Clone, Copy, Debug)]
pub struct QsvOptions {
    pub hevc: bool,
    pub unity: bool,
    pub pch: bool,
}

impl Default for QsvOptions {
    fn default() -> Self {
        Self {
            hevc: true,
            unity: true,
            pch: false,
        }
    }
}

/// Build the original Intel Quick Sync encoder module.
pub fn build_qsv(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    options: &QsvOptions,
) -> Result<Module, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut library = module(build, &root, core, "obs-qsv11")?;
    library.sources.clear();
    crate::platform::configure_qsv(&build.target, &root, options, &mut library)?;
    if !options.unity {
        library.sources = build.separate_group_sources(&root, &library.sources)?;
    }
    if options.pch {
        library.precompiled_headers = crate::groups::headers(&root, &library.sources)?;
    }
    finish(build, &root, library)
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
    crate::platform::configure_ffmpeg_mux(&build.target, &root, &mut mux)?;
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
    Vlc,
    Image,
    Text,
    PulseAudio,
    Jack,
    Sndio,
    PipeWire,
    Alsa,
    V4l2 { udev: bool },
    X11Capture,
    RtmpServices { updates: bool },
}

impl SourceModule {
    pub fn name(self) -> &'static str {
        match self {
            Self::Vlc => "vlc-video",
            Self::Image => "image-source",
            Self::Text => "text-freetype2",
            Self::PulseAudio => "linux-pulseaudio",
            Self::Jack => "linux-jack",
            Self::Sndio => "sndio",
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
    if matches!(source, SourceModule::Jack)
        && build
            .compiler(&library, &build.output, false)
            .get_compiler()
            .is_like_clang()
    {
        library
            .c_compile_options
            .push("-Wno-error=sign-compare".into());
    }
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
