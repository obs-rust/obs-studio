//! Declarative native test targets and their execution contracts.
//!
//! Selection is explicit: callers provide platform, feature and dependency
//! facts. An unknown fact is an error, never an implicit reason to drop a test.
use crate::{NativeBuild, SharedLibrary};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Requirement {
    Constant(bool),
    Fact(String),
    All { all: Vec<Requirement> },
    Any { any: Vec<Requirement> },
    Not { not: Box<Requirement> },
}

impl Default for Requirement {
    fn default() -> Self {
        Self::Constant(true)
    }
}

impl Requirement {
    fn evaluate(&self, facts: &BTreeMap<String, bool>) -> Result<bool, String> {
        match self {
            Self::Constant(value) => Ok(*value),
            Self::Fact(name) => facts
                .get(name)
                .copied()
                .ok_or_else(|| format!("unknown native test requirement {name}")),
            Self::All { all } => {
                let mut selected = true;
                for requirement in all {
                    selected &= requirement.evaluate(facts)?;
                }
                Ok(selected)
            }
            Self::Any { any } => {
                let mut selected = false;
                for requirement in any {
                    selected |= requirement.evaluate(facts)?;
                }
                Ok(selected)
            }
            Self::Not { not } => not.evaluate(facts).map(|value| !value),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ConditionalValue {
    Name(String),
    Conditional { name: String, requires: Requirement },
}

pub type Library = ConditionalValue;
pub type CompilerOption = ConditionalValue;

impl ConditionalValue {
    fn selected(&self, facts: &BTreeMap<String, bool>) -> Result<Option<&str>, String> {
        match self {
            Self::Name(name) => Ok(Some(name)),
            Self::Conditional { name, requires } => {
                Ok(requires.evaluate(facts)?.then_some(name.as_str()))
            }
        }
    }
}

/// Sources may be selected independently of their executable, for example a
/// private C implementation compiled only on toolchains supporting its checks.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Source {
    Path(String),
    Conditional { path: String, requires: Requirement },
}

impl Source {
    fn path(&self) -> &str {
        match self {
            Self::Path(path) | Self::Conditional { path, .. } => path,
        }
    }

    fn selected(&self, facts: &BTreeMap<String, bool>) -> Result<bool, String> {
        match self {
            Self::Path(_) => Ok(true),
            Self::Conditional { requires, .. } => requires.evaluate(facts),
        }
    }
}

/// Explicit Qt generator inputs, separate from compiler inputs.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qt {
    #[serde(default)]
    pub moc: Vec<String>,
    #[serde(default)]
    pub uic: Vec<String>,
    #[serde(default)]
    pub rcc: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub sources: Vec<Source>,
    #[serde(default)]
    pub qt: Qt,
    #[serde(default)]
    pub cpp_standard: Option<crate::CppStandard>,
    #[serde(default)]
    pub requires: Requirement,
    #[serde(default)]
    pub includes: Vec<String>,
    /// Include directories searched before the implicit repository root.
    #[serde(default)]
    pub includes_before: Vec<String>,
    #[serde(default)]
    pub definitions: Vec<String>,
    /// Quoted C string definitions referring to separately built artifacts.
    #[serde(default)]
    pub artifact_definitions: BTreeMap<String, String>,
    /// Repository-relative headers or data read indirectly by these sources/tests.
    #[serde(default)]
    pub watch_inputs: Vec<String>,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    /// Include paths and definitions only; compiler policy belongs in the
    /// target's explicit compile_options, without introducing a link edge.
    pub header_packages: Vec<String>,
    /// Public include directories for tests that supply dependency stubs.
    #[serde(default)]
    pub header_libraries: Vec<String>,
    #[serde(default)]
    pub archives: Vec<String>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub compile_options: Vec<CompilerOption>,
    #[serde(default)]
    pub compile_options_if_supported: Vec<String>,
    #[serde(default)]
    pub link_options: Vec<String>,
    #[serde(default)]
    pub sanitizers: Option<String>,
}

impl Target {
    fn library_names(&self, facts: &BTreeMap<String, bool>) -> Result<Vec<String>, String> {
        let mut names: Vec<_> = self
            .archives
            .iter()
            .map(|name| format!("OBS::{name}"))
            .collect();
        for library in &self.libraries {
            if let Some(name) = library.selected(facts)? {
                names.push(name.into());
            }
        }
        Ok(names)
    }

