//! Safe command parser and production application-build orchestration.
use crate::{NativeBuild, NativeProfile, application, plugins};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    path::PathBuf,
};

pub const USAGE: &str = "Usage: obs-build build [OPTIONS]\n\nBuild and stage the Linux hybrid OBS application through Cargo and cc::Build.\n\n  --root PATH                 Source checkout (default: current directory)\n  --output PATH               Build directory (default: ROOT/build_cargo)\n  --target TRIPLE              Requested native target (default: this target)\n  --profile NAME               debug, release, relwithdebinfo, minsizerel\n  --separate-core             Compile core group members individually\n  --core-pch                  Enable core/backend precompiled headers\n  --disable-hevc              Disable HEVC\n  --disable-service-updates   Disable RTMP service updates\n  --without-nvenc             Omit the NVIDIA hardware encoder and helper\n  --without-nvenc-ffmpeg-ids  Do not register FFmpeg NVENC encoder aliases\n  --without-websocket         Omit the original obs-websocket plugin\n  --without-frontend-tools    Omit the frontend Tools plugin\n  --without-scripting         Omit Lua/Python scripting\n  --without-lua               Omit Lua bindings\n  --without-python            Omit Python bindings\n  --with-vlc | --with-jack | --with-sndio | --with-fdk | --with-qsv\n  --cef-root PATH             Build browser source/panels using this CEF SDK\n  --without-browser-panels    Build browser source without panels\n  --datachannel-include PATH  WebRTC SDK headers (requires library)\n  --datachannel-library PATH  WebRTC SDK library (requires headers)\n  -h, --help                  Show help\n";

pub enum Invocation {
    Help,
    Build(Box<Request>),
}

pub struct Request {
    pub root: PathBuf,
    pub output: PathBuf,
    pub target: String,
    pub profile: NativeProfile,
    pub options: application::BuildOptions,
}

