//! Tier 1: `obs_source::get_color_space`, the shell of
//! `obs_source_get_color_space()` around the colour space routing.
//!
//! A model of what the shell reaches: sources in an arena (index, or `None`
//! for NULL) with their routing fields, filter parent and target, async
//! frame space and optional callback. Every call is traced, so the cases pin
//! the order of the C: `data_valid()` first, then the field read, then one
//! of the parent, the target, the pick or the callback; the parent or target
//! is asked again from the top (`data_valid()` included).

use obs_source::color_space::{ASYNC, Node, TYPE_FILTER, VIDEO};
use obs_source::get_color_space::{Libobs, SRGB, get};
use proptest::prelude::*;

const GS_CS_SRGB_16F: i32 = 1;
const GS_CS_709_EXTENDED: i32 = 2;
const GS_CS_709_SCRGB: i32 = 3;

#[derive(Debug, Clone, Default)]
struct Src {
    node: Node,
    parent: Option<usize>,
    target: Option<usize>,
    async_space: i32,
    callback: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Valid(Option<usize>),
    Node(usize),
    Parent(usize),
    Target(usize),
    AsyncSpace(usize),
    Pick(i32),
    Callback(usize),
}

struct Model<'a> {
    sources: Vec<Src>,
    trace: Vec<Call>,
    logs: usize,
    seen_preferred: Vec<&'a [i32]>,
}

impl<'a> Model<'a> {
    fn new(sources: Vec<Src>) -> Self {
        Self {
            sources,
            trace: Vec::new(),
            logs: 0,
            seen_preferred: Vec::new(),
        }
    }
    fn src(&self, i: usize) -> &Src {
        &self.sources[i]
    }
}

impl<'a> Libobs for Model<'a> {
    type Source = Option<usize>;
    type Preferred = &'a [i32];

    fn data_valid(&mut self, source: Option<usize>) -> bool {
        self.trace.push(Call::Valid(source));
        match source {
            None => {
                self.logs += 1;
                false
            }
            Some(i) => self.src(i).node.has_data,
        }
    }
    fn node(&mut self, source: Option<usize>) -> Node {
        let i = source.unwrap();
        self.trace.push(Call::Node(i));
        self.src(i).node
    }
    fn filter_parent(&mut self, source: Option<usize>) -> Option<usize> {
        let i = source.unwrap();
        self.trace.push(Call::Parent(i));
        self.src(i).parent
    }
    fn filter_target(&mut self, source: Option<usize>) -> Option<usize> {
        let i = source.unwrap();
        self.trace.push(Call::Target(i));
        self.src(i).target
    }
    fn async_space(&mut self, source: Option<usize>) -> i32 {
        let i = source.unwrap();
        self.trace.push(Call::AsyncSpace(i));
        self.src(i).async_space
    }
    fn pick(&mut self, video_space: i32, preferred: &'a [i32]) -> i32 {
        self.trace.push(Call::Pick(video_space));
        self.seen_preferred.push(preferred);
        obs_source::color_space::pick(video_space, preferred)
    }
    fn callback(&mut self, source: Option<usize>, preferred: &'a [i32]) -> Option<i32> {
        let i = source.unwrap();
        self.trace.push(Call::Callback(i));
        self.seen_preferred.push(preferred);
        self.src(i).callback
    }
}

fn input(flags: u32) -> Src {
    Src {
        node: Node {
            source_type: 0,
            output_flags: flags,
            has_data: true,
            enabled: true,
            has_filter_parent: false,
            has_filter_target: false,
        },
        ..Src::default()
    }
}

fn filter(flags: u32, target: usize) -> Src {
    let mut s = input(flags);
    s.node.source_type = TYPE_FILTER;
    s.node.has_filter_target = true;
    s.target = Some(target);
    s
}

const LIST: &[i32] = &[GS_CS_SRGB_16F, GS_CS_709_EXTENDED];

#[test]
fn null_source_logs_once_and_answers_srgb() {
    let mut m = Model::new(vec![]);
    assert_eq!(get(&mut m, None, LIST), SRGB);
    assert_eq!(m.trace, vec![Call::Valid(None)]);
    assert_eq!(m.logs, 1);
}

#[test]
fn no_data_answers_srgb_without_reading_fields() {
    let mut s = input(VIDEO);
    s.node.has_data = false;
    s.callback = Some(GS_CS_709_SCRGB);
    let mut m = Model::new(vec![s]);
    assert_eq!(get(&mut m, Some(0), LIST), SRGB);
    assert_eq!(m.trace, vec![Call::Valid(Some(0))]);
    assert_eq!(m.logs, 0);
}

#[test]
fn callback_answer_is_returned_with_the_callers_list() {
    let mut s = input(VIDEO);
    s.callback = Some(GS_CS_709_SCRGB);
    let mut m = Model::new(vec![s]);
    assert_eq!(get(&mut m, Some(0), LIST), GS_CS_709_SCRGB);
    assert_eq!(
        m.trace,
        vec![Call::Valid(Some(0)), Call::Node(0), Call::Callback(0)]
    );
    assert!(std::ptr::eq(m.seen_preferred[0], LIST));
}

