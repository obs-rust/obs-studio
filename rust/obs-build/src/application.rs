//! Production application assembly and development runtime artifacts.
use crate::{NativeBuild, libobs, plugins, scripting};
use std::path::{Path, PathBuf};

/// Features of the application graph, independent of test-driver environment.
pub struct BuildOptions {
    pub core: libobs::Options,
    pub filters: plugins::FiltersOptions,
    pub sources: Vec<plugins::SourceModule>,
    pub scripting: Option<scripting::Options>,
    pub frontend_tools: bool,
    pub websocket: Option<plugins::WebsocketOptions>,
    pub browser: Option<plugins::BrowserOptions>,
    pub fdk: bool,
    pub qsv: Option<plugins::QsvOptions>,
    pub nvenc: Option<plugins::NvencOptions>,
    pub webrtc: Option<(plugins::DataChannelSdk, plugins::WebRtcOptions)>,
}

impl BuildOptions {
    pub fn for_target(target: &str) -> Result<Self, String> {
        Ok(Self {
            core: libobs::Options::default(),
            filters: plugins::FiltersOptions::default(),
            sources: crate::platform::application_sources(target, true, true)?,
            scripting: Some(scripting::Options::default()),
            frontend_tools: true,
            websocket: Some(plugins::WebsocketOptions::default()),
            browser: None,
            fdk: false,
            qsv: None,
            nvenc: Some(plugins::NvencOptions::default()),
            webrtc: None,
        })
    }

