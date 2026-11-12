//! Safe mirrors of test_scene_transform_states.c; no native handles or threads.
use obs_data::data::{Array, Data, Vec2};
use obs_scene::transform_states::{self as engine, Crop, Host, Info, Pass};

#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub name: String,
    pub uuid: String,
    pub kind: u8,
    pub refs: i32,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    pub parent: usize,
    pub source: usize,
    pub id: i64,
    pub selected: bool,
    pub info: Info,
    pub crop: Crop,
    pub deferred: u32,
    pub signals: u32,
}
#[derive(Clone)]
enum Object {
    Data(Data),
    Array(Array),
}
pub struct Model {
    objects: Vec<Option<Object>>,
    pub sources: Vec<Source>,
    pub items: Vec<Item>,
    pub scenes: Vec<Vec<usize>>,
    pub input: Option<Data>,
}
pub fn configured() -> Info {
    Info {
        pos: [37.25, -19.5],
        scale: [-1.25, 2.5],
        rot: 31.5,
        alignment: 10,
        bounds_type: 3,
        bounds_alignment: 5,
        bounds: [333., 222.],
        crop_to_bounds: true,
    }
}
impl Model {
    pub fn new(groups: bool) -> Self {
        let mut m = Self {
            objects: vec![None],
            sources: vec![],
            items: vec![Item::default()],
            scenes: vec![vec![]; 5],
            input: None,
        };
        for (name, kind) in [
            ("", 0),
            ("snapshot scene", 1),
            ("outer", 2),
            ("inner", 2),
            ("snapshot leaf", 0),
        ] {
            let uuid = format!("uuid-{}", m.sources.len());
            m.sources.push(Source {
                name: name.into(),
                uuid,
                kind,
                refs: 1,
            });
        }
        m.add(1, 4);
        m.add(1, 4);
        if groups {
            m.add(1, 2);
            m.add(2, 3);
            m.add(3, 4);
            m.add(2, 4);
        }
        m
    }
    fn add(&mut self, parent: usize, source: usize) {
        let item = self.items.len();
        self.items.push(Item {
            parent,
            source,
            id: item as i64,
            ..Item::default()
        });
        self.scenes[parent].push(item);
    }
    pub fn configure(&mut self, item: usize) {
        self.items[item].info = configured();
        self.items[item].crop = [3, 5, 7, 11];
    }
    fn put(&mut self, object: Object) -> usize {
        self.objects.push(Some(object));
        self.objects.len() - 1
    }
    pub fn data(&self, h: usize) -> Data {
        match self.objects.get(h).and_then(Option::as_ref) {
            Some(Object::Data(d)) => d.clone(),
            _ => Data::new(),
        }
    }
    fn arr(&self, h: usize) -> Array {
        match self.objects.get(h).and_then(Option::as_ref) {
            Some(Object::Array(a)) => a.clone(),
            _ => Array::new(),
        }
    }
    pub fn snapshot(&mut self, all: bool) -> Data {
        let h = engine::save(self, 1, all);
        let d = self.data(h);
        self.release_data(h);
        d
    }
    pub fn restore(&mut self, data: &Data) {
        self.input = Some(data.clone());
        engine::load(self, "snapshot".into());
        self.input = None;
    }
    fn weak_expired(&self, source: usize) -> bool {
        self.sources[source].refs == 0
    }
    fn weak_get_source(&mut self, source: usize) -> Option<usize> {
        if self.sources[source].refs == 0 {
            None
        } else {
            self.sources[source].refs += 1;
            Some(source)
        }
    }
    pub fn live_objects(&self) -> usize {
        self.objects.iter().filter(|o| o.is_some()).count()
    }
    pub fn clear_transforms(&mut self) {
        for item in &mut self.items[1..] {
            item.info = Info::default();
            item.crop = [0; 4];
            item.signals = 0;
        }
    }
}
impl Host for Model {
    type Handle = usize;
    type Text = String;
    fn create_data(&mut self) -> usize {
        self.put(Object::Data(Data::new()))
    }
    fn create_array(&mut self) -> usize {
        self.put(Object::Array(Array::new()))
    }
    fn parse(&mut self, json: String) -> usize {
        let d = if json == "snapshot" {
            self.input.clone()
        } else {
            Data::from_json(&json)
        };
        d.map_or(0, |d| self.put(Object::Data(d)))
    }
    fn release_data(&mut self, h: usize) {
        if h != 0 {
            assert!(self.objects[h].take().is_some());
        }
    }
    fn release_array(&mut self, h: usize) {
        self.release_data(h);
    }
    fn text(&mut self, d: usize, k: &'static str) -> String {
        self.data(d).get_string(k)
    }
    fn set_text(&mut self, d: usize, k: &'static str, v: String) {
        self.data(d).set_string(k, Some(&v));
    }
    fn int(&mut self, d: usize, k: &'static str) -> i64 {
        self.data(d).get_int(k)
    }
    fn set_int(&mut self, d: usize, k: &'static str, v: i64) {
        self.data(d).set_int(k, v);
    }
    fn double(&mut self, d: usize, k: &'static str) -> f64 {
        self.data(d).get_double(k)
    }
    fn set_double(&mut self, d: usize, k: &'static str, v: f64) {
        self.data(d).set_double(k, v);
    }
    fn boolean(&mut self, d: usize, k: &'static str) -> bool {
        self.data(d).get_bool(k)
    }
    fn set_bool(&mut self, d: usize, k: &'static str, v: bool) {
        self.data(d).set_bool(k, v);
    }
    fn vec2(&mut self, d: usize, k: &'static str) -> [f32; 2] {
        let v = self.data(d).get_vec2(k).unwrap_or(Vec2 { x: 0., y: 0. });
        [v.x, v.y]
    }
    fn set_vec2(&mut self, d: usize, k: &'static str, v: [f32; 2]) {
        self.data(d).set_vec2(k, Vec2 { x: v[0], y: v[1] });
    }
    fn array(&mut self, d: usize, k: &'static str) -> usize {
        self.data(d)
            .get_array(k)
            .map_or(0, |a| self.put(Object::Array(a)))
    }
    fn set_array(&mut self, d: usize, k: &'static str, a: usize) {
        self.data(d).set_array(k, Some(&self.arr(a)));
    }
    fn push(&mut self, a: usize, d: usize) {
        self.arr(a).push(Some(&self.data(d)));
    }
    fn enum_scene(&mut self, scene: usize, pass: &mut Pass<usize>) {
        for item in self.scenes[scene].clone() {
            engine::save_item(self, scene, item, pass);
        }
    }
    fn enum_group(&mut self, item: usize, pass: &mut Pass<usize>) {
        self.enum_scene(self.items[item].source, pass);
    }
    fn enum_data(&mut self, a: usize, scene: usize, groups: bool) {
        for d in self.arr(a).elements() {
            let h = self.put(Object::Data(d));
            if groups {
                engine::load_record(self, h);
            } else {
                engine::load_item(self, h, scene);
            }
            self.release_data(h);
        }
    }
    fn source(&mut self, scene: usize) -> usize {
        scene
    }
    fn item_source(&mut self, item: usize) -> usize {
        self.items[item].source
    }
    fn is_group(&mut self, source: usize) -> bool {
        self.sources[source].kind == 2
    }
    fn name(&mut self, source: usize) -> String {
        self.sources[source].name.clone()
    }
    fn uuid(&mut self, source: usize) -> String {
        if source == 0 {
            String::new()
        } else {
            self.sources[source].uuid.clone()
        }
    }
    fn selected(&mut self, item: usize) -> bool {
        self.items[item].selected
    }
    fn id(&mut self, item: usize) -> i64 {
        self.items[item].id
    }
    fn info(&mut self, item: usize) -> Info {
        self.items[item].info
    }
    fn crop(&mut self, item: usize) -> Crop {
        self.items[item].crop
    }
    fn lookup(&mut self, uuid: String) -> usize {
        let h = self
            .sources
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, s)| s.uuid == uuid)
            .map_or(0, |(i, _)| i);
        if h != 0 {
            self.sources[h].refs += 1;
        }
        h
    }
    fn scene(&mut self, source: usize) -> usize {
        if self.sources[source].kind == 1 {
            source
        } else {
            0
        }
    }
    fn group(&mut self, parent: usize, name: String) -> usize {
        self.scenes[parent]
            .iter()
            .copied()
            .find(|&i| {
                self.sources[self.items[i].source].kind == 2
                    && self.sources[self.items[i].source].name == name
            })
            .unwrap_or(0)
    }
    fn group_scene(&mut self, item: usize) -> usize {
        self.items[item].source
    }
    fn release_source(&mut self, source: usize) {
        if source != 0 {
            self.sources[source].refs -= 1;
        }
    }
    fn find(&mut self, scene: usize, id: i64) -> usize {
        self.scenes[scene]
            .iter()
            .copied()
            .find(|&i| self.items[i].id == id)
            .unwrap_or(0)
    }
    fn defer_begin(&mut self, item: usize) {
        if item != 0 {
            self.items[item].deferred += 1;
        }
    }
    fn set_info(&mut self, item: usize, info: Info) {
        if item != 0 {
            self.items[item].info = info;
        }
    }
    fn set_crop(&mut self, item: usize, crop: Crop) {
        if item != 0 {
            self.items[item].crop = crop.map(|v| v.max(0));
        }
    }
    fn defer_end(&mut self, item: usize) {
        if item != 0 {
            self.items[item].deferred -= 1;
            if self.items[item].deferred == 0 {
                self.items[item].signals += 1;
            }
        }
    }
    fn null(&self) -> usize {
        0
    }
}
fn records(d: &Data) -> Array {
    d.get_array("scenes_and_groups").unwrap()
}
fn record(d: &Data, i: usize) -> Data {
    records(d).item(i).unwrap()
}
fn items(d: &Data) -> Array {
    d.get_array("items").unwrap()
}
fn expect_item(d: &Data, item: &Item) {
    assert_eq!(d.get_int("id"), item.id);
    for (key, value) in [
        ("pos", [37.25, -19.5]),
        ("scale", [-1.25, 2.5]),
        ("bounds", [333., 222.]),
    ] {
        let v = d.get_vec2(key).unwrap_or(Vec2 { x: 0., y: 0. });
        assert_eq!([v.x, v.y], value);
    }
    assert_eq!(d.get_double("rot"), 31.5);
    for (key, value) in [
        ("alignment", 10),
        ("bounds_type", 3),
        ("bounds_alignment", 5),
        ("left", 3),
        ("top", 5),
        ("right", 7),
        ("bottom", 11),
    ] {
        assert_eq!(d.get_int(key), value);
    }
    assert!(d.get_bool("crop_to_bounds"));
}
fn expect_transform(item: &Item) {
    assert_eq!(item.info, configured());
    assert_eq!(item.crop, [3, 5, 7, 11]);
}
#[test]
fn c_selected_and_all_snapshots() {
    let mut m = Model::new(false);
    m.configure(1);
    m.configure(2);
    m.items[2].selected = true;
    let d = m.snapshot(false);
    assert_eq!(records(&d).count(), 1);
    let r = record(&d, 0);
    assert_eq!(r.get_string("scene_name"), "snapshot scene");
    assert_eq!(r.get_string("scene_uuid"), m.sources[1].uuid);
    assert!(!r.get_bool("is_group"));
    assert_eq!(items(&r).count(), 1);
    expect_item(&items(&r).item(0).unwrap(), &m.items[2]);
    let d = m.snapshot(true);
    let r = record(&d, 0);
    assert_eq!(items(&r).count(), 2);
    for i in 0..2 {
        expect_item(&items(&r).item(i).unwrap(), &m.items[i + 1]);
    }
    m.items[2].selected = false;
    let d = m.snapshot(false);
    assert_eq!(items(&record(&d, 0)).count(), 0);
    assert_eq!(m.live_objects(), 0);
}
#[test]
fn c_restore_by_uuid_and_defer() {
    let mut m = Model::new(false);
    m.configure(1);
    m.items[1].selected = true;
    let d = m.snapshot(false);
    m.clear_transforms();
    m.sources[1].name = "renamed scene".into();
    m.items[1].selected = false;
    m.defer_begin(1);
    m.restore(&d);
    assert_eq!(m.items[1].signals, 0);
    expect_transform(&m.items[1]);
    m.defer_end(1);
    assert_eq!(m.items[1].signals, 1);
    assert!(!m.items[1].selected);
    assert!(!m.items[2].selected);
    assert_eq!(m.items[2].info.rot, 0.);
    assert_eq!(m.live_objects(), 0);
}
#[test]
fn c_nested_groups_and_parent_resolution() {
    let mut m = Model::new(true);
    m.configure(5);
    m.configure(6);
    m.items[5].selected = true;
    m.items[6].selected = true;
    let d = m.snapshot(false);
    assert_eq!(records(&d).count(), 3);
    let inner = record(&d, 0);
    let outer = record(&d, 1);
    let root = record(&d, 2);
    assert!(inner.get_bool("is_group"));
    assert!(outer.get_bool("is_group"));
    assert!(!root.get_bool("is_group"));
    assert_eq!(inner.get_string("scene_name"), "inner");
    assert_eq!(inner.get_string("scene_uuid"), m.sources[3].uuid);
    assert_eq!(inner.get_string("group_parent"), m.sources[2].uuid);
    assert_eq!(outer.get_string("group_parent"), m.sources[1].uuid);
    assert_eq!(items(&root).count(), 0);
    assert_eq!(items(&outer).count(), 1);
    expect_item(&items(&outer).item(0).unwrap(), &m.items[6]);
    assert_eq!(items(&inner).count(), 1);
    expect_item(&items(&inner).item(0).unwrap(), &m.items[5]);
    let all = m.snapshot(true);
    for i in 0..3 {
        assert_eq!(items(&record(&all, i)).count(), i + 1);
    }
    m.items[5].info = Info::default();
    m.items[6].info = Info::default();
    m.restore(&d);
    expect_transform(&m.items[6]);
    assert_eq!(m.items[5].info.pos, [0.; 2]);
    assert_eq!(m.items[5].info.rot, 0.);
    outer.set_string("scene_uuid", Some("missing group uuid"));
    m.items[6].info = Info::default();
    m.restore(&d);
    expect_transform(&m.items[6]);
    // Drop the fixture owners, then mirror both weak-expiry and failed-upgrade assertions.
    for source in &mut m.sources[1..4] {
        source.refs -= 1;
    }
    for source in 1..4 {
        assert!(m.weak_expired(source));
        assert_eq!(m.weak_get_source(source), None);
    }
    assert_eq!(m.live_objects(), 0);
}
#[test]
fn c_bounds_and_missing_records() {
    let mut m = Model::new(false);
    m.configure(1);
    let d = m.snapshot(true);
    let r = record(&d, 0);
    let saved = items(&r).item(0).unwrap();
    for v in [-1, 7, i64::MIN, i64::MAX] {
        saved.set_int("bounds_type", v);
        m.restore(&d);
        assert_eq!(m.items[1].info.bounds_type, 0);
    }
    for v in 0..=6 {
        saved.set_int("bounds_type", v);
        m.restore(&d);
        assert_eq!(i64::from(m.items[1].info.bounds_type), v);
    }
    m.configure(1);
    saved.set_int("id", i64::MAX);
    m.restore(&d);
    expect_transform(&m.items[1]);
    r.set_string("scene_uuid", Some("missing scene uuid"));
    m.restore(&d);
    expect_transform(&m.items[1]);
    engine::load(&mut m, "{}".into());
    engine::load(&mut m, "invalid json".into());
    expect_transform(&m.items[1]);
    assert_eq!(m.live_objects(), 0);
}

#[test]
fn null_scene_and_malformed_records_are_empty() {
    let mut m = Model::new(false);
    let h = engine::save(&mut m, 0, true);
    let d = m.data(h);
    m.release_data(h);
    let r = record(&d, 0);
    assert_eq!(r.get_string("scene_name"), "");
    assert_eq!(r.get_string("scene_uuid"), "");
    assert_eq!(items(&r).count(), 0);
    for json in [
        "{}",
        "invalid json",
        r#"{"scenes_and_groups":[{}, {"is_group":true}]}"#,
        r#"{"scenes_and_groups":12}"#,
    ] {
        engine::load(&mut m, json.into());
    }
    assert_eq!(m.live_objects(), 0);
    assert!(m.sources.iter().all(|s| s.refs == 1));
}
