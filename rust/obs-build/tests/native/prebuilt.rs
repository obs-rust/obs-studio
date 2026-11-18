//! Native fixture transport. Compiler assertions run on the Linux cross host;
//! the same tests replay that evidence and execute every load/run on the target.
use super::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, collections::BTreeMap, path::PathBuf, process::Command};

const TARGET: &str = "aarch64-apple-darwin";
const PREPARE: &str = "OBS_NATIVE_FIXTURES_PREPARE";
const PREBUILT: &str = "OBS_NATIVE_FIXTURES";

#[test]
fn windows_evidence_is_target_bound_and_resource_mutations_require_rebuilds() {
    let mut manifest = json!({"version": 1, "host": "x86_64-unknown-linux-gnu",
        "target": "x86_64-pc-windows-msvc", "complete": true, "events": [], "directories": []});
    assert!(validate_manifest_for_target(&manifest, "x86_64-pc-windows-msvc").is_ok());
    assert!(validate_manifest_for_target(&manifest, TARGET).is_err());
    manifest["complete"] = false.into();
    assert!(validate_manifest_for_target(&manifest, "x86_64-pc-windows-msvc").is_err());
    let first = json!({"profile": "Debug", "kind": "shared", "identity": "resources",
        "output": "out", "recipe": "RC /DVALUE=40", "inputs": {"payload.rc": "old"}});
    let mut changed = first.clone();
    changed["inputs"]["payload.rc"] = "new".into();
    let events = vec![json!({"request": first})];
    assert_eq!(rebuild_decision(&events, &changed)["required"], true);
    changed = events[0]["request"].clone();
    assert_eq!(rebuild_decision(&events, &changed)["required"], false);
    changed["recipe"] = "RC /DVALUE=50".into();
    assert_eq!(rebuild_decision(&events, &changed)["recipe"], true);
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}
struct State {
    bundle: PathBuf,
    name: String,
    manifest: Value,
    cursor: usize,
    directories: usize,
    replay: bool,
}

pub struct Scope;
impl Scope {
    pub fn enter() -> Self {
        let prepare = std::env::var_os(PREPARE);
        let replay = std::env::var_os(PREBUILT);
        assert!(
            prepare.is_none() || replay.is_none(),
            "fixture modes are mutually exclusive"
        );
        let Some(bundle) = replay.clone().or(prepare) else {
            return Self;
        };
        let name = std::thread::current().name().unwrap().replace("::", "--");
        let bundle = PathBuf::from(bundle);
        let manifest = if replay.is_some() {
            let manifest = read_manifest(&bundle, &name).unwrap();
            assert_eq!(
                manifest["host"],
                obs_build::BUILD_HOST,
                "BUILD_HOST evidence mismatch"
            );
            assert_eq!(
                manifest["target"],
                obs_build::BUILD_TARGET,
                "BUILD_TARGET evidence mismatch"
            );
            manifest
        } else {
            assert_eq!(
                std::env::consts::OS,
                "linux",
                "native fixture preparation requires a Linux executable"
            );
            json!({"version": 1, "host": obs_build::BUILD_HOST, "target": target(),
                "complete": false, "events": [], "directories": [],
                "host_cmake": std::env::var("OBS_NATIVE_HOST_CMAKE").unwrap_or_default()})
        };
        STATE.with(|state| {
            assert!(state.borrow().is_none(), "nested fixture scope");
            *state.borrow_mut() = Some(State {
                bundle,
                name,
                manifest,
                cursor: 0,
                directories: 0,
                replay: replay.is_some(),
            });
        });
        Self
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        STATE.with(|state| {
            let Some(mut state) = state.borrow_mut().take() else {
                return;
            };
            if std::thread::panicking() {
                return;
            }
            if state.replay {
                assert_eq!(
                    state.cursor,
                    state.manifest["events"].as_array().unwrap().len(),
                    "unconsumed fixture evidence in {}",
                    state.name
                );
                assert_eq!(
                    state.directories,
                    state.manifest["directories"].as_array().unwrap().len()
                );
            } else {
                state.manifest["complete"] = true.into();
                let directory = state.bundle.join(&state.name);
                fs::create_dir_all(&directory).unwrap();
                fs::write(
                    directory.join("manifest.json"),
                    serde_json::to_vec_pretty(&state.manifest).unwrap(),
                )
                .unwrap();
            }
        });
    }
}

