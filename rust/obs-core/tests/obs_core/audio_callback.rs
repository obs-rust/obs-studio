//! Tier 1 for `obs_core::audio_callback`, the audio tick of
//! `libobs/obs-audio.c.h`, over a model of the core.
//!
//! The model keeps the views, mixes and channels, the audio source list,
//! the render order, the root nodes, the buffered timestamps and the task
//! queue, and records every libobs call, decision call and log line in
//! order, with the locks held at the time. The tests check what the tick
//! renders, mixes, discards, releases and logs, and that it holds the
//! locks where the C does and none across a render or a task.
//!
//! `test_audio_buffering_subsample` pins the helpers' decision cores
//! (`audio_discard`, `audio_min_ts`, `audio_mix`, `audio_render`), whose
//! own Tier 1 modules port its assertions; this module covers the tick
//! that calls them.
//!
//! The intentional difference (a root source's `audio_pending` read under
//! its `audio_buf_mutex`) is `pending_is_read_under_the_buffer_lock`.

use std::collections::{BTreeSet, VecDeque};

use obs_core::audio_callback::{
    AUDIO_SIZE, Ignored, MAX_CHANNELS, Tick, audio_callback, push_tree2,
};
use obs_core::audio_mix::{Step, TsInfo};
use obs_core::audio_render::{SOURCE_AUDIO, TYPE_INPUT};

const TICK: u64 = 21_333_333;
const START: u64 = 1_000_000_000;
const END: u64 = START + TICK;
/// `OBS_SOURCE_TYPE_SCENE`.
const TYPE_SCENE: u32 = 3;

#[derive(Clone, Default)]
struct Src {
    active: bool,
    removed: bool,
    /// `obs_source_get_ref()` gives NULL: the source is being destroyed.
    dead: bool,
    duplicated: bool,
    pending: bool,
    audio_ts: u64,
    input_size: usize,
    kind: u32,
    output_flags: u32,
    audio_render: bool,
    output_buf: bool,
    muted: bool,
    volume: f32,
    mixers: u32,
    monitoring: bool,
    /// What `obs_source_enum_active_tree()` visits, in order.
    tree: Vec<usize>,
    /// What `obs_audio_discard_ignore()` reports.
    ignore: Ignored,
    next: Option<usize>,
    refs: i32,
}

/// A plain audio input: active, with a timestamp at the tick start and a
/// whole tick buffered.
fn input() -> Src {
    Src {
        active: true,
        audio_ts: START,
        input_size: AUDIO_SIZE,
        kind: TYPE_INPUT,
        output_flags: SOURCE_AUDIO,
        output_buf: true,
        volume: 1.0,
        mixers: 0x3f,
        ..Src::default()
    }
}

/// A scene: composite, rendered by `audio_render`.
fn scene(tree: Vec<usize>) -> Src {
    Src {
        kind: TYPE_SCENE,
        output_flags: SOURCE_AUDIO | (1 << 6),
        audio_render: true,
        tree,
        ..input()
    }
}

#[derive(Default)]
struct Model {
    src: Vec<Src>,
    first: Option<usize>,
    /// Each view's 64 channels.
    views: Vec<Vec<Option<usize>>>,
    /// Each mix: its view and `mix_audio`.
    mixes: Vec<(Option<usize>, bool)>,
    render: Vec<usize>,
    roots: Vec<usize>,
    stamps: VecDeque<TsInfo>,
    maxed: bool,
    fixed: bool,
    wait: u64,
    fixed_step: Step,
    add_step: Step,
    /// How far back the added buffering moves the tick.
    add_shift: u64,
    dedup: Option<usize>,
    tasks: VecDeque<usize>,
    /// Task `a`, when run, queues task `b`.
    queues: Vec<(usize, usize)>,
    ev: Vec<String>,
    held: BTreeSet<String>,
}

impl Model {
    fn new(src: Vec<Src>) -> Self {
        Self {
            src,
            ..Self::default()
        }
    }

    /// Links `list` as the audio source list, in order.
    fn audio_list(&mut self, list: &[usize]) {
        self.first = list.first().copied();
        for pair in list.windows(2) {
            self.src[pair[0]].next = Some(pair[1]);
        }
        if let Some(&last) = list.last() {
            self.src[last].next = None;
        }
    }

