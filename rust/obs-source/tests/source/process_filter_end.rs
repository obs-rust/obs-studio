//! Tier 1 for `obs_source_process_filter_end()` and
//! `obs_source_process_filter_tech_end()`: the cases of
//! test_source_process_filter_end.c on a model that records every read,
//! write and draw in order, plus every flag combination they look at.
use obs_source::process_filter_end::{FilterEnd, process_filter_end, process_filter_tech_end};
use obs_source::render_route::{FLAG_SRGB, FLAG_VIDEO};

/// The filter and its parent.
const FILTER: usize = 0;
const PARENT: usize = 1;
const NODES: usize = 2;
/// The caller's effect.
const EFFECT: u8 = 7;
/// The filter's texture.
const TEXTURE: u16 = 0xfe;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tech {
    Draw,
    Named(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    LogNull,
    BypassActive(usize),
    ClearBypass(usize),
    Target(usize),
    Parent(usize),
    Flags(usize),
    SetLinear(bool),
    RenderBypass(usize, u8, Tech),
    Texture(usize),
    RenderTex(u16, u8, u32, u32, Tech),
}

struct Model {
    bypass: [bool; NODES],
    target: Option<usize>,
    parent: Option<usize>,
    flags: [u32; NODES],
    linear: bool,
    texture: Option<u16>,
    steps: Vec<Step>,
}

impl Model {
    /// The filter is attached straight to a plain video parent; begin
    /// rendered into its texture.
    fn new() -> Self {
        Self {
            bypass: [false; NODES],
            target: Some(PARENT),
            parent: Some(PARENT),
            flags: [FLAG_VIDEO; NODES],
            linear: false,
            texture: Some(TEXTURE),
            steps: Vec::new(),
        }
    }
}

impl FilterEnd for Model {
    type Source = usize;
    type Effect = u8;
    type Tech = Tech;
    type Texture = u16;

    fn log_null_filter(&mut self) {
        self.steps.push(Step::LogNull);
    }
    fn draw_tech(&self) -> Tech {
        Tech::Draw
    }
    fn bypass_active(&mut self, filter: usize) -> bool {
        self.steps.push(Step::BypassActive(filter));
        self.bypass[filter]
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
    fn output_flags(&mut self, filter: usize) -> u32 {
        self.steps.push(Step::Flags(filter));
        self.flags[filter]
    }
    fn set_linear_srgb(&mut self, linear_srgb: bool) -> bool {
        self.steps.push(Step::SetLinear(linear_srgb));
        core::mem::replace(&mut self.linear, linear_srgb)
    }
    fn render_bypass(&mut self, target: usize, effect: u8, tech: Tech) {
        self.steps.push(Step::RenderBypass(target, effect, tech));
    }
    fn texture(&mut self, filter: usize) -> Option<u16> {
        self.steps.push(Step::Texture(filter));
        self.texture
    }
    fn render_tex(&mut self, texture: u16, effect: u8, width: u32, height: u32, tech: Tech) {
        self.steps
            .push(Step::RenderTex(texture, effect, width, height, tech));
    }
}

/// The reads every attached filter makes before its draw: the bypass flag
/// (read, then cleared), the target, the parent, the filter's flags, then
/// linear sRGB set from them.
fn attached(linear: bool) -> Vec<Step> {
    vec![
        Step::BypassActive(FILTER),
        Step::ClearBypass(FILTER),
        Step::Target(FILTER),
        Step::Parent(FILTER),
        Step::Flags(FILTER),
        Step::SetLinear(linear),
    ]
}

fn with(mut steps: Vec<Step>, tail: &[Step]) -> Vec<Step> {
    steps.extend_from_slice(tail);
    steps
}

#[test]
fn end_null_filter_logs_and_reads_nothing() {
    let mut m = Model::new();
    process_filter_end(&mut m, None, EFFECT, 1, 2);
    assert_eq!(m.steps, [Step::LogNull]);
}

#[test]
fn tech_end_null_filter_is_silent() {
    for tech in [None, Some(Tech::Named("Draw"))] {
        let mut m = Model::new();
        process_filter_tech_end(&mut m, None, EFFECT, 1, 2, tech);
        assert!(m.steps.is_empty());
    }
}

#[test]
fn end_draws_texture_with_draw() {
    let mut m = Model::new();
    process_filter_end(&mut m, Some(FILTER), EFFECT, 16, 9);
    assert_eq!(
        m.steps,
        with(
            attached(false),
            &[
                Step::Texture(FILTER),
                Step::RenderTex(TEXTURE, EFFECT, 16, 9, Tech::Draw),
                Step::SetLinear(false),
            ]
        )
    );
}

#[test]
fn end_draws_bypass_target_and_clears_flag() {
    let mut m = Model::new();
    m.bypass[FILTER] = true;
    process_filter_end(&mut m, Some(FILTER), EFFECT, 0, 0);
    assert_eq!(
        m.steps,
        with(
            attached(false),
            &[
                Step::RenderBypass(PARENT, EFFECT, Tech::Draw),
                Step::SetLinear(false),
            ]
        )
    );
    assert!(!m.bypass[FILTER]);
}

#[test]
fn tech_end_passes_given_name_and_maps_null_to_draw() {
    for (given, used) in [
        (None, Tech::Draw),
        (
            Some(Tech::Named("DrawAlphaDivide")),
            Tech::Named("DrawAlphaDivide"),
        ),
        (
            Some(Tech::Named("NoSuchTechnique")),
            Tech::Named("NoSuchTechnique"),
        ),
    ] {
        for bypass in [false, true] {
            let mut m = Model::new();
            m.bypass[FILTER] = bypass;
            process_filter_tech_end(&mut m, Some(FILTER), EFFECT, 3, 4, given);
            let draw: &[Step] = if bypass {
                &[Step::RenderBypass(PARENT, EFFECT, used)]
            } else {
                &[
                    Step::Texture(FILTER),
                    Step::RenderTex(TEXTURE, EFFECT, 3, 4, used),
                ]
            };
            let mut want = with(attached(false), draw);
            want.push(Step::SetLinear(false));
            assert_eq!(m.steps, want, "given {given:?}, bypass {bypass}");
        }
    }
}

#[test]
fn texture_path_without_texture_draws_nothing_but_restores() {
    let mut m = Model::new();
    m.texture = None;
    m.linear = true;
    process_filter_tech_end(&mut m, Some(FILTER), EFFECT, 0, 0, None);
    assert_eq!(
        m.steps,
        with(
            attached(false),
            &[Step::Texture(FILTER), Step::SetLinear(true)]
        )
    );
    assert!(m.linear);
}

#[test]
fn linear_srgb_follows_filter_flag_and_is_restored() {
    for (flags, linear) in [
        (FLAG_VIDEO, false),
        (FLAG_VIDEO | FLAG_SRGB, true),
        (FLAG_SRGB, true),
        (0, false),
        (u32::MAX, true),
        (!FLAG_SRGB, false),
    ] {
        for previous in [false, true] {
            for bypass in [false, true] {
                let mut m = Model::new();
                m.flags[FILTER] = flags;
                // the parent's flags are never read
                m.flags[PARENT] = !flags;
                m.linear = previous;
                m.bypass[FILTER] = bypass;
                process_filter_tech_end(&mut m, Some(FILTER), EFFECT, 0, 0, None);
                assert_eq!(m.steps.get(5), Some(&Step::SetLinear(linear)));
                assert_eq!(m.steps.last(), Some(&Step::SetLinear(previous)));
                assert_eq!(m.linear, previous);
            }
        }
    }
}

#[test]
fn removed_filter_clears_bypass_reads_both_and_draws_nothing() {
    // The flag is read and cleared, then both lookups are made before
    // either is checked; no flags read, no linear sRGB, no draw.
    for (target, parent) in [(None, None), (None, Some(PARENT)), (Some(PARENT), None)] {
        for bypass in [false, true] {
            let mut m = Model::new();
            m.target = target;
            m.parent = parent;
            m.bypass[FILTER] = bypass;
            m.linear = true;
            process_filter_tech_end(&mut m, Some(FILTER), EFFECT, 0, 0, None);
            assert_eq!(
                m.steps,
                [
                    Step::BypassActive(FILTER),
                    Step::ClearBypass(FILTER),
                    Step::Target(FILTER),
                    Step::Parent(FILTER),
                ]
            );
            assert!(!m.bypass[FILTER]);
            assert!(m.linear);
        }
    }
}

#[test]
fn removed_then_readded_end_takes_texture_path() {
    // test_source_process_filter_end's removed_filter_end_clears_bypass:
    // the end of the removed filter clears the flag, so the next end, once
    // re-added, takes the texture path.
    let mut m = Model::new();
    m.bypass[FILTER] = true;
    m.target = None;
    m.parent = None;
    process_filter_tech_end(&mut m, Some(FILTER), EFFECT, 0, 0, None);
    m.target = Some(PARENT);
    m.parent = Some(PARENT);
    m.texture = None;
    m.steps.clear();
    process_filter_tech_end(&mut m, Some(FILTER), EFFECT, 0, 0, None);
    assert_eq!(
        m.steps,
        with(
            attached(false),
            &[Step::Texture(FILTER), Step::SetLinear(false)]
        )
    );
}

#[test]
fn bypass_draws_target_not_parent() {
    // The target is drawn; a filter whose target is another filter (it is
    // not last) draws that one.
    let mut m = Model::new();
    m.bypass[FILTER] = true;
    m.target = Some(FILTER);
    process_filter_tech_end(&mut m, Some(FILTER), EFFECT, 0, 0, None);
    assert!(
        m.steps
            .contains(&Step::RenderBypass(FILTER, EFFECT, Tech::Draw))
    );
}