    /// Actual link artifacts after conditional and imported dependency resolution.
    /// Header-only imports contribute no artifacts.
    pub fn dependency_artifacts(
        &self,
        facts: &BTreeMap<String, bool>,
        libraries: &BTreeMap<String, Dependency>,
    ) -> Result<Vec<PathBuf>, String> {
        let mut artifacts = Vec::new();
        for name in self.library_names(facts)? {
            let dependency = libraries
                .get(&name)
                .ok_or_else(|| format!("unresolved native test library {name}"))?;
            artifacts.extend(dependency.artifacts.iter().cloned());
        }
        Ok(artifacts)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Test {
    pub target: String,
    #[serde(default = "default_suite")]
    pub suite: String,
    #[serde(default)]
    pub requires: Requirement,
    #[serde(default = "default_command")]
    pub command: Vec<String>,
    /// Outer-to-inner argv prefixes used when their executable is available.
    #[serde(default)]
    pub optional_prefixes: Vec<Vec<String>>,
    /// None inherits the runner's configured default; Some(0) is unbounded,
    /// matching CTest's explicit TIMEOUT 0 semantics.
    #[serde(default)]
    pub timeout_seconds: Option<u32>,
    #[serde(default)]
    pub skip_return_code: Option<i32>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    /// Ordered paths prepended to an explicit value or the inherited environment.
    #[serde(default)]
    pub environment_path_prepend: BTreeMap<String, Vec<String>>,
    /// Imported Qt runtime interface, resolved before preparing the command.
    #[serde(default)]
    pub qt_runtime: Option<String>,
    #[serde(default)]
    pub working_directory: Option<String>,
    #[serde(default)]
    pub resource_locks: Vec<String>,
    #[serde(default)]
    pub run_serial: bool,
}

impl Test {
    pub fn resolve_runtime(
        &self,
        libraries: &BTreeMap<String, Dependency>,
    ) -> Result<Self, String> {
        let mut resolved = self.clone();
        if let Some(name) = resolved.qt_runtime.take() {
            let dependency = libraries
                .get(&name)
                .ok_or_else(|| format!("unresolved native test Qt runtime {name}"))?;
            for (key, paths) in &dependency.runtime_path_prepend {
                resolved
                    .environment_path_prepend
                    .entry(key.clone())
                    .or_default()
                    .extend(paths.iter().cloned());
            }
        }
        Ok(resolved)
    }

    /// Prepare the declared argv/environment without a shell. The runner still
    /// owns timeout, skip and scheduling policy; preparing a command is not a
    /// test result and does not apply those execution policies.
    pub fn command(&self, root: &Path, output: &Path, target: &Path) -> Result<Command, String> {
        self.command_in(root, output, target, output)
    }

    pub fn command_in(
        &self,
        root: &Path,
        output: &Path,
        target: &Path,
        runtime: &Path,
    ) -> Result<Command, String> {
        if let Some(name) = &self.qt_runtime {
            return Err(format!("unresolved native test Qt runtime {name}"));
        }
        let mut arguments = self
            .command
            .iter()
            .map(|value| expand(value, root, output, target, runtime))
            .collect::<Result<Vec<_>, _>>()?;
        for prefix in self.optional_prefixes.iter().rev() {
            if let Some(program) =
                crate::platform::native_test_command_program(&prefix[0], root, output)?
            {
                let mut expanded = vec![
                    program
                        .to_str()
                        .ok_or("non-UTF8 native test command prefix")?
                        .to_owned(),
                ];
                expanded.extend(
                    prefix[1..]
                        .iter()
                        .map(|value| expand(value, root, output, target, runtime))
                        .collect::<Result<Vec<_>, _>>()?,
                );
                arguments.splice(0..0, expanded);
            }
        }
        let (program, arguments) = arguments
            .split_first()
            .ok_or("native test has no command")?;
        let mut command = Command::new(program);
        command.args(arguments);
        for (key, value) in &self.environment {
            command.env(key, expand(value, root, output, target, runtime)?);
        }
        for (key, values) in &self.environment_path_prepend {
            let mut paths = values
                .iter()
                .map(|value| expand(value, root, output, target, runtime).map(PathBuf::from))
                .collect::<Result<Vec<_>, _>>()?;
            let inherited = match self.environment.get(key) {
                Some(value) => Some(expand(value, root, output, target, runtime)?.into()),
                None => std::env::var_os(key),
            };
            if let Some(value) = inherited {
                paths.extend(std::env::split_paths(&value));
            }
            command.env(
                key,
                std::env::join_paths(paths)
                    .map_err(|error| format!("native test path {key}: {error}"))?,
            );
        }
        let directory = match &self.working_directory {
            Some(value) => PathBuf::from(expand(value, root, output, target, runtime)?),
            None => output.to_owned(),
        };
        command.current_dir(directory);
        Ok(command)
    }
}

fn default_suite() -> String {
    "core".into()
}

fn default_command() -> Vec<String> {
    vec!["${target}".into()]
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    pub schema: u32,
    pub targets: BTreeMap<String, Target>,
    pub tests: BTreeMap<String, Test>,
}

#[derive(Debug)]
pub struct Selection<'a> {
    pub targets: BTreeMap<&'a str, &'a Target>,
    pub tests: BTreeMap<&'a str, &'a Test>,
}

/// The selected native artifact and its public compile/link interface, matching
/// the information inherited from a CMake library target. Header-only interfaces
/// have no artifacts; they still carry required headers and definitions.
#[derive(Clone, Debug, Default)]
pub struct Dependency {
    pub runtime_path_prepend: BTreeMap<String, Vec<String>>,
    pub artifacts: Vec<PathBuf>,
    pub include_dirs: Vec<PathBuf>,
    pub definitions: Vec<(String, Option<String>)>,
    pub compile_options: Vec<String>,
    pub c_compile_options: Vec<String>,
    pub cpp_compile_options: Vec<String>,
    pub link_options: Vec<String>,
}

impl From<SharedLibrary> for Dependency {
    fn from(library: SharedLibrary) -> Self {
        Self {
            runtime_path_prepend: BTreeMap::new(),
            artifacts: library.dependencies,
            include_dirs: library.include_dirs,
            definitions: library.definitions,
            compile_options: library.compile_options,
            c_compile_options: library.c_compile_options,
            cpp_compile_options: library.cpp_compile_options,
            link_options: library.link_options,
        }
    }
}

impl Dependency {
    /// Resolve a native SDK package into its compile and link interface.
    /// Qt6 packages come from explicit SDK inputs when given (see [`crate::qt::package`]),
    /// carrying their loader paths; other callers supply runtime paths themselves.
    pub fn package(target: &str, package: &str) -> Result<Self, String> {
        let (host, _) = crate::qt::build_triples();
        let inputs = crate::qt::Inputs::from_env();
        if let Some(qt) = crate::qt::package(&host, target, package, &inputs)? {
            return Ok(qt);
        }
        let mut library = SharedLibrary::default();
        crate::libobs::add_package(&mut library, target, package)?;
        Ok(library.into())
    }
}

impl Graph {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("native test graph {}: {error}", path.display()))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let graph: Self = serde_json::from_str(text).map_err(|error| error.to_string())?;
        if graph.schema != 1 {
            return Err(format!(
                "unsupported native test graph schema {}",
                graph.schema
            ));
        }
        if graph.targets.is_empty() || graph.tests.is_empty() {
            return Err("native test graph has no targets or tests".into());
        }
        let mut ui_outputs = BTreeSet::new();
        for (name, target) in &graph.targets {
            for form in &target.qt.uic {
                let stem = Path::new(form)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .ok_or_else(|| format!("invalid Qt UI output for {form:?}"))?;
                if !ui_outputs.insert(stem.to_ascii_lowercase()) {
                    return Err(format!("duplicate Qt UI output ui_{stem}.h"));
                }
            }
            validate_name(name)?;
            for input in &target.watch_inputs {
                if input.is_empty()
                    || input.starts_with('/')
                    || input.contains(['\\', ':', ';'])
                    || input.chars().any(char::is_control)
                    || input
                        .split('/')
                        .any(|part| part.is_empty() || part == "." || part == "..")
                {
                    return Err(format!("invalid native test watch input {input:?}"));
                }
            }
            for (definition, artifact) in &target.artifact_definitions {
                if definition.is_empty()
                    || !definition.bytes().enumerate().all(|(i, b)| {
                        b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit())
                    })
                    || artifact.is_empty()
                {
                    return Err(format!(
                        "invalid native test artifact definition {definition:?}"
                    ));
                }
            }
            for definition in &target.definitions {
                validate_compile_definition(definition)?;
            }
            if target.sources.is_empty()
                || target.sources.iter().any(|source| source.path().is_empty())
            {
                return Err(format!("native test target {name} has no sources"));
            }
        }
        for (name, test) in &graph.tests {
            validate_name(name)?;
            validate_name(&test.suite)?;
            if !graph.targets.contains_key(&test.target) {
                return Err(format!(
                    "native test {name} references absent target {}",
                    test.target
                ));
            }
            if test.command.is_empty() || test.command[0].is_empty() {
                return Err(format!("native test {name} has no command"));
            }
            for prefix in &test.optional_prefixes {
                if prefix.is_empty() || prefix[0].is_empty() || prefix[0].contains("${") {
                    return Err(format!(
                        "native test {name} command prefix needs a literal program"
                    ));
                }
                for value in &prefix[1..] {
                    validate_template(value)?;
                }
            }
            for value in test
                .command
                .iter()
                .chain(test.environment.values())
                .chain(test.working_directory.iter())
                .chain(test.environment_path_prepend.values().flatten())
            {
                validate_template(value)?;
            }
        }
        Ok(graph)
    }

    pub fn retain_suite(&mut self, suite: &str) {
        self.tests.retain(|_, test| test.suite == suite);
        let targets: BTreeSet<_> = self
            .tests
            .values()
            .map(|test| test.target.as_str())
            .collect();
        self.targets
            .retain(|name, _| targets.contains(name.as_str()));
    }

    pub fn select(&self, facts: &BTreeMap<String, bool>) -> Result<Selection<'_>, String> {
        let mut selected = Selection {
            targets: BTreeMap::new(),
            tests: BTreeMap::new(),
        };
        for (name, target) in &self.targets {
            if target.requires.evaluate(facts)? {
                selected.targets.insert(name.as_str(), target);
            }
        }
        for (name, test) in &self.tests {
            let enabled = test.requires.evaluate(facts)?;
            if enabled && selected.targets.contains_key(test.target.as_str()) {
                selected.tests.insert(name.as_str(), test);
            }
        }
        Ok(selected)
    }

    /// Compile selected targets through cc::Build. Named external libraries
    /// must resolve to the caller's selected artifacts, never a source-tree
    /// search or an implicit same-named system library.
    #[expect(clippy::too_many_lines, reason = "one block per validation rule")]
    pub fn compile(
        &self,
        build: &NativeBuild,
        root: &Path,
        facts: &BTreeMap<String, bool>,
        libraries: &BTreeMap<String, Dependency>,
    ) -> Result<BTreeMap<String, PathBuf>, String> {
        let root = dunce::canonicalize(root).map_err(|error| error.to_string())?;
        let selected = self.select(facts)?;
        let mut artifacts = BTreeMap::new();
        let mut watched_inputs = BTreeSet::new();
        for (name, spec) in selected.targets {
            let mut target = SharedLibrary {
                name: name.into(),
                cpp_standard: spec.cpp_standard,
                include_dirs: spec
                    .includes_before
                    .iter()
                    .map(|directory| root.join(directory))
                    .chain(std::iter::once(root.clone()))
                    .collect(),
                dependencies: spec.dependency_artifacts(facts, libraries)?,
                compile_options: spec
                    .compile_options
                    .iter()
                    .filter_map(|option| option.selected(facts).transpose())
                    .map(|value| value.map(str::to_owned))
                    .collect::<Result<Vec<_>, _>>()?,
                link_options: spec.link_options.clone(),
                ..Default::default()
            };
            for source in &spec.sources {
                if !source.selected(facts)? {
                    continue;
                }
                let source = source.path();
                // Keep component headers in Cargo's invalidation inputs when
                // moving standalone tests out of the legacy native inventory.
                if source.starts_with("plugins/") || source.starts_with("shared/") {
                    let component: PathBuf = Path::new(source).components().take(2).collect();
                    watched_inputs.insert(root.join(component));
                }
                let path = root.join(source);
                println!("cargo:rerun-if-changed={}", path.display());
                let extension = if source.ends_with(".c.h") {
                    Some("c")
                } else if source.ends_with(".cpp.hpp") {
                    Some("cpp")
                } else if source.ends_with(".m.h") {
                    Some("m")
                } else if source.ends_with(".mm.hpp") {
                    Some("mm")
                } else {
                    None
                };
                if let Some(extension) = extension {
                    let include = path
                        .to_str()
                        .ok_or("non-UTF8 native test source")?
                        .replace('\\', "/");
                    if include.contains(['\n', '\r', '"']) {
                        return Err(format!("unrepresentable native test source {include:?}"));
                    }
                    let wrapper = build
                        .output
                        .join("native-tests")
                        .join(name)
                        .join(format!("{source}.{extension}"));
                    fs::create_dir_all(
                        wrapper
                            .parent()
                            .ok_or("native test wrapper has no parent")?,
                    )
                    .map_err(|error| error.to_string())?;
                    crate::frontend::write(&wrapper, &format!("#include \"{include}\"\n"))?;
                    target.sources.push(wrapper);
                } else {
                    target.sources.push(path);
                }
            }
            if target.sources.is_empty() {
                return Err(format!("native test target {name} has no selected sources"));
            }
            for (definition, artifact_name) in &spec.artifact_definitions {
                let dependency = libraries
                    .get(artifact_name)
                    .ok_or_else(|| format!("unresolved native test artifact {artifact_name}"))?;
                if dependency.artifacts.len() != 1 {
                    return Err(format!(
                        "native test artifact {artifact_name} requires exactly one file artifact"
                    ));
                }
                let artifact = dunce::canonicalize(&dependency.artifacts[0])
                    .map_err(|error| format!("native test artifact {artifact_name}: {error}"))?;
                if !artifact.is_file() {
                    return Err(format!(
                        "native test artifact {artifact_name} is not a file"
                    ));
                }
                let path = artifact
                    .to_str()
                    .ok_or("non-UTF8 native test artifact")?
                    .replace('\\', "/");
                target.definitions.push((
                    definition.clone(),
                    Some(serde_json::to_string(&path).map_err(|error| error.to_string())?),
                ));
            }
            for input in &spec.watch_inputs {
                watched_inputs.insert(root.join(input));
            }
            for directory in &spec.includes {
                target.include_dirs.push(root.join(directory));
            }
            for definition in &spec.definitions {
                let definition = expand_compile_definition(definition, &root, &build.output)?;
                let (key, value) = definition
                    .split_once('=')
                    .map_or((definition.as_str(), None), |(key, value)| {
                        (key, Some(value.to_owned()))
                    });
                target.definitions.push((key.into(), value));
            }
            for name in &spec.header_libraries {
                let dependency = libraries
                    .get(name)
                    .ok_or_else(|| format!("unresolved native test header library {name}"))?;
                target
                    .include_dirs
                    .extend(dependency.include_dirs.iter().cloned());
            }
            for package in &spec.packages {
                crate::libobs::add_package(&mut target, &build.target, package)?;
            }
            for package in &spec.header_packages {
                super::headers_only(&mut target, package)?;
            }
            // Static archives precede their dynamic dependencies under
            // --as-needed, as in the existing native compatibility builder.
            for name in spec.library_names(facts)? {
                let dependency = libraries
                    .get(&name)
                    .ok_or_else(|| format!("unresolved native test library {name}"))?;
                target
                    .include_dirs
                    .extend(dependency.include_dirs.iter().cloned());
                target
                    .definitions
                    .extend(dependency.definitions.iter().cloned());
                target
                    .compile_options
                    .extend(dependency.compile_options.iter().cloned());
                target
                    .c_compile_options
                    .extend(dependency.c_compile_options.iter().cloned());
                target
                    .cpp_compile_options
                    .extend(dependency.cpp_compile_options.iter().cloned());
                target
                    .link_options
                    .extend(dependency.link_options.iter().cloned());
            }
            for flag in &spec.compile_options_if_supported {
                if build
                    .compiler(&target, &build.output, false)
                    .is_flag_supported(flag)
                    .map_err(|error| error.to_string())?
                {
                    target.compile_options.push(flag.clone());
                }
            }
            crate::platform::configure_test_sanitizers(
                build,
                &mut target,
                spec.sanitizers.as_deref(),
            )?;
            for (index, header) in spec.qt.moc.iter().enumerate() {
                crate::qt::moc(
                    &mut target,
                    &root.join(header),
                    &build
                        .output
                        .join("native-tests")
                        .join(name)
                        .join("generated")
                        .join(format!("moc_{index}.cpp")),
                )?;
            }
            let generated = build
                .output
                .join("native-tests")
                .join(name)
                .join("generated");
            for form in &spec.qt.uic {
                let stem = Path::new(form)
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| format!("invalid Qt UI input {form:?}"))?;
                crate::qt::uic(
                    &mut target,
                    &root.join(form),
                    &generated.join(format!("ui_{stem}.h")),
                )?;
            }
            for (index, resources) in spec.qt.rcc.iter().enumerate() {
                crate::qt::rcc(
                    &mut target,
                    &root.join(resources),
                    &generated.join(format!("qrc_{index}.cpp")),
                )?;
            }
            artifacts.insert(name.into(), build.executable(&target)?);
        }
        for directory in watched_inputs {
            println!("cargo:rerun-if-changed={}", directory.display());
        }
        Ok(artifacts)
    }
}

