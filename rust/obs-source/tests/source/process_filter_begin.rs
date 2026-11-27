//! Tier 1 for `obs_source_process_filter_begin()` and
//! `obs_source_process_filter_begin_with_color_space()`: the cases of
//! test_source_process_filter_begin.c on a model that records every read,
//! write and draw in order, plus the flag, space and size combinations they
//! look at.
use obs_source::process_filter_begin::{
    FilterBegin, process_filter_begin, process_filter_begin_with_color_space,
};
use obs_source::render_route::{
    ALLOW_DIRECT_RENDERING, FLAG_ASYNC, FLAG_CUSTOM_DRAW, FLAG_SRGB, FLAG_VIDEO,
    GS_CS_709_EXTENDED, GS_CS_SRGB, NO_DIRECT_RENDERING,
};

/// The filter, its parent and another filter below it.
const FILTER: usize = 0;
const PARENT: usize = 1;
const OTHER: usize = 2;
const NODES: usize = 3;
/// `GS_RGBA` and `GS_R8`.
const RGBA: i32 = 3;
const R8: i32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    LogNull,
    ClearBypass(usize),
    Target(usize),
    Parent(usize),
    LogNoTarget(usize),
    LogNoParent(usize),
    Flags(usize),
    Width(usize),
    Height(usize),
    SetAllowDirect(usize, i32),
    ColorSpace,
    SetBypass(usize),
    Skip(usize),
    HasTexrender(usize),
    TexrenderFormat(usize),
    Destroy(usize),
    Create(usize, i32),
    Begin(usize, u32, u32, i32),
    DrawSetup(i32, i32),
    DefaultRender(usize),
    VideoRender(usize),
    DrawFinish(usize),
}

struct Model {
    bypass: [bool; NODES],
    allow_direct: [i32; NODES],
    target: Option<usize>,
    parent: Option<usize>,
    flags: [u32; NODES],
    size: (u32, u32),
    space: i32,
    /// The filter's texrender's format, `None` for no texrender.
    texrender: Option<i32>,
    /// Whether a texrender created now exists (`gs_texrender_create()` can
    /// fail).
    creates: bool,
    begins: bool,
    steps: Vec<Step>,
}

impl Model {
    /// The filter is attached straight to a plain 16x9 video parent, has
    /// no texrender yet, in an sRGB canvas.
    fn new() -> Self {
        Self {
            bypass: [false; NODES],
            allow_direct: [-1; NODES],
            target: Some(PARENT),
            parent: Some(PARENT),
            flags: [FLAG_VIDEO; NODES],
            size: (16, 9),
            space: GS_CS_SRGB,
            texrender: None,
            creates: true,
            begins: true,
            steps: Vec::new(),
        }
    }
}

impl FilterBegin for Model {
    type Source = usize;

    fn log_null_filter(&mut self) {
        self.steps.push(Step::LogNull);
    }
    fn clear_bypass(&mut self, filter: usize) {
        self.steps.push(Step::ClearBypass(filter));
        self.bypass[filter] = false;
    }
    fn target(&mut self, filter: usize) -> Option<usize> {
        self.steps.push(Step::Target(filter));
        self.target
    }
    fn parent(&mut self, filter: usize) -> Option<usize> {
        self.steps.push(Step::Parent(filter));
        self.parent
    }
    fn log_no_target(&mut self, filter: usize) {
        self.steps.push(Step::LogNoTarget(filter));
    }
    fn log_no_parent(&mut self, filter: usize) {
        self.steps.push(Step::LogNoParent(filter));
    }
    fn output_flags(&mut self, source: usize) -> u32 {
        self.steps.push(Step::Flags(source));
        self.flags[source]
    }
    fn base_width(&mut self, target: usize) -> u32 {
        self.steps.push(Step::Width(target));
        self.size.0
    }
    fn base_height(&mut self, target: usize) -> u32 {
        self.steps.push(Step::Height(target));
        self.size.1
    }
    fn set_allow_direct(&mut self, filter: usize, allow_direct: i32) {
        self.steps.push(Step::SetAllowDirect(filter, allow_direct));
        self.allow_direct[filter] = allow_direct;
    }
    fn color_space(&mut self) -> i32 {
        self.steps.push(Step::ColorSpace);
        self.space
    }
    fn set_bypass(&mut self, filter: usize) {
        self.steps.push(Step::SetBypass(filter));
        self.bypass[filter] = true;
    }
    fn skip_video_filter(&mut self, filter: usize) {
        self.steps.push(Step::Skip(filter));
    }
    fn has_texrender(&mut self, filter: usize) -> bool {
        self.steps.push(Step::HasTexrender(filter));
        self.texrender.is_some()
    }
    fn texrender_format(&mut self, filter: usize) -> i32 {
        self.steps.push(Step::TexrenderFormat(filter));
        self.texrender.unwrap()
    }
    fn destroy_texrender(&mut self, filter: usize) {
        self.steps.push(Step::Destroy(filter));
        self.texrender = None;
    }
    fn create_texrender(&mut self, filter: usize, format: i32) {
        self.steps.push(Step::Create(filter, format));
        self.texrender = self.creates.then_some(format);
    }
    fn texrender_begin(&mut self, filter: usize, cx: u32, cy: u32, space: i32) -> bool {
        self.steps.push(Step::Begin(filter, cx, cy, space));
        self.texrender.is_some() && self.begins
    }
    fn draw_setup(&mut self, cx: i32, cy: i32) {
        self.steps.push(Step::DrawSetup(cx, cy));
    }
    fn default_render(&mut self, target: usize) {
        self.steps.push(Step::DefaultRender(target));
    }
    fn video_render(&mut self, target: usize) {
        self.steps.push(Step::VideoRender(target));
    }
    fn draw_finish(&mut self, filter: usize) {
        self.steps.push(Step::DrawFinish(filter));
    }
}