#[test]
fn no_callback_answers_srgb() {
    let mut m = Model::new(vec![input(VIDEO)]);
    assert_eq!(get(&mut m, Some(0), LIST), SRGB);
    assert_eq!(
        m.trace,
        vec![Call::Valid(Some(0)), Call::Node(0), Call::Callback(0)]
    );
}

#[test]
fn async_source_picks_against_its_frame_space() {
    let mut s = input(VIDEO | ASYNC);
    s.async_space = GS_CS_709_EXTENDED;
    s.callback = Some(GS_CS_709_SCRGB);
    let mut m = Model::new(vec![s]);
    assert_eq!(get(&mut m, Some(0), LIST), GS_CS_709_EXTENDED);
    assert_eq!(
        m.trace,
        vec![
            Call::Valid(Some(0)),
            Call::Node(0),
            Call::AsyncSpace(0),
            Call::Pick(GS_CS_709_EXTENDED)
        ]
    );
    let mut m = Model::new(vec![m.sources[0].clone()]);
    assert_eq!(get(&mut m, Some(0), &[]), GS_CS_709_EXTENDED);
    let mut m = Model::new(vec![m.sources[0].clone()]);
    assert_eq!(get(&mut m, Some(0), &[GS_CS_SRGB_16F]), GS_CS_SRGB_16F);
}

#[test]
fn audio_filter_asks_its_target_from_the_top() {
    let mut src = input(VIDEO);
    src.callback = Some(GS_CS_709_EXTENDED);
    let mut m = Model::new(vec![src, filter(0, 0)]);
    assert_eq!(get(&mut m, Some(1), LIST), GS_CS_709_EXTENDED);
    assert_eq!(
        m.trace,
        vec![
            Call::Valid(Some(1)),
            Call::Node(1),
            Call::Target(1),
            Call::Valid(Some(0)),
            Call::Node(0),
            Call::Callback(0)
        ]
    );
}

#[test]
fn disabled_video_filter_asks_its_target() {
    let mut src = input(VIDEO);
    src.callback = Some(GS_CS_709_SCRGB);
    let mut f = filter(VIDEO, 0);
    f.node.enabled = false;
    f.callback = Some(GS_CS_SRGB_16F);
    let mut m = Model::new(vec![src, f]);
    assert_eq!(get(&mut m, Some(1), LIST), GS_CS_709_SCRGB);
}

#[test]
fn audio_only_source_asks_its_filter_parent() {
    let mut parent = input(VIDEO);
    parent.callback = Some(GS_CS_SRGB_16F);
    let mut child = input(0);
    child.node.has_filter_parent = true;
    child.parent = Some(0);
    let mut m = Model::new(vec![parent, child]);
    assert_eq!(get(&mut m, Some(1), LIST), GS_CS_SRGB_16F);
    assert_eq!(
        m.trace,
        vec![
            Call::Valid(Some(1)),
            Call::Node(1),
            Call::Parent(1),
            Call::Valid(Some(0)),
            Call::Node(0),
            Call::Callback(0)
        ]
    );
}

#[test]
fn a_null_hop_logs_and_answers_srgb() {
    // has_filter_target was read as true, then the target read gives NULL.
    let mut f = filter(0, 0);
    f.target = None;
    let mut m = Model::new(vec![f]);
    assert_eq!(get(&mut m, Some(0), LIST), SRGB);
    assert_eq!(m.logs, 1);
    assert_eq!(m.trace.last(), Some(&Call::Valid(None)));
}

#[test]
fn a_chain_of_filters_is_walked_in_order() {
    let mut src = input(VIDEO);
    src.callback = Some(GS_CS_709_SCRGB);
    let mut m = Model::new(vec![src, filter(0, 0), filter(0, 1), filter(0, 2)]);
    assert_eq!(get(&mut m, Some(3), LIST), GS_CS_709_SCRGB);
    let targets: Vec<_> = m
        .trace
        .iter()
        .filter_map(|c| match c {
            Call::Target(i) => Some(*i),
            _ => None,
        })
        .collect();
    assert_eq!(targets, vec![3, 2, 1]);
}

proptest! {
    /// Single source, any flags and fields: the shell answers what the
    /// route sends it to, or sRGB.
    #[test]
    fn single_source_matches_route(flags in any::<u32>(), ty in -2i32..5, data: bool, enabled: bool,
                                   cb in proptest::option::of(-3i32..8), space in -3i32..8,
                                   list in proptest::collection::vec(-3i32..8, 0..6)) {
        let mut s = input(flags);
        s.node.source_type = ty;
        s.node.has_data = data;
        s.node.enabled = enabled;
        s.callback = cb;
        s.async_space = space;
        let mut m = Model::new(vec![s.clone()]);
        let got = get(&mut m, Some(0), &list);
        let want = if !data {
            SRGB
        } else if flags & ASYNC != 0 {
            obs_source::color_space::pick(space, &list)
        } else {
            cb.unwrap_or(SRGB)
        };
        prop_assert_eq!(got, want);
        prop_assert_eq!(m.logs, 0);
    }
}