pub fn target() -> &'static str {
    if std::env::var_os(PREPARE).is_some() {
        match std::env::var("OBS_NATIVE_FIXTURE_TARGET").as_deref() {
            Ok("x86_64-pc-windows-msvc") => "x86_64-pc-windows-msvc",
            Ok(TARGET) | Err(_) => TARGET,
            Ok(other) => panic!("unsupported native fixture target: {other}"),
        }
    } else {
        obs_build::BUILD_TARGET
    }
}

pub fn validate_manifest(manifest: &Value) -> Result<(), String> {
    let expected = target();
    validate_manifest_for_target(manifest, expected)
}
fn validate_manifest_for_target(manifest: &Value, expected: &str) -> Result<(), String> {
    if manifest["version"] != 1
        || manifest["target"] != expected
        || ![TARGET, "x86_64-pc-windows-msvc"].contains(&expected)
        || !manifest["host"]
            .as_str()
            .is_some_and(|host| host.contains("linux"))
        || manifest["complete"] != true
        || !manifest["events"].is_array()
        || !manifest["directories"].is_array()
    {
        return Err("incomplete or incompatible native fixture manifest".into());
    }
    Ok(())
}
fn read_manifest(bundle: &Path, name: &str) -> Result<Value, String> {
    let bytes = fs::read(bundle.join(name).join("manifest.json")).map_err(|e| e.to_string())?;
    let manifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub struct Directory {
    path: PathBuf,
    _temporary: Option<tempfile::TempDir>,
}
impl Directory {
    pub fn path(&self) -> &Path {
        &self.path
    }
}
pub fn tempdir() -> Result<Directory, std::io::Error> {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            let temporary = tempfile::tempdir()?;
            return Ok(Directory {
                path: temporary.path().into(),
                _temporary: Some(temporary),
            });
        };
        let index = state.directories;
        state.directories += 1;
        let path = if state.replay {
            PathBuf::from(state.manifest["directories"][index].as_str().unwrap())
        } else {
            let repository =
                dunce::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
            let run = Sha256::digest(state.bundle.as_os_str().as_encoded_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let path = repository
                .join("target/cross-native-fixture-work")
                .join(run)
                .join(&state.name)
                .join(index.to_string());
            assert!(
                !path.exists(),
                "stale fixture directory: {}",
                path.display()
            );
            state.manifest["directories"]
                .as_array_mut()
                .unwrap()
                .push(portable_path(&path).into());
            path
        };
        fs::create_dir_all(&path)?;
        Ok(Directory {
            path,
            _temporary: None,
        })
    })
}