    /// A view whose channel `i` is `channels[i]`.
    fn view(&mut self, channels: &[Option<usize>]) -> usize {
        let mut all = vec![None; MAX_CHANNELS as usize];
        all[..channels.len()].copy_from_slice(channels);
        self.views.push(all);
        self.views.len() - 1
    }

    fn log(&mut self, ev: String) {
        self.ev.push(ev);
    }

    fn lock(&mut self, name: String) {
        assert!(self.held.insert(name.clone()), "{name} locked twice");
        self.log(format!("lock {name}"));
    }

    fn unlock(&mut self, name: String) {
        assert!(self.held.remove(&name), "{name} unlocked unheld");
        self.log(format!("unlock {name}"));
    }

    fn holds(&self, name: &str) -> bool {
        self.held.contains(name)
    }

    fn run(&mut self, mixers: u32) -> (bool, u64) {
        let out = audio_callback(self, START, END, mixers);
        assert!(self.held.is_empty(), "left locked: {:?}", self.held);
        out
    }

    fn has(&self, ev: &str) -> bool {
        self.ev.iter().any(|e| e == ev)
    }

    fn pos(&self, ev: &str) -> usize {
        self.ev
            .iter()
            .position(|e| e == ev)
            .unwrap_or_else(|| panic!("no {ev:?} in {:#?}", self.ev))
    }

    fn count(&self, prefix: &str) -> usize {
        self.ev.iter().filter(|e| e.starts_with(prefix)).count()
    }
}

impl Tick for Model {
    type Source = usize;
    type View = usize;
    type Name = usize;
    type Task = usize;

