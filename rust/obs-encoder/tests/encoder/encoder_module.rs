//! Tier 1: 1:1 decisions characterized by test_encoder_module.c.
use obs_encoder::encoder_module::{get_module, load_state};
use obs_util::module_registry::Registry;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Module {
    ids: Vec<Vec<u8>>,
    state: i32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Modules {
    enabled: Vec<Module>,
    disabled: Vec<Module>,
}
impl Registry for Modules {
    type Handle = (bool, usize);
    fn first(&self, disabled: bool) -> Option<Self::Handle> {
        (!self.list(disabled).is_empty()).then_some((disabled, 0))
    }
    fn next(&self, (disabled, i): Self::Handle) -> Option<Self::Handle> {
        (i + 1 < self.list(disabled).len()).then_some((disabled, i + 1))
    }
    fn id_count(&self, (d, i): Self::Handle) -> usize {
        self.list(d)[i].ids.len()
    }
    fn id(&self, (d, i): Self::Handle, n: usize) -> &[u8] {
        &self.list(d)[i].ids[n]
    }
    fn load_state(&self, (d, i): Self::Handle) -> i32 {
        self.list(d)[i].state
    }
}
impl Modules {
    fn list(&self, disabled: bool) -> &[Module] {
        if disabled {
            &self.disabled
        } else {
            &self.enabled
        }
    }
    fn check(&self, id: Option<&[u8]>, expected: Option<(bool, usize)>, state: i32) {
        let before = self.clone();
        for _ in 0..3 {
            assert_eq!(get_module(self, id), expected);
            assert_eq!(load_state(self, id), state);
            assert_eq!(*self, before);
        }
    }
}
fn module(ids: &[&[u8]], state: i32) -> Module {
    Module {
        ids: ids.iter().map(|id| id.to_vec()).collect(),
        state,
    }
}
#[test]
fn test_encoder_module_empty_registry() {
    for modules in [
        Modules::default(),
        Modules {
            enabled: vec![module(&[], 1)],
            disabled: vec![module(&[], 3)],
        },
    ] {
        modules.check(None, None, 2);
        for id in [
            b"unknown-encoder-module".as_slice(),
            b"",
            b"Scene",
            b"groups",
        ] {
            modules.check(Some(id), None, 2);
        }
        for id in [b"scene".as_slice(), b"group"] {
            modules.check(Some(id), None, 2);
        }
    }
}
#[test]
fn test_encoder_module_disabled_states() {
    let modules = Modules {
        enabled: vec![],
        disabled: (0..7)
            .map(|s| {
                module(
                    &[b"unrelated-first-id", &[b'0' + s], b"unrelated-last-id"],
                    i32::from(s),
                )
            })
            .collect(),
    };
    for s in 0..7 {
        modules.check(Some(&[b'0' + s]), Some((true, s as usize)), i32::from(s));
    }

    modules.check(Some(b"not-in-any-module"), None, 2);
}
#[test]
fn test_encoder_module_traversal_and_identity() {
    let mut modules = Modules {
        enabled: vec![],
        disabled: vec![
            module(&[], 5),
            module(&[b"duplicate", b"duplicate"], 3),
            module(&[b"duplicate", b"disabled-tail"], 4),
        ],
    };
    modules.check(Some(b"duplicate"), Some((true, 1)), 3);
    modules.check(Some(b"disabled-tail"), Some((true, 2)), 4);
    modules.enabled = vec![
        module(&[], 1),
        module(&[b"other", b"duplicate", b"duplicate"], 1),
        module(&[b"duplicate", b"enabled-tail"], 1),
    ];
    modules.disabled.insert(0, module(&[b"duplicate"], 6));
    modules.check(Some(b"duplicate"), Some((false, 1)), 1);
    modules.check(Some(b"enabled-tail"), Some((false, 2)), 1);
    modules.check(Some(b"disabled-tail"), Some((true, 3)), 4);
    modules.enabled[1].state = 5;
    modules.check(Some(b"duplicate"), Some((false, 1)), 5);
    for id in [b"Duplicate".as_slice(), b"duplicat"] {
        modules.check(Some(id), None, 2);
    }
}
#[test]
fn test_encoder_module_exact_ids_and_registration_order() {
    let modules = Modules {
        enabled: vec![],
        disabled: vec![module(&[b"", b"x\xff", b"scene", b"group"], 4)],
    };
    for id in [b"".as_slice(), b"x\xff"] {
        modules.check(Some(id), Some((true, 0)), 4);
    }
    for id in [b"scene".as_slice(), b"group"] {
        modules.check(Some(id), Some((true, 0)), 4);
    }
    assert_eq!(
        modules.disabled[0].ids,
        [
            b"".to_vec(),
            b"x\xff".to_vec(),
            b"scene".to_vec(),
            b"group".to_vec()
        ]
    );
    for id in [b"source-only".as_slice(), b"output-only", b"service-only"] {
        modules.check(Some(id), None, 2);
    }
    assert_eq!(modules.disabled[0].state, 4);
}

#[test]
fn empty_lists_do_not_compare_query() {
    struct Unread;
    impl obs_util::module_registry::Query for Unread {
        fn matches(&self, _: &[u8]) -> bool {
            panic!("empty encoder lists must never inspect the query");
        }
    }
    for modules in [
        Modules::default(),
        Modules {
            enabled: vec![module(&[], 1)],
            disabled: vec![module(&[], 3)],
        },
    ] {
        assert_eq!(get_module(&modules, Unread), None);
        assert_eq!(load_state(&modules, Unread), 2);
    }
}
