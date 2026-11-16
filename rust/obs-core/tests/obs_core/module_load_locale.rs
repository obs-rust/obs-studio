//! Failure and warning ordering for the safe locale orchestration.
use obs_core::module_load_locale::{LocaleLoader, load};

#[derive(Default)]
struct Loader {
    create: bool,
    add: bool,
    events: Vec<(u8, Vec<u8>)>,
}
impl LocaleLoader for Loader {
    type Lookup = ();
    fn create(&mut self, name: &[u8]) -> Option<()> {
        self.events.push((0, name.to_vec()));
        self.create.then_some(())
    }
    fn add(&mut self, _: &mut (), name: &[u8]) -> bool {
        self.events.push((1, name.to_vec()));
        self.add
    }
    fn invalid(&mut self) {
        self.events.push((2, Vec::new()));
    }
    fn failed(&mut self, locale: &[u8]) {
        self.events.push((3, locale.to_vec()));
    }
}

#[test]
fn invalid_parameters_do_not_touch_module_services() {
    for (module, def, locale) in [
        (false, Some(&b"en"[..]), Some(&b"de"[..])),
        (true, None, Some(&b"de"[..])),
        (true, Some(&b"en"[..]), None),
        (true, None, None),
    ] {
        let mut host = Loader::default();
        assert!(load(&mut host, module, def, locale).is_none());
        assert_eq!(host.events, [(2, Vec::new())]);
    }
}

#[test]
fn failed_default_stops_and_failed_overlay_retains_default() {
    let mut host = Loader::default();
    assert!(load(&mut host, true, Some(b"en"), Some(b"de")).is_none());
    assert_eq!(
        host.events,
        [(0, b"locale/en.ini".to_vec()), (3, b"en".to_vec())]
    );
    let mut host = Loader {
        create: true,
        ..Loader::default()
    };
    assert!(load(&mut host, true, Some(b"en"), Some(b"de")).is_some());
    assert_eq!(
        host.events,
        [
            (0, b"locale/en.ini".to_vec()),
            (1, b"/locale/de.ini".to_vec()),
            (3, b"de".to_vec()),
        ]
    );
}
