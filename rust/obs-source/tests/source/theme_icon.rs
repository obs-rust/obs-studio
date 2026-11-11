//! Tier 1 mirrors test_source_theme_icon.c through the safe registry boundary.
use obs_source::theme_icon::{Registry, Theme, get};
use std::{cell::RefCell, rc::Rc};

type Icon = Rc<RefCell<Vec<u8>>>;
#[derive(Clone)]
struct Info {
    id: Vec<u8>,
    icon: u32,
    data: Option<usize>,
    dark: bool,
    light: bool,
}
#[derive(Debug, PartialEq, Eq)]
enum Event {
    Icon,
    Info,
    Call(Theme, Option<usize>),
}
#[derive(Default)]
struct Fixture {
    entries: Vec<Info>,
    query: Option<Vec<u8>>,
    dark: RefCell<Option<Icon>>,
    light: RefCell<Option<Icon>>,
    events: RefCell<Vec<Event>>,
    missing_second: bool,
}
fn bytes(id: &[u8]) -> &[u8] {
    id.split(|b| *b == 0).next().unwrap()
}
impl Fixture {
    fn register(&mut self, id: &[u8], icon: u32, data: Option<usize>, dark: bool, light: bool) {
        if !self.entries.iter().any(|e| e.id == bytes(id)) {
            self.entries.push(Info {
                id: bytes(id).to_vec(),
                icon,
                data,
                dark,
                light,
            });
        }
    }
    fn find(&self) -> Option<Info> {
        self.entries
            .iter()
            .find(|e| Some(e.id.as_slice()) == self.query.as_deref().map(bytes))
            .cloned()
    }
    fn query(&mut self, id: Option<&[u8]>, theme: Theme) -> Option<Icon> {
        self.query = id.map(<[u8]>::to_vec);
        self.events.borrow_mut().clear();
        get(self, theme)
    }
    fn missing(&mut self, id: Option<&[u8]>) {
        for theme in [Theme::Dark, Theme::Light] {
            assert!(self.query(id, theme).is_none());
            assert!(
                !self
                    .events
                    .borrow()
                    .iter()
                    .any(|e| matches!(e, Event::Call(..)))
            );
        }
    }
}
impl Registry for Fixture {
    type Info = Info;
    type Icon = Icon;
    fn icon_type(&self) -> u32 {
        self.events.borrow_mut().push(Event::Icon);
        self.find().map_or(0, |e| e.icon)
    }
    fn source_info(&self) -> Option<Info> {
        self.events.borrow_mut().push(Event::Info);
        (!self.missing_second).then(|| self.find()).flatten()
    }
    fn call(&self, info: Info, theme: Theme) -> Option<Icon> {
        let (enabled, result) = match theme {
            Theme::Dark => (info.dark, &self.dark),
            Theme::Light => (info.light, &self.light),
        };
        if enabled {
            self.events.borrow_mut().push(Event::Call(theme, info.data));
            result.borrow().clone()
        } else {
            None
        }
    }
}
fn icon(data: &[u8]) -> Icon {
    Rc::new(RefCell::new(data.to_vec()))
}

#[test]
fn null_ids_during_empty_registry() {
    Fixture::default().missing(None);
}

#[test]
fn unknown_ids() {
    let mut f = Fixture::default();
    f.missing(Some(b""));
    f.missing(Some(b"unregistered"));
    f.register(b"known", 13, None, false, false);
    for id in [b"known".as_slice(), b"unknown", b""] {
        f.missing(Some(id));
    }
    f.missing_second = true;
    f.missing(Some(b"known"));
    assert_eq!(*f.events.borrow(), [Event::Icon, Event::Info]);
}

#[test]
fn noncustom_skips_callbacks() {
    let mut f = Fixture::default();
    for kind in (0..=14).chain([u32::MAX]) {
        if kind == 13 {
            continue;
        }
        let id = format!("noncustom-{kind}");
        f.register(id.as_bytes(), kind, Some(1), true, true);
        f.missing(Some(id.as_bytes()));
        assert_eq!(*f.events.borrow(), [Event::Icon]);
    }
}

