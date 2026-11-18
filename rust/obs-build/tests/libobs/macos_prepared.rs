//! Build receipts consumed by the exact same contract on the run host.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub(super) struct Artifacts {
    file: PathBuf,
    read_only: bool,
    paths: BTreeMap<String, Vec<PathBuf>>,
    waiting: BTreeMap<String, String>,
}

impl Artifacts {
    pub(super) fn new(output: &Path, read_only: bool) -> Self {
        let file = output.join("macos-prepared.json");
        let paths = if read_only {
            let receipt: serde_json::Value = serde_json::from_slice(
                &std::fs::read(&file).expect("missing Linux preparation receipt"),
            )
            .expect("invalid Linux preparation receipt");
            assert_eq!(
                receipt["complete"], true,
                "Linux preparation is incomplete; waiting on Swift/SDK or another producer failure"
            );
            serde_json::from_value(receipt["artifacts"].clone()).expect("invalid prepared paths")
        } else {
            BTreeMap::new()
        };
        let artifacts = Self {
            file,
            read_only,
            paths,
            waiting: BTreeMap::new(),
        };
        if !read_only {
            std::fs::create_dir_all(output).unwrap();
            artifacts.save(false);
        }
        artifacts
    }

    pub(super) fn get(&mut self, key: &str, build: impl FnOnce() -> Vec<PathBuf>) -> Vec<PathBuf> {
        let paths = if self.read_only {
            self.paths
                .get(key)
                .unwrap_or_else(|| panic!("missing prepared artifact: {key}"))
                .clone()
        } else {
            let paths = build();
            self.paths.insert(key.into(), paths.clone());
            self.save(false);
            paths
        };
        for path in &paths {
            assert!(
                path.exists(),
                "missing prepared artifact: {}",
                path.display()
            );
        }
        paths
    }

    pub(super) fn finish(&self) {
        if !self.read_only {
            let complete = [
                "cores",
                "capture-probe",
                "mac-capture",
                "mac-avcapture",
                "mac-avcapture-legacy",
                "opengl",
                "metal",
                "graphics-probe",
            ]
            .iter()
            .all(|key| self.paths.contains_key(*key));
            self.save(complete);
        }
    }

    pub(super) fn attempt(
        &mut self,
        key: &str,
        build: impl FnOnce() -> Result<Vec<PathBuf>, String>,
    ) -> Option<Vec<PathBuf>> {
        assert!(!self.read_only, "assertions may never prepare an artifact");
        match build() {
            Ok(paths) => Some(self.get(key, || paths)),
            Err(error) => {
                self.waiting.insert(key.into(), error);
                self.save(false);
                None
            }
        }
    }

    fn save(&self, complete: bool) {
        let receipt = serde_json::json!({"schema": 1, "complete": complete, "artifacts": self.paths, "waiting": self.waiting});
        std::fs::write(&self.file, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
    }
}

pub(super) fn prepare(
    build: &obs_build::NativeBuild,
    root: &Path,
    options: &obs_build::libobs::Options,
) {
    use obs_build::{
        graphics, libobs,
        plugins::{MacosCaptureModule, build_macos_capture},
    };
    let archive = PathBuf::from(
        std::env::var_os("OBS_BUILD_MACOS_RUST_ARCHIVE")
            .expect("Linux preparation requires the real Rust archive"),
    );
    let mut artifacts = Artifacts::new(&build.output, false);
    if let Some(cores) = artifacts.attempt("cores", || {
        libobs::build_pair(build, root, options, &archive)
            .map(|pair| vec![pair.reference, pair.with_rust])
    }) {
        let core = &cores[1];
        for module in [
            MacosCaptureModule::ScreenAudio,
            MacosCaptureModule::Camera,
            MacosCaptureModule::LegacyCamera,
        ] {
            artifacts.attempt(module.name(), || {
                build_macos_capture(build, root, core, options, module)
                    .map(|module| vec![module.library, module.data])
            });
        }
        // Probe builders have no runtime execution. All loader/registration
        // assertions remain in the shared macOS assertion phase.
        artifacts.get("capture-probe", || {
            vec![super::macos_capture::build_probe(
                build, root, options, core,
            )]
        });
        artifacts.attempt("opengl", || {
            graphics::build_opengl(build, root, core, options).map(|path| vec![path])
        });
        artifacts.attempt("metal", || {
            graphics::build_metal(build, root, core, options).map(|path| vec![path])
        });
        artifacts.get("graphics-probe", || {
            vec![super::macos_graphics::build_probe(
                build, root, options, core,
            )]
        });
    }
    artifacts.finish();
    assert!(
        artifacts.waiting.is_empty(),
        "Linux preparation incomplete; see macos-prepared.json waiting entries: {:?}",
        artifacts.waiting.keys().collect::<Vec<_>>()
    );
}

#[test]
fn assertions_never_call_builders() {
    let output = std::env::temp_dir().join(format!("obs-macos-prepared-{}", std::process::id()));
    std::fs::create_dir_all(&output).unwrap();
    let artifact = output.join("image");
    std::fs::write(&artifact, b"test receipt").unwrap();
    let mut prepare = Artifacts::new(&output, false);
    assert_eq!(
        prepare.get("core", || vec![artifact.clone()]),
        vec![artifact.clone()]
    );
    prepare.save(true);
    let mut assertions = Artifacts::new(&output, true);
    assert_eq!(
        assertions.get("core", || panic!("run host compiled")),
        vec![artifact]
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assertions.get("metal", || panic!("run host compiled"));
        }))
        .is_err()
    );
    std::fs::remove_dir_all(output).unwrap();
}

#[test]
fn incomplete_preparation_cannot_be_replayed() {
    let output = std::env::temp_dir().join(format!("obs-macos-incomplete-{}", std::process::id()));
    let prepare = Artifacts::new(&output, false);
    prepare.finish();
    assert!(std::panic::catch_unwind(|| Artifacts::new(&output, true)).is_err());
    std::fs::remove_dir_all(output).unwrap();
}
