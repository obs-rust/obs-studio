//! Tier 1: safe counterpart of test_hotkey_pair_meta.c.
use obs_hotkey::pair_meta::{Field, Registry, set};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    id: usize,
    partner: usize,
    fields: [Option<Vec<u8>>; 2],
}
struct World {
    pairs: Vec<(usize, [usize; 2])>,
    keys: Vec<Key>,
    calls: Vec<(usize, Option<Vec<u8>>, Field)>,
    redirect: bool,
}
impl World {
    fn new() -> Self {
        Self {
            pairs: vec![(9, [0, 1]), (10, [2, 3])],
            keys: (0..5)
                .map(|id| Key {
                    id,
                    partner: if id < 4 { id ^ 1 } else { usize::MAX },
                    fields: [
                        Some(format!("name.{id}").into_bytes()),
                        Some(format!("Desc {id}").into_bytes()),
                    ],
                })
                .collect(),
            calls: vec![],
            redirect: false,
        }
    }
    fn check(&self, names: [Option<&[u8]>; 2], descriptions: [Option<&[u8]>; 2]) {
        for half in 0..2 {
            assert_eq!(self.keys[half].fields[0].as_deref(), names[half]);
            assert_eq!(self.keys[half].fields[1].as_deref(), descriptions[half]);
            assert_eq!(self.keys[half].id, half);
            assert_eq!(self.keys[half].partner, half ^ 1);
        }
        assert_eq!(&self.keys[2..], &Self::new().keys[2..]);
    }
}
impl Registry for World {
    type Pair = usize;
    type Value = Option<&'static [u8]>;
    fn find(&mut self, id: usize) -> Option<usize> {
        self.pairs.iter().position(|p| p.0 == id)
    }
    fn hotkey_id(&self, pair: &usize, half: usize) -> usize {
        self.pairs[*pair].1[half]
    }
    fn set(&mut self, id: usize, value: Self::Value, field: Field) {
        let value = value.map(|v| {
            v.iter()
                .copied()
                .take_while(|b| *b != 0)
                .collect::<Vec<_>>()
        });
        self.calls.push((id, value.clone(), field));
        if let Some(key) = self.keys.iter_mut().find(|k| k.id == id) {
            key.fields[usize::from(field == Field::Description)] = value;
        }
        if self.redirect && id == 0 {
            self.pairs[0].1[1] = 2;
        }
    }
}

#[test]
fn test_hotkey_pair_meta_unknown_deleted_and_missing_halves() {
    let mut w = World::new();
    let initial = w.keys.clone();
    for id in [0, 11, usize::MAX] {
        set(&mut w, id, [None, Some(b"unused")], Field::Name);
        set(&mut w, id, [Some(b"unused"), None], Field::Description);
    }
    assert!(w.calls.is_empty());
    assert_eq!(w.keys, initial);
    w.pairs.clear();
    set(&mut w, 9, [None, None], Field::Name);
    assert!(w.calls.is_empty());
    for missing in [vec![0], vec![1], vec![0, 1]] {
        let mut w = World::new();
        w.keys.retain(|k| !missing.contains(&k.id));
        set(
            &mut w,
            9,
            [Some(b"surviving.0"), Some(b"surviving.1")],
            Field::Name,
        );
        set(
            &mut w,
            9,
            [Some(b"Surviving 0"), Some(b"Surviving 1")],
            Field::Description,
        );
        for key in w.keys.iter().filter(|k| k.id < 2) {
            assert_eq!(
                key.fields[0].as_deref(),
                Some(if key.id == 0 {
                    b"surviving.0".as_slice()
                } else {
                    b"surviving.1"
                })
            );
            assert_eq!(
                key.fields[1].as_deref(),
                Some(if key.id == 0 {
                    b"Surviving 0".as_slice()
                } else {
                    b"Surviving 1"
                })
            );
        }
        set(&mut w, 9, [None, None], Field::Name);
        set(&mut w, 9, [None, None], Field::Description);
        for key in w.keys.iter().filter(|k| k.id < 2) {
            assert_eq!(key.fields, [None, None]);
        }
        assert_eq!(
            w.calls.iter().map(|c| c.0).collect::<Vec<_>>(),
            [0, 1, 0, 1, 0, 1, 0, 1]
        );
    }
}

#[test]
fn test_hotkey_pair_meta_fields_null_empty_raw_and_copied_ownership() {
    let mut w = World::new();
    let desc_ptrs = [
        w.keys[0].fields[1].as_ref().unwrap().as_ptr(),
        w.keys[1].fields[1].as_ref().unwrap().as_ptr(),
    ];
    set(&mut w, 9, [Some(b"new.0"), Some(b"new.1")], Field::Name);
    w.check(
        [Some(b"new.0"), Some(b"new.1")],
        [Some(b"Desc 0"), Some(b"Desc 1")],
    );
    for (half, ptr) in desc_ptrs.into_iter().enumerate() {
        assert_eq!(w.keys[half].fields[1].as_ref().unwrap().as_ptr(), ptr);
    }
    let name_ptrs = [
        w.keys[0].fields[0].as_ref().unwrap().as_ptr(),
        w.keys[1].fields[0].as_ref().unwrap().as_ptr(),
    ];
    set(
        &mut w,
        9,
        [Some(b"New 0"), Some(b"New 1")],
        Field::Description,
    );
    for (half, ptr) in name_ptrs.into_iter().enumerate() {
        assert_eq!(w.keys[half].fields[0].as_ref().unwrap().as_ptr(), ptr);
    }
    for field in [Field::Name, Field::Description] {
        for values in [
            [None, Some(b"".as_slice())],
            [Some(b"raw\xff\x80\t"), None],
            [None, None],
            [Some(b"shared\0ignored"), Some(b"shared\0ignored")],
        ] {
            set(&mut w, 9, values, field);
            let index = usize::from(field == Field::Description);
            for (half, value) in values.into_iter().enumerate() {
                let expected = value.map(|v| v.split(|b| *b == 0).next().unwrap());
                assert_eq!(w.keys[half].fields[index].as_deref(), expected);
                if let Some(input) = value
                    && !input.is_empty()
                {
                    assert_ne!(
                        w.keys[half].fields[index].as_ref().unwrap().as_ptr(),
                        input.as_ptr()
                    );
                }
            }
        }
        let index = usize::from(field == Field::Description);
        assert_ne!(
            w.keys[0].fields[index].as_ref().unwrap().as_ptr(),
            w.keys[1].fields[index].as_ref().unwrap().as_ptr()
        );
    }
    w.check(
        [Some(b"shared"), Some(b"shared")],
        [Some(b"shared"), Some(b"shared")],
    );
}

#[test]
fn second_id_is_read_after_the_first_setter() {
    let mut w = World::new();
    w.redirect = true;
    set(&mut w, 9, [Some(b"first"), Some(b"second")], Field::Name);
    assert_eq!(w.calls.iter().map(|c| c.0).collect::<Vec<_>>(), [0, 2]);
}