fn digest(path: &Path) -> String {
    Sha256::digest(fs::read(path).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn files(root: &Path) -> Vec<PathBuf> {
    fixture_paths(root)
        .into_iter()
        .filter(|path| path.is_file())
        .collect()
}
fn fixture_paths(root: &Path) -> Vec<PathBuf> {
    if !root.exists() {
        return vec![];
    }
    let mut result = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        assert!(!entry.file_type().unwrap().is_symlink(), "fixture symlink");
        let path = entry.path();
        if path.is_dir() {
            result.extend(fixture_paths(&path));
        }
        result.push(path);
    }
    result.sort();
    result
}
fn portable_path(path: &Path) -> String {
    // read_dir returns native separators, while a caller can join a suffix
    // containing '/'. Store one spelling, including on Windows. This is only
    // representation normalization: keep '..' so restore still rejects escapes.
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn fixture_inventory_paths_have_one_portable_separator_form() {
    let expected = "C:/runner/work/output/runtime root/Release";
    for spelling in [
        r"C:\runner\work\output/runtime root/Release",
        r"C:\runner\work\output\runtime root\Release",
        expected,
    ] {
        assert_eq!(portable_path(Path::new(spelling)), expected);
    }
    assert_eq!(
        portable_path(Path::new(
            "/opt/obs-ci-42-1/src/output/runtime root/Release"
        )),
        "/opt/obs-ci-42-1/src/output/runtime root/Release"
    );
}

fn snapshot(state: &State) -> Value {
    let mut snapshots = Vec::new();
    let mut directories = Vec::new();
    for root in state.manifest["directories"].as_array().unwrap() {
        let root = Path::new(root.as_str().unwrap());
        for path in fixture_paths(root) {
            if path.is_dir() {
                directories.push(portable_path(&path));
                continue;
            }
            let hash = digest(&path);
            let stored = PathBuf::from("blobs").join(&hash);
            let destination = state.bundle.join(&stored);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            if !destination.exists() {
                fs::copy(&path, &destination).unwrap();
            }
            snapshots.push(json!({"file": portable_path(&stored), "destination": portable_path(&path), "sha256": hash}));
        }
    }
    json!({"files": snapshots, "directories": directories})
}
fn restore(state: &State, event: &Value) {
    // Files alone cannot preserve empty CTest working directories. Keep the
    // directory inventory in each checksummed event, including names with spaces.
    if let Some(directories) = event.get("directories") {
        for directory in directories.as_array().expect("fixture directory inventory") {
            let destination = Path::new(directory.as_str().unwrap());
            assert!(
                state.manifest["directories"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(
                        |root| destination.strip_prefix(root.as_str().unwrap()).is_ok_and(
                            |relative| relative.components().all(|component| matches!(
                                component,
                                std::path::Component::Normal(_)
                            ))
                        )
                    ),
                "fixture directory destination escape"
            );
            fs::create_dir_all(destination).unwrap();
        }
    }
    for entry in event["files"].as_array().unwrap() {
        let relative = Path::new(entry["file"].as_str().unwrap());
        assert!(
            !relative.is_absolute()
                && relative
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
        );
        let source = state.bundle.join(relative);
        assert_eq!(
            digest(&source),
            entry["sha256"].as_str().unwrap(),
            "corrupt fixture {}",
            source.display()
        );
        let destination = Path::new(entry["destination"].as_str().unwrap());
        assert!(
            state.manifest["directories"]
                .as_array()
                .unwrap()
                .iter()
                .any(|root| destination.starts_with(root.as_str().unwrap())),
            "fixture destination escape"
        );
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(source, destination).unwrap();
        if destination
            .file_name()
            .is_some_and(|name| name == "CTestTestfile.cmake")
        {
            let host_cmake = state.manifest["host_cmake"].as_str().unwrap();
            if !host_cmake.is_empty() {
                let text = fs::read_to_string(destination).unwrap();
                fs::write(destination, text.replace(host_cmake, "cmake")).unwrap();
            }
        }
    }
}
fn replay_event(request: &Value) -> Option<Value> {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let state = state.as_mut()?;
        if !state.replay {
            return None;
        }
        let event = state.manifest["events"][state.cursor].clone();
        assert_eq!(
            event["request"], *request,
            "fixture recipe changed at {} event {}",
            state.name, state.cursor
        );
        assert_eq!(
            event["rebuild"],
            rebuild_decision(
                &state.manifest["events"].as_array().unwrap()[..state.cursor],
                request
            ),
            "rebuild decision mismatch"
        );
        restore(state, &event);
        state.cursor += 1;
        Some(event["result"].clone())
    })
}
fn record(request: Value, result: Value) {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        assert!(!state.replay);
        let index = state.manifest["events"].as_array().unwrap().len();
        let snapshot = snapshot(state);
        let decision = rebuild_decision(
            &state.manifest["events"].as_array().unwrap()[..index],
            &request,
        );
        state.manifest["events"].as_array_mut().unwrap().push(
            json!({"request": request, "result": result, "files": snapshot["files"],
                "directories": snapshot["directories"], "rebuild": decision}),
        );
    });
}
pub fn preparing() -> bool {
    std::env::var_os(PREPARE).is_some()
}
fn active() -> bool {
    let active = STATE.with(|cell| cell.borrow().is_some())
        || preparing()
        || std::env::var_os(PREBUILT).is_some();
    if active {
        STATE.with(|cell| {
            assert!(
                cell.borrow().is_some(),
                "prebuilt fixture call outside its manifest scope"
            )
        });
    }
    active
}