    fn sample_rate(&mut self) -> usize {
        48_000
    }
    fn channels(&mut self) -> usize {
        2
    }
    fn render_order_len(&mut self) -> usize {
        self.render.len()
    }
    fn render_order_get(&mut self, idx: usize) -> usize {
        self.render[idx]
    }
    fn render_order_find(&mut self, source: usize) -> Option<usize> {
        self.render.iter().position(|&s| s == source)
    }
    fn render_order_push(&mut self, source: usize) {
        self.render.push(source);
    }
    fn render_order_clear(&mut self) {
        self.render.clear();
    }
    fn root_nodes_len(&mut self) -> usize {
        self.roots.len()
    }
    fn root_nodes_get(&mut self, idx: usize) -> usize {
        self.roots[idx]
    }
    fn root_nodes_push(&mut self, source: usize) {
        self.roots.push(source);
    }
    fn root_nodes_clear(&mut self) {
        self.roots.clear();
    }
    fn timestamps_push_back(&mut self, ts: TsInfo) {
        self.stamps.push_back(ts);
    }
    fn timestamps_peek_front(&mut self) -> TsInfo {
        self.stamps[0]
    }
    fn timestamps_pop_front(&mut self) {
        self.stamps.pop_front();
    }
    fn buffering_maxed(&mut self) -> bool {
        self.maxed
    }
    fn buffering_fixed(&mut self, sample_rate: usize, ts: &mut TsInfo) -> Step {
        self.log(format!("fixed {sample_rate} {}", ts.start));
        self.fixed_step
    }
    fn buffering_add(&mut self, sample_rate: usize, ts: &mut TsInfo, min_ts: u64) -> Step {
        self.log(format!("add {sample_rate} {} {min_ts}", ts.start));
        if self.add_step.added {
            ts.start -= self.add_shift;
            ts.end -= self.add_shift;
            self.wait += u64::try_from(self.add_step.ticks).unwrap();
        }
        self.add_step
    }
    fn fixed_buffer(&mut self) -> bool {
        self.fixed
    }
    fn wait_ticks(&mut self) -> u64 {
        self.wait
    }
    fn set_wait_ticks(&mut self, value: u64) {
        self.wait = value;
    }
    fn set_out_ts(&mut self, value: u64) {
        self.log(format!("out_ts {value}"));
    }
    fn monitoring_duplicating_source(&mut self) -> Option<usize> {
        self.dedup
    }
    fn lock_tasks(&mut self) {
        self.lock("tasks".into());
    }
    fn unlock_tasks(&mut self) {
        self.unlock("tasks".into());
    }
    fn tasks_size(&mut self) -> usize {
        assert!(self.holds("tasks"));
        self.tasks.len() * 16
    }
    fn pop_task(&mut self) -> usize {
        assert!(self.holds("tasks"));
        self.tasks.pop_front().unwrap()
    }
    fn run_task(&mut self, task: usize) {
        assert!(self.held.is_empty(), "task {task} under {:?}", self.held);
        self.log(format!("task {task}"));
        let queued: Vec<usize> = self
            .queues
            .iter()
            .filter(|(a, _)| *a == task)
            .map(|(_, b)| *b)
            .collect();
        self.tasks.extend(queued);
    }
    fn first_audio_source(&mut self) -> Option<usize> {
        assert!(self.holds("sources"));
        self.first
    }
    fn lock_audio_sources(&mut self) {
        assert!(!self.holds("mixes"), "audio sources under mixes");
        self.lock("sources".into());
    }
    fn unlock_audio_sources(&mut self) {
        self.unlock("sources".into());
    }
    fn lock_mixes(&mut self) {
        self.lock("mixes".into());
    }
    fn unlock_mixes(&mut self) {
        self.unlock("mixes".into());
    }
    fn mixes_len(&mut self) -> usize {
        assert!(self.holds("mixes"));
        self.mixes.len()
    }
    fn mix_view(&mut self, idx: usize) -> Option<usize> {
        assert!(self.holds("mixes"));
        self.mixes[idx].0
    }
    fn mix_audio(&mut self, idx: usize) -> bool {
        assert!(self.holds("mixes"));
        self.mixes[idx].1
    }
    fn lock_channels(&mut self, view: usize) {
        assert!(self.holds("mixes"), "channels outside mixes");
        self.lock(format!("view{view}"));
    }
    fn unlock_channels(&mut self, view: usize) {
        self.unlock(format!("view{view}"));
    }
    fn channel(&mut self, view: usize, idx: u32) -> Option<usize> {
        assert!(self.holds(&format!("view{view}")));
        self.views[view][idx as usize]
    }
    fn next_audio_source(&mut self, source: usize) -> Option<usize> {
        assert!(self.holds("sources"));
        self.src[source].next
    }
    fn lock_buf(&mut self, source: usize) {
        self.lock(format!("buf{source}"));
    }
    fn unlock_buf(&mut self, source: usize) {
        self.unlock(format!("buf{source}"));
    }
    fn is_duplicated(&mut self, source: usize) -> bool {
        self.src[source].duplicated
    }
    fn set_is_duplicated(&mut self, source: usize, value: bool) {
        self.src[source].duplicated = value;
    }
    fn pending(&mut self, source: usize) -> bool {
        let locked = self.holds(&format!("buf{source}"));
        self.log(format!(
            "pending {source} {}",
            if locked { "locked" } else { "unlocked" }
        ));
        self.src[source].pending
    }
    fn set_pending(&mut self, source: usize, value: bool) {
        assert!(self.holds(&format!("buf{source}")));
        self.log(format!("set pending {source} {value}"));
        self.src[source].pending = value;
    }
    fn audio_ts(&mut self, source: usize) -> u64 {
        self.src[source].audio_ts
    }
    fn input_size(&mut self, source: usize) -> usize {
        assert!(self.holds(&format!("buf{source}")));
        self.src[source].input_size
    }
    fn kind(&mut self, source: usize) -> u32 {
        self.src[source].kind
    }
    fn output_flags(&mut self, source: usize) -> u32 {
        self.src[source].output_flags
    }
    fn has_audio_render(&mut self, source: usize) -> bool {
        self.src[source].audio_render
    }
    fn has_output_buf(&mut self, source: usize) -> bool {
        self.src[source].output_buf
    }
    fn muted(&mut self, source: usize) -> bool {
        self.src[source].muted
    }
    fn volume(&mut self, source: usize) -> f32 {
        self.src[source].volume
    }
    fn audio_mixers(&mut self, source: usize) -> u32 {
        self.src[source].mixers
    }
    fn monitoring_enabled(&mut self, source: usize) -> bool {
        self.src[source].monitoring
    }
    fn clear_output(&mut self, source: usize, mixes: u32) {
        self.log(format!("clear {source} {mixes:#x}"));
    }
    fn ignore(
        &mut self,
        source: usize,
        channels: usize,
        sample_rate: usize,
        start_ts: u64,
    ) -> Ignored {
        assert!(self.holds(&format!("buf{source}")));
        self.log(format!(
            "ignore {source} {channels} {sample_rate} {start_ts}"
        ));
        self.src[source].ignore
    }
    fn discard_tick(
        &mut self,
        source: usize,
        audio_render: bool,
        channels: usize,
        sample_rate: usize,
        ts: TsInfo,
    ) {
        assert!(self.holds("sources") && self.holds(&format!("buf{source}")));
        self.log(format!(
            "discard {source} {audio_render} {channels} {sample_rate} {}",
            ts.start
        ));
    }
    fn mix(&mut self, source: usize, channels: usize, sample_rate: usize, ts: TsInfo, mixers: u32) {
        assert!(self.holds(&format!("buf{source}")));
        self.log(format!(
            "mix {source} {channels} {sample_rate} {} {mixers:#x}",
            ts.start
        ));
    }
    fn get_ref(&mut self, source: usize) -> Option<usize> {
        self.log(format!("ref {source}"));
        if self.src[source].dead {
            return None;
        }
        self.src[source].refs += 1;
        Some(source)
    }
    fn release(&mut self, source: usize) {
        self.log(format!("release {source}"));
        self.src[source].refs -= 1;
    }
    fn name(&mut self, source: usize) -> usize {
        source
    }
    fn removed(&mut self, source: usize) -> bool {
        self.src[source].removed
    }
    fn active(&mut self, source: usize) -> bool {
        self.log(format!("active {source}"));
        self.src[source].active
    }
    fn audio_render(
        &mut self,
        source: usize,
        mixers: u32,
        channels: usize,
        sample_rate: usize,
        size: usize,
    ) {
        assert!(
            self.held.is_empty(),
            "render {source} under {:?}",
            self.held
        );
        self.log(format!(
            "render {source} {mixers:#x} {channels} {sample_rate} {size}"
        ));
    }
    fn enum_active_tree(&mut self, source: usize) {
        assert!(self.holds("mixes"));
        self.log(format!("tree {source}"));
        for child in self.src[source].tree.clone() {
            push_tree2(self, child);
        }
    }
    fn log_backwards(&mut self, name: usize) {
        self.log(format!("log backwards {name}"));
    }
    fn log_ts_lag(&mut self, name: usize, lag: u64) {
        self.log(format!("log lag {name} {lag}"));
    }
    fn log_lagging(&mut self, name: usize, lag: u64) {
        self.log(format!("log lagging {name} {lag}"));
    }
    fn log_fixed(&mut self, total_ms: i32) {
        self.log(format!("log fixed {total_ms}"));
    }
    fn log_max_reached(&mut self) {
        self.log("log max".into());
    }
    fn log_adding(&mut self, ms: i32, total_ms: i32, name: Option<usize>) {
        self.log(format!("log adding {ms} {total_ms} {name:?}"));
    }
}

