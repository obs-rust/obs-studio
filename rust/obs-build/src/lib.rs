//! Native compilation and dynamic linking for the Cargo-driven OBS build.
//!
//! This engine owns native artifacts. Cargo remains responsible for compiling
//! Rust crates; native compiles go through cc::Build and inherit soldr's tools.
#![forbid(unsafe_code)]

use std::{fs, path::PathBuf};

pub mod application;
pub mod cli;
mod deps;
pub mod frontend;
pub mod graphics;
mod groups;
pub mod libobs;
mod pch;
mod platform;
pub mod plugins;
pub mod qt;
pub mod scripting;
pub mod validation;

#[derive(Clone, Debug)]
pub struct PrecompiledHeader {
    pub header: PathBuf,
    pub sources: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct SourceCompileOptions {
    pub sources: Vec<PathBuf>,
    /// Applied after target and language defaults, including when building PCH.
    pub options: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LibraryKind {
    #[default]
    Shared,
    /// Loadable module; Apple targets emit MH_BUNDLE rather than MH_DYLIB.
    Module,
}

/// Required ISO C++ mode. Native tests and application targets share this
/// setting, including their PCH compilation; C sources keep their own flags.
#[derive(Clone, Copy, Debug, serde::Deserialize)]
pub enum CppStandard {
    #[serde(rename = "17")]
    Cpp17,
    #[serde(rename = "20")]
    Cpp20,
}

impl CppStandard {
    fn compiler_name(self) -> &'static str {
        match self {
            Self::Cpp17 => "c++17",
            Self::Cpp20 => "c++20",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SharedLibrary {
    pub name: String,
    pub kind: LibraryKind,
    pub sources: Vec<PathBuf>,
    /// Native resource scripts, compiled separately from C-family unity/PCH batches.
    pub resource_files: Vec<PathBuf>,
    pub resource_compile_options: Vec<String>,
    /// Extra resource inputs (included headers, icons, manifests) watched by Cargo.
    pub resource_input_files: Vec<PathBuf>,
    /// Objects compiled once and reused by multiple shared-library variants.
    pub precompiled_objects: Vec<PathBuf>,
    pub include_dirs: Vec<PathBuf>,
    pub dependencies: Vec<PathBuf>,
    pub unity: bool,
    /// Explicit source-to-header assignments; empty disables compiled PCH.
    pub precompiled_headers: Vec<PrecompiledHeader>,
    pub definitions: Vec<(String, Option<String>)>,
    pub compile_options: Vec<String>,
    pub c_compile_options: Vec<String>,
    pub cpp_compile_options: Vec<String>,
    pub cpp_standard: Option<CppStandard>,
    /// Extra flags for Objective-C, in addition to the C-family options.
    pub objc_compile_options: Vec<String>,
    /// Extra flags for Objective-C++, in addition to the C++-family options.
    pub objcpp_compile_options: Vec<String>,
    pub source_compile_options: Vec<SourceCompileOptions>,
    pub link_options: Vec<String>,
    pub file_name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NativeBuild {
    pub host: String,
    pub target: String,
    pub output: PathBuf,
    pub profile: NativeProfile,
}

/// Native configurations corresponding to the supported CMake build profiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeProfile {
    Debug,
    Release,
    #[default]
    RelWithDebInfo,
    MinSizeRel,
    Custom {
        optimization: NativeOptimization,
        debug_info: bool,
        assertions: bool,
    },
}

/// Native compiler optimization levels accepted from Cargo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeOptimization {
    None,
    One,
    Two,
    Three,
    Size,
    MinSize,
}

impl NativeOptimization {
    fn value(self) -> &'static str {
        match self {
            Self::None => "0",
            Self::One => "1",
            Self::Two => "2",
            Self::Three => "3",
            Self::Size => "s",
            Self::MinSize => "z",
        }
    }
}

pub const CARGO_CONFIGURATION_VARIABLES: [&str; 4] = [
    "OPT_LEVEL",
    "DEBUG",
    "OBS_SYS_NATIVE_PROFILE",
    "OBS_SYS_NATIVE_ASSERTIONS",
];

impl NativeProfile {
    /// Select bundled native policy from effective Cargo settings and explicit overrides.
    pub fn from_cargo_lookup(
        mut lookup: impl FnMut(&str) -> Option<std::ffi::OsString>,
    ) -> Result<Self, String> {
        let mut value = |name: &str| -> Result<Option<String>, String> {
            lookup(name)
                .map(|value| {
                    value
                        .into_string()
                        .map_err(|_| format!("{name} must be UTF-8"))
                })
                .transpose()
        };
        let named = value("OBS_SYS_NATIVE_PROFILE")?;
        let assertion_override = match value("OBS_SYS_NATIVE_ASSERTIONS")?.as_deref() {
            None => None,
            Some("0") => Some(false),
            Some("1") => Some(true),
            _ => return Err("OBS_SYS_NATIVE_ASSERTIONS must be 0 or 1".into()),
        };
        let (optimization, debug_info, assertions) = if let Some(named) = named {
            let profile = match named.as_str() {
                "debug" => Self::Debug,
                "release" => Self::Release,
                "relwithdebinfo" => Self::RelWithDebInfo,
                "minsizerel" => Self::MinSizeRel,
                _ => return Err(
                    "OBS_SYS_NATIVE_PROFILE must be debug, release, relwithdebinfo or minsizerel"
                        .into(),
                ),
            };
            (
                profile.optimization().to_owned(),
                profile.debug_info(),
                profile.assertions(),
            )
        } else {
            let optimization = value("OPT_LEVEL")?.ok_or("OPT_LEVEL is required")?;
            let debug_info = match value("DEBUG")?.as_deref() {
                Some("true") => true,
                Some("false") => false,
                _ => return Err("DEBUG must be true or false".into()),
            };
            let assertions = optimization == "0";
            (optimization, debug_info, assertions)
        };
        let optimization = match optimization.as_str() {
            "0" => NativeOptimization::None,
            "1" => NativeOptimization::One,
            "2" => NativeOptimization::Two,
            "3" => NativeOptimization::Three,
            "s" => NativeOptimization::Size,
            "z" => NativeOptimization::MinSize,
            _ => return Err("OPT_LEVEL must be 0, 1, 2, 3, s or z".into()),
        };
        let assertions = assertion_override.unwrap_or(assertions);
        Ok(match (optimization, debug_info, assertions) {
            (NativeOptimization::None, true, true) => Self::Debug,
            (NativeOptimization::Three, false, false) => Self::Release,
            (NativeOptimization::Two, true, false) => Self::RelWithDebInfo,
            (NativeOptimization::Size, false, false) => Self::MinSizeRel,
            _ => Self::Custom {
                optimization,
                debug_info,
                assertions,
            },
        })
    }

    pub(crate) fn optimization(self) -> &'static str {
        match self {
            Self::Debug => "0",
            Self::Release => "3",
            Self::RelWithDebInfo => "2",
            Self::MinSizeRel => "s",
            Self::Custom { optimization, .. } => optimization.value(),
        }
    }

    pub(crate) fn debug_info(self) -> bool {
        match self {
            Self::Custom { debug_info, .. } => debug_info,
            _ => matches!(self, Self::Debug | Self::RelWithDebInfo),
        }
    }

    pub(crate) fn assertions(self) -> bool {
        match self {
            Self::Custom { assertions, .. } => assertions,
            _ => self == Self::Debug,
        }
    }
}

/// Swift module inputs; C-family sources continue to compile through cc::Build.
#[derive(Clone, Debug)]
pub struct SwiftLibrary {
    pub name: String,
    pub module_name: String,
    pub file_name: String,
    pub sources: Vec<PathBuf>,
    pub bridging_header: PathBuf,
    /// Included native headers/generated inputs to track for Cargo rebuilds.
    pub input_files: Vec<PathBuf>,
    pub include_dirs: Vec<PathBuf>,
    /// Definitions passed to the Clang importer for the bridging header.
    pub definitions: Vec<(String, Option<String>)>,
    pub dependencies: Vec<PathBuf>,
    pub deployment_target: String,
    pub compile_options: Vec<String>,
    pub link_options: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Language {
    C,
    Cpp,
    ObjectiveC,
    ObjectiveCpp,
}

impl Language {
    fn cpp(self) -> bool {
        matches!(self, Self::Cpp | Self::ObjectiveCpp)
    }

