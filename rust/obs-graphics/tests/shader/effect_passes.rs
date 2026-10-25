//! Tier 1: the safe core of `libobs/graphics/effect-passes.c.h`.
//!
//! The first 6 tests mirror `test/cmocka/test_effect_pass_loop.c` one to
//! one, with the same names, the same fixture (technique "Draw", shaders
//! 10+i / 20+i, shader params 1, 2, 3 and the texture 100) and the same
//! expected call sequences and state, on a plain [`Model`] with a recording
//! device instead of the C structs and stubs. NULL techniques and effects
//! are `None` here and the shim's; the cmocka NULL calls map to those.
//!
//! The cases after those (`edge_*`) pin the edges, and a proptest checks
//! that no call sequence panics and that a failed `gs_effect_loop` without a
//! warning always resets the loop.

use obs_graphics::effect_passes::{
    LoopWarning, PassModel, Stage, effect_loop, effect_update_params, technique_begin,
    technique_begin_pass, technique_begin_pass_by_name, technique_end, technique_end_pass,
};
use proptest::prelude::*;

#[derive(Debug, Clone, Default)]
struct Param {
    is_texture: bool,
    value: Vec<u8>,
    default: Vec<u8>,
    changed: bool,
    sampler: u32,
}

#[derive(Debug, Clone)]
struct Pass {
    name: &'static [u8],
    vs: u32,
    ps: u32,
    vparams: Vec<(usize, u32)>,
    pparams: Vec<(usize, u32)>,
}

#[derive(Debug, Clone)]
struct Model {
    techniques: Vec<(&'static [u8], Vec<Pass>)>,
    params: Vec<Param>,
    looping: bool,
    loop_pass: usize,
    cur_technique: Option<usize>,
    cur_pass: Option<(usize, usize)>,
    current: bool,
    /// Another effect is current on the device.
    other_active: bool,
    events: Vec<String>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().take(24).map(|b| format!("{b:02x}")).collect()
}

impl PassModel for Model {
    type Technique = usize;
    type Pass = (usize, usize);
    type Param = usize;
    type SParam = u32;