/// The texrender field is read twice on the way to the draw, as the C does:
/// for the stale check, then for the create.
///
/// The reads every attached filter makes before it decides: the bypass
/// flag cleared, the target, the parent, the filter's then the parent's
/// flags, the target's size, `allow_direct` stored.
fn attached(target: usize, allow_direct: i32) -> Vec<Step> {
    vec![
        Step::ClearBypass(FILTER),
        Step::Target(FILTER),
        Step::Parent(FILTER),
        Step::Flags(FILTER),
        Step::Flags(PARENT),
        Step::Width(target),
        Step::Height(target),
        Step::SetAllowDirect(FILTER, allow_direct),
    ]
}

fn with(mut steps: Vec<Step>, tail: &[Step]) -> Vec<Step> {
    steps.extend_from_slice(tail);
    steps
}

/// The texrender drawn into, the target through `render`.
fn drawn(cx: u32, cy: u32, space: i32, render: Step) -> [Step; 4] {
    [
        Step::Begin(FILTER, cx, cy, space),
        Step::DrawSetup(cx.cast_signed(), cy.cast_signed()),
        render,
        Step::DrawFinish(FILTER),
    ]
}

#[test]
fn null_filter_logs_and_fails() {
    let mut m = Model::new();
    assert!(!process_filter_begin(
        &mut m,
        None,
        RGBA,
        ALLOW_DIRECT_RENDERING
    ));
    assert_eq!(m.steps, [Step::LogNull]);
    let mut m = Model::new();
    assert!(!process_filter_begin_with_color_space(
        &mut m,
        None,
        RGBA,
        GS_CS_709_EXTENDED,
        NO_DIRECT_RENDERING
    ));
    assert_eq!(m.steps, [Step::LogNull]);
}

#[test]
fn removed_filter_clears_bypass_and_logs() {
    for (target, parent, log) in [
        (None, None, Step::LogNoTarget(FILTER)),
        (None, Some(PARENT), Step::LogNoTarget(FILTER)),
        (Some(PARENT), None, Step::LogNoParent(FILTER)),
    ] {
        let mut m = Model::new();
        m.bypass[FILTER] = true;
        m.target = target;
        m.parent = parent;
        m.texrender = Some(R8);
        assert!(!process_filter_begin(
            &mut m,
            Some(FILTER),
            RGBA,
            ALLOW_DIRECT_RENDERING
        ));
        assert_eq!(
            m.steps,
            [
                Step::ClearBypass(FILTER),
                Step::Target(FILTER),
                Step::Parent(FILTER),
                log
            ]
        );
        assert!(!m.bypass[FILTER]);
        assert_eq!(m.allow_direct[FILTER], -1);
        assert_eq!(m.texrender, Some(R8));
    }
}

#[test]
fn bypass_sets_flag_and_draws_nothing() {
    let mut m = Model::new();
    assert!(process_filter_begin(
        &mut m,
        Some(FILTER),
        RGBA,
        ALLOW_DIRECT_RENDERING
    ));
    assert_eq!(
        m.steps,
        with(
            attached(PARENT, ALLOW_DIRECT_RENDERING),
            &[Step::ColorSpace, Step::SetBypass(FILTER)]
        )
    );
    assert!(m.bypass[FILTER]);
    assert_eq!(m.allow_direct[FILTER], ALLOW_DIRECT_RENDERING);
    assert_eq!(m.texrender, None);
}

