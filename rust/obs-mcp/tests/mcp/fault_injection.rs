//! The audio mixer fault injection of `obs-mcp` (`fault-injection` feature,
//! `host::fail_audio_mixer`, #3491) must not be reachable in a shipped build.
//!
//! Cargo builds dev-dependencies only for tests, benches and examples, so a
//! feature enabled from `[dev-dependencies]` never reaches `cargo build` or
//! `cargo install`. This checks every manifest in the workspace: the feature
//! is enabled from a dev-dependency table and nowhere else, `obs-mcp` does not
//! turn it on by default, and no build script asks for it on the command line.

use std::path::{Path, PathBuf};

const FEATURE: &str = "fault-injection";

fn rust_dir() -> PathBuf {
    // obs-tests-core is the obs-data package: rust/obs-data.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// The `[section]` each line of a TOML file is in, with the line.
fn sections(text: &str) -> Vec<(String, &str)> {
    let mut section = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            section = trimmed.trim_matches(['[', ']']).to_owned();
        } else if !trimmed.starts_with('#') {
            out.push((section.clone(), line));
        }
    }
    out
}

#[test]
fn audio_fault_injection_is_enabled_only_from_dev_dependencies() {
    let mut enablers = Vec::new();
    for entry in std::fs::read_dir(rust_dir()).unwrap() {
        let manifest = entry.unwrap().path().join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        let own = text.contains("name = \"obs-mcp\"");
        for (section, line) in sections(&text) {
            if !line.contains(FEATURE) {
                continue;
            }
            if own && section == "features" && line.trim_start().starts_with(FEATURE) {
                // The definition itself; it must not pull in anything.
                assert_eq!(line.trim(), "fault-injection = []", "{manifest:?}");
                continue;
            }
            assert!(
                section.ends_with("dev-dependencies"),
                "{manifest:?} enables {FEATURE} outside a dev-dependency table, \
                 in [{section}]: {line}"
            );
            enablers.push(manifest.clone());
        }
        if own {
            for (section, line) in sections(&text) {
                assert!(
                    !(section == "features" && line.trim_start().starts_with("default")),
                    "obs-mcp has default features: {line}"
                );
            }
        }
    }
    assert_eq!(
        enablers,
        [rust_dir().join("obs-studio-rust").join("Cargo.toml")],
        "the app's tests are the one place the seam is enabled"
    );
}

#[test]
fn no_build_script_asks_for_audio_fault_injection() {
    let root = rust_dir().parent().unwrap().to_path_buf();
    let mut stack = vec![
        root.join("ci"),
        root.join("rust").join("tools"),
        root.join(".github"),
    ];
    let mut offenders = Vec::new();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if std::fs::read_to_string(&path).is_ok_and(|t| t.contains(FEATURE)) {
                offenders.push(path);
            }
        }
    }
    assert!(offenders.is_empty(), "{offenders:?} name {FEATURE}");
}