pub fn parse<I, S>(arguments: I) -> Result<Invocation, String>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut arguments = arguments.into_iter().map(Into::into);
    let Some(command) = arguments.next() else {
        return Ok(Invocation::Help);
    };
    if command == "--help" || command == "-h" {
        return Ok(Invocation::Help);
    }
    if command != "build" {
        return Err(format!("unknown command: {}", command.to_string_lossy()));
    }
    let mut values = BTreeMap::<String, OsString>::new();
    let mut flags = BTreeSet::<String>::new();
    let mut seen = BTreeSet::new();
    while let Some(argument) = arguments.next() {
        let argument = argument
            .into_string()
            .map_err(|_| "option name is not UTF-8")?;
        if argument == "--help" || argument == "-h" {
            return Ok(Invocation::Help);
        }
        if !seen.insert(argument.clone()) {
            return Err(format!("repeated option: {argument}"));
        }
        match argument.as_str() {
            "--root"
            | "--output"
            | "--target"
            | "--profile"
            | "--cef-root"
            | "--datachannel-include"
            | "--datachannel-library" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| format!("{argument} needs a value"))?;
                if value.to_str().is_some_and(|value| value.starts_with("--")) {
                    return Err(format!("{argument} needs a value"));
                }
                values.insert(argument, value);
            }
            "--separate-core"
            | "--core-pch"
            | "--disable-hevc"
            | "--disable-service-updates"
            | "--without-websocket"
            | "--without-nvenc"
            | "--without-nvenc-ffmpeg-ids"
            | "--without-frontend-tools"
            | "--without-scripting"
            | "--without-lua"
            | "--without-python"
            | "--with-vlc"
            | "--with-jack"
            | "--with-sndio"
            | "--with-fdk"
            | "--with-qsv"
            | "--without-browser-panels" => {
                flags.insert(argument);
            }
            _ => return Err(format!("unknown option: {argument}")),
        }
    }
    let target = values
        .get("--target")
        .map(|value| value.to_str().ok_or("target is not UTF-8"))
        .transpose()?
        .unwrap_or(crate::BUILD_TARGET)
        .to_owned();
    let mut options = application::BuildOptions::for_target(&target)?;
    let profile = match values
        .get("--profile")
        .map(|value| value.to_str().ok_or("profile is not UTF-8"))
        .transpose()?
    {
        None | Some("relwithdebinfo") => NativeProfile::RelWithDebInfo,
        Some("debug") => NativeProfile::Debug,
        Some("release") => NativeProfile::Release,
        Some("minsizerel") => NativeProfile::MinSizeRel,
        Some(other) => return Err(format!("unknown native profile: {other}")),
    };
    options.core.unity = !flags.contains("--separate-core");
    options.core.pch = flags.contains("--core-pch");
    options.core.hevc = !flags.contains("--disable-hevc");
    if flags.contains("--disable-service-updates") {
        for source in &mut options.sources {
            if let plugins::SourceModule::RtmpServices { updates } = source {
                *updates = false;
            }
        }
    }
    options.frontend_tools = !flags.contains("--without-frontend-tools");
    if flags.contains("--without-nvenc") {
        options.nvenc = None;
    } else if let Some(nvenc) = &mut options.nvenc {
        nvenc.ffmpeg_ids = !flags.contains("--without-nvenc-ffmpeg-ids");
    }
    if flags.contains("--without-websocket") {
        options.websocket = None;
    }
    if flags.contains("--without-scripting") {
        options.scripting = None;
    } else if let Some(scripting) = &mut options.scripting {
        scripting.lua = !flags.contains("--without-lua");
        scripting.python = !flags.contains("--without-python");
        if !scripting.lua && !scripting.python {
            return Err("use --without-scripting to disable both scripting languages".into());
        }
    }
    for (flag, source) in [
        ("--with-vlc", plugins::SourceModule::Vlc),
        ("--with-jack", plugins::SourceModule::Jack),
        ("--with-sndio", plugins::SourceModule::Sndio),
    ] {
        if flags.contains(flag) {
            options.sources.push(source);
        }
    }
    options.fdk = flags.contains("--with-fdk");
    if flags.contains("--with-qsv") {
        options.qsv = Some(plugins::QsvOptions {
            hevc: options.core.hevc,
            ..Default::default()
        });
    }
    if let Some(root) = values.get("--cef-root") {
        options.browser = Some(plugins::BrowserOptions {
            cef_root: root.into(),
            panels: !flags.contains("--without-browser-panels"),
            unity: true,
        });
    } else if flags.contains("--without-browser-panels") {
        return Err("--without-browser-panels requires --cef-root".into());
    }
    match (
        values.get("--datachannel-include"),
        values.get("--datachannel-library"),
    ) {
        (Some(include), Some(library)) => {
            options.webrtc = Some((
                plugins::DataChannelSdk {
                    include_dir: include.into(),
                    library: library.into(),
                    media: true,
                    websocket: true,
                },
                plugins::WebRtcOptions {
                    hevc: options.core.hevc,
                    ..Default::default()
                },
            ));
        }
        (None, None) => {}
        _ => {
            return Err(
                "WebRTC requires both --datachannel-include and --datachannel-library".into(),
            );
        }
    }
    let root = values
        .get("--root")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|e| e.to_string())?;
    let output = values
        .get("--output")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("build_cargo"));
    Ok(Invocation::Build(Box::new(Request {
        root,
        output,
        target,
        profile,
        options,
    })))
}

