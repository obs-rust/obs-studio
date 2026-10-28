//! Runs unchanged native tests with isolated paths and verified core selection.
use obs_build::{NativeBuild, validation::Executable};
use std::path::Path;

#[expect(
    clippy::too_many_lines,
    reason = "one block per module contract checked"
)]
pub fn exercise(
    build: &NativeBuild,
    root: &Path,
    core: &Path,
    rust_core: &Path,
    tests: &[Executable],
    suite: &str,
) {
    assert_ne!(
        core, rust_core,
        "compatibility runs require distinct C and Rust libraries"
    );
    let mut results = Vec::new();
    let mut failures = Vec::new();
    for selected in [core, rust_core] {
        let selected = dunce::canonicalize(selected).unwrap();
        let variant = selected
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let search = std::env::join_paths(
            std::iter::once(selected.parent().unwrap().to_owned()).chain(std::env::split_paths(
                &std::env::var_os("LD_LIBRARY_PATH").unwrap_or_default(),
            )),
        )
        .unwrap();
        for test in tests {
            let directory = build
                .output
                .join(format!("{suite}-c-test-runs"))
                .join(variant)
                .join(&test.name);
            std::fs::create_dir_all(directory.join("tmp")).unwrap();
            let directory = dunce::canonicalize(directory).unwrap();
            let executable = dunce::canonicalize(&test.path).unwrap();
            for name in ["config", "data", "cache", "home"] {
                std::fs::create_dir_all(directory.join(name)).unwrap();
            }
            super::application::copy_tree(
                &root.join("libobs/data"),
                &directory.join("share/obs/libobs"),
            )
            .unwrap();
            let resolution = std::process::Command::new("ldd")
                .arg(&executable)
                .env("LD_LIBRARY_PATH", &search)
                .output()
                .unwrap();
            assert!(resolution.status.success());
            let core_required = test
                .dependencies
                .iter()
                .any(|path| path == core || path == rust_core);
            let resolved = String::from_utf8_lossy(&resolution.stdout);
            let core_linked = resolved.contains("libobs.so");
            if core_required || core_linked {
                assert!(
                    resolved.contains(selected.to_str().unwrap()),
                    "{} did not resolve the selected core",
                    test.name
                );
            }
            std::fs::write(directory.join("dependencies.log"), &resolution.stdout).unwrap();
            let inherited_timeout = match test.name.as_str() {
                "test_vlc_audio_track"
                | "test_vlc_audio_pacing"
                | "test_vlc_playlist_gap"
                | "test_group_hide_transition_audio" => 120,
                _ => 60,
            };
            let seconds = test
                .runtime
                .as_ref()
                .and_then(|test| test.timeout_seconds)
                .unwrap_or(inherited_timeout);
            let prepared = test.command_in(root, &build.output, &directory).unwrap();
            let mut command = if seconds == 0 {
                prepared
            } else {
                let mut bounded = std::process::Command::new("timeout");
                bounded
                    .args(["--kill-after=5s", &format!("{seconds}s")])
                    .arg(prepared.get_program())
                    .args(prepared.get_args());
                for (name, value) in prepared.get_envs() {
                    if let Some(value) = value {
                        bounded.env(name, value);
                    } else {
                        bounded.env_remove(name);
                    }
                }
                if let Some(directory) = prepared.get_current_dir() {
                    bounded.current_dir(directory);
                }
                bounded
            };
            if test
                .runtime
                .as_ref()
                .is_none_or(|test| test.working_directory.is_none())
            {
                command.current_dir(&directory);
            }
            let started = std::time::Instant::now();
            let status = command
                .env("LD_LIBRARY_PATH", &search)
                .env("HOME", directory.join("home"))
                .env("TMPDIR", directory.join("tmp"))
                .env("XDG_CONFIG_HOME", directory.join("config"))
                .env("XDG_DATA_HOME", directory.join("data"))
                .env("XDG_CACHE_HOME", directory.join("cache"))
                .env("CMOCKA_MESSAGE_OUTPUT", "STDOUT")
                .env_remove("CMOCKA_TEST_FILTER")
                .env_remove("CMOCKA_SKIP_FILTER")
                .stdout(std::fs::File::create(directory.join("stdout.log")).unwrap())
                .stderr(std::fs::File::create(directory.join("stderr.log")).unwrap())
                .status()
                .unwrap();
            let stdout = std::fs::read(directory.join("stdout.log")).unwrap();
            let stderr = std::fs::read(directory.join("stderr.log")).unwrap();
            let skip_code = test
                .runtime
                .as_ref()
                .and_then(|test| test.skip_return_code)
                .unwrap_or(77);
            let skipped = status.code() == Some(skip_code)
                || [stdout.as_slice(), stderr.as_slice()].iter().any(|output| {
                    output
                        .windows(b"SKIPPED".len())
                        .any(|window| window == b"SKIPPED")
                });
            let passed = status.success() && !skipped;
            println!(
                "cc::Build {suite} C test: {variant}/{} exit {status}, skipped={skipped}",
                test.name
            );
            results.push(serde_json::json!({"variant":variant,"name":test.name,
                "status":status.code(),"skipped":skipped,"passed":passed,
                "core_required":core_required,"core_linked":core_linked,
                "seconds":started.elapsed().as_secs_f64(),"directory":directory}));
            std::fs::write(
                build.output.join(format!("{suite}-c-tests.json")),
                serde_json::to_vec_pretty(&results).unwrap(),
            )
            .unwrap();
            if !passed {
                failures.push(format!("{variant}/{} ({})", test.name, directory.display()));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{suite} compatibility failures or skips: {}",
        failures.join(", ")
    );
}
