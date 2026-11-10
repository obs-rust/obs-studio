use obs_graphics::sprite::MipWarning;
use obs_graphics::texture_creators::{Device, Request, create};

#[derive(Default)]
struct Fake {
    absent: bool,
    events: Vec<String>,
}
impl Device<u8> for Fake {
    type Texture = u8;
    fn has_context(&self) -> bool {
        !self.absent
    }
    fn no_context(&mut self, cube: bool) {
        self.events.push(format!("absent {cube}"));
    }
    fn warning(&mut self, w: MipWarning) {
        self.events.push(format!("{w:?}"));
    }
    fn create(&mut self, r: Request<u8>) -> Option<u8> {
        self.events.push(format!(
            "{} {}x{} f{} l{} {:?} 0x{:x}",
            r.cube, r.width, r.height, r.format, r.levels, r.data, r.flags
        ));
        Some(42)
    }
}
#[allow(clippy::too_many_arguments)]
fn check(
    cube: bool,
    w: u32,
    h: u32,
    l: u32,
    f: u32,
    d: Option<u8>,
    ol: u32,
    of: u32,
    od: Option<u8>,
    warn: Option<MipWarning>,
) {
    let mut host = Fake::default();
    assert_eq!(
        create(
            &mut host,
            Request {
                width: w,
                height: h,
                format: 3,
                levels: l,
                flags: f,
                data: d,
                cube
            }
        ),
        Some(42)
    );
    let mut events = Vec::new();
    if let Some(w) = warn {
        events.push(format!("{w:?}"));
    }
    events.push(format!("{cube} {w}x{h} f3 l{ol} {od:?} 0x{of:x}"));
    assert_eq!(host.events, events);
}
#[test]
fn test_texture_create_mip_data_render_targets() {
    for cube in [false, true] {
        let d = if cube { None } else { Some(7) };
        let h = if cube { 64 } else { 32 };
        check(
            cube,
            64,
            h,
            1,
            5,
            Some(7),
            1,
            4,
            d,
            Some(MipWarning::RenderTarget),
        );
        check(
            cube,
            64,
            h,
            0,
            6,
            Some(7),
            1,
            6,
            d,
            Some(MipWarning::RenderTarget),
        );
        check(
            cube,
            64,
            h,
            7,
            4,
            None,
            1,
            4,
            None,
            Some(MipWarning::RenderTarget),
        );
    }
}
#[test]
fn test_texture_create_mip_data_not_pow2() {
    check(
        false,
        100,
        64,
        1,
        5,
        Some(7),
        1,
        4,
        Some(7),
        Some(MipWarning::NotPow2),
    );
    check(
        false,
        64,
        100,
        7,
        0,
        Some(7),
        1,
        0,
        Some(7),
        Some(MipWarning::NotPow2),
    );
    check(
        false,
        1,
        1,
        1,
        1,
        Some(7),
        1,
        0,
        Some(7),
        Some(MipWarning::NotPow2),
    );
    check(
        true,
        100,
        100,
        7,
        5,
        None,
        1,
        4,
        None,
        Some(MipWarning::NotPow2),
    );
    check(
        true,
        1,
        1,
        1,
        1,
        None,
        1,
        0,
        None,
        Some(MipWarning::NotPow2),
    );
    check(
        true,
        100,
        100,
        7,
        5,
        Some(7),
        1,
        4,
        Some(7),
        Some(MipWarning::NotPow2),
    );
}
#[test]
fn test_texture_create_mip_data_unchanged() {
    for cube in [false, true] {
        for (w, h, l, f) in [
            (64, 32, 1, 4),
            (100, 64, 1, 2),
            (64, 32, 7, 1),
            (64, 32, 0, 0),
        ] {
            let h = if cube { w } else { h };
            check(cube, w, h, l, f, Some(7), l, f, Some(7), None);
        }
    }
}
#[test]
fn no_context_never_warns_or_dispatches() {
    for cube in [false, true] {
        let mut h = Fake {
            absent: true,
            ..Fake::default()
        };
        assert_eq!(
            create(
                &mut h,
                Request {
                    width: 1,
                    height: 1,
                    format: 3,
                    levels: 0,
                    flags: 5,
                    data: Some(7),
                    cube
                }
            ),
            None
        );
        assert_eq!(h.events, [format!("absent {cube}")]);
    }
}
