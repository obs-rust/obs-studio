//! Tier 1 for `obs_source_skip_video_filter()`: the cases of
//! test_source_skip_video_filter.c and test_filter_skip_removed.c on a
//! model that records every read and draw in order, plus every flag
//! combination the route looks at.
use obs_source::render_route::{FLAG_ASYNC, FLAG_CUSTOM_DRAW, FLAG_VIDEO};
use obs_source::skip_video_filter::{Skip, skip_video_filter};

/// The filter, its parent, and another filter below it.
const FILTER: usize = 0;
const PARENT: usize = 1;
const BELOW: usize = 2;
const NODES: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    LogNull,
    Target(usize),
    Parent(usize),
    Flags(usize),
    HasVideoRender(usize),
    Deinterlacing(usize),
    DefaultRender(usize),
    MainRender(usize),
    DeinterlaceRender(usize),
    AsyncVideo(usize),
    VideoRender(usize),
}

struct Model {
    target: Option<usize>,
    parent: Option<usize>,
    flags: [u32; NODES],
    video_render: [bool; NODES],
    deinterlacing: [bool; NODES],
    steps: Vec<Step>,
}

impl Model {
    /// The filter is attached straight to a plain video parent.
    fn new() -> Self {
        Self {
            target: Some(PARENT),
            parent: Some(PARENT),
            flags: [FLAG_VIDEO; NODES],
            video_render: [true; NODES],
            deinterlacing: [false; NODES],
            steps: Vec::new(),
        }
    }
}

impl Skip for Model {
    type Source = usize;

    fn log_null_filter(&mut self) {
        self.steps.push(Step::LogNull);
    }
    fn target(&mut self, filter: usize) -> Option<usize> {
        self.steps.push(Step::Target(filter));
        self.target
    }
    fn parent(&mut self, filter: usize) -> Option<usize> {
        self.steps.push(Step::Parent(filter));
        self.parent
    }
    fn output_flags(&mut self, parent: usize) -> u32 {
        self.steps.push(Step::Flags(parent));
        self.flags[parent]
    }
    fn has_video_render(&mut self, target: usize) -> bool {
        self.steps.push(Step::HasVideoRender(target));
        self.video_render[target]
    }
    fn deinterlacing(&mut self, target: usize) -> bool {
        self.steps.push(Step::Deinterlacing(target));
        self.deinterlacing[target]
    }
    fn default_render(&mut self, target: usize) {
        self.steps.push(Step::DefaultRender(target));
    }
    fn main_render(&mut self, target: usize) {
        self.steps.push(Step::MainRender(target));
    }
    fn deinterlace_render(&mut self, target: usize) {
        self.steps.push(Step::DeinterlaceRender(target));
    }
    fn async_video(&mut self, target: usize) {
        self.steps.push(Step::AsyncVideo(target));
    }
    fn video_render(&mut self, target: usize) {
        self.steps.push(Step::VideoRender(target));
    }
}

/// The reads every attached filter makes: target, parent, then the
/// parent's flags.
fn attached() -> Vec<Step> {
    vec![
        Step::Target(FILTER),
        Step::Parent(FILTER),
        Step::Flags(PARENT),
    ]
}

#[test]
fn null_filter_logs_and_reads_nothing() {
    let mut m = Model::new();
    skip_video_filter(&mut m, None);
    assert_eq!(m.steps, [Step::LogNull]);
}

#[test]
fn removed_filter_reads_both_and_draws_nothing() {
    // Both are read before either is checked; no flags read, no draw.
    for (target, parent) in [(None, None), (None, Some(PARENT)), (Some(PARENT), None)] {
        let mut m = Model::new();
        m.target = target;
        m.parent = parent;
        skip_video_filter(&mut m, Some(FILTER));
        assert_eq!(
            m.steps,
            [Step::Target(FILTER), Step::Parent(FILTER)],
            "{target:?} {parent:?}"
        );
    }
}

