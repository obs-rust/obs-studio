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

/// #5323: a newer lockfile that adds a crate libobs-rust does not use leaves
/// the archive fresh; a change to a package libobs-rust uses, or a lockfile
/// with no stamp from a fresh check, is stale.
#[test]
fn hybrid_archive_checks_only_its_own_packages_in_a_newer_lockfile() {
    let directory = tempfile::Builder::new()
        .prefix("archive tree ")
        .tempdir()
        .unwrap();
    let root = directory.path().join("tree");
    let fixture = LockFixture::new(&root);
    // An archive newer than the lockfile: fresh, and the stamp is saved.
    fixture.write_lock(&lockfile(DEPS, "1.0.1", ""), false);
    assert_fresh(fixture.check());
    // The same lockfile, only touched.
    fixture.write_lock(&lockfile(DEPS, "1.0.1", ""), true);
    assert_fresh(fixture.check());
    // A new workspace crate and its dependency: not this archive's packages.
    fixture.write_lock(&lockfile(DEPS, "1.0.1", UNRELATED), true);
    assert_fresh(fixture.check());
    // A second serde for another crate: libobs-rust still uses 1.0.1.
    let pinned = DEPS.replace("\"serde\"", "\"serde 1.0.1\"");
    fixture.write_lock(&lockfile(&pinned, "1.0.1", SECOND_SERDE), true);
    assert_fresh(fixture.check());
    // A dependency of libobs-rust bumped with the build skipped.
    fixture.write_lock(&lockfile(DEPS, "1.0.2", ""), true);
    assert_stale(fixture.check(), "it changes the packages of libobs-rust");
    // A local crate of libobs-rust now depends on another package.
    let lock = lockfile(DEPS, "1.0.1", UNRELATED);
    fixture.write_lock(
        &lock.replace(
            " \"pre-release-name\",\n",
            " \"pre-release-name\",\n \"obs-unrelated-fixture\",\n",
        ),
        true,
    );
    assert_stale(fixture.check(), "it changes the packages of libobs-rust");
    // A lockfile that cannot be read.
    fixture.write_lock("not a lockfile", true);
    assert_stale(fixture.check(), "cannot be read");
    // An archive with no stamp, or a stamp of another archive build.
    fixture.write_lock(&lockfile(DEPS, "1.0.1", ""), false);
    assert_fresh(fixture.check());
    fixture.write_lock(&lockfile(DEPS, "1.0.1", ""), true);
    set_modified(
        &fixture.archive,
        fixture.old + std::time::Duration::from_secs(1),
    );
    assert_stale(fixture.check(), "another build of the archive");
    fs::remove_file(fixture.archive.with_extension("lock-closure")).unwrap();
    assert_stale(fixture.check(), "no lockfile stamp");
    assert!(!fixture.build.output.exists());
}

/// #5323: the check reads the repository's real `Cargo.lock`, so a format
/// Cargo writes cannot turn every touched lockfile stale again.
#[test]
fn hybrid_archive_reads_the_repository_lockfile() {
    let text = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock"))
        .unwrap()
        .replace("\r\n", "\n");
    let directory = tempfile::tempdir().unwrap();
    let fixture = LockFixture::new(&directory.path().join("tree"));
    fixture.write_lock(&text, false);
    assert_fresh(fixture.check());
    fixture.write_lock(&format!("{text}{UNRELATED}"), true);
    assert_fresh(fixture.check());
    // libobs-rust's own entry is part of its closure.
    let entry = text
        .split("[[package]]")
        .find(|block| block.starts_with("\nname = \"libobs-rust\"\n"))
        .unwrap();
    fixture.write_lock(
        &text.replace(
            entry,
            &entry.replace(
                "dependencies = [\n",
                "dependencies = [\n \"obs-unrelated-fixture\",\n",
            ),
        ),
        true,
    );
    assert_stale(fixture.check(), "cannot be read");
    fixture.write_lock(
        &format!("{text}{UNRELATED}").replace(
            entry,
            &entry.replace(
                "dependencies = [\n",
                "dependencies = [\n \"obs-unrelated-fixture\",\n",
            ),
        ),
        true,
    );
    assert_stale(fixture.check(), "it changes the packages of libobs-rust");
}

/// A libobs-rust archive with its depfile and two local crates (one whose
/// package name is not its directory), all as old as the archive.
struct LockFixture {
    root: std::path::PathBuf,
    archive: std::path::PathBuf,
    build: NativeBuild,
    old: std::time::SystemTime,
}

