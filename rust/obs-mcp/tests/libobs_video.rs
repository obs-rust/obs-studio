//! obs-mcp over a libobs started like the application: video, audio, modules
//! and the record and stream outputs (#3317, #3312).
//!
//! Included by `obs-tests-app` under `bundled-native`, the only category that
//! links libobs. libobs is process-global, so each case re-execs this
//! executable with an exact selector and runs the body in the child (testing
//! policy, "Test binary budget").
//!
//! - `mcp_video_failures_degrade_to_errors` needs nothing but libobs: a graphics
//!   module that does not exist must leave the tools returning errors that say
//!   why, never crash.
//! - `mcp_screenshot_and_record_over_real_video` needs a graphics module, an X
//!   display (Tier 2 runs display tests under `xvfb-run`) and the plugins that
//!   provide `color_source_v3`, `ffmpeg_muxer`, `obs_x264` and `ffmpeg_aac`
//!   (`OBS_MCP_GRAPHICS_MODULE`, `OBS_MCP_DATA_PATH`, `OBS_MCP_MODULE_BIN`,
//!   `OBS_MCP_MODULE_DATA`, see `StartOptions::from_env`). Without them it
//!   prints a `SKIP` line and passes, unless `OBS_MCP_REQUIRE_VIDEO` is set,
//!   which turns every skip into a failure.

use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use obs_mcp::host::{Runtime, StartOptions};
use obs_mcp::tools::outputs::{OutputBackend, OutputKind};
use obs_mcp::tools::screenshot::CaptureBackend;
use obs_mcp::tools::sources::SourceBackend;
use serde_json::{Value, json};

const CHILD: &str = "OBS_MCP_LIBOBS_VIDEO_CHILD";

/// Runs `case` in a child process of this executable; the child runs `body`.
fn in_child(case: &str, body: fn()) {
    if std::env::var(CHILD).as_deref() == Ok(case) {
        body();
        return;
    }
    let selector = format!("{}::{case}", module_path!().split_once("::").unwrap().1);
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &selector, "--nocapture", "--test-threads=1"])
        .env(CHILD, case)
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

#[test]
fn mcp_video_failures_degrade_to_errors() {
    in_child("mcp_video_failures_degrade_to_errors", degraded);
}

#[test]
fn mcp_screenshot_and_record_over_real_video() {
    in_child("mcp_screenshot_and_record_over_real_video", real_video);
}

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("obs-mcp-{name}-{}", std::process::id()))
}

fn degraded() {
    let mut options = StartOptions::from_env(Some(scratch("degraded")));
    options.graphics_module = "libobs-module-that-does-not-exist".to_owned();
    let runtime = Runtime::start(&options).expect("libobs itself starts");
    let reason = runtime
        .video_error()
        .expect("video cannot start")
        .to_owned();
    assert!(
        reason.contains("libobs-module-that-does-not-exist"),
        "{reason}"
    );

    let backend = Rc::new(runtime.backend());
    let error = backend.capture(None, None, None).unwrap_err();
    assert!(error.contains("program"), "{error}");
    backend_scene(&backend);
    let error = backend.capture(Some("Main"), None, None).unwrap_err();
    assert!(error.contains(&reason), "{error}");

    let error = backend.start(OutputKind::Record).unwrap_err();
    assert!(error.contains("simple_file_output"), "{error}");
    assert!(error.contains("video"), "{error}");
    assert!(!backend.status(OutputKind::Record).active);
    assert!(backend.start(OutputKind::Stream).is_err());

    // The tools report the same errors to a client instead of failing the call.
    let mut server = obs_mcp::backend::ObsBackend::server(&backend);
    let request = json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
        "params":{"name":"record_start","arguments":{}}});
    let mut out = Vec::new();
    server
        .serve(format!("{request}\n").as_bytes(), &mut out)
        .unwrap();
    let reply: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(reply["result"]["isError"], true, "{reply}");
}

fn backend_scene(backend: &obs_mcp::backend::ObsBackend) {
    use obs_mcp::tools::scenes::SceneBackend;
    SceneBackend::create(backend, "Main").unwrap();
    backend.set_current("Main").unwrap();
}

