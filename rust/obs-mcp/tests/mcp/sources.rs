//! The source tools against a fake [`SourceBackend`].

use std::cell::RefCell;
use std::rc::Rc;

use obs_mcp::server::{Server, ToolRegistry};
use obs_mcp::tools::sources::{SourceBackend, SourceInfo, Transform, register};
use serde_json::{Value, json};

#[derive(Default)]
struct Fake {
    sources: RefCell<Vec<(SourceInfo, Value)>>,
    visible: RefCell<Vec<(String, String, bool)>>,
    transforms: RefCell<Vec<(String, String, Transform)>>,
}

impl Fake {
    fn has(&self, name: &str) -> bool {
        self.sources.borrow().iter().any(|(i, _)| i.name == name)
    }
}

impl SourceBackend for Fake {
    fn sources(&self) -> Vec<SourceInfo> {
        self.sources
            .borrow()
            .iter()
            .map(|(i, _)| i.clone())
            .collect()
    }
    fn create(&self, kind: &str, name: &str, settings: &Value) -> Result<(), String> {
        if kind == "nope" {
            return Err(format!("unknown source kind {kind}"));
        }
        if self.has(name) {
            return Err(format!("source {name} already exists"));
        }
        let info = SourceInfo {
            name: name.into(),
            kind: kind.into(),
        };
        self.sources.borrow_mut().push((info, settings.clone()));
        Ok(())
    }
    fn set_settings(&self, name: &str, settings: &Value) -> Result<(), String> {
        let mut all = self.sources.borrow_mut();
        let Some((_, cur)) = all.iter_mut().find(|(i, _)| i.name == name) else {
            return Err(format!("no source named {name}"));
        };
        for (k, v) in settings.as_object().unwrap() {
            cur[k] = v.clone();
        }
        Ok(())
    }
    fn set_item_visible(&self, scene: &str, source: &str, visible: bool) -> Result<(), String> {
        if scene != "A" || !self.has(source) {
            return Err(format!("no item {source} in scene {scene}"));
        }
        self.visible
            .borrow_mut()
            .push((scene.into(), source.into(), visible));
        Ok(())
    }
    fn set_item_transform(
        &self,
        scene: &str,
        source: &str,
        transform: &Transform,
    ) -> Result<(), String> {
        if scene != "A" || !self.has(source) {
            return Err(format!("no item {source} in scene {scene}"));
        }
        self.transforms
            .borrow_mut()
            .push((scene.into(), source.into(), *transform));
        Ok(())
    }
}

fn server(fake: &Rc<Fake>) -> Server {
    let backend: Rc<dyn SourceBackend> = fake.clone();
    let mut reg = ToolRegistry::new();
    register(&mut reg, &backend);
    Server::with_tools(reg)
}

fn rpc(s: &mut Server, method: &str, params: &Value) -> Value {
    let line = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    let mut out = Vec::new();
    s.serve(format!("{line}\n").as_bytes(), &mut out).unwrap();
    serde_json::from_slice(&out).unwrap()
}

/// Calls a tool; returns (isError, parsed JSON text or the error text).
fn call(s: &mut Server, name: &str, args: &Value) -> (bool, Value) {
    let r = rpc(s, "tools/call", &json!({"name": name, "arguments": args}));
    let res = &r["result"];
    let text = res["content"][0]["text"].as_str().expect("text content");
    let is_err = res["isError"] == true;
    let value = if is_err {
        Value::String(text.into())
    } else {
        serde_json::from_str(text).expect("tool text is JSON")
    };
    (is_err, value)
}

fn with_source(name: &str) -> (Rc<Fake>, Server) {
    let fake = Rc::new(Fake::default());
    let mut s = server(&fake);
    let args = json!({"kind": "color_source", "name": name, "settings": {"width": 10}});
    assert!(!call(&mut s, "source_create", &args).0);
    (fake, s)
}

#[test]
fn lists_five_tools_with_object_schemas() {
    let mut s = server(&Rc::new(Fake::default()));
    let r = rpc(&mut s, "tools/list", &json!({}));
    let tools = r["result"]["tools"].as_array().unwrap();
    let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "scene_item_set_transform",
            "scene_item_set_visible",
            "source_create",
            "source_set_settings",
            "sources_list"
        ]
    );
    for t in tools {
        assert_eq!(t["inputSchema"]["type"], "object");
        assert!(!t["description"].as_str().unwrap().is_empty());
    }
    let find = |n: &str| tools.iter().find(|t| t["name"] == n).unwrap();
    assert_eq!(
        find("source_create")["inputSchema"]["required"],
        json!(["kind", "name", "settings"])
    );
    assert_eq!(
        find("source_create")["inputSchema"]["properties"]["settings"]["type"],
        "object"
    );
    let xf = &find("scene_item_set_transform")["inputSchema"]["properties"];
    assert_eq!(xf["position"]["required"], json!(["x", "y"]));
    assert_eq!(xf["rotation"]["type"], "number");
}