    fn looping(&self) -> bool {
        self.looping
    }
    fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }
    fn loop_pass(&self) -> usize {
        self.loop_pass
    }
    fn set_loop_pass(&mut self, pass: usize) {
        self.loop_pass = pass;
    }
    fn cur_technique(&self) -> Option<usize> {
        self.cur_technique
    }
    fn set_cur_technique(&mut self, tech: Option<usize>) {
        self.cur_technique = tech;
    }
    fn cur_pass(&self) -> Option<(usize, usize)> {
        self.cur_pass
    }
    fn set_cur_pass(&mut self, pass: Option<(usize, usize)>) {
        self.cur_pass = pass;
    }
    fn set_current_effect(&mut self, current: bool) {
        self.current = current;
    }
    fn find_technique(&self, name: &[u8]) -> Option<usize> {
        self.techniques.iter().position(|(n, _)| *n == name)
    }
    fn num_passes(&self, tech: usize) -> usize {
        self.techniques[tech].1.len()
    }
    fn pass(&self, tech: usize, idx: usize) -> (usize, usize) {
        (tech, idx)
    }
    fn pass_name_is(&self, (t, i): (usize, usize), name: &[u8]) -> bool {
        self.techniques[t].1[i].name == name
    }
    fn num_shader_params(&self, (t, i): (usize, usize), stage: Stage) -> usize {
        let p = &self.techniques[t].1[i];
        match stage {
            Stage::Vertex => p.vparams.len(),
            Stage::Pixel => p.pparams.len(),
        }
    }
    fn shader_param(&self, (t, i): (usize, usize), stage: Stage, idx: usize) -> (usize, u32) {
        let p = &self.techniques[t].1[i];
        match stage {
            Stage::Vertex => p.vparams[idx],
            Stage::Pixel => p.pparams[idx],
        }
    }
    fn num_params(&self) -> usize {
        self.params.len()
    }
    fn param(&self, idx: usize) -> usize {
        idx
    }
    fn changed(&self, p: usize) -> bool {
        self.params[p].changed
    }
    fn set_changed(&mut self, p: usize, changed: bool) {
        self.params[p].changed = changed;
    }
    fn has_next_sampler(&self, p: usize) -> bool {
        self.params[p].sampler != 0
    }
    fn clear_next_sampler(&mut self, p: usize) {
        self.params[p].sampler = 0;
    }
    fn value_len(&self, p: usize) -> usize {
        self.params[p].value.len()
    }
    fn default_len(&self, p: usize) -> usize {
        self.params[p].default.len()
    }
    fn copy_default_to_value(&mut self, p: usize) {
        self.params[p].value = self.params[p].default.clone();
    }
    fn empty_value(&mut self, p: usize) {
        self.params[p].value.clear();
    }
    fn effect_active(&self) -> bool {
        self.other_active || self.current
    }
    fn load_shaders(&mut self, pass: Option<(usize, usize)>, stage: Stage) {
        let tag = pass.map_or(0, |(t, i)| {
            let p = &self.techniques[t].1[i];
            if stage == Stage::Vertex { p.vs } else { p.ps }
        });
        let kind = if stage == Stage::Vertex {
            "load_vs"
        } else {
            "load_ps"
        };
        self.events.push(format!("{kind} {tag}"));
    }
    fn send_next_sampler(&mut self, sparam: u32, p: usize) {
        let s = self.params[p].sampler;
        self.events.push(format!("next_sampler {sparam} {s}"));
    }
    fn send_value(&mut self, sparam: u32, p: usize) {
        let v = &self.params[p].value;
        self.events
            .push(format!("set_val {sparam} {} {}", v.len(), hex(v)));
    }
    fn is_texture(&mut self, sparam: u32) -> bool {
        sparam >= 100
    }
    fn clear_texture(&mut self, sparam: u32) {
        self.events.push(format!("set_texture {sparam} 0"));
    }
    fn warn(&mut self, warning: LoopWarning, _name: &[u8]) {
        self.events.push(format!("warn {warning:?}"));
    }
}

fn pass(name: &'static [u8], i: u32) -> Pass {
    Pass {
        name,
        vs: 10 + i,
        ps: 20 + i,
        vparams: vec![(0, 1), (1, 2)],
        pparams: vec![(2, 3), (3, 100)],
    }
}

