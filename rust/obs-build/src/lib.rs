//! Native compilation and dynamic linking for the Cargo-driven OBS build.
//!
//! This engine owns native artifacts. Cargo remains responsible for compiling
//! Rust crates; native compiles go through cc::Build and inherit soldr's tools.
#![forbid(unsafe_code)]

use std::{fs, path::PathBuf};

mod deps;
pub mod frontend;
pub mod graphics;
pub mod libobs;
mod platform;
pub mod plugins;
pub mod qt;

#[derive(Clone, Debug, Default)]
pub struct SharedLibrary {
    pub name: String,
    pub sources: Vec<PathBuf>,
    /// Objects compiled once and reused by multiple shared-library variants.
    pub precompiled_objects: Vec<PathBuf>,
    pub include_dirs: Vec<PathBuf>,
    pub dependencies: Vec<PathBuf>,
    pub unity: bool,
    pub definitions: Vec<(String, Option<String>)>,
    pub compile_options: Vec<String>,
    pub c_compile_options: Vec<String>,
    pub cpp_compile_options: Vec<String>,
    pub link_options: Vec<String>,
    pub file_name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NativeBuild {
    pub host: String,
    pub target: String,
    pub output: PathBuf,
}

impl NativeBuild {
    /// Compile through cc::Build, then link a native shared library.
    pub fn shared_library(&self, library: &SharedLibrary) -> Result<PathBuf, String> {
        let (output, cpp_link, mut objects) = self.objects(library)?;
        objects.extend(library.precompiled_objects.iter().cloned());
        let artifact = output.join(
            library
                .file_name
                .clone()
                .unwrap_or_else(|| platform::shared_filename(&self.target, &library.name)),
        );
        let compiler = self.compiler(library, &output, cpp_link).get_compiler();
        let mut command = compiler.to_command();
        platform::configure_shared_link(
            &self.target,
            &mut command,
            &artifact,
            &objects,
            &library.dependencies,
            compiler.is_like_msvc(),
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
        objects.extend(target.precompiled_objects.iter().cloned());
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
        let (output, cpp, objects) = self.objects(library)?;
        self.compiler(library, &output, cpp)
            .try_create_archive(&library.name, objects)
            .map_err(|e| e.to_string())
    }

    fn objects(&self, library: &SharedLibrary) -> Result<(PathBuf, bool, Vec<PathBuf>), String> {
        if library.name.is_empty()
            || !library
                .name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err(format!("invalid native target name {:?}", library.name));
        }
        if library.sources.is_empty() {
            return Err(format!("{} has no sources", library.name));
        }
        let output = self.output.join(&library.name);
        fs::create_dir_all(&output).map_err(|e| format!("create {}: {e}", output.display()))?;
        let output = output.canonicalize().map_err(|e| e.to_string())?;
        let mut c_sources = Vec::new();
        let mut cpp_sources = Vec::new();
        for source in &library.sources {
            let absolute = source
                .canonicalize()
                .map_err(|e| format!("{}: {e}", source.display()))?;
            match source.extension().and_then(|e| e.to_str()) {
                Some("c") => c_sources.push(absolute),
                Some("cpp" | "cc" | "cxx") => cpp_sources.push(absolute),
                other => {
                    return Err(format!(
                        "unsupported native source language {other:?}: {}",
                        source.display()
                    ));
                }
            }
        }
        let cpp_link = !cpp_sources.is_empty();
        let mut objects = Vec::new();
        for (cpp, sources) in [(false, c_sources), (true, cpp_sources)] {
            if sources.is_empty() {
                continue;
            }
            let mut compiler = self.compiler(library, &output, cpp);
            if library.unity {
                let group = output.join(if cpp { "unity.cpp" } else { "unity.c" });
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
        Ok((output, cpp_link, objects))
    }

    fn compiler(&self, library: &SharedLibrary, output: &std::path::Path, cpp: bool) -> cc::Build {
        let mut compiler = cc::Build::new();
        compiler
            .host(&self.host)
            .target(&self.target)
            .out_dir(output)
            .opt_level(2)
            .debug(true)
            .pic(true)
            .cpp(cpp)
            // Intermediate compilation and archive creation do not emit link
            // directives. Keep cc's compiler/archiver environment tracking so
            // target-qualified flags invalidate Cargo's bundled native build.
            .cargo_metadata(true);
        let mut includes = std::collections::HashSet::new();
        for directory in &library.include_dirs {
            if includes.insert(directory) {
                compiler.include(directory);
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
        compiler
    }
}
