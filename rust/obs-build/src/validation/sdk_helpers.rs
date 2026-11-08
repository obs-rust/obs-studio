//! Optional dependency builders for headless native SDK contracts.
use super::graph::Dependency;
use crate::{NativeBuild, SharedLibrary};
use std::{fs, path::Path, process::Command};

/// Build the original generated core-module object with the owner's module list.
pub fn core_manifest(
    build: &NativeBuild,
    root: &Path,
    names: &[&str],
) -> Result<Dependency, String> {
    let output = build.output.join("sdk-helpers");
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let entries = names
        .iter()
        .map(|name| serde_json::to_string(name).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let template = root.join("libobs/obs-core-modules.c.in");
    println!("cargo:rerun-if-changed={}", template.display());
    let text = fs::read_to_string(template)
        .map_err(|e| e.to_string())?
        .replace(
            "@OBS_CORE_MODULE_LIST@",
            &if entries.is_empty() {
                "NULL".into()
            } else {
                entries.join(",\n")
            },
        )
        .replace("@OBS_CORE_MODULE_COUNT@", &names.len().to_string());
    let source = output.join("obs-core-modules.c");
    crate::frontend::write(&source, &text)?;
    let library = SharedLibrary {
        name: "headless-core-manifest".into(),
        sources: vec![source],
        include_dirs: vec![root.join("libobs")],
        ..Default::default()
    };
    Ok(Dependency {
        artifacts: vec![build.static_library(&library)?],
        include_dirs: library.include_dirs,
        ..Default::default()
    })
}

/// Generate the real Lua/Python SWIG interfaces, matching test_scripting_obs_defs.cmake.
/// This builds bindings only; it never starts OBS, a renderer or the scripting runtime.
pub fn scripting_defs(
    build: &NativeBuild,
    root: &Path,
    core: &Dependency,
) -> Result<Dependency, String> {
    let directory = root.join("shared/obs-scripting");
    let output = build.output.join("sdk-helpers");
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    crate::frontend::write(
        &output.join("obspython.h"),
        "#pragma once\n#define PYTHON_LIB \"\"\n",
    )?;
    let swig = crate::scripting::swig()?;
    let mut result = Dependency::default();
    for (language, module, package) in [
        ("lua", "obslua", "luajit"),
        ("python", "obspython", "python3-embed"),
    ] {
        let generated = output.join(language);
        fs::create_dir_all(&generated).map_err(|e| e.to_string())?;
        let source = generated.join(format!("{module}_wrap.c"));
        let mut command = Command::new(&swig);
        command
            .arg(format!("-{language}"))
            .arg(format!("-I{}", directory.display()))
            .arg(format!("-I{}", output.display()));
        for include in &core.include_dirs {
            command.arg(format!("-I{}", include.display()));
        }
        if language == "python" {
            command.args(["-interface", "_obspython"]);
        }
        command
            .arg("-outdir")
            .arg(&generated)
            .arg("-o")
            .arg(&source)
            .arg(directory.join(module).join(format!("{module}.i")));
        let status = command.output().map_err(|e| format!("{command:?}: {e}"))?;
        if !status.status.success() {
            return Err(format!(
                "{command:?}: {}",
                String::from_utf8_lossy(&status.stderr)
            ));
        }
        let sdk = Dependency::package(&build.target, package)?;
        let mut library = SharedLibrary {
            name: format!("headless-{module}-defs"),
            sources: vec![source],
            include_dirs: vec![directory.clone(), output.clone()],
            definitions: vec![
                ("SWIG_TYPE_TABLE".into(), Some(module.into())),
                (
                    format!("SWIG_{}_INTERPRETER_NO_DEBUG", language.to_uppercase()),
                    None,
                ),
            ],
            ..Default::default()
        };
        if language == "python" {
            library
                .definitions
                .push(("Py_ENABLE_SHARED".into(), Some("1".into())));
        }
        library
            .include_dirs
            .extend(core.include_dirs.iter().chain(&sdk.include_dirs).cloned());
        library
            .compile_options
            .extend(sdk.compile_options.iter().cloned());
        result.artifacts.push(build.static_library(&library)?);
        result.include_dirs.extend(sdk.include_dirs);
        result.link_options.extend(sdk.link_options);
    }
    result.artifacts.extend(core.artifacts.iter().cloned());
    result
        .include_dirs
        .extend(core.include_dirs.iter().cloned());
    result
        .link_options
        .extend(core.link_options.iter().cloned());
    result.link_options.push("-lm".into());
    result.definitions.push((
        "OBSPYTHON_DIR".into(),
        Some(format!("\"{}\"", output.join("python").display())),
    ));
    for input in [directory, root.join("libobs")] {
        println!("cargo:rerun-if-changed={}", input.display());
    }
    Ok(result)
}