    /// Core-module classification shared with the compatibility driver.
    pub fn core_module_names(&self) -> Vec<String> {
        let mut names: Vec<String> = [
            "obs-transitions",
            "obs-filters",
            "obs-vst",
            "obs-x264",
            "obs-outputs",
            "obs-ffmpeg",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        names.extend(self.sources.iter().map(|source| source.name().into()));
        for (enabled, name) in [
            (self.frontend_tools, "frontend-tools"),
            (self.websocket.is_some(), "obs-websocket"),
            (self.fdk, "obs-libfdk"),
            (self.qsv.is_some(), "obs-qsv11"),
            (self.nvenc.is_some(), "obs-nvenc"),
            (self.webrtc.is_some(), "obs-webrtc"),
            (self.browser.is_some(), "obs-browser"),
        ] {
            if enabled {
                names.push(name.into());
            }
        }
        names.extend(self.core.core_modules.iter().cloned());
        let mut unique = std::collections::BTreeSet::new();
        names.retain(|name| unique.insert(name.clone()));
        names
    }
}

pub fn source_modules(
    target: &str,
    udev: bool,
    service_updates: bool,
) -> Result<Vec<plugins::SourceModule>, String> {
    crate::platform::application_sources(target, udev, service_updates)
}

pub struct RustArchives {
    pub core: PathBuf,
    pub gpu_status: PathBuf,
    pub nvenc: PathBuf,
}

/// Artifacts of one production hybrid application, without compatibility probes.
pub struct Artifacts {
    pub core: PathBuf,
    pub api: PathBuf,
    pub renderer: PathBuf,
    pub executable: PathBuf,
    pub modules: Vec<plugins::Module>,
    pub ffmpeg: plugins::FfmpegArtifacts,
    pub scripting: Option<scripting::Artifacts>,
    pub browser: Option<plugins::BrowserArtifacts>,
    pub nvenc: Option<plugins::NvencProductionArtifacts>,
}

impl Artifacts {
    pub fn stage(
        &self,
        build: &NativeBuild,
        root: &Path,
        destination: &Path,
    ) -> Result<StagedRuntime, String> {
        let modules: Vec<_> = self
            .modules
            .iter()
            .chain(std::iter::once(&self.ffmpeg.module))
            .chain(self.browser.as_ref().map(|browser| &browser.module))
            .chain(self.nvenc.as_ref().map(|nvenc| &nvenc.module))
            .collect();
        stage_runtime(
            build,
            &RuntimeInputs {
                root,
                executable: &self.executable,
                core: &self.core,
                api: &self.api,
                renderer: &self.renderer,
                ffmpeg: &self.ffmpeg,
                nvenc_helper: self.nvenc.as_ref().map(|nvenc| nvenc.helper.as_path()),
                modules: &modules,
                browser: self.browser.as_ref(),
                scripting: self.scripting.as_ref(),
            },
            destination,
        )
    }
}

/// Assemble a real application through the canonical component builders.
/// Rust archives are supplied by Cargo orchestration, never built recursively
/// from a build script. The C compatibility core is not part of this graph.
#[expect(
    clippy::too_many_lines,
    reason = "flat ordered build stages, kept in one place"
)]
pub fn build(
    build: &NativeBuild,
    root: &Path,
    archives: &RustArchives,
    options: &BuildOptions,
) -> Result<Artifacts, String> {
    crate::platform::validate_application_build(build)?;
    for archive in [&archives.core, &archives.gpu_status]
        .into_iter()
        .chain(options.nvenc.as_ref().map(|_| &archives.nvenc))
    {
        if !archive.is_file() {
            return Err(format!(
                "Rust archive does not exist: {}",
                archive.display()
            ));
        }
    }
    let mut unique = std::collections::BTreeSet::new();
    for source in &options.sources {
        if !unique.insert(source.name()) {
            return Err(format!(
                "duplicate application source module: {}",
                source.name()
            ));
        }
    }
    let mut core_options = options.core.clone();
    core_options.core_modules = options.core_module_names();
    let core = libobs::build_hybrid_with_options(build, root, &core_options, &archives.core)?;
    let api = crate::frontend::build_api(build, root, &core)?;
    let renderer = crate::graphics::build_opengl(build, root, &core, &core_options)?;
    let mut modules = vec![
        plugins::build_transitions(build, root, &core)?,
        plugins::build_filters(build, root, &core, &options.filters)?,
    ];
    for source in &options.sources {
        modules.push(plugins::build_source(build, root, &core, *source)?);
    }
    modules.push(plugins::build_vst(build, root, &core)?);
    modules.push(plugins::build_x264(build, root, &core)?);
    modules.push(plugins::build_outputs(build, root, &core, &core_options)?);
    let ffmpeg = plugins::build_ffmpeg(
        build,
        root,
        &core,
        &core_options,
        &plugins::FfmpegOptions::default(),
    )?;
    if options.fdk {
        modules.push(plugins::build_fdk(build, root, &core)?);
    }
    if let Some(qsv) = &options.qsv {
        modules.push(plugins::build_qsv(build, root, &core, qsv)?);
    }
    let nvenc = options
        .nvenc
        .as_ref()
        .map(|options| plugins::build_nvenc(build, root, &core, &archives.nvenc, options))
        .transpose()?;
    if let Some((sdk, webrtc)) = &options.webrtc {
        modules.push(plugins::build_webrtc(build, root, &core, sdk, webrtc)?);
    }
    let scripting = options
        .scripting
        .as_ref()
        .map(|options| scripting::build(build, root, &core, Some(&api), options))
        .transpose()?;
    if options.frontend_tools {
        modules.push(plugins::build_frontend_tools(
            build,
            root,
            &core,
            &api,
            scripting.as_ref(),
        )?);
    }
    if let Some(websocket) = &options.websocket {
        modules.push(plugins::build_websocket(
            build, root, &core, &api, websocket,
        )?);
    }
    let browser = options
        .browser
        .as_ref()
        .map(|options| plugins::build_browser(build, root, &core, &api, options))
        .transpose()?;
    let executable = crate::frontend::build_application_with_options(
        build,
        root,
        &core,
        &api,
        &archives.gpu_status,
        &crate::frontend::ApplicationOptions {
            browser_panels: options
                .browser
                .as_ref()
                .is_some_and(|browser| browser.panels),
        },
    )?;
    Ok(Artifacts {
        core,
        api,
        renderer,
        executable,
        modules,
        ffmpeg,
        scripting,
        browser,
        nvenc,
    })
}

/// Explicit artifacts to stage; no discovery of same-named source-tree outputs.
pub struct RuntimeInputs<'a> {
    pub root: &'a Path,
    pub executable: &'a Path,
    pub core: &'a Path,
    pub api: &'a Path,
    pub renderer: &'a Path,
    pub ffmpeg: &'a plugins::FfmpegArtifacts,
    pub nvenc_helper: Option<&'a Path>,
    pub modules: &'a [&'a plugins::Module],
    pub browser: Option<&'a plugins::BrowserArtifacts>,
    pub scripting: Option<&'a scripting::Artifacts>,
}

pub struct StagedRuntime {
    pub directory: PathBuf,
    pub executable: PathBuf,
    pub modules: Vec<String>,
    pub scripting: Option<serde_json::Value>,
}

/// Stage into an empty destination, retaining only explicitly selected inputs.
/// This is a development runtime, not a portable signed release package.
pub fn stage_runtime(
    build: &NativeBuild,
    inputs: &RuntimeInputs<'_>,
    destination: &Path,
) -> Result<StagedRuntime, String> {
    crate::platform::stage_application_runtime(build, inputs, destination)
}