/// A scene in one view, with two inputs under it, the inputs also in the
/// audio source list.
fn one_scene() -> Model {
    let mut m = Model::new(vec![scene(vec![1, 2]), input(), input()]);
    let view = m.view(&[Some(0)]);
    m.mixes.push((Some(view), true));
    m.audio_list(&[1, 2]);
    m
}

#[test]
fn a_scene_renders_its_tree_and_mixes_as_the_root() {
    let mut m = one_scene();
    assert_eq!(m.run(0x1), (true, START));
    assert_eq!(m.render, vec![1, 2, 0]);
    assert_eq!(m.roots, vec![0]);
    for s in [0, 1, 2] {
        assert!(m.has(&format!("render {s} 0x1 2 48000 {AUDIO_SIZE}")));
        assert!(m.has(&format!("discard {s} {} 2 48000 {START}", s == 0)) == (s != 0));
        assert_eq!(m.src[s].refs, 0, "source {s} released");
    }
    assert!(m.has(&format!("mix 0 2 48000 {START} 0x1")));
    assert_eq!(m.count("mix "), 1);
    // the render order is the tree, then the channel source
    assert!(m.pos("render 1 0x1 2 48000 4096") < m.pos("render 2 0x1 2 48000 4096"));
    assert!(m.pos("render 2 0x1 2 48000 4096") < m.pos("render 0 0x1 2 48000 4096"));
    assert!(m.stamps.is_empty());
}