#[test]
fn recorded_rebuild_decisions_cover_profiles_flags_headers_and_noop() {
    let request = json!({"kind": "shared", "output": "output", "profile": "Debug",
        "recipe": "unity=true,pch=true,flag=1", "inputs": {"source.c": "one", "header.h": "two"}});
    assert_eq!(
        rebuild_decision(&[], &request),
        json!({"required": true, "initial": true})
    );
    let events = [json!({"request": request})];
    assert_eq!(rebuild_decision(&events, &request)["required"], false);
    for (field, value) in [
        ("profile", json!("Release")),
        ("recipe", json!("unity=false,pch=false,flag=2")),
        (
            "inputs",
            json!({"source.c": "mutated", "header.h": "edited"}),
        ),
    ] {
        let mut changed = request.clone();
        changed[field] = value;
        assert_eq!(
            rebuild_decision(&events, &changed)["required"],
            true,
            "{field}"
        );
    }
}

#[test]
fn shared_output_rebuild_history_distinguishes_core_from_plugin() {
    let core = json!({"kind": "shared", "identity": "core", "output": "artifacts",
        "profile": "Debug", "recipe": "core", "inputs": {"core.c": "one"}});
    let mut plugin = core.clone();
    plugin["identity"] = "plugin".into();
    let mut events = vec![json!({"request": core})];
    assert_eq!(
        rebuild_decision(&events, &plugin),
        json!({"required": true, "initial": true})
    );
    events.push(json!({"request": plugin}));
    assert_eq!(rebuild_decision(&events, &core)["required"], false);
    let mut changed = core.clone();
    changed["inputs"]["core.c"] = "two".into();
    assert_eq!(
        rebuild_decision(&events, &changed),
        json!({"required": true, "profile": false,
        "recipe": false, "inputs": true})
    );
}

#[test]
fn replay_rejects_recipe_changes_before_touching_artifacts() {
    let directory = tempfile::tempdir().unwrap();
    let expected = json!({"kind": "shared", "profile": "Debug"});
    let manifest = json!({"events": [{"request": expected, "result": null, "files": [],
        "rebuild": {"required": true, "initial": true}}], "directories": []});
    STATE.with(|cell| {
        *cell.borrow_mut() = Some(State {
            bundle: directory.path().into(),
            name: "probe".into(),
            manifest,
            cursor: 0,
            directories: 0,
            replay: true,
        })
    });
    let result =
        std::panic::catch_unwind(|| replay_event(&json!({"kind": "shared", "profile": "Release"})));
    STATE.with(|cell| {
        cell.borrow_mut().take();
    });
    assert!(result.is_err());
}

#[test]
fn snapshot_replay_preserves_empty_space_named_working_directories() {
    let directory = tempfile::tempdir().unwrap();
    let work = directory.path().join("work");
    let runtime = work.join("output/runtime root/Release");
    fs::create_dir_all(&runtime).unwrap();
    fs::write(work.join("output/CTestTestfile.cmake"), "fixture").unwrap();
    let state = State {
        bundle: directory.path().join("bundle"),
        name: "probe".into(),
        manifest: json!({"directories": [work], "host_cmake": ""}),
        cursor: 0,
        directories: 0,
        replay: true,
    };
    // Serialize the producer snapshot as the bundle does; replay from a clean tree.
    let event: Value =
        serde_json::from_slice(&serde_json::to_vec(&snapshot(&state)).unwrap()).unwrap();
    assert!(
        event["directories"]
            .as_array()
            .unwrap()
            .contains(&json!(portable_path(&runtime)))
    );
    fs::remove_dir_all(&work).unwrap();
    restore(&state, &event);
    assert!(runtime.is_dir());
    assert_eq!(
        fs::read_to_string(work.join("output/CTestTestfile.cmake")).unwrap(),
        "fixture"
    );
    assert_eq!(fs::read_dir(runtime).unwrap().count(), 0);
    for escape in [directory.path().join("escape"), work.join("../escape")] {
        let event = json!({"files": [], "directories": [escape]});
        assert!(std::panic::catch_unwind(|| restore(&state, &event)).is_err());
        assert!(!directory.path().join("escape").exists());
    }
}

