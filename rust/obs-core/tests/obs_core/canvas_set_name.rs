//! Tier 1: mirrors test_canvas_rules_callers::set_name_rules and
//! test_context_names::test_canvases using the safe orchestration API.
use obs_core::canvas_set_name::{Host, set_name};
use obs_core::{canvas_rules, context_names};

#[derive(Default)]
struct Canvas {
    name: Vec<u8>,
    requested: Option<Vec<u8>>,
    flags: u32,
    private: bool,
    taken: Vec<Vec<u8>>,
    events: Vec<(&'static str, Vec<u8>, Vec<u8>)>,
    trace: Vec<&'static str>,
    table: std::collections::BTreeSet<Vec<u8>>,
    privatize_on_signal: bool,
}
impl Canvas {
    fn new(name: &[u8], private: bool) -> Self {
        Self {
            name: name.to_vec(),
            private,
            table: if private {
                Default::default()
            } else {
                [name.to_vec()].into()
            },
            ..Self::default()
        }
    }
    fn rename(&mut self, name: Option<&[u8]>) {
        self.requested = name.map(<[u8]>::to_vec);
        set_name(self);
    }
}
impl Host for Canvas {
    type Previous = Vec<u8>;
    type Data = (Vec<u8>, Vec<u8>);
    fn requested(&self) -> Option<&[u8]> {
        self.requested.as_deref()
    }
    fn flags(&self) -> u32 {
        self.flags
    }
    fn current(&self) -> &[u8] {
        &self.name
    }
    fn private(&self) -> bool {
        self.private
    }
    fn save_previous(&mut self) -> Vec<u8> {
        self.trace.push("save");
        self.name.clone()
    }
    fn rename_private(&mut self) {
        self.trace.push("private");
        self.name = self.requested.clone().unwrap();
    }
    fn rename_public(&mut self) {
        self.trace.push("public");
        self.table.remove(&self.name);
        let requested = self.requested.as_deref().unwrap();
        self.name = context_names::deduplicate(Some(requested), |candidate| {
            let candidate = match candidate {
                context_names::Lookup::Name(name) => name.unwrap(),
                context_names::Lookup::Candidate(name) => name,
            };
            self.taken.iter().any(|name| name == candidate)
        })
        .unwrap_or_else(|| requested.to_vec());
        self.table.insert(self.name.clone());
    }
    fn data(&mut self, previous: &Vec<u8>) -> Self::Data {
        self.trace.push("data");
        (previous.clone(), self.name.clone())
    }
    fn signal_local(&mut self, data: &mut Self::Data) {
        self.trace.push("local");
        self.events.push(("local", data.0.clone(), data.1.clone()));
        if self.privatize_on_signal {
            self.private = true;
        }
    }
    fn signal_global(&mut self, data: &mut Self::Data) {
        self.trace.push("global");
        self.events.push(("global", data.0.clone(), data.1.clone()));
    }
    fn free_data(&mut self, _: Self::Data) {
        self.trace.push("free data");
    }
    fn free_previous(&mut self, _: Vec<u8>) {
        self.trace.push("free previous");
    }
}

#[test]
fn test_canvas_rules_callers_set_name_rules() {
    let mut public = Canvas::new(b"rules name public", false);
    let mut private = Canvas::new(b"rules name private", true);
    let mut main = Canvas::new(b"Main", false);
    main.flags = canvas_rules::MAIN;
    for name in [
        None,
        Some(b"".as_slice()),
        Some(b"rules name public".as_slice()),
    ] {
        public.rename(name);
    }
    main.rename(Some(b"rules name not main"));
    assert_eq!(public.name, b"rules name public");
    assert_eq!(main.name, b"Main");
    assert!(public.events.is_empty());
    assert!(main.events.is_empty());
    assert!(public.trace.is_empty());
    public.rename(Some(b"rules name public 2"));
    assert_eq!(public.name, b"rules name public 2");
    assert_eq!(public.events.len(), 2);
    assert_eq!(
        public.events[0],
        (
            "local",
            b"rules name public".to_vec(),
            b"rules name public 2".to_vec()
        )
    );
    assert_eq!(public.events[1].0, "global");
    assert!(public.table.contains(b"rules name public 2".as_slice()));
    assert!(!public.table.contains(b"rules name public".as_slice()));
    assert_eq!(
        public.trace,
        [
            "save",
            "public",
            "data",
            "local",
            "global",
            "free data",
            "free previous"
        ]
    );
    private.rename(Some(b"rules name private 2"));
    assert_eq!(private.name, b"rules name private 2");
    assert_eq!(private.events.len(), 1);
    assert_eq!(private.events[0].0, "local");
    assert!(!private.table.contains(b"rules name private 2".as_slice()));
    assert_eq!(
        private.trace,
        [
            "save",
            "private",
            "data",
            "local",
            "free data",
            "free previous"
        ]
    );
    public.flags |= canvas_rules::REMOVED;
    public.rename(Some(b"rules name public 3"));
    assert_eq!(public.name, b"rules name public 3");
    assert_eq!(public.events.iter().filter(|e| e.0 == "local").count(), 2);
}

#[test]
fn test_context_names_test_canvases() {
    // Constructor assertions from test_canvases: use the already ported
    // context naming core for the setup outside this setter's responsibility.
    let mut names = vec![b"Main".to_vec()];
    let mut create = |requested: &[u8], private: bool| {
        let name = if private {
            requested.to_vec()
        } else {
            context_names::deduplicate(Some(requested), |lookup| {
                let candidate = match lookup {
                    context_names::Lookup::Name(name) => name.unwrap(),
                    context_names::Lookup::Candidate(name) => name,
                };
                names.iter().any(|name| name == candidate)
            })
            .unwrap_or_else(|| requested.to_vec())
        };
        if !private {
            names.push(name.clone());
        }
        Canvas::new(&name, private)
    };
    let c1 = create(b"Main", false);
    let c2 = create(b"K", false);
    let mut canvas = create(b"K", false);
    let private = create(b"K", true);
    assert_eq!(c1.name, b"Main 2");
    assert_eq!(c2.name, b"K");
    assert_eq!(canvas.name, b"K 2");
    assert_eq!(private.name, b"K");
    canvas.taken = [b"Main".to_vec(), b"Main 2".to_vec(), b"K".to_vec()].to_vec();
    for name in [None, Some(b"".as_slice()), Some(b"K 2".as_slice())] {
        canvas.rename(name);
        assert_eq!(canvas.name, b"K 2");
    }
    canvas.rename(Some(b"K"));
    assert_eq!(canvas.name, b"K 2");
    assert_eq!(canvas.events.len(), 2); // request differs even if resolved name stays the same
    canvas.rename(Some(b"Main"));
    assert_eq!(canvas.name, b"Main 3");
    assert_eq!(canvas.events[2].2, b"Main 3");
    assert!(canvas.table.contains(b"Main 3".as_slice()));
}

#[test]
fn local_callback_controls_later_global_signal_and_bytes_are_preserved() {
    let mut canvas = Canvas::new(b"old", false);
    canvas.privatize_on_signal = true;
    canvas.rename(Some(b"\xff%name"));
    assert_eq!(canvas.name, b"\xff%name");
    assert_eq!(canvas.events.len(), 1);
    assert_eq!(canvas.trace.last(), Some(&"free previous"));
}