#[test]
fn locks_follow_the_c_order() {
    let mut m = one_scene();
    m.run(0x3f);
    assert!(m.pos("lock mixes") < m.pos("lock view0"));
    assert!(m.pos("unlock view0") < m.pos("unlock mixes"));
    assert!(m.pos("unlock mixes") < m.pos("lock sources"));
    assert!(m.pos("tree 0") > m.pos("lock view0"));
    assert!(m.pos("tree 0") < m.pos("unlock view0"));
}

#[test]
fn a_shared_view_is_walked_once() {
    let mut m = one_scene();
    m.mixes.push((Some(0), true));
    m.run(0x1);
    assert_eq!(m.count("tree 0"), 1);
    assert_eq!(m.roots, vec![0]);
}

#[test]
fn empty_views_and_mixes_without_audio() {
    let mut m = one_scene();
    m.mixes[0].1 = false;
    m.mixes.insert(0, (None, true));
    m.run(0x1);
    assert_eq!(m.render, vec![1, 2, 0]);
    assert!(m.roots.is_empty());
    assert_eq!(m.count("mix "), 0);
}

#[test]
fn inactive_and_removed_channels_are_skipped() {
    let mut m = Model::new(vec![scene(vec![]), scene(vec![]), input()]);
    m.src[0].active = false;
    m.src[1].removed = true;
    let view = m.view(&[Some(0), None, Some(1), Some(2)]);
    m.mixes.push((Some(view), true));
    m.run(0x1);
    assert_eq!(m.render, vec![2]);
    assert_eq!(m.roots, vec![2]);
    assert!(m.has("active 0") && m.has("active 1") && m.has("active 2"));
    assert_eq!(m.count("tree "), 1);
}

#[test]
fn a_source_reached_twice_is_a_root_node() {
    // input 2 under both scenes
    let mut m = Model::new(vec![scene(vec![2]), scene(vec![2]), input()]);
    let view = m.view(&[Some(0), Some(1)]);
    m.mixes.push((Some(view), true));
    m.audio_list(&[2]);
    m.run(0x1);
    assert_eq!(m.render, vec![2, 0, 1]);
    // scene 1 sees input 2 already in the order: tagged and a root
    assert!(m.src[2].duplicated);
    assert_eq!(m.roots, vec![0, 1, 2]);
    assert_eq!(m.count("ref 2"), 1);
    // ... but a third visit doesn't tag it again
    let mut m = Model::new(vec![scene(vec![1, 1, 1])]);
    m.src.push(input());
    let view = m.view(&[Some(0)]);
    m.mixes.push((Some(view), false));
    m.run(0x1);
    assert_eq!(m.roots, vec![1]);
}

#[test]
fn a_composite_reached_twice_is_not_tagged() {
    let mut m = Model::new(vec![scene(vec![1]), scene(vec![1]), scene(vec![])]);
    let view = m.view(&[Some(0), Some(2)]);
    m.mixes.push((Some(view), false));
    m.src[2].tree = vec![1];
    m.run(0x1);
    assert!(!m.src[1].duplicated);
    assert!(m.roots.is_empty());
}

#[test]
fn render_order_resets_the_duplicate_tag() {
    let mut m = one_scene();
    m.src[1].duplicated = true;
    m.run(0x1);
    assert!(!m.src[1].duplicated);
}

#[test]
fn removed_and_dying_sources_stay_out() {
    let mut m = Model::new(vec![input(), input(), input()]);
    m.src[1].removed = true;
    m.src[2].dead = true;
    m.audio_list(&[0, 1, 2]);
    m.run(0x1);
    assert_eq!(m.render, vec![0]);
    assert!(!m.has("ref 1"));
    assert!(m.has("ref 2"));
    // every audio source is still discarded
    for s in [0, 1, 2] {
        assert!(m.has(&format!("discard {s} false 2 48000 {START}")));
    }
    // a removed source in a tree is skipped
    let mut m = Model::new(vec![scene(vec![1]), input()]);
    m.src[1].removed = true;
    let view = m.view(&[Some(0)]);
    m.mixes.push((Some(view), true));
    m.run(0x1);
    assert_eq!(m.render, vec![0]);
}

/// A deduplication source 0 and a monitored source 1.
fn monitored() -> Model {
    let mut m = Model::new(vec![input(), input()]);
    m.src[0].mixers = 0b0110;
    m.src[1].mixers = 0b0011;
    m.src[1].monitoring = true;
    m.audio_list(&[0, 1]);
    m.dedup = Some(0);
    m
}

