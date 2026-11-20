#[path = "../../src/libobs/archive_path.rs"]
mod archive_path;

use super::*;

/// #1390: a persistent build volume must not silently pair old Rust with new C.
#[test]
fn hybrid_rejects_stale_rust_archive_before_native_compilation() {
    for input in [
        "rust/obs-core/src/audio_render.rs",
        "rust/libobs-rust/Cargo.toml",
        "rust/obs-util/build.rs",
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        ".cargo/config.toml",
        "libobs/obs-hotkeys.h",
    ] {
        let directory = tempfile::Builder::new()
            .prefix("archive tree ")
            .tempdir()
            .unwrap();
        let root = directory.path();
        fs::create_dir_all(root.join("rust")).unwrap();
        let archive = root.join("liblibobs_rust.a");
        fs::write(&archive, "old archive").unwrap();
        let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(100);
        set_modified(&archive, old);
        let anchor = root.join("rust/libobs-rust/src/lib.rs");
        fs::create_dir_all(anchor.parent().unwrap()).unwrap();
        fs::write(&anchor, "archive root").unwrap();
        set_modified(&anchor, old);
        let escape = |path: &Path| {
            path.to_string_lossy()
                .replace('\\', "/")
                .replace(' ', "\\ ")
        };
        fs::write(
            archive.with_extension("d"),
            format!(
                "{}: {} {}\n",
                escape(&archive),
                escape(&anchor),
                escape(&root.join(input))
            ),
        )
        .unwrap();
        let source = root.join(input);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, "new input").unwrap();
        let build = NativeBuild {
            profile: Default::default(),
            host: obs_build::BUILD_HOST.into(),
            target: obs_build::BUILD_TARGET.into(),
            output: root.join("build"),
        };
        for pair in [false, true] {
            let options = obs_build::libobs::Options::default();
            let error = if pair {
                obs_build::libobs::build_pair(&build, root, &options, &archive)
                    .map(|_| ())
                    .unwrap_err()
            } else {
                obs_build::libobs::build_hybrid_with_options(&build, root, &options, &archive)
                    .unwrap_err()
            };
            assert!(error.contains("stale Rust archive"), "{input}: {error}");
            assert!(error.contains(input), "{input}: {error}");
            assert!(error.contains("soldr cargo build"), "{error}");
            assert!(!build.output.exists(), "must reject before a native build");
        }
        // A refreshed archive gets past the guard to this intentionally absent
        // native fixture; unrelated test edits do not invalidate it.
        let fresh = std::time::SystemTime::now() + std::time::Duration::from_secs(10);
        set_modified(&archive, fresh);
        let tests = root.join("rust/obs-core/tests/new_case.rs");
        fs::create_dir_all(tests.parent().unwrap()).unwrap();
        fs::write(&tests, "not part of the archive").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&tests)
            .unwrap()
            .set_modified(fresh + std::time::Duration::from_secs(10))
            .unwrap();
        let error = obs_build::libobs::build_hybrid_with_options(
            &build,
            root,
            &obs_build::libobs::Options::default(),
            &archive,
        )
        .unwrap_err();
        assert!(error.contains("source-sets.d"), "{input}: {error}");
    }
}

#[test]
fn hybrid_archive_requires_current_tree_depfile_and_observes_listed_sources() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("tree");
    let source = root.join("rust/libobs-rust/src/lib.rs");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, "archive input").unwrap();
    let archive = root.join("liblibobs_rust.a");
    fs::write(&archive, "archive").unwrap();
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: root.join("build"),
    };
    let error = || {
        obs_build::libobs::build_hybrid_with_options(
            &build,
            &root,
            &obs_build::libobs::Options::default(),
            &archive,
        )
        .unwrap_err()
    };
    assert!(error().contains("cannot verify Rust archive"));
    let foreign = directory.path().join("other.rs");
    fs::write(&foreign, "another checkout").unwrap();
    let escape = |path: &Path| {
        path.to_string_lossy()
            .replace('\\', "/")
            .replace(' ', "\\ ")
    };
    fs::write(
        archive.with_extension("d"),
        format!("{}: {}\n", escape(&archive), escape(&foreign)),
    )
    .unwrap();
    assert!(error().contains("does not describe this tree"));
    fs::write(
        archive.with_extension("d"),
        format!("{}: {}\n", escape(&archive), escape(&source)),
    )
    .unwrap();
    let new = source.with_file_name("new_module.rs");
    fs::write(&new, "new input not yet in the depfile").unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&new)
        .unwrap()
        .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(10))
        .unwrap();
    let result = error();
    assert!(result.contains("source-sets.d"), "{result}");
    // Cargo will track a new module through the source that declares it. Once
    // it is listed, the archive guard must check it too.
    fs::write(
        archive.with_extension("d"),
        format!(
            "{}: {} {}\n",
            escape(&archive),
            escape(&source),
            escape(&new)
        ),
    )
    .unwrap();
    let error = error();
    assert!(error.contains("stale Rust archive"), "{error}");
    assert!(error.contains("new_module.rs"), "{error}");
    assert!(!build.output.exists());
}

