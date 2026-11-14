//! Tier 1: test_source_set_name.c ported 1:1 over a model source, plus the
//! NULL, empty and unchanged-name edges and the order of every read and call.
//!
//! The model stands in for what stays in C, as the C test sees it through
//! libobs: the public source table, the canvas's source table, and
//! `obs_canvas_rename_source()`, which renames through the canvas table and
//! signals the source's `rename`, the canvas's `source_rename` and, on the
//! main canvas only, the core `source_rename`. Every handler checks the
//! payload and that the new name is already published, as `renamed()` does.
use obs_source::set_name::{Source, set_name};
use std::cell::RefCell;

type Name = Option<Vec<u8>>;

#[derive(Clone, Debug, PartialEq)]
enum Op {
    Requested,
    Current,
    Flags,
    Private,
    Canvas(Name),
    Save(Name),
    Public(Name),
    PrivateTable(Name),
    Data { new: Name, prev: Name },
    Global { new: Name, prev: Name },
    Local { new: Name, prev: Name },
    FreeData,
    FreePrevious(Name),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Canvas {
    /// An input: no `OBS_SOURCE_REQUIRES_CANVAS`.
    None,
    /// A scene on the main canvas, whose renames reach the core handler.
    Main,
    /// A scene on another canvas, whose renames stay on the canvas.
    Other,
}

/// One entry of a name table: whether it is the model source, and its name.
type Table = Vec<(bool, Vec<u8>)>;

/// One signal as its handler saw it: `G` core, `L` source, `C` canvas.
#[derive(Clone, Debug, PartialEq)]
struct Event {
    handler: char,
    new: Name,
    prev: Name,
    published: Name,
}

struct Model {
    requested: Name,
    name: Name,
    canvas: Canvas,
    private: bool,
    public: Table,
    canvas_sources: Table,
    events: Vec<Event>,
    log: RefCell<Vec<Op>>,
}

fn name(text: &str) -> Name {
    Some(text.as_bytes().to_vec())
}

fn rename_in(table: &mut Table, to: &Name) {
    for entry in table.iter_mut().filter(|entry| entry.0) {
        entry.1 = to.clone().unwrap_or_default();
    }
}

/// `obs_get_source_by_name()` or `obs_canvas_get_scene_by_name()`: whether
/// the name is found, and whether it is the model source.
fn lookup(table: &Table, text: &str) -> Option<bool> {
    table
        .iter()
        .find(|entry| entry.1 == text.as_bytes())
        .map(|entry| entry.0)
}

impl Model {
    /// A source as libobs creates it: a public input or scene is in the
    /// public table, and a public scene is in its canvas's table too.
    fn new(current: &str, canvas: Canvas, private: bool) -> Self {
        let listed = |yes: bool| {
            if yes {
                vec![(true, current.as_bytes().to_vec())]
            } else {
                Vec::new()
            }
        };
        Self {
            requested: None,
            name: name(current),
            canvas,
            private,
            public: listed(!private),
            canvas_sources: listed(!private && canvas != Canvas::None),
            events: Vec::new(),
            log: RefCell::new(Vec::new()),
        }
    }

    fn rename(&mut self, requested: Name) -> Vec<Op> {
        self.requested = requested;
        self.log.borrow_mut().clear();
        set_name(Some(self), || panic!("valid source"));
        self.log.borrow().clone()
    }

    fn push(&self, op: Op) {
        self.log.borrow_mut().push(op);
    }

    fn signal(&mut self, handler: char, new: &Name, prev: &Name) {
        self.events.push(Event {
            handler,
            new: new.clone(),
            prev: prev.clone(),
            published: self.name.clone(),
        });
    }

    fn handlers(&self) -> String {
        self.events.iter().map(|event| event.handler).collect()
    }
}

impl Source for Model {
    type Previous = Name;
    type Data = (Name, Name);