/// Execute only after parsing; help and invalid requests do not create outputs.
pub fn run<I, S>(arguments: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let request = match parse(arguments)? {
        Invocation::Help => {
            print!("{USAGE}");
            return Ok(());
        }
        Invocation::Build(request) => request,
    };
    let root = request
        .root
        .canonicalize()
        .map_err(|e| format!("source checkout: {e}"))?;
    if !root.join("rust/libobs-rust/Cargo.toml").is_file()
        || !root.join("frontend/build/cargo-sources.json").is_file()
    {
        return Err(format!("not an OBS source checkout: {}", root.display()));
    }
    let profile_name = match request.profile {
        NativeProfile::Debug => "debug",
        NativeProfile::Release => "release",
        NativeProfile::RelWithDebInfo => "relwithdebinfo",
        NativeProfile::MinSizeRel => "minsizerel",
        NativeProfile::Custom { .. } => "custom",
    };
    let mut build = NativeBuild {
        host: crate::BUILD_HOST.into(),
        target: request.target,
        output: request.output.join(profile_name),
        profile: request.profile,
    };
    crate::platform::validate_application_build(&build)?;
    std::fs::create_dir_all(&build.output).map_err(|e| e.to_string())?;
    build.output = build.output.canonicalize().map_err(|e| e.to_string())?;
    let rust_output = build.output.join("rust");
    let debug = request.profile == NativeProfile::Debug;
    let cargo_profile = if debug { "DEV" } else { "RELEASE" };
    let mut cargo = std::process::Command::new("soldr");
    cargo
        // Match the native Rust consumers in CI; keep standalone C/C++ caching.
        .env("SOLDR_RUSTC_WRAPPER", "none")
        .current_dir(&root)
        .args([
            "cargo",
            "build",
            "--locked",
            "-p",
            "libobs-rust",
            "-p",
            "obs-frontend",
            "--target",
        ])
        .arg(&build.target)
        .arg("--target-dir")
        .arg(&rust_output);
    if request.options.nvenc.is_some() {
        cargo.args(["-p", "obs-nvenc-plugin"]);
    }
    if !debug {
        cargo.arg("--release");
    }
    cargo
        .env(
            format!("CARGO_PROFILE_{cargo_profile}_OPT_LEVEL"),
            request.profile.optimization(),
        )
        .env(
            format!("CARGO_PROFILE_{cargo_profile}_DEBUG"),
            if request.profile.debug_info() {
                "2"
            } else {
                "0"
            },
        )
        .env(
            format!("CARGO_PROFILE_{cargo_profile}_DEBUG_ASSERTIONS"),
            request.profile.assertions().to_string(),
        )
        .env(format!("CARGO_PROFILE_{cargo_profile}_PANIC"), "abort");
    crate::platform::configure_application_archive_command(&build, &mut cargo)?;
    let status = cargo
        .status()
        .map_err(|e| format!("start soldr Cargo archive build: {e}"))?;
    if !status.success() {
        return Err(format!("Rust archive build failed: {status}"));
    }
    let archives = rust_output
        .join(&build.target)
        .join(if debug { "debug" } else { "release" });
    let artifacts = application::build(
        &build,
        &root,
        &crate::platform::application_rust_archives(&build.target, &archives)?,
        &request.options,
    )?;
    let runtime_parent = build.output.join("runtimes");
    std::fs::create_dir_all(&runtime_parent).map_err(|e| e.to_string())?;
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let runtime = artifacts.stage(
        &build,
        &root,
        &runtime_parent.join(format!("{started}-{}", std::process::id())),
    )?;
    let manifest = serde_json::json!({
        "schema": 1, "target": build.target, "profile": profile_name,
        "core_unity": request.options.core.unity, "core_pch": request.options.core.pch,
        "hevc": request.options.core.hevc, "runtime": runtime.directory,
        "executable": runtime.executable, "modules": runtime.modules,
        "scripting": runtime.scripting,
        "nvenc": request.options.nvenc.as_ref().map(|options| serde_json::json!({
            "ffmpeg_ids": options.ffmpeg_ids,
        })),
    });
    let manifest_path = runtime.directory.join("application-build.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("Application executable: {}", runtime.executable.display());
    println!("Build manifest: {}", manifest_path.display());
    Ok(())
}
