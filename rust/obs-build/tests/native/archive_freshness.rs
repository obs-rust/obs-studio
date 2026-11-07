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
fn hybrid_archive_requires_current_tree_depfile_and_observes_added_sources() {
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
    let error = error();
    assert!(error.contains("stale Rust archive"), "{error}");
    assert!(error.contains("new_module.rs"), "{error}");
    assert!(!build.output.exists());
}

fn set_modified(path: &Path, time: std::time::SystemTime) {
    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}