    fn extension(self) -> &'static str {
        match self {
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::ObjectiveC => "m",
            Self::ObjectiveCpp => "mm",
        }
    }

    pub(crate) fn header_language(self) -> &'static str {
        match self {
            Self::C => "c-header",
            Self::Cpp => "c++-header",
            Self::ObjectiveC => "objective-c-header",
            Self::ObjectiveCpp => "objective-c++-header",
        }
    }
}

struct CompilationBatch {
    language: Language,
    header: Option<PathBuf>,
    options: Vec<String>,
    sources: Vec<PathBuf>,
}

impl NativeBuild {
    /// Select the native configuration explicitly for an application build.
    pub fn with_profile(&self, profile: NativeProfile) -> Self {
        Self {
            profile,
            ..self.clone()
        }
    }

    /// Compile and link a complete Swift module with an explicit native bridge.
    pub fn swift_shared_library(&self, library: &SwiftLibrary) -> Result<PathBuf, String> {
        platform::build_swift_library(self, library)
    }

    /// Expand explicit BUILD.md groups into individual translation units.
    pub fn separate_group_sources(
        &self,
        root: &std::path::Path,
        sources: &[PathBuf],
    ) -> Result<Vec<PathBuf>, String> {
        groups::separate(root, &self.output.join("individual-sources"), sources)
    }

