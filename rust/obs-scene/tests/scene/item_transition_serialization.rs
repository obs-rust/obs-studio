//! Tier 1 mirrors the two C characterization files using safe owned handles.
use obs_scene::item_transition_serialization::{self as core, Host};
use std::collections::BTreeMap;
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Data {
    pub text: BTreeMap<&'static str, String>,
    pub duration: i64,
    pub object: usize,
    pub refs: i32,
}
#[derive(Clone, Debug)]
pub struct Source {
    pub id: String,
    pub versioned: String,
    pub name: String,
    pub settings: usize,
    pub refs: i32,
}
#[derive(Default)]
pub struct Model {
    pub data: Vec<Data>,
    pub sources: Vec<Source>,
    pub slots: [Option<usize>; 2],
    pub durations: [u32; 2],
    pub strings: Vec<String>,
    pub created: usize,
    pub destroyed: usize,
}
impl Model {
    pub fn new() -> Self {
        Self {
            data: vec![Data::default()],
            ..Self::default()
        }
    }
    pub fn string(&mut self, value: &str) -> usize {
        self.strings.push(value.into());
        self.strings.len() - 1
    }
    pub fn source(&mut self, id: &str, versioned: &str, name: &str, settings: usize) -> usize {
        self.sources.push(Source {
            id: id.into(),
            versioned: versioned.into(),
            name: name.into(),
            settings,
            refs: 1,
        });
        self.created += 1;
        self.sources.len() - 1
    }
}
impl Host for Model {
    type Item = ();
    type Data = usize;
    type Source = usize;
    type Text = usize;
    fn create_data(&mut self) -> usize {
        self.data.push(Data {
            refs: 1,
            ..Data::default()
        });
        self.data.len() - 1
    }
    fn text(&mut self, data: usize, key: &'static str) -> usize {
        let value = self.data[data].text.get(key).cloned().unwrap_or_default();
        self.string(&value)
    }
    fn nonempty(&self, text: usize) -> bool {
        !self.strings[text].is_empty()
    }
    fn object(&mut self, data: usize, _: &'static str) -> usize {
        let object = self.data[data].object;
        self.data[object].refs += 1;
        object
    }
    fn integer(&mut self, data: usize, _: &'static str) -> i64 {
        self.data[data].duration
    }
    fn create_source(&mut self, id: usize, name: usize, settings: usize) -> usize {
        let id = self.strings[id].clone();
        let name = self.strings[name].clone();
        self.source(&id, &id, &name, settings)
    }
    fn release_source(&mut self, source: usize) {
        self.sources[source].refs -= 1;
        if self.sources[source].refs == 0 {
            self.destroyed += 1;
        }
    }
    fn release_data(&mut self, data: usize) {
        self.data[data].refs -= 1;
    }
    fn set_transition(&mut self, _: (), show: bool, source: Option<usize>) {
        if let Some(old) = self.slots[show as usize] {
            self.release_source(old);
        }
        if let Some(source) = source {
            self.sources[source].refs += 1;
        }
        self.slots[show as usize] = source;
    }
    fn set_duration(&mut self, _: (), show: bool, duration: u32) {
        self.durations[show as usize] = duration;
    }
    fn transition(&mut self, _: (), show: bool) -> Option<usize> {
        self.slots[show as usize]
    }
    fn duration(&mut self, _: (), show: bool) -> u32 {
        self.durations[show as usize]
    }
    fn source_text(&mut self, source: usize, key: &'static str) -> usize {
        let source = &self.sources[source];
        let value = match key {
            "id" => &source.id,
            "versioned_id" => &source.versioned,
            "name" => &source.name,
            _ => unreachable!(),
        }
        .clone();
        self.string(&value)
    }
    fn settings(&mut self, source: usize) -> usize {
        let settings = self.sources[source].settings;
        self.data[settings].refs += 1;
        settings
    }
    fn set_text(&mut self, data: usize, key: &'static str, text: usize) {
        self.data[data].text.insert(key, self.strings[text].clone());
    }
    fn set_object(&mut self, data: usize, _: &'static str, object: usize) {
        self.data[data].object = object;
        self.data[object].refs += 1;
    }
    fn set_integer(&mut self, data: usize, _: &'static str, integer: i64) {
        self.data[data].duration = integer;
    }
}
#[test]
fn c_empty_slots() {
    let mut m = Model::new();
    m.durations = [u32::MAX, 123];
    for show in [false, true] {
        let d = core::save(&mut m, (), show);
        assert_eq!(m.data[d].duration, i64::from(m.durations[show as usize]));
        for key in ["id", "versioned_id", "name", "transition"] {
            assert!(!m.data[d].text.contains_key(key));
        }
        assert_eq!(m.data[d].object, 0);
    }
}
#[test]
fn c_load_guards_and_durations() {
    let mut m = Model::new();
    m.durations = [22, 11];
    core::load(&mut m, None, None, false);
    core::load(&mut m, Some(()), None, true);
    assert_eq!(m.durations, [22, 11]);
    let d = m.create_data();
    core::load(&mut m, None, Some(d), true);
    assert_eq!(m.durations, [22, 11]);
    for show in [false, true] {
        for duration in [
            0,
            1,
            -1,
            i64::from(u32::MAX),
            i64::from(u32::MAX) + 2,
            i64::MIN,
            i64::MAX,
        ] {
            m.durations = [22, 11];
            m.data[d].duration = duration;
            core::load(&mut m, Some(()), Some(d), show);
            assert_eq!(m.durations[show as usize], duration as u32);
            assert_eq!(m.durations[!show as usize], if show { 22 } else { 11 });
            assert_eq!(m.slots, [None, None]);
        }
    }
    m.data[d].duration = 0;
    core::load(&mut m, Some(()), Some(d), false);
    assert_eq!(m.durations[0], 0);
}
#[test]
fn c_saved_identity_and_settings() {
    let mut m = Model::new();
    for (show, id, versioned, name, marker, duration) in [
        (
            true,
            "show",
            "show_v2",
            "Show transition",
            "show settings",
            125,
        ),
        (
            false,
            "hide",
            "hide_v3",
            "Hide transition",
            "hide settings",
            750,
        ),
    ] {
        let settings = m.create_data();
        m.data[settings].text.insert("marker", marker.into());
        let source = m.source(id, versioned, name, settings);
        m.slots[show as usize] = Some(source);
        m.durations[show as usize] = duration;
        let d = core::save(&mut m, (), show);
        for (key, value) in [("id", id), ("versioned_id", versioned), ("name", name)] {
            assert_eq!(m.data[d].text[key], value);
        }
        assert_eq!(m.data[d].duration, i64::from(duration));
        assert_eq!(m.data[d].object, settings);
        assert_eq!(m.data[settings].text["marker"], marker);
        assert_eq!(m.data[settings].refs, 2); // original plus saved object
        assert_eq!(m.sources[source].refs, 1);
    }
}
#[test]
fn c_roundtrip_replace_clear() {
    let mut m = Model::new();
    let settings = m.create_data();
    m.data[settings].text.insert("marker", "kept".into());
    for show in [false, true] {
        let start = m.created;
        let source = m.source(
            "serialization_fixture",
            "serialization_fixture",
            "Original",
            settings,
        );
        m.set_transition((), show, Some(source));
        m.set_duration((), show, 777);
        m.release_source(source);
        let saved = core::save(&mut m, (), show);
        assert_eq!(m.data[saved].text["id"], "serialization_fixture");
        assert_eq!(m.data[saved].text["name"], "Original");
        m.data[saved]
            .text
            .insert("versioned_id", "unused_wrong_id".into());
        core::load(&mut m, Some(()), Some(saved), show);
        assert_eq!(m.sources[source].refs, 0);
        let loaded = m.slots[show as usize].unwrap();
        assert_eq!(m.sources[loaded].refs, 1);
        assert_eq!(m.sources[loaded].id, "serialization_fixture");
        assert_eq!(m.sources[loaded].name, "Original");
        assert_eq!(m.data[m.sources[loaded].settings].text["marker"], "kept");
        assert_eq!(m.durations[show as usize], 777);
        assert_eq!(m.slots[!show as usize], None);
        let empty = m.create_data();
        core::load(&mut m, Some(()), Some(empty), show);
        assert_eq!(m.slots[show as usize], None);
        assert_eq!(m.sources[loaded].refs, 0);
        assert_eq!(m.durations[show as usize], 0);
        assert_eq!(m.created, start + 2);
        assert_eq!(m.destroyed, m.created);
    }
    assert_eq!(m.data[settings].text["marker"], "kept");
}