#[test]
fn replay_restores_artifacts_without_compiling_and_rejects_missing_or_corrupt_files() {
    let directory = tempfile::tempdir().unwrap();
    let work = directory.path().join("work");
    let bundle = directory.path().join("bundle");
    fs::create_dir_all(&work).unwrap();
    fs::create_dir_all(bundle.join("blobs")).unwrap();
    let image = bundle.join("blobs/image");
    fs::write(&image, b"fixture payload").unwrap();
    let destination = work.join("output/library");
    let build = NativeBuild {
        host: obs_build::BUILD_HOST.into(),
        target: TARGET.into(),
        output: work.join("output"),
        profile: Default::default(),
    };
    for mutation in ["valid", "corrupt", "missing"] {
        let manifest = json!({"events": [], "directories": [work]});
        STATE.with(|cell| {
            *cell.borrow_mut() = Some(State {
                bundle: bundle.clone(),
                name: "probe".into(),
                manifest,
                cursor: 0,
                directories: 0,
                replay: true,
            })
        });
        let request = build_request(&build, "shared", "library", "recipe".into());
        let result: Result<PathBuf, String> = Ok(destination.clone());
        let event = json!({"request": request, "result": result,
            "rebuild": {"required": true, "initial": true},
            "files": [{"file": "blobs/image", "destination": destination, "sha256": digest(&image)}]});
        STATE.with(|cell| cell.borrow_mut().as_mut().unwrap().manifest["events"] = json!([event]));
        if mutation == "corrupt" {
            fs::write(&image, b"changed payload").unwrap();
        }
        if mutation == "missing" {
            fs::remove_file(&image).unwrap();
        }
        let replayed = std::panic::catch_unwind(|| {
            artifact(&build, "shared", "library", "recipe".into(), || {
                panic!("prebuilt mode must never invoke the compiler")
            })
        });
        STATE.with(|cell| {
            cell.borrow_mut().take();
        });
        if mutation == "valid" {
            assert_eq!(replayed.unwrap().unwrap(), destination);
            assert_eq!(fs::read(&destination).unwrap(), b"fixture payload");
        } else {
            assert!(replayed.is_err(), "{mutation}");
        }
        fs::write(&image, b"fixture payload").unwrap();
    }
}

fn build_request(build: &NativeBuild, kind: &str, identity: &str, recipe: String) -> Value {
    let inputs: BTreeMap<_, _> = STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|state| {
                state.manifest["directories"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|root| files(Path::new(root.as_str().unwrap())))
                    .filter(|p| {
                        p.extension().is_some_and(|s| {
                            ["c", "cpp", "h", "hpp", "m", "mm", "swift", "in", "rc"]
                                .iter()
                                .any(|e| s == *e)
                        })
                    })
                    .filter(|p| !p.starts_with(&build.output))
                    .map(|path| (path.to_string_lossy().into_owned(), digest(&path)))
                    .collect()
            })
            .unwrap_or_default()
    });
    json!({"kind": kind, "identity": identity, "host": build.host, "target": build.target, "output": build.output,
        "profile": format!("{:?}", build.profile), "recipe": recipe, "inputs": inputs})
}

fn rebuild_decision(events: &[Value], request: &Value) -> Value {
    if request.get("profile").is_none() {
        return Value::Null;
    }
    let previous = events
        .iter()
        .rev()
        .map(|event| &event["request"])
        .find(|previous| {
            previous["kind"] == request["kind"]
                && previous["output"] == request["output"]
                && previous["identity"] == request["identity"]
        });
    let Some(previous) = previous else {
        return json!({"required": true, "initial": true});
    };
    let profile = previous["profile"] != request["profile"];
    let recipe = previous["recipe"] != request["recipe"];
    let inputs = previous["inputs"] != request["inputs"];
    json!({"required": profile || recipe || inputs, "profile": profile, "recipe": recipe, "inputs": inputs})
}
fn artifact(
    build: &NativeBuild,
    kind: &str,
    identity: &str,
    recipe: String,
    compile: impl FnOnce() -> Result<PathBuf, String>,
) -> Result<PathBuf, String> {
    if !active() {
        return compile();
    }
    let request = build_request(build, kind, identity, recipe);
    if let Some(result) = replay_event(&request) {
        return serde_json::from_value(result).unwrap();
    }
    let result = compile();
    record(request, serde_json::to_value(&result).unwrap());
    result
}
pub trait FixtureBuild {
    fn fixture_shared_library(&self, library: &SharedLibrary) -> Result<PathBuf, String>;
    fn fixture_executable(&self, library: &SharedLibrary) -> Result<PathBuf, String>;
    fn fixture_static_library(&self, library: &SharedLibrary) -> Result<PathBuf, String>;
    fn fixture_swift_shared_library(
        &self,
        library: &obs_build::SwiftLibrary,
    ) -> Result<PathBuf, String>;
}
impl FixtureBuild for NativeBuild {
    fn fixture_shared_library(&self, library: &SharedLibrary) -> Result<PathBuf, String> {
        artifact(
            self,
            "shared",
            &library.name,
            format!("{library:?}"),
            || self.shared_library(library),
        )
    }
    fn fixture_executable(&self, library: &SharedLibrary) -> Result<PathBuf, String> {
        artifact(
            self,
            "executable",
            &library.name,
            format!("{library:?}"),
            || self.executable(library),
        )
    }
    fn fixture_static_library(&self, library: &SharedLibrary) -> Result<PathBuf, String> {
        artifact(
            self,
            "archive",
            &library.name,
            format!("{library:?}"),
            || self.static_library(library),
        )
    }
    fn fixture_swift_shared_library(
        &self,
        library: &obs_build::SwiftLibrary,
    ) -> Result<PathBuf, String> {
        artifact(self, "swift", &library.name, format!("{library:?}"), || {
            self.swift_shared_library(library)
        })
    }
}