impl LockFixture {
    fn new(root: &Path) -> Self {
        let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(100);
        let archive = root.join("liblibobs_rust.a");
        let files = [
            root.join("rust/libobs-rust/src/lib.rs"),
            root.join("rust/libobs-rust/Cargo.toml"),
            root.join("rust/obs-core/src/lib.rs"),
            root.join("rust/obs-core/Cargo.toml"),
        ];
        for file in &files {
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, "tracked input").unwrap();
        }
        fs::write(
            &files[1],
            "[package]\nname = \"libobs-rust\" # the archive's crate\n",
        )
        .unwrap();
        fs::write(&files[3], "[package]\nname = \"obs-core-pkg\"\n").unwrap();
        fs::write(&archive, "archive").unwrap();
        for file in files.iter().chain([&archive]) {
            set_modified(file, old);
        }
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
                escape(&files[0]),
                escape(&files[2])
            ),
        )
        .unwrap();
        let build = NativeBuild {
            profile: Default::default(),
            host: obs_build::BUILD_HOST.into(),
            target: obs_build::BUILD_TARGET.into(),
            output: root.join("build"),
        };
        Self {
            root: root.to_owned(),
            archive,
            build,
            old,
        }
    }

    /// Writes `text` as the lockfile, older or newer than the archive.
    fn write_lock(&self, text: &str, newer: bool) {
        let lock = self.root.join("Cargo.lock");
        fs::write(&lock, text).unwrap();
        let offset = std::time::Duration::from_secs(10);
        set_modified(
            &lock,
            if newer {
                self.old + offset
            } else {
                self.old - offset
            },
        );
    }

    fn check(&self) -> String {
        obs_build::libobs::build_hybrid_with_options(
            &self.build,
            &self.root,
            &obs_build::libobs::Options::default(),
            &self.archive,
        )
        .unwrap_err()
    }
}

const DEPS: &str = " \"obs-core-pkg\",\n \"serde\",\n";

/// A crate libobs-rust does not use, and its registry dependency. Their
/// names are in no real lockfile (obs-dock-layout was, once #5362 added it).
const UNRELATED: &str = "\n[[package]]\nname = \"obs-unrelated-fixture\"\nversion = \"0.1.0\"\n\
    dependencies = [\n \"unrelated-fixture-dep\",\n]\n\
    \n[[package]]\nname = \"unrelated-fixture-dep\"\nversion = \"1.0.9\"\n\
    source = \"registry+https://github.com/rust-lang/crates.io-index\"\n";

/// Another crate that locks a second `serde`. Cargo then writes the
/// version into every `serde` entry.
const SECOND_SERDE: &str = "\n[[package]]\nname = \"obs-frontend\"\nversion = \"0.1.0\"\n\
    dependencies = [\n \"serde 1.0.9\",\n]\n\
    \n[[package]]\nname = \"serde\"\nversion = \"1.0.9\"\n\
    source = \"registry+https://github.com/rust-lang/crates.io-index\"\n";

/// A small lockfile: libobs-rust with `libobs_deps`, a local crate, a
/// pre-release and `serde`.
fn lockfile(libobs_deps: &str, serde: &str, extra: &str) -> String {
    format!(
        "# This file is automatically @generated by Cargo.\n\
         # It is not intended for manual editing.\n\
         version = 4\n\
         \n\
         [[package]]\n\
         name = \"libobs-rust\"\n\
         version = \"0.1.0\"\n\
         dependencies = [\n{libobs_deps}]\n\
         \n\
         [[package]]\n\
         name = \"obs-core-pkg\"\n\
         version = \"0.1.0\"\n\
         dependencies = [\n \"pre-release-name\",\n]\n\
         \n\
         [[package]]\n\
         name = \"pre-release-name\"\n\
         version = \"2.0.0-beta.1\"\n\
         source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
         \n\
         [[package]]\n\
         name = \"serde\"\n\
         version = \"{serde}\"\n\
         source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
         checksum = \"00\"\n\
         {extra}"
    )
}

fn assert_fresh(result: String) {
    assert!(result.contains("source-sets.d"), "{result}");
}

fn assert_stale(result: String, detail: &str) {
    assert!(result.contains("stale Rust archive"), "{result}");
    assert!(result.contains("Cargo.lock"), "{result}");
    assert!(result.contains(detail), "{result}");
    assert!(result.contains("soldr cargo build"), "{result}");
}

fn set_modified(path: &Path, time: std::time::SystemTime) {
    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}