#[test]
fn optional_callbacks_are_independent() {
    let mut f = Fixture::default();
    let dark = icon(b"dark.svg");
    let light = icon(b"light.svg");
    *f.dark.borrow_mut() = Some(dark.clone());
    *f.light.borrow_mut() = Some(light.clone());
    f.register(b"dark-only", 13, Some(1), true, false);
    f.register(b"light-only", 13, Some(2), false, true);
    for (id, theme, expected, data) in [
        (b"dark-only".as_slice(), Theme::Dark, dark, 1),
        (b"light-only", Theme::Light, light, 2),
    ] {
        assert!(Rc::ptr_eq(&f.query(Some(id), theme).unwrap(), &expected));
        assert_eq!(
            *f.events.borrow(),
            [Event::Icon, Event::Info, Event::Call(theme, Some(data))]
        );
    }
    assert!(f.query(Some(b"dark-only"), Theme::Light).is_none());
    assert_eq!(*f.events.borrow(), [Event::Icon, Event::Info]);
    assert!(f.query(Some(b"light-only"), Theme::Dark).is_none());
    assert_eq!(*f.events.borrow(), [Event::Icon, Event::Info]);
}

#[test]
fn exact_type_data_and_borrowed_results() {
    let mut f = Fixture::default();
    f.register(b"with-data", 13, Some(1), true, true);
    f.register(b"null-data", 13, None, true, true);
    let first = icon(b"../Mixed Case.svg");
    let same_text = icon(b"../Mixed Case.svg");
    let results = [
        None,
        Some(icon(b"")),
        Some(first.clone()),
        Some(same_text.clone()),
        Some(icon(&[0xfe, 0x81])),
    ];
    assert!(!Rc::ptr_eq(&first, &same_text));
    assert_eq!(*first.borrow(), *same_text.borrow());
    for (id, data) in [(b"with-data".as_slice(), Some(1)), (b"null-data", None)] {
        for i in 0..results.len() {
            *f.dark.borrow_mut() = results[i].clone();
            *f.light.borrow_mut() = results[4 - i].clone();
            for (theme, expected) in [(Theme::Dark, &results[i]), (Theme::Light, &results[4 - i])] {
                let got = f.query(Some(id), theme);
                match (got, expected) {
                    (Some(got), Some(expected)) => assert!(Rc::ptr_eq(&got, expected)),
                    (None, None) => (),
                    _ => panic!("borrowed result differs"),
                }
                assert_eq!(
                    *f.events.borrow(),
                    [Event::Icon, Event::Info, Event::Call(theme, data)]
                );
            }
        }
    }
    *f.dark.borrow_mut() = Some(first.clone());
    *f.light.borrow_mut() = Some(first.clone());
    let dark = f.query(Some(b"with-data"), Theme::Dark).unwrap();
    let light = f.query(Some(b"with-data"), Theme::Light).unwrap();
    first.borrow_mut()[0] = b'X';
    assert_eq!(dark.borrow()[0], b'X');
    assert_eq!(light.borrow()[0], b'X');
}

#[test]
fn first_registration_and_exact_ids() {
    let mut f = Fixture::default();
    f.register(b"first", 13, Some(1), true, true);
    f.register(b"first", 0, Some(2), true, true);
    f.register(b"first-suffix", 0, Some(2), true, true);
    f.register(b"", 13, Some(2), true, true);
    f.register(&[0xfe, 0x81], 13, Some(3), true, true);
    let result = icon(b"first.svg");
    *f.dark.borrow_mut() = Some(result.clone());
    *f.light.borrow_mut() = Some(result.clone());
    for (id, data) in [
        (b"first\0ignored".as_slice(), 1),
        (b"", 2),
        (&[0xfe, 0x81], 3),
    ] {
        for theme in [Theme::Dark, Theme::Light] {
            assert!(Rc::ptr_eq(&f.query(Some(id), theme).unwrap(), &result));
            assert_eq!(
                *f.events.borrow(),
                [Event::Icon, Event::Info, Event::Call(theme, Some(data))]
            );
        }
    }
    for id in [b"First".as_slice(), b"firs", b"first-suffix"] {
        f.missing(Some(id));
    }
    // The registry boundary has no name/create/destroy operation to invoke.
}
