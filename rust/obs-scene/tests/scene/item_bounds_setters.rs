use obs_graphics::vec2::Vec2;
use obs_scene::item_bounds_setters::{Target, Write, apply, bounds_size, valid_bounds_type};

#[derive(Default)]
struct Item {
    events: Vec<&'static str>,
    kind: u32,
    alignment: u32,
    crop: bool,
    size: [f32; 2],
    normal: bool,
    deferred: bool,
    update_transform: bool,
    relative: bool,
}
impl Target for Item {
    fn lock(&mut self) {
        self.events.push("lock");
    }
    fn write(&mut self, value: Write) {
        self.events.push("write");
        match value {
            Write::Type(value) => self.kind = valid_bounds_type(value),
            Write::Alignment(value) => self.alignment = value,
            Write::Crop(value) => self.crop = value,
            Write::Size(value) => {
                let v = bounds_size(value, !self.relative, 1080.0);
                self.size = [v.x, v.y];
            }
        }
    }
    fn unlock(&mut self) {
        self.events.push("unlock");
    }
    fn update(&mut self) {
        self.events.push("update");
        if !self.normal {
            self.update_transform = true;
        }
    }
}
fn write(item: &mut Item, value: Write) {
    item.events.clear();
    item.update_transform = false;
    apply(Some(item), value);
    assert_eq!(item.events, ["lock", "write", "unlock", "update"]);
    assert_eq!(item.update_transform, !item.normal);
}
#[test]
fn bounds_writes_mirrors_c() {
    for value in [
        Write::Type(i64::MAX),
        Write::Alignment(u32::MAX),
        Write::Crop(true),
        Write::Size(Vec2::new(0.0, 0.0)),
    ] {
        apply::<Item>(None, value);
    }
    for _parented in [false, true] {
        let mut item = Item::default();
        for kind in 0..=6 {
            write(&mut item, Write::Type(kind));
            assert_eq!(item.kind, kind as u32);
        }
        for kind in [-1, 7, i32::MAX as i64, i64::MIN, i64::MAX, 1_i64 << 32] {
            item.kind = 1;
            write(&mut item, Write::Type(kind));
            assert_eq!(item.kind, 0);
        }
        for alignment in [0, 5, u32::MAX] {
            write(&mut item, Write::Alignment(alignment));
            assert_eq!(item.alignment, alignment);
        }
        for crop in [false, true] {
            write(&mut item, Write::Crop(crop));
            assert_eq!(item.crop, crop);
        }
        for size in [[0.0, 0.0], [1920.0, 1080.0], [-20.0, 35.5]] {
            write(&mut item, Write::Size(Vec2::new(size[0], size[1])));
            assert_eq!(item.size, size);
        }
    }
}
#[test]
fn relative_bounds_mirrors_c() {
    let mut item = Item {
        normal: true,
        deferred: true,
        relative: true,
        ..Item::default()
    };
    let input = Vec2::new(540.0, -270.0);
    write(&mut item, Write::Size(input));
    assert_eq!(item.size, [1.0, -0.5]);
    assert!(!item.update_transform);
    assert!(item.deferred);
    assert_eq!([input.x, input.y], [540.0, -270.0]);
}