pub(crate) fn expand_compile_definition(
    value: &str,
    root: &Path,
    output: &Path,
) -> Result<String, String> {
    validate_compile_definition(value)?;
    // CMake represents paths in compile definitions with forward slashes.
    // Native Windows separators would become C string escapes (for example
    // C:\Users contains a universal-character escape), unlike runtime argv.
    let root = PathBuf::from(
        root.to_str()
            .ok_or("non-UTF8 native test source path")?
            .replace('\\', "/"),
    );
    let output = PathBuf::from(
        output
            .to_str()
            .ok_or("non-UTF8 native test build path")?
            .replace('\\', "/"),
    );
    expand(value, &root, &output, Path::new(""), Path::new(""))
}

fn validate_compile_definition(value: &str) -> Result<(), String> {
    validate_template(value)?;
    if value.contains("${target}") || value.contains("${runtime}") {
        return Err("native test compile definition cannot reference its target executable or runtime directory".into());
    }
    Ok(())
}

fn expand(
    value: &str,
    root: &Path,
    output: &Path,
    target: &Path,
    runtime: &Path,
) -> Result<String, String> {
    let mut expanded = String::new();
    let mut rest = value;
    while let Some((prefix, suffix)) = rest.split_once("${") {
        expanded.push_str(prefix);
        let (token, suffix) = suffix
            .split_once('}')
            .ok_or("unterminated native test template")?;
        let path = match token {
            "source" => root,
            "build" => output,
            "target" => target,
            "runtime" => runtime,
            _ => return Err(format!("unknown native test template {token}")),
        };
        expanded.push_str(path.to_str().ok_or("non-UTF8 native test runtime path")?);
        rest = suffix;
    }
    expanded.push_str(rest);
    Ok(expanded)
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-.".contains(&byte))
    {
        return Err(format!("invalid native test name {name:?}"));
    }
    Ok(())
}

fn validate_template(mut value: &str) -> Result<(), String> {
    while let Some((_, suffix)) = value.split_once("${") {
        let (token, rest) = suffix
            .split_once('}')
            .ok_or_else(|| format!("unterminated native test template {value:?}"))?;
        if !["source", "build", "target", "runtime"].contains(&token) {
            return Err(format!("unknown native test template {token}"));
        }
        value = rest;
    }
    Ok(())
}
