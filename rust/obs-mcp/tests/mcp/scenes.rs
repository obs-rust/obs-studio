//! The scene tools against a fake [`SceneBackend`].

use std::cell::RefCell;
use std::rc::Rc;

use obs_mcp::server::{Server, ToolRegistry};
use obs_mcp::tools::scenes::{SceneBackend, register};
use serde_json::{Value, json};

#[derive(Default)]
struct Fake {
    scenes: RefCell<Vec<String>>,
    current: RefCell<Option<String>>,
}

impl SceneBackend for Fake {
    fn scenes(&self) -> Vec<String> {
        self.scenes.borrow().clone()
    }
    fn current(&self) -> Option<String> {
        self.current.borrow().clone()
    }
    fn set_current(&self, name: &str) -> Result<(), String> {
        if !self.scenes.borrow().iter().any(|s| s == name) {
            return Err(format!("no scene named {name}"));
        }
        *self.current.borrow_mut() = Some(name.into());
        Ok(())
    }
    fn create(&self, name: &str) -> Result<(), String> {
        if self.scenes.borrow().iter().any(|s| s == name) {
            return Err(format!("scene {name} already exists"));
        }
        self.scenes.borrow_mut().push(name.into());
        Ok(())
    }
    fn remove(&self, name: &str) -> Result<(), String> {
        let mut scenes = self.scenes.borrow_mut();
        let Some(i) = scenes.iter().position(|s| s == name) else {
            return Err(format!("no scene named {name}"));
        };
        scenes.remove(i);
        if self.current.borrow().as_deref() == Some(name) {
            *self.current.borrow_mut() = scenes.first().cloned();
        }
        Ok(())
    }
}

fn server(fake: &Rc<Fake>) -> Server {
    let backend: Rc<dyn SceneBackend> = fake.clone();
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
            "scene_create",
            "scene_get_current",
            "scene_remove",
            "scene_set_current",
            "scenes_list"
        ]
    );
    for t in tools {
        assert_eq!(t["inputSchema"]["type"], "object");
        assert!(!t["description"].as_str().unwrap().is_empty());
    }
    let create = tools.iter().find(|t| t["name"] == "scene_create").unwrap();
    assert_eq!(create["inputSchema"]["required"], json!(["name"]));
    assert_eq!(
        create["inputSchema"]["properties"]["name"]["type"],
        "string"
    );
}

#[test]
fn create_list_and_current() {
    let fake = Rc::new(Fake::default());
    let mut s = server(&fake);
    assert_eq!(
        call(&mut s, "scenes_list", &json!({})),
        (false, json!({"scenes": []}))
    );
    assert_eq!(
        call(&mut s, "scene_get_current", &json!({})),
        (false, json!({"name": null}))
    );
    assert!(!call(&mut s, "scene_create", &json!({"name": "A"})).0);
    assert!(!call(&mut s, "scene_create", &json!({"name": "B"})).0);
    assert_eq!(
        call(&mut s, "scenes_list", &json!({})),
        (false, json!({"scenes": ["A", "B"]}))
    );
    assert!(!call(&mut s, "scene_set_current", &json!({"name": "B"})).0);
    assert_eq!(
        call(&mut s, "scene_get_current", &json!({})),
        (false, json!({"name": "B"}))
    );
    assert_eq!(fake.current().as_deref(), Some("B"));
}

#[test]
fn remove_scene() {
    let fake = Rc::new(Fake::default());
    let mut s = server(&fake);
    call(&mut s, "scene_create", &json!({"name": "A"}));
    assert!(!call(&mut s, "scene_remove", &json!({"name": "A"})).0);
    assert!(fake.scenes().is_empty());
}

#[test]
fn backend_errors_become_tool_errors() {
    let mut s = server(&Rc::new(Fake::default()));
    let (err, msg) = call(&mut s, "scene_set_current", &json!({"name": "X"}));
    assert!(err);
    assert!(msg.as_str().unwrap().contains("no scene named X"));
    assert!(call(&mut s, "scene_remove", &json!({"name": "X"})).0);
    call(&mut s, "scene_create", &json!({"name": "A"}));
    assert!(call(&mut s, "scene_create", &json!({"name": "A"})).0);
}

#[test]
fn bad_arguments_are_tool_errors() {
    let mut s = server(&Rc::new(Fake::default()));
    for tool in ["scene_set_current", "scene_create", "scene_remove"] {
        for args in [json!({}), json!({"name": 5}), json!({"name": ""})] {
            let (err, msg) = call(&mut s, tool, &args);
            assert!(err, "{tool} {args}");
            assert!(msg.as_str().unwrap().contains("name"), "{tool}: {msg}");
        }
    }
}

#[test]
fn scene_create_documents_that_the_first_scene_becomes_current() {
    let mut s = server(&Rc::default());
    let r = rpc(&mut s, "tools/list", &json!({}));
    let tools = r["result"]["tools"].as_array().unwrap();
    let create = tools.iter().find(|t| t["name"] == "scene_create").unwrap();
    let description = create["description"].as_str().unwrap();
    assert!(
        description.contains("no program scene") && description.contains("current"),
        "{description}"
    );
}
