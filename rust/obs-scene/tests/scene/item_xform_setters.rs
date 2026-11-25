//! Tier 1 mirrors test_sceneitem_xform_setters.c, including unchanged writes.
use obs_graphics::vec2::Vec2;
use obs_scene::{
    coords,
    item_bounds_setters::Fields,
    item_xform_setters::{Item, Value, set},
};

struct Model {
    fields: ([f32; 2], f32, [f32; 2]),
    input: Value,
    absolute: bool,
    keep_scale: bool,
    path: u8,
    defer: i32,
    dirty: bool,
    calls: Vec<&'static str>,
    signals: u32,
    widths: u32,
    heights: u32,
}
impl Model {
    fn new(input: Value) -> Self {
        Self {
            fields: ([0.; 2], 0., [1.; 2]),
            input,
            absolute: true,
            keep_scale: false,
            path: 2,
            defer: 0,
            dirty: false,
            calls: vec![],
            signals: 0,
            widths: 0,
            heights: 0,
        }
    }
}
impl Fields for Model {
    fn lock(&mut self) {
        assert!(self.calls.is_empty());
        self.calls.push("lock");
    }
    fn unlock(&mut self) {
        assert_eq!(self.calls.last(), Some(&"store"));
        self.calls.push("unlock");
    }
    fn update(&mut self) {
        assert_eq!(self.calls.last(), Some(&"unlock"));
        self.calls.push("update");
        if self.path != 2 {
            self.dirty = true;
        } else if self.defer <= 0 {
            self.signals += 1;
            self.widths += 1;
            self.heights += 1;
        }
    }
}
impl Item for Model {
    fn input(&self) -> Value {
        assert_eq!(self.calls, ["lock"]);
        self.input
    }
    fn absolute(&self) -> bool {
        self.absolute
    }
    fn relative(&mut self, value: Value) -> Value {
        assert_eq!(self.calls, ["lock"]);
        self.calls.push("relative");
        match value {
            Value::Position([x, y]) => {
                let v = coords::pos_from_absolute(Vec2::new(x, y), 1920., 1080.);
                Value::Position([v.x, v.y])
            }
            Value::Scale([x, y]) if !self.keep_scale => {
                let v = coords::item_relative_scale(Vec2::new(x, y), 1080., 2160.);
                Value::Scale([v.x, v.y])
            }
            _ => value,
        }
    }
    fn store(&mut self, value: Value) {
        assert!(matches!(self.calls.last(), Some(&"lock" | &"relative")));
        match value {
            Value::Position(v) => self.fields.0 = v,
            Value::Rotation(v) => self.fields.1 = v,
            Value::Scale(v) => self.fields.2 = v,
        }
        self.calls.push("store");
    }
}
#[test]
fn test_sceneitem_xform_setters_null_is_silent() {
    set::<Model>(None);
}

#[test]
fn test_sceneitem_xform_setters_absolute_update_paths() {
    for [x, y] in [[0., 0.], [-2., 3.5], [450., -720.]] {
        for value in [
            Value::Position([x, y]),
            Value::Rotation(x),
            Value::Scale([x, y]),
        ] {
            for path in 0..3 {
                for defer in [-1, 0, 1, 3] {
                    let mut m = Model::new(value);
                    m.path = path;
                    m.defer = defer;
                    for _ in 0..2 {
                        m.calls.clear();
                        m.signals = 0;
                        m.widths = 0;
                        m.heights = 0;
                        m.dirty = false;
                        set(Some(&mut m));
                        let expected = match value {
                            Value::Position(v) => (v, 0., [1.; 2]),
                            Value::Rotation(v) => ([0.; 2], v, [1.; 2]),
                            Value::Scale(v) => ([0.; 2], 0., v),
                        };
                        assert_eq!(m.fields, expected);
                        assert_eq!(m.defer, defer);
                        assert_eq!(m.dirty, path != 2);
                        let immediate = u32::from(path == 2 && defer <= 0);
                        assert_eq!(
                            (m.widths, m.heights, m.signals),
                            (immediate, immediate, immediate)
                        );
                        assert_eq!(m.calls, ["lock", "store", "unlock", "update"]);
                    }
                }
            }
        }
    }
}
#[test]
fn test_sceneitem_xform_setters_relative_values_and_aliasing() {
    for group in [false, true] {
        for nested in [false, true] {
            let mut m = Model::new(Value::Position([480., 810.]));
            m.absolute = false;
            m.defer = 1;
            m.keep_scale = group || nested;
            for alias in [false, true] {
                m.input = Value::Position([480., 810.]);
                if alias {
                    m.fields.0 = [480., 810.];
                }
                m.calls.clear();
                set(Some(&mut m));
                assert!((m.fields.0[0] + 8. / 9.).abs() < 0.00001);
                assert_eq!(m.fields.0[1], 0.5);
                assert_eq!(m.input, Value::Position([480., 810.]));
                m.input = Value::Scale([-2., 0.5]);
                if alias {
                    m.fields.2 = [-2., 0.5];
                }
                m.calls.clear();
                set(Some(&mut m));
                let factor = if group || nested { 1. } else { 2. };
                assert_eq!(m.fields.2, [-2. * factor, 0.5 * factor]);
                assert_eq!(m.input, Value::Scale([-2., 0.5]));
                assert_eq!(m.defer, 1);
                assert!(!m.dirty);
                assert_eq!(m.signals, 0);
            }
        }
    }
}
