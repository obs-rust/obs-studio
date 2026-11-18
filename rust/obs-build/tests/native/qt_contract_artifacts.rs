//! Split the four published-SDK contracts into Linux preparation and target execution.
//! Every graph/CMake assertion stays in its original test. Only target process
//! execution is deferred; a successful preparation receipt is not a runtime pass.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub(super) struct Directory {
    path: PathBuf,
    _temporary: Option<tempfile::TempDir>,
}

impl Directory {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

fn preparation_directory() -> Option<std::ffi::OsString> {
    std::env::var_os("OBS_QT_CONTRACT_PREPARE").or_else(|| std::env::var_os("OBS_MACOS_QT_PREPARE"))
}

pub(super) fn preparing() -> bool {
    preparation_directory().is_some()
}

pub(super) fn relocatable(
    interfaces: &mut std::collections::BTreeMap<String, obs_build::validation::graph::Dependency>,
) {
    if preparing() && target().contains("apple") {
        for dependency in interfaces.values_mut() {
            // The native linker adds search rpaths for dependency artifacts,
            // including static archives. Keep the same real archive on the
            // link line without embedding its Linux build directory as rpath.
            for path in std::mem::take(&mut dependency.artifacts) {
                if path.extension().is_some_and(|extension| extension == "a") {
                    dependency.link_options.push(path.display().to_string());
                } else {
                    dependency.artifacts.push(path);
                }
            }
            dependency
                .link_options
                .retain(|option| !option.starts_with("-Wl,-rpath,"));
            dependency
                .link_options
                .push("-Wl,-rpath,@executable_path/../../../../qt/lib".into());
        }
    }
}

pub(super) fn target() -> String {
    if preparing() {
        std::env::var("OBS_QT_CONTRACT_TARGET").unwrap_or_else(|_| "aarch64-apple-darwin".into())
    } else {
        obs_build::BUILD_TARGET.into()
    }
}

pub(super) fn directory() -> Directory {
    if let Some(work) = preparation_directory() {
        let thread = std::thread::current();
        let name = thread.name().expect("named contract test");
        let name = if target().contains("windows") {
            name.replace("::", "--")
        } else {
            name.into()
        };
        let path = PathBuf::from(work).join(name);
        std::fs::create_dir(&path).expect("fresh Qt contract directory");
        Directory {
            path,
            _temporary: None,
        }
    } else {
        let temporary = tempfile::tempdir().unwrap();
        Directory {
            path: temporary.path().into(),
            _temporary: Some(temporary),
        }
    }
}

pub(super) fn output(mut command: Command) -> std::io::Result<Output> {
    let Some(work) = preparation_directory() else {
        return command.output();
    };
    let program = PathBuf::from(command.get_program());
    let args: Vec<_> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    if program == Path::new("cmake") || args.iter().any(|arg| arg == "--show-only=json-v1") {
        return command.output();
    }
    let contract = std::thread::current()
        .name()
        .expect("named contract test")
        .to_owned();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let helper = if target().contains("windows") {
        "ci/windows_qt_artifact.py"
    } else {
        "ci/macos_qt_artifact.py"
    };
    let mut recorder = Command::new("uv");
    recorder
        .args(["run", "--no-project"])
        .arg(root.join(helper))
        .arg("record")
        .arg("--work")
        .arg(work)
        .arg("--contract")
        .arg(contract);
    if program == Path::new("ctest") {
        let index = args
            .iter()
            .position(|arg| arg == "--test-dir")
            .expect("ctest build directory");
        recorder.arg("--ctest").arg(&args[index + 1]);
    } else {
        let env: std::collections::BTreeMap<_, _> = command
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value
                        .expect("explicit runtime value")
                        .to_string_lossy()
                        .into_owned(),
                )
            })
            .collect();
        let entry = serde_json::json!({
            "name": program.file_name().unwrap().to_string_lossy(),
            "binary": program, "args": args,
            "cwd": command.get_current_dir().map(Path::to_path_buf).unwrap_or(std::env::current_dir()?),
            "env": env, "timeout": 60.0,
        });
        recorder.arg("--entry").arg(entry.to_string());
    }
    recorder.output()
}
