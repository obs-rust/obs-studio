use obs_graphics::viewport::{self, Backend};
#[derive(Default)]
struct Recording {
    present: bool,
    size: (u32, u32),
    maximum: Option<u32>,
    kind: i32,
    events: Vec<&'static str>,
    viewport: Option<[i32; 4]>,
    ortho: Option<[f32; 6]>,
}
impl Backend for Recording {
    fn valid(&mut self, _: &'static str) -> bool {
        self.events.push("valid");
        self.present
    }
    fn size(&mut self) -> (u32, u32) {
        self.events.push("size");
        self.size
    }
    fn viewport(&mut self, v: [i32; 4]) {
        self.events.push("viewport");
        self.viewport = Some(v);
    }
    fn ortho(&mut self, v: [f32; 6]) {
        self.events.push("ortho");
        self.ortho = Some(v);
    }
    fn maximum(&mut self) -> Option<u32> {
        self.events.push("maximum");
        self.maximum
    }
    fn device_type(&mut self) -> i32 {
        self.events.push("type");
        self.kind
    }
}
#[test]
fn test_graphics_viewport_dimensions_order_and_no_context() {
    for size in [
        (0, 0),
        (1, 7),
        (1920, 1080),
        (65535, 32769),
        (16_777_217, 16_777_219),
        (u32::MAX, 0),
    ] {
        let mut b = Recording {
            present: true,
            size,
            ..Default::default()
        };
        viewport::reset_viewport(&mut b);
        assert_eq!(b.viewport, Some([0, 0, size.0 as i32, size.1 as i32]));
        assert_eq!(b.events, ["valid", "size", "viewport"]);
        b.events.clear();
        viewport::set_2d_mode(&mut b);
        assert_eq!(
            b.ortho,
            Some([0., size.0 as f32, 0., size.1 as f32, -1., -1024.])
        );
        assert_eq!(b.events, ["valid", "size", "ortho"]);
        b.size = (17, 31);
        b.events.clear();
        viewport::reset_viewport(&mut b);
        viewport::set_2d_mode(&mut b);
        assert_eq!(b.viewport, Some([0, 0, 17, 31]));
        assert_eq!(b.ortho, Some([0., 17., 0., 31., -1., -1024.]));
    }
    let mut b = Recording::default();
    viewport::reset_viewport(&mut b);
    viewport::set_2d_mode(&mut b);
    assert_eq!(b.events, ["valid", "valid"]);
    assert!(b.viewport.is_none());
    assert!(b.ortho.is_none());
}
#[test]
fn test_graphics_viewport_optional_maximum_precedence() {
    for kind in [-7, 0, 1, 2, 3, 99] {
        for maximum in [None, Some(0), Some(7), Some(8192), Some(u32::MAX)] {
            let mut b = Recording {
                kind,
                maximum,
                ..Default::default()
            };
            assert_eq!(
                viewport::max_texture_size(&mut b),
                maximum.unwrap_or(if kind == 2 || kind == 3 { 16384 } else { 0 })
            );
            assert_eq!(
                b.events,
                if maximum.is_some() {
                    vec!["maximum"]
                } else {
                    vec!["maximum", "type"]
                }
            );
        }
    }
}