/// The cmocka fixture: technique "Draw" with `passes` passes.
fn fixture(passes: u32) -> Model {
    let names: [&'static [u8]; 2] = [b"First", b"Second"];
    Model {
        techniques: vec![(
            b"Draw",
            (0..passes).map(|i| pass(names[i as usize], i)).collect(),
        )],
        params: (0..4)
            .map(|i| Param {
                is_texture: i == 3,
                ..Param::default()
            })
            .collect(),
        looping: false,
        loop_pass: 0,
        cur_technique: None,
        cur_pass: None,
        current: false,
        other_active: false,
        events: Vec::new(),
    }
}

fn f32b(v: f32) -> Vec<u8> {
    v.to_ne_bytes().to_vec()
}

fn take(m: &mut Model) -> Vec<String> {
    std::mem::take(&mut m.events)
}

#[test]
fn one_pass_loop() {
    let mut m = fixture(1);
    m.params[0].value = f32b(1.0);
    m.params[2].value = f32b(2.0);
    m.params[0].changed = true;

    assert!(effect_loop(&mut m, b"Draw"));
    assert!(m.looping);
    assert_eq!(m.loop_pass, 1);
    assert_eq!(m.cur_technique, Some(0));
    assert_eq!(m.cur_pass, Some((0, 0)));
    assert!(m.current);
    assert_eq!(
        take(&mut m),
        [
            "load_vs 10",
            "load_ps 20",
            "set_val 1 4 0000803f",
            "set_val 3 4 00000040"
        ]
    );
    assert!(!m.params[0].changed);

    assert!(!effect_loop(&mut m, b"Draw"));
    assert_eq!(
        take(&mut m),
        ["set_texture 100 0", "load_vs 0", "load_ps 0"]
    );
    assert!(!m.looping);
    assert_eq!(m.loop_pass, 0);
    assert_eq!(m.cur_technique, None);
    assert_eq!(m.cur_pass, None);
    assert!(!m.current);
    assert!(m.params.iter().all(|p| p.value.is_empty()));
}

#[test]
fn two_pass_loop() {
    let mut m = fixture(2);
    m.params[0].value = f32b(1.0);

    assert!(effect_loop(&mut m, b"Draw"));
    take(&mut m);
    assert!(effect_loop(&mut m, b"Draw"));
    assert_eq!(m.loop_pass, 2);
    assert_eq!(m.cur_pass, Some((0, 1)));
    assert_eq!(
        take(&mut m),
        [
            "set_texture 100 0",
            "load_vs 11",
            "load_ps 21",
            "set_val 1 4 0000803f"
        ]
    );
    assert!(!effect_loop(&mut m, b"Draw"));
    assert!(!m.looping);
    assert_eq!(m.loop_pass, 0);
}

#[test]
fn missing_technique_and_active_effect() {
    let mut m = fixture(1);

    assert!(!effect_loop(&mut m, b"Nope"));
    assert!(!m.looping);
    assert_eq!(m.cur_technique, None);
    assert_eq!(take(&mut m), ["warn TechniqueNotFound"]);

    m.other_active = true;
    assert!(!effect_loop(&mut m, b"Draw"));
    assert!(!m.looping);
    assert_eq!(take(&mut m), ["warn AlreadyActive"]);
}

#[test]
fn default_upload_and_next_sampler() {
    let mut m = fixture(1);
    m.params[1].default = f32b(3.0);
    let mut tex = vec![0u8; 16];
    tex[0] = 7;
    m.params[3].value = tex;
    m.params[3].sampler = 55;

    assert!(effect_loop(&mut m, b"Draw"));
    let events = take(&mut m);
    assert!(events.contains(&"set_val 2 4 00004040".to_string()));
    assert_eq!(m.params[1].value.len(), 4);
    assert!(events.contains(&"next_sampler 100 55".to_string()));
    assert_eq!(m.params[3].sampler, 55);
    assert!(!effect_loop(&mut m, b"Draw"));
    assert_eq!(m.params[3].sampler, 0);
}

#[test]
fn update_params_sends_only_changed() {
    let mut m = fixture(1);
    m.params[0].value = f32b(1.0);
    m.params[2].value = f32b(2.0);
    m.params[3].sampler = 56;
    assert!(effect_loop(&mut m, b"Draw"));

    take(&mut m);
    effect_update_params(&mut m);
    assert_eq!(take(&mut m), ["next_sampler 100 56"]);

    m.params[2].value = f32b(4.0);
    m.params[2].changed = true;
    effect_update_params(&mut m);
    assert_eq!(
        take(&mut m),
        ["set_val 3 4 00008040", "next_sampler 100 56"]
    );
    assert!(!m.params[2].changed);

    let tech = m.cur_technique;
    technique_end_pass(&mut m, tech);
    take(&mut m);
    m.params[2].changed = true;
    effect_update_params(&mut m);
    assert!(m.events.is_empty());
    assert!(m.params[2].changed);
    technique_end(&mut m, tech);
}

#[test]
fn begin_pass_by_name_and_bounds() {
    let mut m = fixture(2);

    assert_eq!(technique_begin(&mut m, Some(0)), 2);
    assert!(m.current);
    assert!(technique_begin_pass_by_name(&mut m, Some(0), b"Second"));
    assert_eq!(m.cur_pass, Some((0, 1)));
    assert!(!technique_begin_pass_by_name(&mut m, Some(0), b"Third"));
    assert!(!technique_begin_pass(&mut m, Some(0), 2));
    assert_eq!(m.cur_pass, Some((0, 1)));
    technique_end_pass(&mut m, Some(0));
    assert_eq!(m.cur_pass, None);
    take(&mut m);
    technique_end_pass(&mut m, Some(0));
    assert!(m.events.is_empty());
    technique_end(&mut m, Some(0));

    assert_eq!(technique_begin(&mut m, None), 0);
    assert!(!technique_begin_pass(&mut m, None, 0));
    assert!(!technique_begin_pass_by_name(&mut m, None, b"First"));
    take(&mut m);
    technique_end(&mut m, None);
    technique_end_pass(&mut m, None);
    assert!(m.events.is_empty());
}

#[test]
fn edge_loop_pass_increments_past_the_last_pass() {
    // begin_pass gets the old loop_pass; the increment happens even when
    // that pass does not exist, and the reset follows.
    let mut m = fixture(0);
    assert!(!effect_loop(&mut m, b"Draw"));
    assert_eq!(m.loop_pass, 0);
    assert!(!m.looping);
    assert_eq!(take(&mut m), ["load_vs 0", "load_ps 0"]);
}

#[test]
fn edge_texture_flag_is_the_device_s() {
    // The model's is_texture flag on the parameter is not what clears a
    // texture: the shader parameter's type from the device is.
    let mut m = fixture(1);
    assert!(m.params[3].is_texture);
    m.techniques[0].1[0].pparams[1].1 = 4;
    assert!(effect_loop(&mut m, b"Draw"));
    take(&mut m);
    assert!(!effect_loop(&mut m, b"Draw"));
    assert_eq!(take(&mut m), ["load_vs 0", "load_ps 0"]);
}

#[derive(Debug, Clone)]
enum Op {
    Loop(bool),
    Update,
    Begin,
    BeginPass(usize),
    BeginPassByName(bool),
    EndPass,
    End,
}

proptest! {
    /// Any call sequence, misuse included: no panic, and a failed
    /// gs_effect_loop that logged no warning has reset the loop.
    #[test]
    fn prop_state_consistent(
        passes in 0u32..3,
        ops in prop::collection::vec(prop_oneof![
            any::<bool>().prop_map(Op::Loop),
            Just(Op::Update),
            Just(Op::Begin),
            (0usize..4).prop_map(Op::BeginPass),
            any::<bool>().prop_map(Op::BeginPassByName),
            Just(Op::EndPass),
            Just(Op::End),
        ], 0..30),
    ) {
        let mut m = fixture(passes);
        m.params[0].value = f32b(1.0);
        for op in ops {
            match op {
                Op::Loop(good) => {
                    let name: &[u8] = if good { b"Draw" } else { b"Nope" };
                    let ok = effect_loop(&mut m, name);
                    if !ok && !m.events.iter().any(|e| e.starts_with("warn")) {
                        prop_assert!(!m.looping);
                        prop_assert_eq!(m.loop_pass, 0);
                    }
                }
                Op::Update => effect_update_params(&mut m),
                Op::Begin => { technique_begin(&mut m, Some(0)); }
                Op::BeginPass(i) => { technique_begin_pass(&mut m, Some(0), i); }
                Op::BeginPassByName(first) => {
                    let name: &[u8] = if first { b"First" } else { b"Second" };
                    technique_begin_pass_by_name(&mut m, Some(0), name);
                }
                Op::EndPass => {
                    let t = m.cur_technique.or(Some(0));
                    technique_end_pass(&mut m, t);
                }
                Op::End => {
                    let t = m.cur_technique.or(Some(0));
                    technique_end(&mut m, t);
                }
            }
            m.events.clear();
        }
    }
}