#[test]
fn create_and_list() {
    let mut s = server(&Rc::new(Fake::default()));
    assert_eq!(
        call(&mut s, "sources_list", &json!({})),
        (false, json!({"sources": []}))
    );
    let args = json!({"kind": "color_source", "name": "bg", "settings": {}});
    assert_eq!(
        call(&mut s, "source_create", &args),
        (false, json!({"name": "bg", "kind": "color_source"}))
    );
    assert_eq!(
        call(&mut s, "sources_list", &json!({})),
        (
            false,
            json!({"sources": [{"name": "bg", "kind": "color_source"}]})
        )
    );
}

#[test]
fn set_settings_merges() {
    let (fake, mut s) = with_source("bg");
    let args = json!({"name": "bg", "settings": {"height": 20}});
    assert_eq!(
        call(&mut s, "source_set_settings", &args),
        (false, json!({"name": "bg"}))
    );
    assert_eq!(
        fake.sources.borrow()[0].1,
        json!({"width": 10, "height": 20})
    );
}

#[test]
fn item_visibility() {
    let (fake, mut s) = with_source("bg");
    let args = json!({"scene": "A", "source": "bg", "visible": false});
    assert_eq!(
        call(&mut s, "scene_item_set_visible", &args),
        (
            false,
            json!({"scene": "A", "source": "bg", "visible": false})
        )
    );
    assert_eq!(
        *fake.visible.borrow(),
        [("A".to_string(), "bg".to_string(), false)]
    );
}

#[test]
fn item_transform_is_partial() {
    let (fake, mut s) = with_source("bg");
    let args =
        json!({"scene": "A", "source": "bg", "position": {"x": 1.5, "y": -2}, "rotation": 90});
    assert!(!call(&mut s, "scene_item_set_transform", &args).0);
    let args = json!({"scene": "A", "source": "bg", "scale": {"x": 2, "y": 3}});
    assert!(!call(&mut s, "scene_item_set_transform", &args).0);
    let t = fake.transforms.borrow();
    assert_eq!(
        t[0].2,
        Transform {
            position: Some((1.5, -2.0)),
            scale: None,
            rotation: Some(90.0)
        }
    );
    assert_eq!(
        t[1].2,
        Transform {
            position: None,
            scale: Some((2.0, 3.0)),
            rotation: None
        }
    );
}

#[test]
fn backend_errors_become_tool_errors() {
    let (_, mut s) = with_source("bg");
    let dup = json!({"kind": "color_source", "name": "bg", "settings": {}});
    let (err, msg) = call(&mut s, "source_create", &dup);
    assert!(err && msg.as_str().unwrap().contains("already exists"));
    let bad = json!({"kind": "nope", "name": "x", "settings": {}});
    assert!(call(&mut s, "source_create", &bad).0);
    let (err, msg) = call(
        &mut s,
        "source_set_settings",
        &json!({"name": "zz", "settings": {}}),
    );
    assert!(err && msg.as_str().unwrap().contains("no source named zz"));
    let args = json!({"scene": "B", "source": "bg", "visible": true});
    assert!(call(&mut s, "scene_item_set_visible", &args).0);
    let args = json!({"scene": "B", "source": "bg", "rotation": 1});
    assert!(call(&mut s, "scene_item_set_transform", &args).0);
}

#[test]
fn bad_arguments_are_tool_errors() {
    let (fake, mut s) = with_source("bg");
    let item = |extra: Value| {
        let mut v = json!({"scene": "A", "source": "bg"});
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        v
    };
    let cases = [
        ("source_create", json!({}), "kind"),
        ("source_create", json!({"kind": "k", "name": ""}), "name"),
        (
            "source_create",
            json!({"kind": "k", "name": "n"}),
            "settings",
        ),
        (
            "source_create",
            json!({"kind": "k", "name": "n", "settings": []}),
            "settings",
        ),
        (
            "source_set_settings",
            json!({"name": 5, "settings": {}}),
            "name",
        ),
        ("source_set_settings", json!({"name": "bg"}), "settings"),
        (
            "scene_item_set_visible",
            item(json!({"visible": "yes"})),
            "visible",
        ),
        (
            "scene_item_set_visible",
            json!({"source": "bg", "visible": true}),
            "scene",
        ),
        ("scene_item_set_transform", item(json!({})), "at least one"),
        (
            "scene_item_set_transform",
            item(json!({"position": {"x": 1}})),
            "position",
        ),
        (
            "scene_item_set_transform",
            item(json!({"scale": "big"})),
            "scale",
        ),
        (
            "scene_item_set_transform",
            item(json!({"rotation": "90"})),
            "rotation",
        ),
    ];
    for (tool, args, needle) in cases {
        let (err, msg) = call(&mut s, tool, &args);
        assert!(err, "{tool} {args}");
        assert!(msg.as_str().unwrap().contains(needle), "{tool}: {msg}");
    }
    assert_eq!(fake.sources.borrow().len(), 1);
    assert!(fake.transforms.borrow().is_empty());
}