pub fn defer_answer(library: &Path, symbol: &str, expected: i32) -> bool {
    if !active() {
        return false;
    }
    let request =
        json!({"kind": "answer", "library": library, "symbol": symbol, "expected": expected});
    if preparing() {
        record(request, Value::Null);
        true
    } else {
        assert!(replay_event(&request).is_some());
        false
    }
}

#[test]
fn host_tool_recipes_preserve_every_non_tool_field() {
    let recipe = |cmake: &str| {
        let mut command = Command::new(cmake);
        command
            .args([
                "-E",
                "env",
                "OBS_WRAPPER_VALUE=outer",
                cmake,
                "-E",
                "env",
                "OBS_WRAPPER_VALUE=wrapped",
                "/fixture/answer",
                "two words",
            ])
            .env("OBS_GRAPH_VALUE", "first;second")
            .current_dir("/fixture/runtime root");
        command_recipe_with_tools(
            &command,
            &BTreeMap::from([("cmake".into(), PathBuf::from(cmake))]),
        )
    };
    let linux = recipe("/usr/local/bin/cmake");
    let macos = recipe("/opt/homebrew/bin/cmake");
    assert_eq!(linux, macos);
    assert_eq!(linux["program"], json!({"tool": "cmake"}));
    assert_eq!(linux["args"][3], json!({"tool": "cmake"}));
    for (pointer, replacement) in [
        ("/args/2", "OBS_WRAPPER_VALUE=changed"),
        ("/args/7", "/different/answer"),
        ("/args/8", "changed words"),
        ("/env/OBS_GRAPH_VALUE", "changed"),
        ("/cwd", "/different/cwd"),
    ] {
        let mut changed = macos.clone();
        *changed.pointer_mut(pointer).unwrap() = json!(replacement);
        assert_ne!(linux, changed, "{pointer}");
    }
    let tools = BTreeMap::from([("cmake".into(), PathBuf::from("/trusted/cmake"))]);
    let unknown = command_recipe_with_tools(Command::new("/arbitrary/cmake").arg("-E"), &tools);
    assert_eq!(unknown["program"], "/arbitrary/cmake");
    let literal = command_recipe_with_tools(
        Command::new("/trusted/cmake").args(["-E", "echo", "/trusted/cmake"]),
        &tools,
    );
    assert_eq!(literal["args"][2], "/trusted/cmake");
}

fn resolved_host_tools() -> BTreeMap<String, PathBuf> {
    let mut tools = BTreeMap::new();
    for name in ["cmake", "ctest"] {
        let mut suffixes = vec![String::new()];
        if cfg!(windows) {
            suffixes.extend(
                std::env::var("PATHEXT")
                    .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
                    .split(';')
                    .filter(|suffix| !suffix.is_empty())
                    .map(str::to_owned),
            );
        }
        let path = std::env::var_os("PATH").unwrap_or_default();
        let found = std::env::split_paths(&path)
            .flat_map(|directory| {
                suffixes
                    .iter()
                    .map(move |suffix| directory.join(format!("{name}{suffix}")))
            })
            .find(|candidate| host_tool_executable(candidate));
        if let Some(path) = found {
            let absolute = if path.is_absolute() {
                path
            } else {
                std::env::current_dir().unwrap().join(path)
            };
            // Match native_test_program: retain symlinks/argv[0], don't resolve
            // arbitrary aliases based on a basename or canonical target.
            tools.insert(name.into(), absolute);
        }
    }
    tools
}

