//! `ObsBackend` against a real in-process libobs (#3301, #3304, #3306, #3308).
//!
//! Included by `obs-tests-app` under the `bundled-native` feature, the only
//! category that links libobs; `obs-tests-core` cannot (it has no libobs
//! symbols). libobs is process-global, so the case re-execs this executable
//! with an exact selector and runs the body in the child, like the other
//! process-state tests (testing policy, "Test binary budget").
//!
//! The child starts libobs without a video subsystem: scenes, sources, scene
//! items, statistics and the "not configured" paths are exercised for real;
//! screenshots and live outputs need a graphics module and are reported as
//! errors, which the case checks too.

use std::process::Command;
use std::rc::Rc;

use obs_mcp::backend::ObsBackend;
use obs_mcp::resources::stats::StatsBackend;
use obs_mcp::tools::outputs::{OutputBackend, OutputKind};
use obs_mcp::tools::scenes::SceneBackend;
use obs_mcp::tools::screenshot::CaptureBackend;
use obs_mcp::tools::sources::{ItemRef, SourceBackend, Transform};
use serde_json::{Value, json};

const CHILD: &str = "OBS_MCP_LIBOBS_BACKEND_CHILD";

#[test]
fn mcp_backend_drives_in_process_libobs() {
    if std::env::var_os(CHILD).is_some() {
        child();
        return;
    }
    let selector = format!(
        "{}::mcp_backend_drives_in_process_libobs",
        module_path!().split_once("::").unwrap().1
    );
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &selector, "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "the child ran zero tests for {selector}"
    );
}

/// Reads a scene item back through the C API.
fn item(scene: &str, source: &str) -> (bool, (f32, f32), (f32, f32), f32) {
    use obs_sys::ffi::*;
    let scene = std::ffi::CString::new(scene).unwrap();
    let source = std::ffi::CString::new(source).unwrap();
    // SAFETY: libobs is started; both names are NUL-terminated; the scene item
    // is borrowed from the scene, which this reference keeps alive.
    unsafe {
        let scene_source = obs_get_source_by_name(scene.as_ptr());
        assert!(!scene_source.is_null(), "scene exists");
        let item = obs_scene_find_source(obs_scene_from_source(scene_source), source.as_ptr());
        assert!(!item.is_null(), "item exists");
        let (mut pos, mut scale) = (vec2::default(), vec2::default());
        obs_sceneitem_get_pos(item, &raw mut pos);
        obs_sceneitem_get_scale(item, &raw mut scale);
        let result = (
            obs_sceneitem_visible(item),
            (pos.x, pos.y),
            (scale.x, scale.y),
            obs_sceneitem_get_rot(item),
        );
        obs_source_release(scene_source);
        result
    }
}

fn child() {
    // SAFETY: the locale is a NUL-terminated static string; null optional
    // paths and name store are supported by obs_startup.
    unsafe {
        assert!(obs_sys::ffi::obs_startup(
            c"en-US".as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut()
        ));
    }
    let backend = Rc::new(ObsBackend::new());

    scenes_and_program(&backend);
    sources_and_items(&backend);
    stats_capture_and_outputs(&backend);
    end_to_end(&backend);

    drop(backend);
    // SAFETY: every backend handle is dropped; this is the process's only libobs user.
    unsafe {
        while obs_sys::ffi::obs_wait_for_destroy_queue() {}
        obs_sys::ffi::obs_shutdown();
    }
}

fn scenes_and_program(backend: &ObsBackend) {
    assert!(backend.scenes().is_empty());
    assert_eq!(backend.current(), None);
    SceneBackend::create(backend, "Main").unwrap();
    SceneBackend::create(backend, "Other").unwrap();
    assert!(
        SceneBackend::create(backend, "Main")
            .unwrap_err()
            .contains("exists")
    );
    assert_eq!(backend.scenes(), ["Main", "Other"]);
    // The first scene made while no program scene is set becomes the program
    // scene; later ones leave it alone (#3339).
    assert_eq!(backend.current().as_deref(), Some("Main"));

    backend.set_current("Other").unwrap();
    assert_eq!(backend.current().as_deref(), Some("Other"));

    backend.set_current("Main").unwrap();
    assert_eq!(backend.current().as_deref(), Some("Main"));
    assert!(backend.set_current("Nope").unwrap_err().contains("Nope"));
    assert_eq!(backend.current().as_deref(), Some("Main"));

    // Removing the program scene clears the program output, so the next
    // scene made becomes the program scene again.
    backend.remove("Main").unwrap();
    backend.set_current("Other").unwrap();
    backend.remove("Other").unwrap();
    assert_eq!(backend.current(), None);
    assert_eq!(backend.scenes(), Vec::<String>::new());
    assert!(backend.remove("Main").unwrap_err().contains("Main"));
    SceneBackend::create(backend, "Main").unwrap();
    assert_eq!(backend.current().as_deref(), Some("Main"));
}