#[test]
fn texture_path_creates_and_draws_default() {
    let mut m = Model::new();
    assert!(process_filter_begin(
        &mut m,
        Some(FILTER),
        RGBA,
        NO_DIRECT_RENDERING
    ));
    let mut expected = with(
        attached(PARENT, NO_DIRECT_RENDERING),
        &[
            Step::HasTexrender(FILTER),
            Step::HasTexrender(FILTER),
            Step::Create(FILTER, RGBA),
        ],
    );
    expected.extend(drawn(16, 9, GS_CS_SRGB, Step::DefaultRender(PARENT)));
    assert_eq!(m.steps, expected);
    assert!(!m.bypass[FILTER]);
    assert_eq!(m.texrender, Some(RGBA));

    // The texrender kept: its format read, nothing created.
    m.steps.clear();
    assert!(process_filter_begin(
        &mut m,
        Some(FILTER),
        RGBA,
        NO_DIRECT_RENDERING
    ));
    let mut expected = with(
        attached(PARENT, NO_DIRECT_RENDERING),
        &[
            Step::HasTexrender(FILTER),
            Step::TexrenderFormat(FILTER),
            Step::HasTexrender(FILTER),
        ],
    );
    expected.extend(drawn(16, 9, GS_CS_SRGB, Step::DefaultRender(PARENT)));
    assert_eq!(m.steps, expected);
}

#[test]
fn format_change_replaces_texrender() {
    let mut m = Model::new();
    m.texrender = Some(R8);
    assert!(process_filter_begin_with_color_space(
        &mut m,
        Some(FILTER),
        RGBA,
        GS_CS_709_EXTENDED,
        NO_DIRECT_RENDERING
    ));
    let mut expected = with(
        attached(PARENT, NO_DIRECT_RENDERING),
        &[
            Step::HasTexrender(FILTER),
            Step::TexrenderFormat(FILTER),
            Step::Destroy(FILTER),
            Step::HasTexrender(FILTER),
            Step::Create(FILTER, RGBA),
        ],
    );
    expected.extend(drawn(
        16,
        9,
        GS_CS_709_EXTENDED,
        Step::DefaultRender(PARENT),
    ));
    assert_eq!(m.steps, expected);
    assert_eq!(m.texrender, Some(RGBA));
}

#[test]
fn texrender_that_does_not_begin_still_succeeds() {
    for creates in [false, true] {
        let mut m = Model::new();
        m.creates = creates;
        m.begins = false;
        assert!(process_filter_begin(
            &mut m,
            Some(FILTER),
            RGBA,
            NO_DIRECT_RENDERING
        ));
        assert_eq!(
            m.steps,
            with(
                attached(PARENT, NO_DIRECT_RENDERING),
                &[
                    Step::HasTexrender(FILTER),
                    Step::HasTexrender(FILTER),
                    Step::Create(FILTER, RGBA),
                    Step::Begin(FILTER, 16, 9, GS_CS_SRGB),
                ]
            )
        );
    }
}

#[test]
fn zero_size_skips_after_the_bypass_check() {
    for size in [(0, 9), (16, 0), (0, 0)] {
        let mut m = Model::new();
        m.size = size;
        m.texrender = Some(R8);
        assert!(!process_filter_begin(
            &mut m,
            Some(FILTER),
            RGBA,
            NO_DIRECT_RENDERING
        ));
        assert_eq!(
            m.steps,
            with(attached(PARENT, NO_DIRECT_RENDERING), &[Step::Skip(FILTER)])
        );
        assert_eq!(m.texrender, Some(R8));

        let mut m = Model::new();
        m.size = size;
        assert!(process_filter_begin(
            &mut m,
            Some(FILTER),
            RGBA,
            ALLOW_DIRECT_RENDERING
        ));
        assert!(m.bypass[FILTER]);
    }
}