fn host_tool_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        path.metadata().unwrap().permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    true
}

fn logical_tool(value: &str, tools: &BTreeMap<String, PathBuf>) -> Value {
    tools
        .iter()
        .find_map(|(name, path)| {
            (value == name || Path::new(value) == path).then(|| json!({"tool": name}))
        })
        .unwrap_or_else(|| json!(value))
}

fn logical_wrapper_args(program: &Value, args: &mut [Value], tools: &BTreeMap<String, PathBuf>) {
    let mut wrapper = program.clone();
    let mut offset = 0;
    // Only executable slots in cmake -E env wrappers are tool identities.
    // Literal path arguments (including cmake -E echo) stay byte-for-byte exact.
    while wrapper == json!({"tool": "cmake"})
        && args.get(offset) == Some(&json!("-E"))
        && args.get(offset + 1) == Some(&json!("env"))
    {
        offset += 2;
        while args
            .get(offset)
            .and_then(Value::as_str)
            .is_some_and(|arg| arg.contains('='))
        {
            offset += 1;
        }
        if args.get(offset) == Some(&json!("--")) {
            offset += 1;
        }
        let Some(child) = args.get_mut(offset) else {
            break;
        };
        *child = logical_tool(child.as_str().unwrap(), tools);
        wrapper = child.clone();
        offset += 1;
    }
}

fn command_recipe(command: &Command) -> Value {
    command_recipe_with_tools(command, &resolved_host_tools())
}

fn command_recipe_with_tools(command: &Command, tools: &BTreeMap<String, PathBuf>) -> Value {
    let environment: BTreeMap<_, _> = command
        .get_envs()
        .map(|(key, value)| {
            let mut value = value.map(|v| v.to_str().unwrap().to_owned());
            if key == "PATH"
                && let (Some(text), Ok(inherited)) = (&mut value, std::env::var("PATH"))
                && let Some(prefix) = text.strip_suffix(&inherited)
            {
                *text = format!("{prefix}{{inherited PATH}}");
            }
            (key.to_str().unwrap(), value)
        })
        .collect();
    let program = logical_tool(command.get_program().to_str().unwrap(), tools);
    let mut args = command
        .get_args()
        .map(|value| json!(value.to_str().unwrap()))
        .collect::<Vec<_>>();
    logical_wrapper_args(&program, &mut args, tools);
    json!({"program": program, "args": args,
        "cwd": command.get_current_dir(), "env": environment})
}

#[test]
fn logical_recipe_keeps_native_cmake_env_execution() {
    let tools = resolved_host_tools();
    let cmake = tools.get("cmake").expect("native contract requires cmake");
    let mut command = Command::new(cmake);
    command.args([
        "-E",
        "env",
        "OBS_WRAPPER_VALUE=outer",
        cmake.to_str().unwrap(),
        "-E",
        "env",
        "OBS_WRAPPER_VALUE=wrapped",
        cmake.to_str().unwrap(),
        "-E",
        "echo",
        "two words",
    ]);
    let recipe = command_recipe(&command);
    assert_eq!(recipe["program"], json!({"tool": "cmake"}));
    assert_eq!(recipe["args"][3], json!({"tool": "cmake"}));
    assert_eq!(recipe["args"][7], json!({"tool": "cmake"}));
    assert_eq!(command.get_program(), cmake.as_os_str());
    let output = command.output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "two words"
    );
}

