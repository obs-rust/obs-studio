//! Compiled path qualification in the existing Application category.
use std::path::Path;
use std::process::Command;

fn logical_path(path: &str) -> Result<String, &'static str> {
    let normalized = path.replace('\\', "/");
    let rooted = normalized.strip_prefix("C:").unwrap_or(&normalized);
    if !rooted.starts_with('/')
        || rooted.starts_with("//")
        || rooted.contains(':')
        || rooted.split('/').any(|part| part == "..")
    {
        return Err("unsupported root or drive");
    }
    Ok(rooted.to_owned())
}

#[test]
fn wrong_compiled_roots_and_drives_are_rejected() {
    assert_eq!(
        logical_path(r"C:\opt\obs-win-42-3\src").unwrap(),
        "/opt/obs-win-42-3/src"
    );
    assert_eq!(
        logical_path("/opt/obs-win-42-3/src").unwrap(),
        "/opt/obs-win-42-3/src"
    );
    for bad in [
        r"D:\opt\obs-win-42-3\src",
        r"\\server\share",
        "/opt/obs-win-42-3/../src",
    ] {
        assert!(logical_path(bad).is_err(), "{bad}");
    }
}

#[test]
fn compiled_paths_match_receipt() {
    let Ok(root) = std::env::var("OBS_WINDOWS_PROOF_ROOT") else {
        return;
    };
    let compiled = logical_path(env!("CARGO_MANIFEST_DIR")).unwrap();
    assert_eq!(
        compiled,
        format!("{root}/rust/obs-studio-rust"),
        "compiled root mismatch"
    );
    let package = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        std::fs::read(package.join("tests/cli/main.rs")).unwrap(),
        include_bytes!("main.rs")
    );
    let helper = env!("CARGO_BIN_EXE_obs-studio-rust");
    assert!(
        logical_path(helper)
            .unwrap()
            .starts_with(&format!("{root}/target/"))
    );
    let output = Command::new(helper)
        .arg("--help")
        .current_dir(package)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    if std::env::var_os("OBS_WINDOWS_ROOT_CHILD").is_some() {
        assert!(
            std::env::current_dir()
                .unwrap()
                .ends_with("runtime root/Release")
        );
        return;
    }
    let work = tempfile::tempdir().unwrap();
    let runtime = work.path().join("runtime root/Release");
    std::fs::create_dir_all(&runtime).unwrap();
    let cmake = work.path().join("CMakeLists.txt");
    // CMake configures no language and CTest re-executes this category. No build.
    std::fs::write(&cmake, format!(
        "cmake_minimum_required(VERSION 3.28)\nproject(roots NONE)\nenable_testing()\n\
         add_test(NAME roots COMMAND [==[{}]==] --exact windows_paths::compiled_paths_match_receipt)\n\
         set_tests_properties(roots PROPERTIES WORKING_DIRECTORY [==[{}]==] ENVIRONMENT OBS_WINDOWS_ROOT_CHILD=1)\n",
        std::env::current_exe().unwrap().to_string_lossy().replace('\\', "/"),
        runtime.to_string_lossy().replace('\\', "/"))).unwrap();
    let output = work.path().join("build root");
    for (program, args) in [
        (
            "cmake",
            vec![
                "-S",
                work.path().to_str().unwrap(),
                "-B",
                output.to_str().unwrap(),
            ],
        ),
        (
            "ctest",
            vec![
                "--test-dir",
                output.to_str().unwrap(),
                "--output-on-failure",
            ],
        ),
    ] {
        let result = Command::new(program).args(args).output().unwrap();
        assert!(result.status.success(), "{result:?}");
    }
}