    fn requested(&self) -> Option<&[u8]> {
        self.push(Op::Requested);
        self.requested.as_deref()
    }
    fn current(&self) -> Option<&[u8]> {
        self.push(Op::Current);
        self.name.as_deref()
    }
    fn requires_canvas(&self) -> bool {
        self.push(Op::Flags);
        self.canvas != Canvas::None
    }
    fn private(&self) -> bool {
        self.push(Op::Private);
        self.private
    }
    fn rename_canvas(&mut self) {
        self.push(Op::Canvas(self.requested.clone()));
        // obs_canvas_rename_source(): its own copy, table rename and signals.
        let prev = self.name.clone();
        self.name.clone_from(&self.requested);
        let new = self.name.clone();
        rename_in(&mut self.canvas_sources, &new);
        self.signal('L', &new, &prev);
        self.signal('C', &new, &prev);
        if self.canvas == Canvas::Main {
            self.signal('G', &new, &prev);
        }
    }
    fn save_previous(&mut self) -> Name {
        self.push(Op::Save(self.name.clone()));
        self.name.clone()
    }
    fn rename_public(&mut self) {
        self.push(Op::Public(self.requested.clone()));
        self.name.clone_from(&self.requested);
        let new = self.name.clone();
        rename_in(&mut self.public, &new);
    }
    fn rename_private(&mut self) {
        self.push(Op::PrivateTable(self.requested.clone()));
        self.name.clone_from(&self.requested);
    }
    fn data(&mut self, previous: &Name) -> (Name, Name) {
        let data = (self.name.clone(), previous.clone());
        self.push(Op::Data {
            new: data.0.clone(),
            prev: data.1.clone(),
        });
        data
    }
    fn signal_global(&mut self, data: &mut (Name, Name)) {
        self.push(Op::Global {
            new: data.0.clone(),
            prev: data.1.clone(),
        });
        self.signal('G', &data.0, &data.1);
    }
    fn signal_local(&mut self, data: &mut (Name, Name)) {
        self.push(Op::Local {
            new: data.0.clone(),
            prev: data.1.clone(),
        });
        self.signal('L', &data.0, &data.1);
    }
    fn free_data(&mut self, _data: (Name, Name)) {
        self.push(Op::FreeData);
    }
    fn free_previous(&mut self, previous: Name) {
        self.push(Op::FreePrevious(previous));
    }
}

fn table_path(private: bool, canvas: bool, new: &str, prev: &str) -> Vec<Op> {
    let (new, prev) = (name(new), name(prev));
    let mut ops = vec![Op::Requested, Op::Current, Op::Flags];
    if canvas {
        ops.push(Op::Private);
    }
    ops.push(Op::Save(prev.clone()));
    ops.push(Op::Private);
    ops.push(if private {
        Op::PrivateTable(new.clone())
    } else {
        Op::Public(new.clone())
    });
    ops.push(Op::Data {
        new: new.clone(),
        prev: prev.clone(),
    });
    ops.push(Op::Private);
    if !private {
        ops.push(Op::Global {
            new: new.clone(),
            prev: prev.clone(),
        });
    }
    ops.push(Op::Local {
        new,
        prev: prev.clone(),
    });
    ops.push(Op::FreeData);
    ops.push(Op::FreePrevious(prev));
    ops
}

const CANVAS_PATH: [Op; 4] = [Op::Requested, Op::Current, Op::Flags, Op::Private];

/// check_rename(): renaming to the current name signals nothing; renaming to
/// "after" signals `events` in order, each handler seeing the source, both
/// names and the new name already published; then neither an equal string
/// nor the current name itself signals again. `renamed` is the call order.
fn check_rename(source: &mut Model, events: &str, renamed: &[Op]) {
    let unchanged = [Op::Requested, Op::Current];
    assert_eq!(source.rename(name("before")), unchanged);
    assert_eq!(source.handlers(), "");
    assert_eq!(source.rename(name("after")), renamed);
    assert_eq!(source.handlers(), events);
    for event in &source.events {
        assert_eq!(event.prev, name("before"));
        assert_eq!(event.new, name("after"));
        assert_eq!(event.published, name("after"));
    }
    assert_eq!(source.name, name("after"));
    let count = source.events.len();
    assert_eq!(source.rename(name("after")), unchanged);
    let current = source.name.clone();
    assert_eq!(source.rename(current), unchanged);
    assert_eq!(source.events.len(), count);
}

#[test]
fn test_source_set_name_public_input_order() {
    let mut source = Model::new("before", Canvas::None, false);
    check_rename(
        &mut source,
        "GL",
        &table_path(false, false, "after", "before"),
    );
    assert_eq!(lookup(&source.public, "before"), None);
    assert_eq!(lookup(&source.public, "after"), Some(true));
}

#[test]
fn test_source_set_name_private_input_order() {
    let mut source = Model::new("before", Canvas::None, true);
    check_rename(
        &mut source,
        "L",
        &table_path(true, false, "after", "before"),
    );
    assert_eq!(lookup(&source.public, "after"), None);
}

#[test]
fn test_source_set_name_main_canvas_order() {
    let mut source = Model::new("before", Canvas::Main, false);
    let mut renamed = CANVAS_PATH.to_vec();
    renamed.push(Op::Canvas(name("after")));
    check_rename(&mut source, "LCG", &renamed);
    assert_eq!(lookup(&source.canvas_sources, "before"), None);
    assert_eq!(lookup(&source.canvas_sources, "after"), Some(true));
}

#[test]
fn test_source_set_name_other_canvas_order() {
    let mut source = Model::new("before", Canvas::Other, false);
    let mut renamed = CANVAS_PATH.to_vec();
    renamed.push(Op::Canvas(name("after")));
    check_rename(&mut source, "LC", &renamed);
}

#[test]
fn test_source_set_name_private_scene_order() {
    // A public scene already holds "after" on the main canvas.
    let mut source = Model::new("before", Canvas::Main, true);
    source.canvas_sources.push((false, b"after".to_vec()));
    check_rename(&mut source, "L", &table_path(true, true, "after", "before"));
    // A private scene has a canvas but must leave its public table intact.
    assert_eq!(lookup(&source.canvas_sources, "after"), Some(false));
}

#[test]
fn null_and_empty_names_take_the_rename_path() {
    for private in [false, true] {
        let mut source = Model::new("before", Canvas::None, private);
        let ops = source.rename(None);
        assert_eq!(ops[..2], [Op::Requested, Op::Flags]);
        assert!(ops.contains(&Op::Local {
            new: None,
            prev: name("before")
        }));
        assert_eq!(source.name, None);

        let mut source = Model::new("before", Canvas::None, private);
        let ops = source.rename(name(""));
        assert_eq!(ops[..2], [Op::Requested, Op::Flags]);
        assert!(ops.contains(&Op::Local {
            new: name(""),
            prev: name("before")
        }));
    }
}

#[test]
fn an_unnamed_source_always_renames() {
    let mut source = Model::new("", Canvas::None, false);
    source.name = None;
    let ops = source.rename(name("after"));
    assert_eq!(ops[..3], [Op::Requested, Op::Current, Op::Flags]);
    assert_eq!(ops[3], Op::Save(None));
    assert!(ops.contains(&Op::Global {
        new: name("after"),
        prev: None
    }));
    // A NULL request on a NULL name renames too.
    let mut source = Model::new("", Canvas::Main, false);
    source.name = None;
    assert_eq!(
        source.rename(None),
        [Op::Requested, Op::Flags, Op::Private, Op::Canvas(None)]
    );
}

#[test]
fn non_utf8_names_compare_as_bytes() {
    let mut source = Model::new("", Canvas::None, false);
    source.name = Some(vec![0xff, 0xfe]);
    assert_eq!(
        source.rename(Some(vec![0xff, 0xfe])),
        [Op::Requested, Op::Current]
    );
    let ops = source.rename(Some(vec![0xff]));
    assert!(ops.contains(&Op::Public(Some(vec![0xff]))));
}

#[test]
fn null_source_logs_once_and_does_nothing() {
    let mut logs = 0;
    set_name(None::<&mut Model>, || logs += 1);
    assert_eq!(logs, 1);
}