pub fn assert_runtime(command: &mut Command, stdout: Option<&[u8]>, required: &[PathBuf]) {
    let request = json!({"kind": "run", "command": command_recipe(command),
        "stdout": stdout, "required": required});
    if preparing() {
        record(request, Value::Null);
        return;
    }
    if active() {
        assert!(replay_event(&request).is_some());
    }
    for path in required {
        if path.exists() {
            fs::remove_file(path).unwrap();
        }
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if let Some(expected) = stdout {
        assert_eq!(output.stdout, expected);
    }
    for path in required {
        assert!(path.is_file(), "{}", path.display());
    }
}

#[test]
fn prebuilt_mode_rejects_missing_fixture_instead_of_compiling() {
    let directory = tempfile::tempdir().unwrap();
    assert!(read_manifest(directory.path(), "absent").is_err());
}
#[test]
fn prebuilt_mode_rejects_incomplete_or_wrong_host_target_evidence() {
    for (host, target, complete) in [
        ("aarch64-apple-darwin", "aarch64-apple-darwin", true),
        ("x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", true),
        ("x86_64-unknown-linux-gnu", "aarch64-apple-darwin", false),
    ] {
        let manifest = json!({"version": 1, "host": host, "target": target,
            "complete": complete, "events": [], "directories": []});
        assert!(validate_manifest(&manifest).is_err());
    }
}

pub trait FixtureGraph {
    fn fixture_compile(
        &self,
        build: &NativeBuild,
        root: &Path,
        facts: &BTreeMap<String, bool>,
        libraries: &BTreeMap<String, obs_build::validation::graph::Dependency>,
    ) -> Result<BTreeMap<String, PathBuf>, String>;
}
impl FixtureGraph for obs_build::validation::graph::Graph {
    fn fixture_compile(
        &self,
        build: &NativeBuild,
        root: &Path,
        facts: &BTreeMap<String, bool>,
        libraries: &BTreeMap<String, obs_build::validation::graph::Dependency>,
    ) -> Result<BTreeMap<String, PathBuf>, String> {
        if !active() {
            return self.compile(build, root, facts, libraries);
        }
        let request = build_request(
            build,
            "graph",
            "graph",
            format!("{self:?} {facts:?} {libraries:?}"),
        );
        if let Some(result) = replay_event(&request) {
            return serde_json::from_value(result).unwrap();
        }
        let result = self.compile(build, root, facts, libraries);
        record(request, serde_json::to_value(&result).unwrap());
        result
    }
}

pub fn compiler_output(command: &mut Command) -> std::io::Result<std::process::Output> {
    if !active() {
        return command.output();
    }
    let request = json!({"kind": "compiler-command", "command": command_recipe(command)});
    if let Some(result) = replay_event(&request) {
        return Ok(std::process::Output {
            status: status(result["status"].as_i64().unwrap().try_into().unwrap()),
            stdout: serde_json::from_value(result["stdout"].clone()).unwrap(),
            stderr: serde_json::from_value(result["stderr"].clone()).unwrap(),
        });
    }
    let result = command.output()?;
    record(
        request,
        json!({"status": result.status.code().expect("compiler killed by signal"),
        "stdout": result.stdout, "stderr": result.stderr}),
    );
    Ok(result)
}
#[cfg(unix)]
fn status(code: i32) -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt as _;
    std::process::ExitStatus::from_raw(code << 8)
}
#[cfg(windows)]
fn status(code: i32) -> std::process::ExitStatus {
    use std::os::windows::process::ExitStatusExt as _;
    std::process::ExitStatus::from_raw(code.cast_unsigned())
}

pub fn assert_runtime_suffix(command: &mut Command, suffix: &str) {
    let request =
        json!({"kind": "run-suffix", "command": command_recipe(command), "suffix": suffix});
    if preparing() {
        record(request, Value::Null);
        return;
    }
    if active() {
        assert!(replay_event(&request).is_some());
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line.ends_with(suffix))
    );
}

pub fn assert_cmake_build_and_run(root: &Path, output: &Path) {
    for (program, arguments) in [
        (
            "cmake",
            vec!["-S", root.to_str().unwrap(), "-B", output.to_str().unwrap()],
        ),
        (
            "cmake",
            vec!["--build", output.to_str().unwrap(), "--config", "Release"],
        ),
        (
            "ctest",
            vec![
                "--test-dir",
                output.to_str().unwrap(),
                "-C",
                "Release",
                "--output-on-failure",
            ],
        ),
    ] {
        let mut command = Command::new(program);
        command.args(arguments);
        if program == "ctest" {
            assert_runtime(&mut command, None, &[]);
        } else {
            let result = compiler_output(&mut command).unwrap();
            assert!(
                result.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
}
