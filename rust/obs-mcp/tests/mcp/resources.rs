//! The `obs://stats` and `obs://scenes` resources, `resources/list` and
//! `resources/read` against fake backends.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use obs_mcp::resources::ResourceRegistry;
use obs_mcp::resources::stats::{Stats, StatsBackend};
use obs_mcp::resources::{scenes as scenes_resource, stats as stats_resource};
use obs_mcp::server::Server;
use obs_mcp::tools::scenes::SceneBackend;
use obs_mcp::tools::sources::{SourceBackend, SourceInfo, Transform};
use serde_json::{Value, json};

struct FakeStats {
    dropped: Cell<u64>,
}

impl StatsBackend for FakeStats {
    fn stats(&self) -> Stats {
        Stats {
            fps: 29.97,
            cpu_usage: 12.5,
            dropped_frames: self.dropped.get(),
        }
    }
}

struct FakeScenes {
    current: RefCell<Option<String>>,
}

impl SceneBackend for FakeScenes {
    fn scenes(&self) -> Vec<String> {
        vec!["Main".into(), "Aux".into()]
    }
    fn current(&self) -> Option<String> {
        self.current.borrow().clone()
    }
    fn set_current(&self, name: &str) -> Result<(), String> {
        *self.current.borrow_mut() = Some(name.into());
        Ok(())
    }
    fn create(&self, _: &str) -> Result<(), String> {
        Err("unused".into())
    }
    fn remove(&self, _: &str) -> Result<(), String> {
        Err("unused".into())
    }
}

struct FakeSources;

impl SourceBackend for FakeSources {
    fn sources(&self) -> Vec<SourceInfo> {
        vec![SourceInfo {
            name: "Cam".into(),
            kind: "v4l2_input".into(),
        }]
    }
    fn create(&self, _: &str, _: &str, _: &Value) -> Result<(), String> {
        Err("unused".into())
    }
    fn set_settings(&self, _: &str, _: &Value) -> Result<(), String> {
        Err("unused".into())
    }
    fn set_item_visible(&self, _: &str, _: &str, _: bool) -> Result<(), String> {
        Err("unused".into())
    }
    fn set_item_transform(&self, _: &str, _: &str, _: &Transform) -> Result<(), String> {
        Err("unused".into())
    }
}

struct Fixture {
    server: Server,
    stats: Rc<FakeStats>,
    scenes: Rc<FakeScenes>,
}

fn fixture() -> Fixture {
    let stats = Rc::new(FakeStats {
        dropped: Cell::new(3),
    });
    let scenes = Rc::new(FakeScenes {
        current: RefCell::new(Some("Main".into())),
    });
    let stats_backend: Rc<dyn StatsBackend> = stats.clone();
    let scene_backend: Rc<dyn SceneBackend> = scenes.clone();
    let source_backend: Rc<dyn SourceBackend> = Rc::new(FakeSources);
    let mut reg = ResourceRegistry::new();
    stats_resource::register(&mut reg, &stats_backend);
    scenes_resource::register(&mut reg, &scene_backend, &source_backend);
    Fixture {
        server: Server::new().with_resources(reg),
        stats,
        scenes,
    }
}

fn rpc(s: &mut Server, method: &str, params: &Value) -> Value {
    let line = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    let mut out = Vec::new();
    s.serve(format!("{line}\n").as_bytes(), &mut out).unwrap();
    serde_json::from_slice(&out).unwrap()
}

/// Reads a resource and returns its parsed JSON text, checking the envelope.
fn read_json(s: &mut Server, uri: &str) -> Value {
    let r = rpc(s, "resources/read", &json!({"uri": uri}));
    let contents = r["result"]["contents"].as_array().expect("contents");
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["uri"], uri);
    assert_eq!(contents[0]["mimeType"], "application/json");
    serde_json::from_str(contents[0]["text"].as_str().unwrap()).unwrap()
}

#[test]
fn initialize_advertises_resources() {
    let mut s = Server::new();
    let r = rpc(&mut s, "initialize", &json!({}));
    assert_eq!(r["result"]["capabilities"]["resources"], json!({}));
}

#[test]
fn list_describes_both_resources() {
    let mut f = fixture();
    let r = rpc(&mut f.server, "resources/list", &json!({}));
    let list = r["result"]["resources"].as_array().unwrap();
    let mut uris: Vec<&str> = list.iter().map(|x| x["uri"].as_str().unwrap()).collect();
    uris.sort_unstable();
    assert_eq!(uris, ["obs://scenes", "obs://stats"]);
    for item in list {
        assert!(!item["name"].as_str().unwrap().is_empty());
        assert!(!item["description"].as_str().unwrap().is_empty());
        assert_eq!(item["mimeType"], "application/json");
    }
}

#[test]
fn list_is_empty_without_resources() {
    let mut s = Server::new();
    let r = rpc(&mut s, "resources/list", &json!({}));
    assert_eq!(r["result"], json!({"resources": []}));
}

#[test]
fn stats_reports_fps_cpu_and_dropped_frames() {
    let mut f = fixture();
    assert_eq!(
        read_json(&mut f.server, "obs://stats"),
        json!({"fps": 29.97, "cpu_usage": 12.5, "dropped_frames": 3})
    );
    f.stats.dropped.set(9);
    assert_eq!(read_json(&mut f.server, "obs://stats")["dropped_frames"], 9);
}

#[test]
fn scenes_reports_the_scene_graph() {
    let mut f = fixture();
    let expected = json!({
        "current": "Main",
        "scenes": ["Main", "Aux"],
        "sources": [{"name": "Cam", "kind": "v4l2_input"}],
    });
    assert_eq!(read_json(&mut f.server, "obs://scenes"), expected);
    f.scenes.set_current("Aux").unwrap();
    assert_eq!(read_json(&mut f.server, "obs://scenes")["current"], "Aux");
    *f.scenes.current.borrow_mut() = None;
    assert_eq!(
        read_json(&mut f.server, "obs://scenes")["current"],
        Value::Null
    );
}

#[test]
fn unknown_uri_is_resource_not_found() {
    let mut f = fixture();
    let r = rpc(
        &mut f.server,
        "resources/read",
        &json!({"uri": "obs://nope"}),
    );
    assert_eq!(r["error"]["code"], -32002);
    assert!(
        r["error"]["message"]
            .as_str()
            .unwrap()
            .contains("obs://nope")
    );
}

#[test]
fn missing_uri_is_invalid_params() {
    let mut f = fixture();
    let r = rpc(&mut f.server, "resources/read", &json!({}));
    assert_eq!(r["error"]["code"], -32602);
}

#[test]
fn registering_a_uri_twice_keeps_one() {
    let mut f = fixture();
    let again: Rc<dyn StatsBackend> = f.stats.clone();
    let mut reg = ResourceRegistry::new();
    stats_resource::register(&mut reg, &again);
    stats_resource::register(&mut reg, &again);
    f.server = Server::new().with_resources(reg);
    let r = rpc(&mut f.server, "resources/list", &json!({}));
    assert_eq!(r["result"]["resources"].as_array().unwrap().len(), 1);
}