#[test]
fn a_monitored_source_is_silenced_while_the_capture_records_it() {
    let mut m = monitored();
    m.run(0x3f);
    assert!(m.has("clear 1 0x2"));
    assert!(!m.has("clear 0 0x6"));
    assert!(m.pos("render 1 0x3f 2 48000 4096") < m.pos("clear 1 0x2"));
    // the active check is the dedup source's
    assert!(m.has("active 0"));
}

#[test]
fn a_muted_or_silent_capture_silences_nothing() {
    let changes: [fn(&mut Model); 5] = [
        |m: &mut Model| m.src[0].muted = true,
        |m: &mut Model| m.src[0].volume = 0.0,
        |m: &mut Model| m.src[0].active = false,
        |m: &mut Model| m.src[1].monitoring = false,
        |m: &mut Model| m.dedup = None,
    ];
    for change in changes {
        let mut m = monitored();
        change(&mut m);
        m.run(0x3f);
        assert_eq!(m.count("clear "), 0);
    }
}

#[test]
fn a_source_with_audio_render_gone_backwards_goes_pending() {
    let mut m = Model::new(vec![scene(vec![])]);
    m.src[0].audio_ts = START - 1;
    m.maxed = true;
    m.audio_list(&[0]);
    m.run(0x1);
    assert!(m.has("log backwards 0"));
    assert!(m.has("set pending 0 true"));
    assert_eq!(m.count("render 0"), 1);
    assert!(m.pos("lock buf0") < m.pos("set pending 0 true"));
}

#[test]
fn an_input_gone_backwards_is_ignored_and_rendered_again() {
    let mut m = Model::new(vec![input()]);
    m.src[0].audio_ts = START - 1;
    m.src[0].ignore = Ignored {
        rerender: true,
        buffered: 8,
        lag: 77,
        warn: true,
        warn_lag: 1_500_000,
        ..Ignored::default()
    };
    m.maxed = true;
    m.audio_list(&[0]);
    m.run(0x1);
    assert!(m.has(&format!("ignore 0 2 48000 {START}")));
    assert!(m.has("log lag 0 77"));
    assert!(m.has("log lagging 0 1500000"));
    assert_eq!(m.count("render 0"), 2);
    // the second render waits for the buffer lock to go
    let second =
        m.ev.iter()
            .rposition(|e| e.starts_with("render 0"))
            .unwrap();
    assert!(m.pos("unlock buf0") < second);
}

#[test]
fn an_ignore_without_timestamp_logs_nothing() {
    let mut m = Model::new(vec![input()]);
    m.src[0].audio_ts = START - 1;
    m.src[0].ignore = Ignored {
        rerender: true,
        no_timestamp: true,
        buffered: 8,
        warn: true,
        ..Ignored::default()
    };
    m.maxed = true;
    m.audio_list(&[0]);
    m.run(0x1);
    assert_eq!(m.count("log "), 0);
    assert_eq!(m.count("render 0"), 1);
    // nothing buffered: no lag line, the warning alone
    let mut m = Model::new(vec![input()]);
    m.src[0].audio_ts = START - 1;
    m.src[0].ignore = Ignored {
        warn: true,
        warn_lag: 9,
        ..Ignored::default()
    };
    m.maxed = true;
    m.audio_list(&[0]);
    m.run(0x1);
    assert_eq!(m.count("log lag "), 0);
    assert!(m.has("log lagging 0 9"));
    assert_eq!(m.count("render 0"), 1);
}

#[test]
fn not_maxed_drops_nothing() {
    let mut m = Model::new(vec![input()]);
    m.src[0].audio_ts = START - 1;
    m.audio_list(&[0]);
    m.run(0x1);
    assert_eq!(m.count("ignore "), 0);
    assert_eq!(m.count("set pending"), 0);
}

