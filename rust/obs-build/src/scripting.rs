//! Native scripting library and generated language bindings.
use crate::{NativeBuild, SharedLibrary, libobs::add_package};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Options {
    pub lua: bool,
    pub python: bool,
    /// Binding directory relative to the runtime root containing bin/.
    pub module_dir: PathBuf,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            lua: true,
            python: true,
            module_dir: "lib64/obs-scripting".into(),
        }
    }
}

#[derive(Debug)]
pub struct Artifacts {
    pub library: PathBuf,
    pub runtime: PathBuf,
    pub lua_module: Option<PathBuf>,
    pub python_module: Option<PathBuf>,
    pub python_wrapper: Option<PathBuf>,
}

#[expect(
    clippy::too_many_lines,
    reason = "flat ordered build stages, kept in one place"
)]
pub fn build(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    frontend: Option<&Path>,
    options: &Options,
) -> Result<Artifacts, String> {
    if !options.lua && !options.python {
        return Err("scripting requires at least one enabled language".into());
    }
    let module_dir = options
        .module_dir
        .to_str()
        .ok_or("non-UTF8 scripting module path")?;
    if module_dir.is_empty()
        || module_dir.contains(['"', '\\', '\n', '\r'])
        || !options
            .module_dir
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
    {
        return Err("scripting module path must be a relative runtime directory".into());
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let directory = root.join("shared/obs-scripting");
    let generated = build.output.join("scripting-generated");
    std::fs::create_dir_all(generated.join("swig")).map_err(|e| e.to_string())?;
    let generated = generated.canonicalize().map_err(|e| e.to_string())?;
    let runtime_root = build.output.join("runtime");
    std::fs::create_dir_all(&runtime_root).map_err(|e| e.to_string())?;
    let runtime_root = runtime_root.canonicalize().map_err(|e| e.to_string())?;
    let bindings = runtime_root.join(&options.module_dir);
    std::fs::create_dir_all(&bindings).map_err(|e| e.to_string())?;
    let bindings = bindings.canonicalize().map_err(|e| e.to_string())?;
    let swig = swig()?;
    let mut library = SharedLibrary {
        name: "obs-scripting".into(),
        sources: vec![
            directory.join("build/obs-scripting.core+.h.c"),
            directory.join("build/obs-scripting.cstrcache+.hpp.cpp"),
        ],
        include_dirs: vec![
            generated.clone(),
            root.clone(),
            directory.clone(),
            root.join("libobs"),
            build.output.join("config"),
        ],
        dependencies: vec![core.canonicalize().map_err(|e| e.to_string())?],
        definitions: vec![("SCRIPT_DIR".into(), Some(format!("\"{module_dir}\"")))],
        ..SharedLibrary::default()
    };
    crate::platform::configure_scripting(&build.target, &mut library)?;
    add_package(&mut library, &build.target, "simde")?;
    if let Some(api) = frontend {
        library
            .dependencies
            .push(api.canonicalize().map_err(|e| e.to_string())?);
        library.include_dirs.push(root.join("frontend/api"));
        library.definitions.push(("ENABLE_FRONTEND".into(), None));
    }
    let binding_base = library.clone();
    if options.lua {
        add_package(&mut library, &build.target, "luajit")?;
        library.definitions.push(("LUAJIT_FOUND".into(), None));
        library
            .sources
            .push(directory.join("build/obs-scripting.lua+.h.c"));
        runtime(&swig, "lua", &generated.join("swig/swigluarun.h"))?;
        if frontend.is_some() {
            library
                .sources
                .push(directory.join("build/obs-scripting.lua-frontend+.h.c"));
        }
    }
    if options.python {
        let python = pkg_config::Config::new()
            .cargo_metadata(false)
            .atleast_version("3.8")
            .probe("python3-embed")
            .map_err(|e| e.to_string())?;
        let python_name = crate::platform::python_library_name(&build.target, &python.libs)?;
        write(
            &generated.join("obspython.h"),
            &format!("#pragma once\n#define PYTHON_LIB \"{python_name}\"\n"),
        )?;
        library.definitions.extend([
            ("Python_FOUND".into(), None),
            ("ENABLE_SCRIPTING".into(), None),
            ("PYTHON_LIB".into(), Some(format!("\"{python_name}\""))),
        ]);
        add_package(&mut library, &build.target, "python3-embed")?;
        library
            .sources
            .push(directory.join("build/obs-scripting.python+.h.c"));
        runtime(&swig, "python", &generated.join("swig/swigpyrun.h"))?;
        if frontend.is_some() {
            library
                .sources
                .push(directory.join("build/obs-scripting.python-frontend+.h.c"));
        }
    }
    // Track the input trees SWIG reads, including nested %include files.
    for input in [&directory, &root.join("libobs"), &root.join("frontend/api")] {
        println!("cargo:rerun-if-changed={}", input.display());
    }
    let artifact = build.shared_library(&library)?;
    let mut result = Artifacts {
        library: artifact.clone(),
        runtime: runtime_root,
        lua_module: None,
        python_module: None,
        python_wrapper: None,
    };
    for (enabled, language, module_name) in [
        (options.lua, "lua", "obslua"),
        (options.python, "python", "obspython"),
    ] {
        if !enabled {
            continue;
        }
        let output = generated.join(language);
        std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
        let temporary = output.join(format!("{module_name}_wrap.next.c"));
        let mut command = std::process::Command::new(&swig);
        command.arg(format!("-{language}"));
        for include in &library.include_dirs {
            command.arg(format!("-I{}", include.display()));
        }
        command.arg(format!("-DSWIG_TYPE_TABLE={module_name}"));
        command.arg(if language == "lua" {
            "-DSWIG_LUA_INTERPRETER_NO_DEBUG"
        } else {
            "-DSWIG_PYTHON_INTERPRETER_NO_DEBUG"
        });
        if language == "python" {
            command.arg("-DPy_ENABLE_SHARED=1");
        }
        if frontend.is_some() {
            command.arg("-DENABLE_FRONTEND");
        }
        command
            .arg("-outdir")
            .arg(&output)
            .arg("-o")
            .arg(&temporary)
            .arg(directory.join(module_name).join(format!("{module_name}.i")));
        run(&mut command)?;
        let source = output.join(format!("{module_name}_wrap.c"));
        write(
            &source,
            &std::fs::read_to_string(&temporary).map_err(|e| e.to_string())?,
        )?;
        let mut module = binding_base.clone();
        add_package(
            &mut module,
            &build.target,
            if language == "lua" {
                "luajit"
            } else {
                "python3-embed"
            },
        )?;
        module.name = module_name.into();
        module.sources = vec![
            source,
            directory.join("build/obs-scripting.cstrcache+.hpp.cpp"),
        ];
        module.dependencies.push(artifact.clone());
        module
            .definitions
            .push(("SWIG_TYPE_TABLE".into(), Some(module_name.into())));
        module.definitions.push((
            if language == "lua" {
                "SWIG_LUA_INTERPRETER_NO_DEBUG"
            } else {
                "SWIG_PYTHON_INTERPRETER_NO_DEBUG"
            }
            .into(),
            None,
        ));
        if language == "python" {
            module
                .definitions
                .push(("Py_ENABLE_SHARED".into(), Some("1".into())));
        }
        module.file_name = Some(crate::platform::scripting_module_name(
            &build.target,
            language,
        )?);
        let compiled = build.shared_library(&module)?;
        let installed = bindings.join(
            compiled
                .file_name()
                .ok_or("binding module has no filename")?,
        );
        std::fs::copy(&compiled, &installed).map_err(|e| e.to_string())?;
        if language == "lua" {
            result.lua_module = Some(installed);
        } else {
            result.python_module = Some(installed);
            let wrapper = bindings.join("obspython.py");
            write(
                &wrapper,
                &std::fs::read_to_string(output.join("obspython.py")).map_err(|e| e.to_string())?,
            )?;
            result.python_wrapper = Some(wrapper);
        }
    }
    Ok(result)
}

fn swig() -> Result<PathBuf, String> {
    println!("cargo:rerun-if-env-changed=OBS_SWIG");
    println!("cargo:rerun-if-env-changed=SWIG_LIB");
    let tool = std::env::var_os("OBS_SWIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| "swig".into());
    let output = std::process::Command::new(&tool)
        .arg("-version")
        .output()
        .map_err(|e| format!("execute SWIG {}: {e}", tool.display()))?;
    let version = String::from_utf8_lossy(&output.stdout);
    let major = version
        .lines()
        .find_map(|line| line.trim().strip_prefix("SWIG Version "))
        .and_then(|value| value.split('.').next())
        .and_then(|value| value.parse::<u32>().ok());
    if !output.status.success() || !major.is_some_and(|major| major >= 4) {
        return Err(format!("SWIG >=4 is required: {version}"));
    }
    Ok(tool)
}

fn runtime(swig: &Path, language: &str, path: &Path) -> Result<(), String> {
    let temporary = path.with_extension("next.h");
    run(std::process::Command::new(swig)
        .arg(format!("-{language}"))
        .arg("-external-runtime")
        .arg(&temporary))?;
    write(
        path,
        &std::fs::read_to_string(&temporary).map_err(|e| e.to_string())?,
    )
}

fn run(command: &mut std::process::Command) -> Result<(), String> {
    let result = command
        .output()
        .map_err(|e| format!("execute {command:?}: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "SWIG {command:?} failed: {}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    Ok(())
}

fn write(path: &Path, contents: &str) -> Result<(), String> {
    if std::fs::read_to_string(path).ok().as_deref() != Some(contents) {
        std::fs::write(path, contents).map_err(|e| e.to_string())?;
    }
    Ok(())
}
