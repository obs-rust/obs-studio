//! The source tools against a fake [`SourceBackend`].

use std::cell::RefCell;
use std::rc::Rc;

use obs_mcp::server::{Server, ToolRegistry};
use obs_mcp::tools::sources::{ItemInfo, ItemRef, SourceBackend, SourceInfo, Transform, register};
use serde_json::{Value, json};

#[derive(Default)]
struct Fake {
    sources: RefCell<Vec<(SourceInfo, Value)>>,
    visible: RefCell<Vec<(String, ItemRef, bool)>>,
    transforms: RefCell<Vec<(String, ItemRef, Transform)>>,
    /// The items of scene `A`, in order; ids count up from 1.
    items: RefCell<Vec<ItemInfo>>,
}

impl Fake {
    fn has(&self, name: &str) -> bool {
        self.sources.borrow().iter().any(|(i, _)| i.name == name)
    }

    /// Runs `f` on the item `item` of scene `scene`.
    fn with_item(
        &self,
        scene: &str,
        item: &ItemRef,
        f: impl FnOnce(&mut ItemInfo),
    ) -> Result<(), String> {
        let mut items = self.items.borrow_mut();
        let found = items.iter_mut().find(|i| match item {
            ItemRef::Id(id) => i.id == *id,
            ItemRef::Source(name) => i.source == *name,
        });
        match (scene, found) {
            ("A", Some(i)) => {
                f(i);
                Ok(())
            }
            _ => Err(format!("no item {item:?} in scene {scene}")),
        }
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
    fn add_item(
        &self,
        scene: &str,
        source: &str,
        position: Option<(f64, f64)>,
    ) -> Result<ItemInfo, String> {
        if scene != "A" {
            return Err(format!("no scene named {scene}"));
        }
        if !self.has(source) {
            return Err(format!("no source named {source}"));
        }
        let mut items = self.items.borrow_mut();
        let item = ItemInfo {
            id: i64::try_from(items.len()).unwrap() + 1,
            source: source.into(),
            visible: true,
            position: position.unwrap_or((0.0, 0.0)),
            scale: (1.0, 1.0),
            rotation: 0.0,
        };
        items.push(item.clone());
        Ok(item)
    }
    fn items(&self, scene: &str) -> Result<Vec<ItemInfo>, String> {
        if scene != "A" {
            return Err(format!("no scene named {scene}"));
        }
        Ok(self.items.borrow().clone())
    }
    fn set_item_visible(&self, scene: &str, item: &ItemRef, visible: bool) -> Result<(), String> {
        self.with_item(scene, item, |i| i.visible = visible)?;
        self.visible
            .borrow_mut()
            .push((scene.into(), item.clone(), visible));
        Ok(())
    }
    fn set_item_transform(
        &self,
        scene: &str,
        item: &ItemRef,
        transform: &Transform,
    ) -> Result<(), String> {
        self.with_item(scene, item, |i| {
            i.position = transform.position.unwrap_or(i.position);
            i.scale = transform.scale.unwrap_or(i.scale);
            i.rotation = transform.rotation.unwrap_or(i.rotation);
        })?;
        self.transforms
            .borrow_mut()
            .push((scene.into(), item.clone(), *transform));
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
fn lists_seven_tools_with_object_schemas() {
    let mut s = server(&Rc::new(Fake::default()));
    let r = rpc(&mut s, "tools/list", &json!({}));
    let tools = r["result"]["tools"].as_array().unwrap();
    let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "scene_item_add",
            "scene_item_set_transform",
            "scene_item_set_visible",
            "scene_items_list",
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
    add(&mut s, "bg");
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
        [("A".to_string(), ItemRef::Source("bg".to_string()), false)]
    );
}

#[test]
fn item_transform_is_partial() {
    let (fake, mut s) = with_source("bg");
    add(&mut s, "bg");
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
    let args = json!({"scene": "B", "source": "bg"});
    assert!(call(&mut s, "scene_item_add", &args).0);
    assert!(call(&mut s, "scene_items_list", &json!({"scene": "B"})).0);
    let args = json!({"scene": "A", "source": "zz"});
    let (err, msg) = call(&mut s, "scene_item_add", &args);
    assert!(err && msg.as_str().unwrap().contains("no source named zz"));
}

fn add(s: &mut Server, source: &str) -> Value {
    let (err, v) = call(
        s,
        "scene_item_add",
        &json!({"scene": "A", "source": source}),
    );
    assert!(!err, "{v}");
    v
}

#[test]
fn item_add_and_list_report_ids_visibility_and_transform() {
    let (_, mut s) = with_source("bg");
    let args = json!({"kind": "color_source", "name": "fg", "settings": {}});
    assert!(!call(&mut s, "source_create", &args).0);
    assert_eq!(
        call(&mut s, "scene_items_list", &json!({"scene": "A"})),
        (false, json!({"scene": "A", "items": []}))
    );

    let added = add(&mut s, "bg");
    assert_eq!(added["item"], 1);
    assert_eq!(added["source"], "bg");
    let args = json!({"scene": "A", "source": "fg", "position": {"x": 10, "y": 20.5}});
    let (err, added) = call(&mut s, "scene_item_add", &args);
    assert!(!err);
    assert_eq!(added["item"], 2);
    assert_eq!(added["position"], json!({"x": 10.0, "y": 20.5}));

    let (err, list) = call(&mut s, "scene_items_list", &json!({"scene": "A"}));
    assert!(!err);
    assert_eq!(
        list,
        json!({"scene": "A", "items": [
            {"item": 1, "source": "bg", "visible": true,
             "position": {"x": 0.0, "y": 0.0}, "scale": {"x": 1.0, "y": 1.0}, "rotation": 0.0},
            {"item": 2, "source": "fg", "visible": true,
             "position": {"x": 10.0, "y": 20.5}, "scale": {"x": 1.0, "y": 1.0}, "rotation": 0.0},
        ]})
    );
}

#[test]
fn item_ids_from_the_list_drive_visibility_and_transform() {
    let (fake, mut s) = with_source("bg");
    add(&mut s, "bg");
    let args = json!({"scene": "A", "item": 1, "visible": false});
    assert_eq!(
        call(&mut s, "scene_item_set_visible", &args),
        (false, json!({"scene": "A", "item": 1, "visible": false}))
    );
    let args = json!({"scene": "A", "item": 1, "position": {"x": 5, "y": 6}, "rotation": 30});
    assert_eq!(
        call(&mut s, "scene_item_set_transform", &args),
        (false, json!({"scene": "A", "item": 1}))
    );
    assert_eq!(
        fake.visible.borrow()[0].1,
        ItemRef::Id(1),
        "the id reaches the backend as an id"
    );
    let (_, list) = call(&mut s, "scene_items_list", &json!({"scene": "A"}));
    let item = &list["items"][0];
    assert_eq!(item["visible"], false);
    assert_eq!(item["position"], json!({"x": 5.0, "y": 6.0}));
    assert_eq!(item["rotation"], 30.0);

    let args = json!({"scene": "A", "item": 9, "visible": true});
    let (err, msg) = call(&mut s, "scene_item_set_visible", &args);
    assert!(err && msg.as_str().unwrap().contains("no item"), "{msg}");
}

#[test]
fn source_create_can_add_the_source_to_a_scene() {
    let mut s = server(&Rc::new(Fake::default()));
    let args = json!({"kind": "color_source", "name": "red", "settings": {},
        "scene": "A", "position": {"x": 3, "y": 4}});
    let (err, v) = call(&mut s, "source_create", &args);
    assert!(!err, "{v}");
    assert_eq!(v["name"], "red");
    assert_eq!(v["kind"], "color_source");
    assert_eq!(v["scene"], "A");
    assert_eq!(v["item"], 1);
    let (_, list) = call(&mut s, "scene_items_list", &json!({"scene": "A"}));
    assert_eq!(list["items"][0]["source"], "red");
    assert_eq!(list["items"][0]["position"], json!({"x": 3.0, "y": 4.0}));

    // A position without a scene has nothing to apply to.
    let args = json!({"kind": "color_source", "name": "x", "settings": {},
        "position": {"x": 3, "y": 4}});
    let (err, msg) = call(&mut s, "source_create", &args);
    assert!(err && msg.as_str().unwrap().contains("scene"), "{msg}");

    // A missing scene is reported, and says the source itself was created.
    let args = json!({"kind": "color_source", "name": "y", "settings": {}, "scene": "B"});
    let (err, msg) = call(&mut s, "source_create", &args);
    let msg = msg.as_str().unwrap();
    assert!(
        err && msg.contains("'y'") && msg.contains("no scene named B"),
        "{msg}"
    );
}

#[test]
fn item_tools_need_exactly_one_of_item_and_source() {
    let (_, mut s) = with_source("bg");
    add(&mut s, "bg");
    for args in [
        json!({"scene": "A", "visible": true}),
        json!({"scene": "A", "item": 1, "source": "bg", "visible": true}),
        json!({"scene": "A", "item": "1", "visible": true}),
    ] {
        let (err, msg) = call(&mut s, "scene_item_set_visible", &args);
        assert!(
            err && msg.as_str().unwrap().contains("item"),
            "{args}: {msg}"
        );
    }
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