#[test]
fn an_early_source_adds_buffering_and_names_itself() {
    let mut m = Model::new(vec![input(), input()]);
    m.src[1].audio_ts = START - TICK;
    m.audio_list(&[0, 1]);
    m.add_step = Step {
        added: true,
        max_reached: true,
        ticks: 1,
        ms: 21,
        total_ms: 42,
    };
    m.add_shift = TICK;
    let view = m.view(&[Some(1)]);
    m.mixes.push((Some(view), true));
    assert_eq!(m.run(0x1), (false, START - TICK));
    assert!(m.has(&format!("add 48000 {START} {}", START - TICK)));
    assert!(m.pos("log max") < m.pos("log adding 21 42 Some(1)"));
    // one wait tick: no mix, no tasks, counted down
    assert_eq!(m.count("mix "), 0);
    assert_eq!(m.wait, 0);
    // the discard runs on the moved tick
    assert!(m.has(&format!("discard 0 false 2 48000 {}", START - TICK)));
}

#[test]
fn buffering_that_adds_nothing_logs_nothing() {
    let mut m = Model::new(vec![input()]);
    m.src[0].audio_ts = START - TICK;
    m.audio_list(&[0]);
    m.run(0x1);
    assert!(m.has(&format!("add 48000 {START} {}", START - TICK)));
    assert_eq!(m.count("log "), 0);
}

#[test]
fn pending_and_untimed_sources_do_not_lower_the_minimum() {
    let mut m = Model::new(vec![input(), input()]);
    m.src[0].audio_ts = START - TICK;
    m.src[0].pending = true;
    m.src[1].audio_ts = 0;
    m.audio_list(&[0, 1]);
    m.run(0x1);
    assert_eq!(m.count("add "), 0);
}

#[test]
fn a_short_source_goes_pending_and_the_minimum_is_found_again() {
    // source 0 is early but short; source 1 is early and full
    let mut m = Model::new(vec![input(), input()]);
    m.src[0].audio_ts = START - 2 * TICK;
    m.src[0].input_size = 4;
    m.src[1].audio_ts = START - TICK;
    m.audio_list(&[0, 1]);
    m.add_step = Step {
        added: true,
        ticks: 1,
        ms: 21,
        total_ms: 21,
        ..Step::default()
    };
    m.run(0x1);
    assert!(m.has("set pending 0 true"));
    // Characterized, not endorsed: the second search starts from the
    // first's minimum, so it keeps that minimum and, its comparison being
    // strict, names no source.
    assert!(m.has(&format!("add 48000 {START} {}", START - 2 * TICK)));
    assert!(m.has("log adding 21 21 None"));
}

#[test]
fn an_early_source_with_enough_audio_is_named() {
    let mut m = Model::new(vec![input(), input()]);
    m.src[1].audio_ts = START - TICK;
    m.audio_list(&[0, 1]);
    m.add_step = Step {
        added: true,
        ..Step::default()
    };
    m.run(0x1);
    assert!(m.has("log adding 0 0 Some(1)"));
    assert_eq!(m.count("set pending"), 0);
}

#[test]
fn a_minimum_from_the_first_pass_with_no_source_left_names_none() {
    let mut m = Model::new(vec![input()]);
    m.src[0].audio_ts = START - TICK;
    m.src[0].input_size = 4;
    m.audio_list(&[0]);
    m.add_step = Step {
        added: true,
        ticks: 1,
        ..Step::default()
    };
    m.run(0x1);
    assert!(m.has("log adding 0 0 None"));
}

#[test]
fn fixed_buffering_fills_until_maxed() {
    let mut m = Model::new(vec![input()]);
    m.audio_list(&[0]);
    m.fixed = true;
    m.fixed_step = Step {
        added: true,
        total_ms: 1000,
        ..Step::default()
    };
    m.run(0x1);
    assert!(m.has(&format!("fixed 48000 {START}")));
    assert!(m.has("log fixed 1000"));
    assert_eq!(m.count("add "), 0);

    let mut m = Model::new(vec![input()]);
    m.src[0].audio_ts = START - TICK;
    m.audio_list(&[0]);
    m.fixed = true;
    m.maxed = true;
    m.run(0x1);
    assert_eq!(m.count("fixed "), 0);
    assert_eq!(m.count("add "), 0);

    let mut m = Model::new(vec![input()]);
    m.audio_list(&[0]);
    m.fixed = true;
    m.run(0x1);
    assert!(m.has(&format!("fixed 48000 {START}")));
    assert_eq!(m.count("log "), 0);
}

