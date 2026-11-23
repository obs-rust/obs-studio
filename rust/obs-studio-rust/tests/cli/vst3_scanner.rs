use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const EFFECT: &str = "57B71384C2EE4713B586F2FD7FDB08A1";
const INSTRUMENT: &str = "6BD4700D163F4219A265882F52B373E8";

fn run(cache: Option<&Path>, roots: &[PathBuf]) -> (ExitCode, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = obs_vst3_scanner::run(cache, roots, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn module_info(root: &Path) -> (PathBuf, PathBuf) {
    let bundle = root.join("nested/Éffects.vst3");
    let metadata = bundle.join("Contents/Resources").join("moduleinfo.json");
    fs::create_dir_all(metadata.parent().unwrap()).unwrap();
    // Windows discovers the .vst3 binary within the bundle.
    if cfg!(target_os = "windows") {
        let binary = bundle.join("Contents/x86_64-win/Éffects.vst3");
        fs::create_dir_all(binary.parent().unwrap()).unwrap();
        fs::write(binary, []).unwrap();
    }
    (bundle, metadata)
}

#[test]
fn scanner_reports_missing_argument_and_empty_roots() {
    let tmp = tempfile::tempdir().unwrap();
    let roots = [tmp.path().join("absent")];
    let cache = tmp.path().join("new/vst3list.json");
    assert_eq!(
        run(None, &roots),
        (
            ExitCode::FAILURE,
            String::new(),
            "Usage: obs-vst3-scanner <vst3list.json>\n".into(),
        )
    );
    let (code, out, _) = run(Some(&cache), &roots);
    assert_eq!(code, ExitCode::FAILURE);
    assert_eq!(out, "NO_VST3\n");
    assert!(!cache.exists());
}

#[test]
fn scanner_reads_json5_metadata_and_emits_canonical_cache_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let roots = [tmp.path().join("plugins")];
    let cache = tmp.path().join("new/vst3list.json");
    let (bundle, metadata) = module_info(&roots[0]);
    fs::write(
        metadata,
        format!(
            r#"{{
                // Steinberg moduleinfo uses JSON5.
                'Name': 'Éffects',
                'Version': '1.0.0',
                'Factory Info': {{'Vendor': 'OBS', 'URL': '', 'E-Mail': '', 'Flags': {{}}}},
                'Classes': [
                    {{
                        'CID': '{EFFECT}', 'Category': 'Audio Module Class', 'Name': 'z EQ',
                        'Vendor': 'OBS', 'Version': '1.0.0', 'SDKVersion': 'VST 3.8.0',
                        'Sub Categories': ['Fx',], 'Class Flags': 0, 'Cardinality': 2147483647,
                    }},
                    {{
                        'CID': '{INSTRUMENT}', 'Category': 'Audio Module Class', 'Name': 'A synth',
                        'Vendor': 'OBS', 'Version': '1.0.0', 'SDKVersion': 'VST 3.8.0',
                        'Sub Categories': ['Instrument',], 'Class Flags': 0, 'Cardinality': 2147483647,
                    }},
                ],
            }}"#
        ),
    )
    .unwrap();

    let (code, out, err) = run(Some(&cache), &roots);
    assert_eq!(code, ExitCode::SUCCESS, "{err}");
    assert_eq!(out, "VST3_SCAN_DONE\n");
    let value: Value = serde_json::from_slice(&fs::read(cache).unwrap()).unwrap();
    assert_eq!(
        value,
        json!({
            "version": 1,
            "plugins": [
                {"name": "A synth", "id": INSTRUMENT, "path": bundle, "pluginName": "Éffects",
                 "discardable": false, "isFX": false},
                {"name": "z EQ", "id": EFFECT, "path": bundle, "pluginName": "Éffects",
                 "discardable": false, "isFX": true},
            ]
        })
    );
}

#[test]
fn scanner_distinguishes_an_invalid_candidate_from_no_candidates() {
    let tmp = tempfile::tempdir().unwrap();
    let roots = [tmp.path().join("plugins")];
    let cache = tmp.path().join("new/vst3list.json");
    let (_, metadata) = module_info(&roots[0]);
    fs::write(metadata, "{invalid").unwrap();
    let (code, out, err) = run(Some(&cache), &roots);
    assert_eq!(code, ExitCode::SUCCESS, "{err}");
    assert_eq!(out, "VST3_SCAN_DONE\n");
    let value: Value = serde_json::from_slice(&fs::read(cache).unwrap()).unwrap();
    assert_eq!(value, json!({"version": 1, "plugins": []}));
}

#[test]
fn scanner_recovers_a_valid_cache_backup_and_discards_removed_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let roots = [tmp.path().join("plugins")];
    let cache = tmp.path().join("vst3list.json");
    let backup = tmp.path().join("vst3list.json.bak");
    let (bundle, _) = module_info(&roots[0]);
    let binary = if cfg!(target_os = "windows") {
        bundle.join("Contents/x86_64-win/Éffects.vst3")
    } else {
        bundle
    };
    let existing = json!({
        "name": "Cached effect", "id": EFFECT, "path": binary, "pluginName": "Éffects",
        "discardable": false, "isFX": true,
    });
    let removed = json!({
        "name": "Removed effect", "id": INSTRUMENT, "path": roots[0].join("removed.vst3"),
        "pluginName": "removed", "discardable": false, "isFX": true,
    });
    fs::write(&cache, b"{truncated").unwrap();
    fs::write(
        &backup,
        serde_json::to_vec(&json!({"version": 1, "plugins": [existing.clone(), removed]})).unwrap(),
    )
    .unwrap();

    let (code, out, err) = run(Some(&cache), &roots);
    assert_eq!(code, ExitCode::SUCCESS, "{err}");
    assert_eq!(out, "VST3_SCAN_DONE\n");
    let updated: Value = serde_json::from_slice(&fs::read(cache).unwrap()).unwrap();
    assert_eq!(updated, json!({"version": 1, "plugins": [existing]}));
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(backup).unwrap()).unwrap()["plugins"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}
