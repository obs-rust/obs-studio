//! Tier 1 mirrors every assertion in test_set_locale.c through safe host calls.
use obs_core::set_locale::{self, Host};

#[derive(Clone, Debug, Default, PartialEq)]
struct Module {
    next: Option<usize>,
    callback: Option<usize>,
    loaded: bool,
    disabled: bool,
}
#[derive(Default)]
struct Core<'a> {
    owned: Option<Vec<u8>>,
    modules: [Module; 7],
    first: Option<usize>,
    disabled: Option<usize>,
    visits: Vec<usize>,
    expected: Option<&'a [u8]>,
    events: Vec<&'static str>,
}
impl<'a> Host for Core<'a> {
    type Locale = Option<&'a [u8]>;
    type Owned = Option<usize>;
    type Module = usize;
    type Callback = usize;
    fn owned_locale(&mut self) -> Option<Self::Owned> {
        self.owned.as_ref().map(|_| Some(1))
    }
    fn free_locale(&mut self, owned: Self::Owned) {
        assert_eq!(owned, Some(1));
        assert!(self.owned.take().is_some());
        self.events.push("free");
    }
    fn copy_locale(&mut self, locale: Self::Locale) -> Self::Owned {
        assert!(self.owned.is_none(), "free must precede copy");
        self.owned = locale.map(|input| {
            let mut copy = Vec::with_capacity(input.len().max(1));
            copy.extend_from_slice(input);
            copy
        });
        self.events.push("copy");
        locale.map(|_| 1)
    }
    fn store_locale(&mut self, owned: Self::Owned) {
        assert_eq!(owned.is_some(), self.owned.is_some());
        self.events.push("store");
    }
    fn first_module(&mut self) -> Option<usize> {
        self.first
    }
    fn callback(&mut self, module: usize) -> Option<usize> {
        self.modules[module].callback
    }
    fn notify(&mut self, callback: usize, locale: Self::Locale) {
        assert_eq!(locale, self.expected);
        if let Some(input) = locale {
            assert_eq!(input.as_ptr(), self.expected.unwrap().as_ptr());
            assert_ne!(input.as_ptr(), self.owned.as_ref().unwrap().as_ptr());
        }
        assert_eq!(self.owned.as_deref(), self.expected);
        assert_eq!(self.events.last(), Some(&"store"));
        if callback == 2 {
            assert_eq!(self.visits.last(), Some(&1));
        }
        assert!(self.visits.len() < 12);
        assert_ne!(callback, 99, "disabled-list callback must not run");
        self.visits.push(callback);
    }
    fn next_module(&mut self, module: usize) -> Option<usize> {
        self.modules[module].next
    }
}
fn assert_copy(core: &Core<'_>, input: &[u8]) {
    assert_eq!(core.owned.as_deref(), Some(input));
    assert_ne!(core.owned.as_ref().unwrap().as_ptr(), input.as_ptr());
    assert_eq!(usize::from(core.owned.is_some()), 1);
}
#[test]
fn test_set_locale_replacement_owns_one_copy() {
    let mut initial = b"en-US".to_vec();
    let mut replacement = b"a-much-longer-replacement-locale".to_vec();
    let empty = b"";
    let mut core = Core::default();
    set_locale::set_locale(&mut core, Some(&initial));
    assert_copy(&core, &initial);
    let owned = core.owned.take();
    drop(core);
    initial[0] = b'x';
    assert_eq!(owned.as_deref(), Some(b"en-US".as_slice()));
    let mut core = Core {
        owned,
        ..Core::default()
    };
    set_locale::set_locale(&mut core, Some(&replacement));
    assert_copy(&core, &replacement);
    assert_eq!(&core.events, &["free", "copy", "store"]);
    let owned = core.owned.take();
    drop(core);
    replacement[0] = b'x';
    assert_eq!(
        owned.as_deref(),
        Some(b"a-much-longer-replacement-locale".as_slice())
    );
    let mut core = Core {
        owned,
        ..Core::default()
    };
    set_locale::set_locale(&mut core, Some(empty));
    assert_copy(&core, empty);
    set_locale::set_locale(&mut core, None);
    assert!(core.owned.is_none());
    set_locale::set_locale(&mut core, None);
    assert!(core.owned.is_none());
    assert_eq!(
        &core.events,
        &[
            "free", "copy", "store", "free", "copy", "store", "copy", "store"
        ]
    );
}
#[test]
fn test_set_locale_ordered_callbacks_skip_missing() {
    let locale = b"fr-FR";
    let mut core = Core {
        first: Some(5),
        disabled: Some(6),
        expected: Some(locale),
        ..Core::default()
    };
    for (module, next, callback) in [
        (5, Some(2), None),
        (2, Some(3), Some(3)),
        (3, Some(0), None),
        (0, Some(1), Some(1)),
        (1, Some(4), Some(2)),
        (6, None, Some(99)),
    ] {
        core.modules[module] = Module {
            next,
            callback,
            loaded: false,
            disabled: module == 0,
        };
    }
    let before = core.modules.clone();
    for i in 0..2 {
        set_locale::set_locale(&mut core, Some(locale));
        assert_eq!(core.visits.len(), (i + 1) * 3);
        assert_eq!(&core.visits[i * 3..], &[3, 1, 2]);
    }
    core.expected = None;
    set_locale::set_locale(&mut core, None);
    assert_eq!(core.visits.len(), 9);
    assert_eq!(&core.visits[6..], &[3, 1, 2]);
    assert_eq!(core.modules, before);
    assert_eq!(core.first, Some(5));
    assert_eq!(core.disabled, Some(6));
    assert!(core.owned.is_none());
}
#[test]
fn test_set_locale_all_callbacks_missing() {
    let mut core = Core {
        first: Some(0),
        ..Core::default()
    };
    core.modules[0].next = Some(1);
    core.modules[1].next = Some(2);
    set_locale::set_locale(&mut core, Some(b"de-DE"));
    assert_copy(&core, b"de-DE");
    assert!(core.visits.is_empty());
}
