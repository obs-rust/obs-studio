//! Windows resources stay out of C-family compilation, unity groups and PCH.
use crate::{NativeBuild, SharedLibrary};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Command,
};

#[expect(clippy::too_many_lines, reason = "one block per generated resource")]
pub(crate) fn compile(
    build: &NativeBuild,
    library: &SharedLibrary,
    output: &Path,
    compiler: &cc::Tool,
) -> Result<Vec<PathBuf>, String> {
    if !build.target.contains("windows-msvc") || !compiler.is_like_msvc() {
        return Err(format!(
            "native resources require an MSVC Windows target, got {}",
            build.target
        ));
    }
    let variables = [
        format!("RC_{}", build.target),
        format!("RC_{}", build.target.replace('-', "_")),
        format!(
            "{}_RC",
            if build.host == build.target {
                "HOST"
            } else {
                "TARGET"
            }
        ),
        "RC".into(),
    ];
    let executable = variables.iter().find_map(|name| {
        println!("cargo:rerun-if-env-changed={name}");
        std::env::var_os(name)
    });
    let mut base = if let Some(path) = executable {
        Command::new(path)
    } else if let Some(tool) = cc::windows_registry::find_tool(&build.target, "rc.exe") {
        tool.to_command()
    } else {
        Command::new(if compiler.is_like_clang() {
            "llvm-rc"
        } else {
            "rc"
        })
    };
    base.envs(compiler.env().iter().cloned());
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut common = Vec::<std::ffi::OsString>::new();
    for include in &library.include_dirs {
        common.push("/i".into());
        common.push(dunce::simplified(&cwd.join(include)).as_os_str().into());
    }
    for (name, value) in &library.definitions {
        common.push("/d".into());
        common.push(
            match value {
                Some(value) => format!("{name}={value}"),
                None => name.clone(),
            }
            .into(),
        );
    }
    common.extend(library.resource_compile_options.iter().map(Into::into));
    for input in &library.resource_input_files {
        let input = dunce::canonicalize(input)
            .map_err(|e| format!("resource input {}: {e}", input.display()))?;
        println!("cargo:rerun-if-changed={}", input.display());
    }
    let mut seen = HashSet::new();
    let mut objects = Vec::new();
    for (index, input) in library.resource_files.iter().enumerate() {
        let input =
            dunce::canonicalize(input).map_err(|e| format!("resource {}: {e}", input.display()))?;
        if !seen.insert(input.clone()) {
            return Err(format!("duplicate resource input: {}", input.display()));
        }
        if !input
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("rc"))
        {
            return Err(format!(
                "native resource input must be an .rc script: {}",
                input.display()
            ));
        }
        println!("cargo:rerun-if-changed={}", input.display());
        let artifact = output.join(format!("resource-{index}.res"));
        let mut command = Command::new(base.get_program());
        command.args(base.get_args());
        for (name, value) in base.get_envs() {
            if let Some(value) = value {
                command.env(name, value);
            } else {
                command.env_remove(name);
            }
        }
        command
            .current_dir(input.parent().ok_or("resource has no parent")?)
            .args(&common)
            .arg("/fo")
            .arg(dunce::simplified(&artifact))
            .arg(dunce::simplified(&input));
        let result = command
            .output()
            .map_err(|e| format!("execute resource compiler: {e}"))?;
        if !result.status.success() {
            return Err(format!(
                "resource {} failed ({})\n{}\n{}",
                input.display(),
                result.status,
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            ));
        }
        if !artifact.is_file() {
            return Err(format!(
                "resource compiler did not produce {}",
                artifact.display()
            ));
        }
        objects.push(artifact);
    }
    Ok(objects)
}