#[test]
fn roots_mix_only_when_ready() {
    // pending, no output buffer, no timestamp: none mixed
    let mut m = Model::new(vec![input(), input(), input(), input()]);
    m.src[0].pending = true;
    m.src[1].output_buf = false;
    m.src[2].audio_ts = 0;
    let view = m.view(&[Some(0), Some(1), Some(2), Some(3)]);
    m.mixes.push((Some(view), true));
    m.run(0x5);
    assert_eq!(m.roots, vec![0, 1, 2, 3]);
    assert_eq!(m.count("mix "), 1);
    assert!(m.has(&format!("mix 3 2 48000 {START} 0x5")));
}

#[test]
fn pending_is_read_under_the_buffer_lock() {
    let mut m = Model::new(vec![input(), input()]);
    m.src[0].pending = true;
    let view = m.view(&[Some(0), Some(1)]);
    m.mixes.push((Some(view), true));
    m.audio_list(&[0, 1]);
    m.run(0x1);
    let reads: Vec<&String> = m.ev.iter().filter(|e| e.starts_with("pending ")).collect();
    assert!(!reads.is_empty());
    for read in reads {
        assert!(read.ends_with(" locked"), "{read}");
    }
    assert_eq!(m.count("mix 0"), 0);
    assert!(m.has(&format!("mix 1 2 48000 {START} 0x1")));
}

#[test]
fn a_wait_tick_is_counted_down_without_mixing_or_tasks() {
    let mut m = one_scene();
    m.wait = 3;
    m.tasks.push_back(1);
    assert_eq!(m.run(0x1), (false, START));
    assert_eq!(m.wait, 2);
    assert_eq!(m.count("mix "), 0);
    assert_eq!(m.count("task "), 0);
    // still discarded and released
    assert_eq!(m.count("discard "), 2);
    assert_eq!(m.count("release "), 3);
}

#[test]
fn tasks_run_in_order_outside_the_lock_and_see_new_ones() {
    let mut m = one_scene();
    m.tasks.extend([1, 2]);
    m.queues.push((1, 3));
    assert_eq!(m.run(0x1), (true, START));
    let tasks: Vec<&String> = m.ev.iter().filter(|e| e.starts_with("task ")).collect();
    assert_eq!(tasks, vec!["task 1", "task 2", "task 3"]);
    assert!(m.tasks.is_empty());
    // tasks run after the releases
    assert!(m.pos("task 1") > m.ev.iter().rposition(|e| e.starts_with("release")).unwrap());
}

#[test]
fn the_tick_comes_from_the_front_of_the_queue() {
    // an earlier tick still queued: this one is buffered behind it
    let mut m = one_scene();
    let early = TsInfo {
        start: START - TICK,
        end: START,
    };
    m.stamps.push_back(early);
    assert_eq!(m.run(0x1), (true, START - TICK));
    assert_eq!(
        m.stamps,
        VecDeque::from([TsInfo {
            start: START,
            end: END
        }])
    );
    assert!(m.has(&format!("mix 0 2 48000 {} 0x1", START - TICK)));
}

#[test]
fn every_source_is_released_once() {
    let mut m = Model::new(vec![scene(vec![1, 2]), input(), input(), input()]);
    let view = m.view(&[Some(0), Some(0)]);
    m.mixes.push((Some(view), true));
    m.audio_list(&[1, 2, 3]);
    m.run(0x1);
    assert_eq!(m.render, vec![1, 2, 0, 3]);
    for s in 0..4 {
        assert_eq!(m.src[s].refs, 0);
        assert_eq!(m.count(&format!("release {s}")), 1);
    }
    // the render order is cleared at the start of the next tick
    m.run(0x1);
    assert_eq!(m.render, vec![1, 2, 0, 3]);
}

#[test]
fn out_ts_is_set_before_the_wait_tick_and_the_tasks() {
    // audio_callback() sets *out_ts before it counts a wait tick down or
    // runs the tasks, so a task sees this tick's start.
    let mut m = one_scene();
    m.tasks.push_back(1);
    m.run(0x1);
    assert_eq!(m.count("out_ts "), 1);
    assert!(m.pos(&format!("out_ts {START}")) < m.pos("task 1"));
    let last_release = m.ev.iter().rposition(|e| e.starts_with("release")).unwrap();
    assert!(m.pos(&format!("out_ts {START}")) > last_release);

    let mut m = one_scene();
    m.wait = 1;
    assert_eq!(m.run(0x1), (false, START));
    assert!(m.has(&format!("out_ts {START}")));
}