#[test]
fn space_decides_the_bypass() {
    // Asked for 709 extended in an sRGB canvas: the space is read, no
    // bypass, the texture drawn in 709 extended.
    let mut m = Model::new();
    assert!(process_filter_begin_with_color_space(
        &mut m,
        Some(FILTER),
        RGBA,
        GS_CS_709_EXTENDED,
        ALLOW_DIRECT_RENDERING
    ));
    let mut expected = with(
        attached(PARENT, ALLOW_DIRECT_RENDERING),
        &[
            Step::ColorSpace,
            Step::HasTexrender(FILTER),
            Step::HasTexrender(FILTER),
            Step::Create(FILTER, RGBA),
        ],
    );
    expected.extend(drawn(
        16,
        9,
        GS_CS_709_EXTENDED,
        Step::DefaultRender(PARENT),
    ));
    assert_eq!(m.steps, expected);
    assert!(!m.bypass[FILTER]);

    // The same in a 709 extended canvas: the bypass.
    let mut m = Model::new();
    m.space = GS_CS_709_EXTENDED;
    assert!(process_filter_begin_with_color_space(
        &mut m,
        Some(FILTER),
        RGBA,
        GS_CS_709_EXTENDED,
        ALLOW_DIRECT_RENDERING
    ));
    assert!(m.bypass[FILTER]);

    // The plain begin asks for sRGB.
    let mut m = Model::new();
    m.space = GS_CS_709_EXTENDED;
    assert!(process_filter_begin(
        &mut m,
        Some(FILTER),
        RGBA,
        ALLOW_DIRECT_RENDERING
    ));
    assert!(!m.bypass[FILTER]);
    assert!(m.steps.contains(&Step::Begin(FILTER, 16, 9, GS_CS_SRGB)));
}

#[test]
fn space_read_only_when_the_rest_allows_the_bypass() {
    // Each of these fails the bypass before the space: never read.
    let cases: [(usize, u32, u32, i32); 5] = [
        (OTHER, FLAG_VIDEO, FLAG_VIDEO, ALLOW_DIRECT_RENDERING),
        (
            PARENT,
            FLAG_VIDEO,
            FLAG_VIDEO | FLAG_CUSTOM_DRAW,
            ALLOW_DIRECT_RENDERING,
        ),
        (
            PARENT,
            FLAG_VIDEO,
            FLAG_VIDEO | FLAG_ASYNC,
            ALLOW_DIRECT_RENDERING,
        ),
        (
            PARENT,
            FLAG_VIDEO | FLAG_SRGB,
            FLAG_VIDEO,
            ALLOW_DIRECT_RENDERING,
        ),
        (PARENT, FLAG_VIDEO, FLAG_VIDEO, NO_DIRECT_RENDERING),
    ];
    for (target, filter_flags, parent_flags, allow_direct) in cases {
        let mut m = Model::new();
        m.target = Some(target);
        m.flags[FILTER] = filter_flags;
        m.flags[PARENT] = parent_flags;
        assert!(process_filter_begin(
            &mut m,
            Some(FILTER),
            RGBA,
            allow_direct
        ));
        assert!(!m.steps.contains(&Step::ColorSpace));
        assert!(!m.bypass[FILTER]);
    }
    // Both sRGB-aware: the bypass.
    let mut m = Model::new();
    m.flags = [FLAG_VIDEO | FLAG_SRGB; NODES];
    assert!(process_filter_begin(
        &mut m,
        Some(FILTER),
        RGBA,
        ALLOW_DIRECT_RENDERING
    ));
    assert!(m.bypass[FILTER]);
}

#[test]
fn target_drawn_as_the_route_says() {
    // Another filter below, or a custom-draw or async parent: its video
    // render; the parent itself otherwise: its default render.
    for (target, parent_flags, render) in [
        (PARENT, FLAG_VIDEO, Step::DefaultRender(PARENT)),
        (OTHER, FLAG_VIDEO, Step::VideoRender(OTHER)),
        (
            PARENT,
            FLAG_VIDEO | FLAG_CUSTOM_DRAW,
            Step::VideoRender(PARENT),
        ),
        (PARENT, FLAG_VIDEO | FLAG_ASYNC, Step::VideoRender(PARENT)),
    ] {
        let mut m = Model::new();
        m.target = Some(target);
        m.flags[PARENT] = parent_flags;
        assert!(process_filter_begin(
            &mut m,
            Some(FILTER),
            RGBA,
            NO_DIRECT_RENDERING
        ));
        let mut expected = with(
            attached(target, NO_DIRECT_RENDERING),
            &[
                Step::HasTexrender(FILTER),
                Step::HasTexrender(FILTER),
                Step::Create(FILTER, RGBA),
            ],
        );
        expected.extend(drawn(16, 9, GS_CS_SRGB, render));
        assert_eq!(m.steps, expected);
    }
}

#[test]
fn size_kept_as_the_c_int_round_trip() {
    // The C keeps the size as int: a width past INT_MAX is negative in the
    // ortho and back to the same u32 for the texrender.
    let mut m = Model::new();
    m.size = (u32::MAX, 0x8000_0000);
    assert!(process_filter_begin(
        &mut m,
        Some(FILTER),
        RGBA,
        NO_DIRECT_RENDERING
    ));
    assert!(
        m.steps
            .contains(&Step::Begin(FILTER, u32::MAX, 0x8000_0000, GS_CS_SRGB))
    );
    assert!(m.steps.contains(&Step::DrawSetup(-1, i32::MIN)));
}
