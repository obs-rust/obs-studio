//! The original browser source, browser-panel interfaces and CEF helper.
use super::{Module, finish, module};
use crate::{NativeBuild, SharedLibrary, frontend, libobs::add_package};
use std::path::{Path, PathBuf};

pub struct Options {
    pub cef_root: PathBuf,
    pub panels: bool,
    pub unity: bool,
}

pub struct Artifacts {
    pub module: Module,
    pub helper: PathBuf,
    pub cef_root: PathBuf,
}

impl Artifacts {
    /// Place CEF resources and its subprocess helper beside the browser module.
    pub fn deploy_runtime(&self, directory: &Path) -> Result<(), String> {
        fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
            std::fs::create_dir_all(destination).map_err(|e| e.to_string())?;
            for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let target = destination.join(entry.file_name());
                if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                    copy_tree(&entry.path(), &target)?;
                } else {
                    std::fs::copy(entry.path(), target).map_err(|e| e.to_string())?;
                }
            }
            Ok(())
        }
        for name in ["Release", "Resources"] {
            let source = self.cef_root.join(name);
            println!("cargo:rerun-if-changed={}", source.display());
            copy_tree(&source, directory)?;
        }
        std::fs::copy(
            &self.helper,
            directory.join(
                self.helper
                    .file_name()
                    .ok_or("browser helper has no filename")?,
            ),
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "flat ordered build stages, kept in one place"
)]
pub fn build(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    api: &Path,
    options: &Options,
) -> Result<Artifacts, String> {
    let root = dunce::canonicalize(root).map_err(|e| e.to_string())?;
    let source = root.join("plugins/obs-browser");
    let sdk = dunce::canonicalize(&options.cef_root).map_err(|e| format!("CEF SDK: {e}"))?;
    let mut library = module(build, &root, core, "obs-browser")?;
    library.sources.clear();
    crate::platform::configure_browser(&build.target, &root, &mut library)?;
    let host_sources = std::mem::take(&mut library.sources);
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed={}", sdk.join("include").display());
    let version = std::fs::read_to_string(sdk.join("include/cef_version.h"))
        .map_err(|e| format!("CEF version header: {e}"))?;
    let major: u32 = version
        .lines()
        .find_map(|line| line.strip_prefix("#define CEF_VERSION_MAJOR "))
        .ok_or("CEF major version missing")?
        .trim()
        .parse()
        .map_err(|e| format!("CEF version: {e}"))?;
    if major < 95 {
        return Err("obs-browser requires CEF >=95".into());
    }
    let find = |candidates: &[&str]| -> Result<PathBuf, String> {
        candidates
            .iter()
            .map(|path| sdk.join(path))
            .find(|path| path.is_file())
            .ok_or_else(|| format!("CEF SDK lacks {}", candidates.join(" or ")))
            .and_then(|path| dunce::canonicalize(path).map_err(|e| e.to_string()))
    };
    let cef = find(&["Release/libcef.so", "libcef.so"])?;
    let wrapper = find(&[
        "build/libcef_dll_wrapper/libcef_dll_wrapper.a",
        "libcef_dll_wrapper/libcef_dll_wrapper.a",
        "build/libcef_dll/libcef_dll_wrapper.a",
    ])?;
    for path in [&cef, &wrapper] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    library.include_dirs.extend([
        sdk.clone(),
        sdk.join("include"),
        source.join("deps"),
        source.join("panel"),
        root.join("frontend/api"),
        root.join("plugins/obs-websocket/lib"),
    ]);
    library
        .dependencies
        .extend([api.into(), wrapper.clone(), cef.clone()]);
    pkg_config::Config::new()
        .cargo_metadata(false)
        .atleast_version("3.11")
        .probe("nlohmann_json")
        .map_err(|e| format!("obs-browser requires nlohmann_json >=3.11: {e}"))?;
    // The pinned source uses QApplication headers and QToolTip symbols outside
    // the optional panel implementation, so both modes require Qt Widgets.
    for package in ["nlohmann_json", "Qt6Widgets"] {
        add_package(&mut library, &build.target, package)?;
    }
    let manifest: std::collections::BTreeMap<String, Vec<String>> =
        serde_json::from_str(include_str!("../../manifests/obs-browser.json"))
            .map_err(|e| e.to_string())?;
    let inputs = manifest
        .get("common")
        .ok_or("browser sources missing")?
        .clone();
    if options.panels {
        library.definitions.push(("BROWSER_AVAILABLE".into(), None));
    }
    let inputs: Vec<_> = inputs
        .iter()
        .map(|path| format!("plugins/obs-browser/{path}"))
        .collect();
    frontend::add_inputs(build, &root, &mut library, &inputs)?;
    library.sources.extend(host_sources);
    if options.panels {
        let inputs: Vec<_> = manifest
            .get("panels")
            .ok_or("browser panel sources missing")?
            .iter()
            .map(|path| format!("plugins/obs-browser/{path}"))
            .collect();
        frontend::add_inputs(build, &root, &mut library, &inputs)?;
    }
    if options.unity {
        library.sources = unity_groups(
            build,
            &source,
            &library.sources,
            manifest
                .get("unity_start")
                .ok_or("browser unity boundaries missing")?,
        )?;
    }
    let module = finish(build, &root, library)?;
    let mut helper = SharedLibrary {
        name: "obs-browser-page".into(),
        file_name: Some("obs-browser-page".into()),
        sources: manifest
            .get("helper")
            .ok_or("browser helper sources missing")?
            .iter()
            .map(|path| source.join(path))
            .collect(),
        include_dirs: vec![
            sdk.clone(),
            sdk.join("include"),
            source.clone(),
            source.join("deps"),
            source.join("obs-browser-page"),
        ],
        dependencies: vec![wrapper, cef],
        unity: options.unity,
        ..SharedLibrary::default()
    };
    crate::platform::configure_browser_helper(&build.target, &mut helper)?;
    add_package(&mut helper, &build.target, "nlohmann_json")?;
    Ok(Artifacts {
        module,
        helper: build.executable(&helper)?,
        cef_root: sdk,
    })
}

fn unity_groups(
    build: &NativeBuild,
    source: &Path,
    sources: &[PathBuf],
    starts: &[String],
) -> Result<Vec<PathBuf>, String> {
    let boundaries: Vec<_> = starts.iter().map(|path| source.join(path)).collect();
    if boundaries.first() != sources.first()
        || boundaries
            .iter()
            .any(|path| sources.iter().filter(|p| *p == path).count() != 1)
    {
        return Err("browser unity boundaries do not match source inventory".into());
    }
    let output = build.output.join("obs-browser/unity");
    std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let mut groups = Vec::new();
    let mut content = String::new();
    for path in sources {
        if boundaries.contains(path) {
            if !content.is_empty() {
                let group = output.join(format!("browser-{}.cpp", groups.len()));
                frontend::write(&group, &content)?;
                groups.push(group);
            }
            content = String::from("/* Generated explicit browser unity group. */\n");
        }
        let path = dunce::canonicalize(path).map_err(|e| e.to_string())?;
        let path = path
            .to_str()
            .ok_or("non-UTF8 browser source path")?
            .replace('\\', "/");
        if path.contains(['"', '\n', '\r']) {
            return Err("unrepresentable browser source path".into());
        }
        content.push_str(&format!("#include \"{path}\"\n"));
    }
    if !content.is_empty() {
        let group = output.join(format!("browser-{}.cpp", groups.len()));
        frontend::write(&group, &content)?;
        groups.push(group);
    }
    Ok(groups)
}