    /// Expand groups and carry per-source flags and PCH assignments to their members.
    pub fn separate_target_sources(
        &self,
        root: &std::path::Path,
        library: &mut SharedLibrary,
    ) -> Result<(), String> {
        let expanded = groups::expand(
            root,
            &self.output.join("individual-sources"),
            &library.sources,
        )?;
        let mut members = std::collections::BTreeMap::new();
        for (source, outputs) in &expanded {
            let source = dunce::canonicalize(source).map_err(|e| e.to_string())?;
            if members.insert(source.clone(), outputs).is_some() {
                return Err(format!("duplicate target source: {}", source.display()));
            }
        }
        let remap = |sources: &[PathBuf]| -> Result<Vec<PathBuf>, String> {
            let mut result = Vec::new();
            for source in sources {
                let source = dunce::canonicalize(source).map_err(|e| e.to_string())?;
                result.extend(
                    members
                        .get(&source)
                        .ok_or_else(|| format!("annotation outside target: {}", source.display()))?
                        .iter()
                        .cloned(),
                );
            }
            Ok(result)
        };
        let options = library
            .source_compile_options
            .iter()
            .map(|entry| {
                Ok(SourceCompileOptions {
                    sources: remap(&entry.sources)?,
                    options: entry.options.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let headers = library
            .precompiled_headers
            .iter()
            .map(|entry| {
                Ok(PrecompiledHeader {
                    sources: remap(&entry.sources)?,
                    header: entry.header.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        library.sources = expanded
            .into_iter()
            .flat_map(|(_, outputs)| outputs)
            .collect();
        library.source_compile_options = options;
        library.precompiled_headers = headers;
        Ok(())
    }

    /// Compile through cc::Build, then link a shared library or loadable module.
    pub fn shared_library(&self, library: &SharedLibrary) -> Result<PathBuf, String> {
        let (output, cpp_link, mut objects) = self.objects(library)?;
        append_precompiled(&mut objects, library);
        let artifact = output.join(library.file_name.clone().unwrap_or_else(|| {
            platform::library_filename(&self.target, &library.name, library.kind)
        }));
        let compiler = self.compiler(library, &output, cpp_link).get_compiler();
        let mut command = compiler.to_command();
        platform::configure_shared_link(
            &self.target,
            &mut command,
            &artifact,
            &objects,
            &library.dependencies,
            compiler.is_like_msvc(),
            library.kind,
        )?;
        command.args(&library.link_options);
        let result = command
            .output()
            .map_err(|e| format!("execute linker {command:?}: {e}"))?;
        if !result.status.success() {
            return Err(format!(
                "link {} failed ({})\n{}\n{}",
                library.name,
                result.status,
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            ));
        }
        Ok(artifact)
    }

    /// Compile and link an executable with the native shared dependencies.
    pub fn executable(&self, target: &SharedLibrary) -> Result<PathBuf, String> {
        let (output, cpp, mut objects) = self.objects(target)?;
        append_precompiled(&mut objects, target);
        let artifact = output.join(
            target
                .file_name
                .clone()
                .unwrap_or_else(|| platform::executable_filename(&self.target, &target.name)),
        );
        let compiler = self.compiler(target, &output, cpp).get_compiler();
        let mut command = compiler.to_command();
        platform::configure_executable_link(
            &self.target,
            &mut command,
            &artifact,
            &objects,
            &target.dependencies,
            compiler.is_like_msvc(),
        )?;
        command.args(&target.link_options);
        let result = command
            .output()
            .map_err(|e| format!("execute linker {command:?}: {e}"))?;
        if !result.status.success() {
            return Err(format!(
                "link executable {} failed ({})\n{}\n{}",
                target.name,
                result.status,
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            ));
        }
        Ok(artifact)
    }

    /// Build an archive with the same compiler, flags and source grouping.
    pub fn static_library(&self, library: &SharedLibrary) -> Result<PathBuf, String> {
        if !library.resource_files.is_empty() {
            return Err(
                "native resources must be linked into a DLL or executable, not an archive".into(),
            );
        }
        let (output, cpp, mut objects) = self.objects(library)?;
        append_precompiled(&mut objects, library);
        self.compiler(library, &output, cpp)
            .try_create_archive(&library.name, objects)
            .map_err(|e| e.to_string())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one block per library kind, in link order"
    )]
    fn objects(&self, library: &SharedLibrary) -> Result<(PathBuf, bool, Vec<PathBuf>), String> {
        if library.name.is_empty()
            || !library
                .name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err(format!("invalid native target name {:?}", library.name));
        }
        if library.sources.is_empty()
            && library.resource_files.is_empty()
            && library.precompiled_objects.is_empty()
        {
            return Err(format!("{} has no sources", library.name));
        }
        let output = self.output.join(&library.name);
        fs::create_dir_all(&output).map_err(|e| format!("create {}: {e}", output.display()))?;
        let output = dunce::canonicalize(&output).map_err(|e| e.to_string())?;
        let mut assignments = std::collections::BTreeMap::new();
        for assignment in &library.precompiled_headers {
            let header = dunce::canonicalize(&assignment.header)
                .map_err(|e| format!("{}: {e}", assignment.header.display()))?;
            for source in &assignment.sources {
                let source = dunce::canonicalize(source).map_err(|e| e.to_string())?;
                if assignments.insert(source.clone(), header.clone()).is_some() {
                    return Err(format!(
                        "duplicate PCH source assignment: {}",
                        source.display()
                    ));
                }
            }
        }
        let mut batches: Vec<CompilationBatch> = Vec::new();
        let mut target_sources = std::collections::HashSet::new();
        let mut source_options = std::collections::BTreeMap::new();
        for entry in &library.source_compile_options {
            for source in &entry.sources {
                let source = dunce::canonicalize(source).map_err(|e| e.to_string())?;
                if source_options
                    .insert(source.clone(), entry.options.clone())
                    .is_some()
                {
                    return Err(format!(
                        "duplicate compiler options for {}",
                        source.display()
                    ));
                }
            }
        }
        for source in &library.sources {
            let absolute =
                dunce::canonicalize(source).map_err(|e| format!("{}: {e}", source.display()))?;
            if !target_sources.insert(absolute.clone()) {
                return Err(format!("duplicate target source: {}", source.display()));
            }
            let language = match source.extension().and_then(|e| e.to_str()) {
                Some("c") => Language::C,
                Some("cpp" | "cc" | "cxx") => Language::Cpp,
                Some("m") => Language::ObjectiveC,
                Some("mm") => Language::ObjectiveCpp,
                other => {
                    return Err(format!(
                        "unsupported native source language {other:?}: {}",
                        source.display()
                    ));
                }
            };
            let header = assignments.remove(&absolute);
            let options = source_options.remove(&absolute).unwrap_or_default();
            // Keep translation-unit and constructor order. Combining all users
            // of the same header would move interleaved source files around.
            match batches.last_mut() {
                Some(previous)
                    if previous.language == language
                        && previous.header == header
                        && previous.options == options =>
                {
                    previous.sources.push(absolute);
                }
                _ => batches.push(CompilationBatch {
                    language,
                    header,
                    options,
                    sources: vec![absolute],
                }),
            }
        }
        if !assignments.is_empty() {
            return Err("PCH assignment refers to sources outside this target".into());
        }
        if !source_options.is_empty() {
            return Err("compiler options refer to sources outside this target".into());
        }
        let cpp_link = batches.iter().any(|batch| batch.language.cpp());
        let mut objects = Vec::new();
        for (index, batch) in batches.into_iter().enumerate() {
            let CompilationBatch {
                language,
                header,
                options,
                sources,
            } = batch;
            let mut compiler = self.compiler(library, &output, language.cpp());
            compiler.flags(match language {
                Language::ObjectiveC => library.objc_compile_options.as_slice(),
                Language::ObjectiveCpp => library.objcpp_compile_options.as_slice(),
                Language::C | Language::Cpp => &[],
            });
            compiler.flags(&options);
            if let Some(header) = header
                && let Some(object) = pch::apply(self, &mut compiler, language, &header)?
                && !objects.contains(&object)
            {
                objects.push(object);
            }
            if library.unity {
                let group = output.join(format!("unity-{index}.{}", language.extension()));
                let mut content = String::from("/* Generated by obs-build. */\n");
                for source in &sources {
                    let path = source.to_str().ok_or("non-UTF8 native source path")?;
                    if path.contains(['\n', '\r', '"']) {
                        return Err(format!(
                            "source path cannot be represented in an include: {path:?}"
                        ));
                    }
                    content.push_str(&format!("#include \"{}\"\n", path.replace('\\', "/")));
                }
                // Do not invalidate a warm compiler cache by touching unchanged groups.
                if fs::read_to_string(&group).ok().as_deref() != Some(&content) {
                    fs::write(&group, content).map_err(|e| e.to_string())?;
                }
                compiler.file(group);
            } else {
                compiler.files(sources);
            }
            objects.extend(
                compiler
                    .try_compile_intermediates()
                    .map_err(|e| e.to_string())?,
            );
        }
        if !library.resource_files.is_empty() {
            let compiler = self.compiler(library, &output, false).get_compiler();
            objects.extend(platform::resources::compile(
                self, library, &output, &compiler,
            )?);
        }
        Ok((output, cpp_link, objects))
    }

    fn compiler(&self, library: &SharedLibrary, output: &std::path::Path, cpp: bool) -> cc::Build {
        let mut compiler = cc::Build::new();
        compiler
            .host(&self.host)
            .target(&self.target)
            .out_dir(output)
            .opt_level_str(self.profile.optimization())
            .debug(self.profile.debug_info())
            .pic(true)
            .cpp(cpp)
            // Intermediate compilation and archive creation do not emit link
            // directives. Keep cc's compiler/archiver environment tracking so
            // target-qualified flags invalidate Cargo's bundled native build.
            .cargo_metadata(true);
        if !self.profile.assertions() {
            compiler.define("NDEBUG", None);
        }
        let mut includes = std::collections::HashSet::new();
        for directory in &library.include_dirs {
            if includes.insert(directory) {
                compiler.include(dunce::simplified(directory));
            }
        }
        let mut definitions = std::collections::HashSet::new();
        for (name, value) in &library.definitions {
            if definitions.insert((name, value)) {
                compiler.define(name, value.as_deref());
            }
        }
        compiler.flags(&library.compile_options);
        compiler.flags(if cpp {
            &library.cpp_compile_options
        } else {
            &library.c_compile_options
        });
        if cpp && let Some(standard) = library.cpp_standard {
            compiler.std(standard.compiler_name());
        }
        compiler
    }
}

fn append_precompiled(objects: &mut Vec<PathBuf>, library: &SharedLibrary) {
    // Common and variant batches can consume the same MSVC PCH. Its generated
    // object must reach the linker exactly once, even when the common objects
    // were compiled before the variant was selected.
    let mut seen: std::collections::HashSet<_> = objects.iter().cloned().collect();
    objects.extend(
        library
            .precompiled_objects
            .iter()
            .filter(|object| seen.insert((*object).clone()))
            .cloned(),
    );
}

/// Host triple used to compile this build engine.
pub const BUILD_HOST: &str = env!("OBS_BUILD_HOST");
/// Target triple used to compile this build engine.
pub const BUILD_TARGET: &str = env!("OBS_BUILD_TARGET");