fn sources_and_items(backend: &ObsBackend) {
    // Core libobs registers no input kinds (they come from modules).
    assert!(backend.sources().is_empty());
    let error = SourceBackend::create(backend, "no_such_kind", "X", &json!({})).unwrap_err();
    assert!(error.contains("no_such_kind"), "{error}");
    assert!(SourceBackend::create(backend, "scene", "Main", &json!({})).is_err());
    assert!(
        backend
            .set_settings("Missing", &json!({"a": 1}))
            .unwrap_err()
            .contains("Missing")
    );

    // A nested scene stands in for an input: it is a source with scene items.
    SourceBackend::create(backend, "scene", "Inner", &json!({})).unwrap();
    assert!(backend.items("Main").unwrap().is_empty());
    let added = backend.add_item("Main", "Inner", Some((7.0, 9.0))).unwrap();
    assert_eq!(added.source, "Inner");
    assert!(added.visible);
    assert_eq!(added.position, (7.0, 9.0));
    assert_eq!(item("Main", "Inner").1, (7.0, 9.0));
    backend
        .set_settings("Inner", &json!({"note": "x"}))
        .unwrap();

    // `scene_items_list` ids and source names name the same item.
    let listed = backend.items("Main").unwrap();
    assert_eq!(listed, [added.clone()]);
    let id = ItemRef::Id(added.id);
    backend.set_item_visible("Main", &id, false).unwrap();
    assert!(!item("Main", "Inner").0);
    assert!(!backend.items("Main").unwrap()[0].visible);
    let by_name = ItemRef::Source("Inner".into());
    backend.set_item_visible("Main", &by_name, true).unwrap();
    assert!(item("Main", "Inner").0);
    assert!(
        backend
            .set_item_visible("Main", &ItemRef::Id(added.id + 100), true)
            .unwrap_err()
            .contains("item")
    );
    assert!(
        backend
            .add_item("Main", "Nope", None)
            .unwrap_err()
            .contains("Nope")
    );
    assert!(
        backend
            .add_item("Nope", "Inner", None)
            .unwrap_err()
            .contains("Nope")
    );
    assert!(
        backend.add_item("Main", "Main", None).is_err(),
        "a scene cannot contain itself"
    );

    let partial = Transform {
        position: Some((10.0, 20.0)),
        rotation: Some(45.0),
        ..Transform::default()
    };
    backend.set_item_transform("Main", &id, &partial).unwrap();
    let (_, pos, scale, rot) = item("Main", "Inner");
    assert_eq!((pos, rot), ((10.0, 20.0), 45.0));
    assert_eq!(scale, (1.0, 1.0), "unset fields keep their value");
    let scaled = Transform {
        scale: Some((2.0, 3.0)),
        ..Transform::default()
    };
    backend
        .set_item_transform("Main", &by_name, &scaled)
        .unwrap();
    let (_, pos, scale, rot) = item("Main", "Inner");
    assert_eq!((pos, scale, rot), ((10.0, 20.0), (2.0, 3.0), 45.0));

    assert!(
        backend
            .set_item_visible("Nope", &by_name, true)
            .unwrap_err()
            .contains("Nope")
    );
    assert!(
        backend
            .set_item_visible("Main", &ItemRef::Source("Nope".into()), true)
            .unwrap_err()
            .contains("Nope")
    );
    assert!(
        backend
            .set_item_transform("Main", &ItemRef::Source("Nope".into()), &scaled)
            .is_err()
    );
}

fn stats_capture_and_outputs(backend: &ObsBackend) {
    let stats = backend.stats();
    assert_eq!(stats.fps, 0.0, "no video subsystem, no frames");
    assert_eq!(stats.dropped_frames, 0);
    assert!(stats.cpu_usage.is_finite() && stats.cpu_usage >= 0.0);

    // Without a video subsystem there is no graphics context to render with.
    let error = backend.capture(Some("Main"), None, None).unwrap_err();
    assert!(error.contains("video"), "{error}");
    let error = backend.capture(Some("Missing"), None, None).unwrap_err();
    assert!(error.contains("Missing"), "{error}");

    let error = backend.start(OutputKind::Record).unwrap_err();
    assert!(error.contains("not configured"), "{error}");
    assert!(backend.stop(OutputKind::Stream).is_err());
    assert!(!backend.status(OutputKind::Record).active);
    let named = ObsBackend::new().with_output_names(Some("missing_record"), None);
    let error = named.start(OutputKind::Record).unwrap_err();
    assert!(error.contains("missing_record"), "{error}");
    assert!(!named.status(OutputKind::Record).active);
}

/// The tools and resources a client calls reach the same libobs state.
fn end_to_end(backend: &Rc<ObsBackend>) {
    let mut server = ObsBackend::server(backend);
    let input = [
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"scene_create","arguments":{"name":"Via MCP"}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"scene_set_current","arguments":{"name":"Via MCP"}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"scenes_list","arguments":{}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":"obs://stats"}}),
        json!({"jsonrpc":"2.0","id":5,"method":"resources/read","params":{"uri":"obs://scenes"}}),
        json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"screenshot","arguments":{}}}),
    ]
    .iter()
    .map(|v| format!("{v}\n"))
    .collect::<String>();
    let mut out = Vec::new();
    server.serve(input.as_bytes(), &mut out).unwrap();
    let replies: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(replies.len(), 6);
    assert_eq!(replies[0]["result"]["isError"], false, "{}", replies[0]);
    assert_eq!(replies[1]["result"]["isError"], false, "{}", replies[1]);
    let listed = replies[2]["result"]["content"][0]["text"].as_str().unwrap();
    assert!(listed.contains("Via MCP"), "{listed}");
    let stats = replies[3]["result"]["contents"][0]["text"]
        .as_str()
        .unwrap();
    assert!(stats.contains("\"fps\""), "{stats}");
    let graph: Value = serde_json::from_str(
        replies[4]["result"]["contents"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(graph["current"], "Via MCP");
    assert_eq!(
        replies[5]["result"]["isError"], true,
        "screenshot needs video"
    );
}