/// #5323: a vendored header does not make its crate's build script an input.
#[test]
fn hybrid_archive_checks_depfile_inputs_without_expanding_vendor_crates() {
    let directory = tempfile::Builder::new()
        .prefix("archive tree ")
        .tempdir()
        .unwrap();
    let root = directory.path();
    let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(100);
    let newer = old + std::time::Duration::from_secs(10);
    let archive = root.join("liblibobs_rust.a");
    fs::write(&archive, "archive").unwrap();
    set_modified(&archive, old);
    let inputs = [
        "rust/libobs-rust/src/lib.rs",
        "rust/obs-c-oracle/vendor/jansson/jansson.h",
        "generated/build-script/out/generated.rs",
    ];
    for input in inputs {
        let path = root.join(input);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "tracked input").unwrap();
        set_modified(&path, old);
    }
    let generated = root.join("generated/build-script/out/tests/generated.rs");
    fs::create_dir_all(generated.parent().unwrap()).unwrap();
    fs::write(&generated, "directory input").unwrap();
    set_modified(&generated, old);
    let escape = |path: &Path| {
        path.to_string_lossy()
            .replace('\\', "/")
            .replace(' ', "\\ ")
    };
    fs::write(
        archive.with_extension("d"),
        format!(
            "{}: {} {}\n",
            escape(&archive),
            inputs
                .iter()
                .map(|input| escape(&root.join(input)))
                .collect::<Vec<_>>()
                .join(" "),
            escape(generated.parent().unwrap().parent().unwrap())
        ),
    )
    .unwrap();
    let unrelated = root.join("rust/obs-c-oracle/build.rs");
    fs::write(&unrelated, "not used by this archive").unwrap();
    set_modified(&unrelated, newer);
    let build = NativeBuild {
        profile: Default::default(),
        host: obs_build::BUILD_HOST.into(),
        target: obs_build::BUILD_TARGET.into(),
        output: root.join("build"),
    };
    let error = || {
        obs_build::libobs::build_hybrid_with_options(
            &build,
            root,
            &obs_build::libobs::Options::default(),
            &archive,
        )
        .unwrap_err()
    };
    // Passing the freshness guard reaches the deliberately absent native graph.
    let result = error();
    assert!(result.contains("source-sets.d"), "{result}");
    for input in inputs {
        let path = root.join(input);
        set_modified(&path, newer);
        let result = error();
        assert!(result.contains("stale Rust archive"), "{input}: {result}");
        assert!(result.contains(input), "{input}: {result}");
        set_modified(&path, old);
    }
    // Explicit directory dependencies include every child, even a directory
    // named tests inside a build-script output tree.
    set_modified(&generated, newer);
    let result = error();
    assert!(result.contains("stale Rust archive"), "{result}");
    assert!(result.contains("generated.rs"), "{result}");
    set_modified(&generated, old);
    // Manifests and configuration remain inputs even when absent from the .d.
    for input in [
        "Cargo.toml",
        "rust/libobs-rust/Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        ".cargo/config.toml",
        "libobs/obs-hotkeys.h",
    ] {
        let path = root.join(input);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "explicit input").unwrap();
        set_modified(&path, newer);
        let result = error();
        assert!(result.contains("stale Rust archive"), "{input}: {result}");
        assert!(result.contains(input), "{input}: {result}");
        set_modified(&path, old);
    }
}

fn set_modified(path: &Path, time: std::time::SystemTime) {
    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}