#[test]
fn filter_below_draws_its_target() {
    // Another filter is the target: its render, whatever the parent's
    // flags, with nothing of the target read.
    for flags in [0, FLAG_VIDEO, FLAG_ASYNC, FLAG_CUSTOM_DRAW, u32::MAX] {
        let mut m = Model::new();
        m.target = Some(BELOW);
        m.flags[PARENT] = flags;
        skip_video_filter(&mut m, Some(FILTER));
        let mut want = attached();
        want.push(Step::VideoRender(BELOW));
        assert_eq!(m.steps, want, "{flags:#x}");
    }
}

#[test]
fn plain_parent_draws_with_the_default_effect() {
    for flags in [0, FLAG_VIDEO, !(FLAG_ASYNC | FLAG_CUSTOM_DRAW)] {
        let mut m = Model::new();
        m.flags[PARENT] = flags;
        skip_video_filter(&mut m, Some(FILTER));
        let mut want = attached();
        want.push(Step::DefaultRender(PARENT));
        assert_eq!(m.steps, want, "{flags:#x}");
    }
}

#[test]
fn parent_with_video_render_draws_its_main_render() {
    for flags in [
        FLAG_CUSTOM_DRAW,
        FLAG_ASYNC,
        FLAG_ASYNC | FLAG_CUSTOM_DRAW,
        u32::MAX,
    ] {
        let mut m = Model::new();
        m.flags[PARENT] = flags;
        m.deinterlacing[PARENT] = true;
        skip_video_filter(&mut m, Some(FILTER));
        let mut want = attached();
        want.extend([Step::HasVideoRender(PARENT), Step::MainRender(PARENT)]);
        assert_eq!(m.steps, want, "{flags:#x}");
    }
}

#[test]
fn parent_without_video_render_draws_async_or_deinterlaced() {
    for flags in [
        FLAG_CUSTOM_DRAW,
        FLAG_ASYNC,
        FLAG_ASYNC | FLAG_CUSTOM_DRAW,
        u32::MAX,
    ] {
        for deinterlacing in [false, true] {
            let mut m = Model::new();
            m.flags[PARENT] = flags;
            m.video_render[PARENT] = false;
            m.deinterlacing[PARENT] = deinterlacing;
            skip_video_filter(&mut m, Some(FILTER));
            let mut want = attached();
            want.extend([
                Step::HasVideoRender(PARENT),
                Step::Deinterlacing(PARENT),
                if deinterlacing {
                    Step::DeinterlaceRender(PARENT)
                } else {
                    Step::AsyncVideo(PARENT)
                },
            ]);
            assert_eq!(m.steps, want, "{flags:#x} {deinterlacing}");
        }
    }
}

#[test]
fn exactly_one_draw_for_every_input() {
    let draws = |s: &Step| {
        matches!(
            s,
            Step::DefaultRender(_)
                | Step::MainRender(_)
                | Step::DeinterlaceRender(_)
                | Step::AsyncVideo(_)
                | Step::VideoRender(_)
        )
    };
    for target in [PARENT, BELOW] {
        for bits in 0..16u32 {
            // async and custom draw in each combination, with the video
            // flag and the top bit on or off
            let flags = ((bits & 3) << 2) | ((bits >> 2) & 1) | ((bits >> 3) << 31);
            for (video_render, deinterlacing) in
                [(false, false), (false, true), (true, false), (true, true)]
            {
                let mut m = Model::new();
                m.target = Some(target);
                m.flags[PARENT] = flags;
                m.video_render[target] = video_render;
                m.deinterlacing[target] = deinterlacing;
                skip_video_filter(&mut m, Some(FILTER));
                assert_eq!(m.steps.iter().filter(|s| draws(s)).count(), 1);
                assert!(draws(m.steps.last().unwrap_or(&Step::LogNull)));
            }
        }
    }
}