/// Reports why the real-video case cannot run: a skip, or a failure when
/// `OBS_MCP_REQUIRE_VIDEO` is set.
fn skip(why: &str) {
    assert!(
        std::env::var_os("OBS_MCP_REQUIRE_VIDEO").is_none(),
        "OBS_MCP_REQUIRE_VIDEO is set but: {why}"
    );
    println!("SKIP: {why}");
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// libobs stores a colour source's colour as `0xAABBGGRR`.
const COLOR_ABGR: u32 = 0xFF_E0_80_20;
const COLOR_RGBA: [u8; 4] = [0x20, 0x80, 0xE0, 0xFF];
const TOLERANCE: u8 = 4;

fn assert_solid(rgba: &[u8], what: &str) {
    for (i, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
        for (channel, (got, want)) in pixel.iter().zip(COLOR_RGBA).enumerate() {
            assert!(
                got.abs_diff(want) <= TOLERANCE,
                "{what}: pixel {i} channel {channel} is {got}, expected {want} (pixel {pixel:?})"
            );
        }
    }
}

fn real_video() {
    let record_dir = scratch("record");
    let options = StartOptions::from_env(Some(record_dir.clone()));
    let runtime = Runtime::start(&options).expect("libobs itself starts");
    if let Some(reason) = runtime.video_error() {
        return skip(&format!("no video: {reason}"));
    }
    let backend = Rc::new(runtime.backend());

    let created = SourceBackend::create(
        &*backend,
        "color_source_v3",
        "Solid",
        &json!({"color": COLOR_ABGR, "width": 64, "height": 32}),
    );
    if let Err(error) = created {
        return skip(&format!("no solid colour source: {error}"));
    }
    backend_scene(&backend);
    add_item("Main", "Solid");

    // The source at its native size, then scaled keeping the aspect ratio,
    // then to an explicit size.
    let native = backend.capture(Some("Solid"), None, None).unwrap();
    assert_eq!((native.width, native.height), (64, 32));
    assert_eq!(native.rgba.len(), 64 * 32 * 4);
    assert_solid(&native.rgba, "native");
    let half = backend.capture(Some("Solid"), Some(32), None).unwrap();
    assert_eq!((half.width, half.height), (32, 16));
    assert_solid(&half.rgba, "half width, aspect kept");
    let tall = backend.capture(Some("Solid"), None, Some(64)).unwrap();
    assert_eq!((tall.width, tall.height), (128, 64));
    assert_solid(&tall.rgba, "double height, aspect kept");
    let boxed = backend.capture(Some("Solid"), Some(10), Some(20)).unwrap();
    assert_eq!((boxed.width, boxed.height), (10, 20));
    assert_solid(&boxed.rgba, "explicit size");

    // The program scene is the canvas: the source sits in its top left corner
    // and the rest is transparent.
    let scene = backend.capture(None, None, None).unwrap();
    assert_eq!((scene.width, scene.height), options.size);
    let at = |x: usize, y: usize| {
        let i = (y * scene.width as usize + x) * 4;
        &scene.rgba[i..i + 4]
    };
    assert_solid(at(10, 10), "scene, inside the source");
    assert_eq!(
        at(scene.width as usize - 1, scene.height as usize - 1)[3],
        0
    );

    // Over MCP the same image arrives as a PNG of the right size.
    let mut server = obs_mcp::backend::ObsBackend::server(&backend);
    let request = json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
        "params":{"name":"screenshot","arguments":{"source":"Solid","width":32}}});
    let mut out = Vec::new();
    server
        .serve(format!("{request}\n").as_bytes(), &mut out)
        .unwrap();
    let reply: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(reply["result"]["isError"], false, "{reply}");
    let image = &reply["result"]["content"][0];
    assert_eq!(image["mimeType"], "image/png", "{reply}");
    let png = decode_base64(image["data"].as_str().unwrap());
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 32);
    assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 16);

    // record_start -> record_status(active, bytes > 0) -> record_stop.
    if runtime_output_missing(&backend) {
        return skip("no record output (ffmpeg_muxer, obs_x264 or ffmpeg_aac missing)");
    }
    assert!(!backend.status(OutputKind::Record).active);
    backend.start(OutputKind::Record).unwrap();
    assert!(
        backend.start(OutputKind::Record).is_err(),
        "already running"
    );
    wait_for("recorded bytes", || {
        let status = backend.status(OutputKind::Record);
        assert!(status.active, "the recording stopped by itself");
        status.bytes.unwrap_or(0) > 0
    });
    backend.stop(OutputKind::Record).unwrap();
    wait_for("the recording to stop", || {
        !backend.status(OutputKind::Record).active
    });
    let files: Vec<_> = std::fs::read_dir(&record_dir)
        .unwrap()
        .map(|e| e.unwrap().metadata().unwrap().len())
        .collect();
    assert_eq!(files.len(), 1, "one recording file in {record_dir:?}");
    assert!(files[0] > 0);
    drop(server);
}

/// Whether starting the record output fails because it was never created.
fn runtime_output_missing(backend: &obs_mcp::backend::ObsBackend) -> bool {
    backend
        .stop(OutputKind::Record)
        .err()
        .is_some_and(|e| e.contains("no output named"))
}

/// Puts `source` in `scene` (there is no add-item tool yet).
fn add_item(scene: &str, source: &str) {
    use obs_sys::ffi::*;
    let scene = std::ffi::CString::new(scene).unwrap();
    let source = std::ffi::CString::new(source).unwrap();
    // SAFETY: libobs is started; names are NUL-terminated; references taken
    // here are released before returning.
    unsafe {
        let scene_source = obs_get_source_by_name(scene.as_ptr());
        let child = obs_get_source_by_name(source.as_ptr());
        assert!(!scene_source.is_null() && !child.is_null());
        assert!(!obs_scene_add(obs_scene_from_source(scene_source), child).is_null());
        obs_source_release(child);
        obs_source_release(scene_source);
    }
}

fn decode_base64(text: &str) -> Vec<u8> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .unwrap()
}
