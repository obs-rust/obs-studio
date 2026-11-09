//! Tier 1 for `obs_graphics::effect_destroy`, the port of
//! `libobs/graphics/effect.c.h` (`gs_effect_destroy`,
//! `gs_effect_actually_destroy` and the frees of `graphics/effect.h`).
//!
//! The `test_effect_destroy_*` tests port `test/cmocka/test_effect_destroy.c`
//! one to one: where the C test counts the allocations an effect leaves, the
//! model here lists every free it gets, in order, and checks each of its
//! allocations is freed exactly once. The other tests add edge cases.

use std::collections::BTreeMap;

use obs_graphics::effect_destroy::{
    EffectModel, Free, effect_actually_destroy, effect_destroy, effect_free, effect_param_free,
    effect_pass_free, effect_technique_free,
};

/// A parameter or annotation of the spec: its label and annotations.
#[derive(Clone)]
struct P(&'static str, Vec<P>);

fn p(label: &'static str) -> P {
    P(label, Vec::new())
}

fn pa(label: &'static str, annotations: Vec<P>) -> P {
    P(label, annotations)
}

/// A pass of the spec: its label and whether each shader is there.
#[derive(Clone, Copy)]
struct S(&'static str, bool, bool);

fn s(label: &'static str) -> S {
    S(label, true, true)
}

#[derive(Clone)]
struct Spec {
    cached: bool,
    params: Vec<P>,
    techniques: Vec<(&'static str, Vec<S>)>,
}

struct ParamNode {
    label: &'static str,
    annotations: Vec<usize>,
}

struct TechniqueNode {
    label: &'static str,
    passes: Vec<usize>,
}

struct PassNode {
    label: &'static str,
    vert: bool,
    pixel: bool,
}

/// The effect as flat arenas, with a log of every free (and, when `reads`
/// is set, every read).
struct Model {
    cached: bool,
    params: Vec<usize>,
    param_nodes: Vec<ParamNode>,
    techniques: Vec<TechniqueNode>,
    passes: Vec<PassNode>,
    reads: bool,
    log: std::cell::RefCell<Vec<String>>,
}

impl Model {
    fn new(spec: &Spec) -> Self {
        let mut model = Self {
            cached: spec.cached,
            params: Vec::new(),
            param_nodes: Vec::new(),
            techniques: Vec::new(),
            passes: Vec::new(),
            reads: false,
            log: std::cell::RefCell::new(Vec::new()),
        };
        for param in &spec.params {
            let index = model.add_param(param);
            model.params.push(index);
        }
        for (label, passes) in &spec.techniques {
            let mut indices = Vec::new();
            for pass in passes {
                indices.push(model.passes.len());
                model.passes.push(PassNode {
                    label: pass.0,
                    vert: pass.1,
                    pixel: pass.2,
                });
            }
            model.techniques.push(TechniqueNode {
                label,
                passes: indices,
            });
        }
        model
    }

    fn add_param(&mut self, param: &P) -> usize {
        let annotations = param.1.iter().map(|a| self.add_param(a)).collect();
        self.param_nodes.push(ParamNode {
            label: param.0,
            annotations,
        });
        self.param_nodes.len() - 1
    }

    fn read(&self, what: String) {
        if self.reads {
            self.log.borrow_mut().push(format!("read {what}"));
        }
    }

    fn log(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}

impl EffectModel for Model {
    type Param = usize;
    type Technique = usize;
    type Pass = usize;

    fn cached(&self) -> bool {
        self.read("cached".into());
        self.cached
    }

    fn param_count(&self) -> usize {
        self.read("params.num".into());
        self.params.len()
    }

    fn param(&self, index: usize) -> usize {
        self.read(format!("params[{index}]"));
        self.params[index]
    }

    fn annotation_count(&self, param: usize) -> usize {
        let node = &self.param_nodes[param];
        self.read(format!("{}.annotations.num", node.label));
        node.annotations.len()
    }

    fn annotation(&self, param: usize, index: usize) -> usize {
        let node = &self.param_nodes[param];
        self.read(format!("{}.annotations[{index}]", node.label));
        node.annotations[index]
    }

    fn technique_count(&self) -> usize {
        self.read("techniques.num".into());
        self.techniques.len()
    }

    fn technique(&self, index: usize) -> usize {
        self.read(format!("techniques[{index}]"));
        index
    }

    fn pass_count(&self, technique: usize) -> usize {
        let node = &self.techniques[technique];
        self.read(format!("{}.passes.num", node.label));
        node.passes.len()
    }

    fn pass(&self, technique: usize, index: usize) -> usize {
        let node = &self.techniques[technique];
        self.read(format!("{}.passes[{index}]", node.label));
        node.passes[index]
    }

    fn free(&mut self, step: Free<usize, usize, usize>) {
        let param = |i: usize| self.param_nodes[i].label;
        let technique = |i: usize| self.techniques[i].label;
        let pass = |i: usize| self.passes[i].label;
        let shader = |present: bool, label: &str, which: &str| {
            if present {
                format!("gs_shader_destroy {label}.{which}")
            } else {
                format!("gs_shader_destroy NULL ({label}.{which})")
            }
        };
        let line = match step {
            Free::ParamName(i) => format!("bfree {}.name", param(i)),
            Free::ParamCurVal(i) => format!("da_free {}.cur_val", param(i)),
            Free::ParamDefaultVal(i) => format!("da_free {}.default_val", param(i)),
            Free::ParamAnnotations(i) => format!("da_free {}.annotations", param(i)),
            Free::PassName(i) => format!("bfree {}.name", pass(i)),
            Free::PassVertParams(i) => format!("da_free {}.vertshader_params", pass(i)),
            Free::PassPixelParams(i) => format!("da_free {}.pixelshader_params", pass(i)),
            Free::PassVertShader(i) => shader(self.passes[i].vert, pass(i), "vertshader"),
            Free::PassPixelShader(i) => shader(self.passes[i].pixel, pass(i), "pixelshader"),
            Free::TechniquePasses(i) => format!("da_free {}.passes", technique(i)),
            Free::TechniqueName(i) => format!("bfree {}.name", technique(i)),
            Free::Params => "da_free params".into(),
            Free::Techniques => "da_free techniques".into(),
            Free::EffectPath => "bfree effect_path".into(),
            Free::EffectDir => "bfree effect_dir".into(),
            Free::ClearPaths => "clear effect_path effect_dir".into(),
            Free::Effect => "bfree effect".into(),
        };
        self.log.borrow_mut().push(line);
    }
}

/// The frees of a parameter, as `effect_param_free` makes them.
fn param_frees(param: &P, out: &mut Vec<String>) {
    out.push(format!("bfree {}.name", param.0));
    out.push(format!("da_free {}.cur_val", param.0));
    out.push(format!("da_free {}.default_val", param.0));
    for annotation in &param.1 {
        param_frees(annotation, out);
    }
    out.push(format!("da_free {}.annotations", param.0));
}

/// Every allocation the spec's effect holds, each once: what the C test's
/// allocation count stands for.
fn allocations(spec: &Spec) -> BTreeMap<String, usize> {
    let mut all = Vec::new();
    for param in &spec.params {
        param_frees(param, &mut all);
    }
    for (label, passes) in &spec.techniques {
        for pass in passes {
            all.push(format!("bfree {}.name", pass.0));
            all.push(format!("da_free {}.vertshader_params", pass.0));
            all.push(format!("da_free {}.pixelshader_params", pass.0));
            if pass.1 {
                all.push(format!("gs_shader_destroy {}.vertshader", pass.0));
            }
            if pass.2 {
                all.push(format!("gs_shader_destroy {}.pixelshader", pass.0));
            }
        }
        all.push(format!("da_free {label}.passes"));
        all.push(format!("bfree {label}.name"));
    }
    for line in [
        "da_free params",
        "da_free techniques",
        "bfree effect_path",
        "bfree effect_dir",
        "bfree effect",
    ] {
        all.push(line.into());
    }
    let mut counts = BTreeMap::new();
    for line in all {
        *counts.entry(line).or_insert(0) += 1;
    }
    counts
}

/// The log's allocation frees (no NULL shaders, no clearing), counted.
fn freed(log: &[String]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for line in log {
        if line.contains("NULL") || line.starts_with("clear") || line.starts_with("read") {
            continue;
        }
        *counts.entry(line.clone()).or_insert(0) += 1;
    }
    counts
}

/// The effect of the C test: two techniques, one with two passes; params
/// with and without annotations.
fn c_test_effect(cached: bool) -> Spec {
    Spec {
        cached,
        params: vec![
            p("ViewProj"),
            pa("color", vec![p("color.name"), p("color.minimum")]),
            pa("gain", vec![p("gain.step")]),
            p("level"),
        ],
        techniques: vec![
            ("Draw", vec![s("Draw.pass0"), s("Draw.second")]),
            ("Level", vec![s("Level.pass0")]),
        ],
    }
}

fn lines(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| (*line).to_owned()).collect()
}

fn destroy(spec: &Spec) -> Vec<String> {
    let mut model = Model::new(spec);
    effect_destroy(Some(&mut model));
    model.log()
}

#[test]
fn test_effect_destroy_uncached_destroy_frees_everything() {
    let spec = c_test_effect(false);
    let log = destroy(&spec);
    assert_eq!(
        log,
        lines(&[
            "bfree ViewProj.name",
            "da_free ViewProj.cur_val",
            "da_free ViewProj.default_val",
            "da_free ViewProj.annotations",
            "bfree color.name",
            "da_free color.cur_val",
            "da_free color.default_val",
            "bfree color.name.name",
            "da_free color.name.cur_val",
            "da_free color.name.default_val",
            "da_free color.name.annotations",
            "bfree color.minimum.name",
            "da_free color.minimum.cur_val",
            "da_free color.minimum.default_val",
            "da_free color.minimum.annotations",
            "da_free color.annotations",
            "bfree gain.name",
            "da_free gain.cur_val",
            "da_free gain.default_val",
            "bfree gain.step.name",
            "da_free gain.step.cur_val",
            "da_free gain.step.default_val",
            "da_free gain.step.annotations",
            "da_free gain.annotations",
            "bfree level.name",
            "da_free level.cur_val",
            "da_free level.default_val",
            "da_free level.annotations",
            "bfree Draw.pass0.name",
            "da_free Draw.pass0.vertshader_params",
            "da_free Draw.pass0.pixelshader_params",
            "gs_shader_destroy Draw.pass0.vertshader",
            "gs_shader_destroy Draw.pass0.pixelshader",
            "bfree Draw.second.name",
            "da_free Draw.second.vertshader_params",
            "da_free Draw.second.pixelshader_params",
            "gs_shader_destroy Draw.second.vertshader",
            "gs_shader_destroy Draw.second.pixelshader",
            "da_free Draw.passes",
            "bfree Draw.name",
            "bfree Level.pass0.name",
            "da_free Level.pass0.vertshader_params",
            "da_free Level.pass0.pixelshader_params",
            "gs_shader_destroy Level.pass0.vertshader",
            "gs_shader_destroy Level.pass0.pixelshader",
            "da_free Level.passes",
            "bfree Level.name",
            "da_free params",
            "da_free techniques",
            "bfree effect_path",
            "bfree effect_dir",
            "clear effect_path effect_dir",
            "bfree effect",
        ])
    );
    assert_eq!(freed(&log), allocations(&spec));
}

/// The C test draws with the effect first; a draw changes nothing the
/// destroy reads, so the frees are the same.
#[test]
fn test_effect_destroy_uncached_destroy_after_draw() {
    let spec = c_test_effect(false);
    assert_eq!(freed(&destroy(&spec)), allocations(&spec));
}

/// `gs_effect_create` destroys the effect it could not parse: it has a path
/// but is not cached yet, and holds what the parse got to.
#[test]
fn test_effect_destroy_failed_create_destroys_effect() {
    let spec = Spec {
        cached: false,
        params: vec![p("ViewProj"), pa("gain", vec![p("gain.step")])],
        techniques: vec![],
    };
    let log = destroy(&spec);
    assert_eq!(freed(&log), allocations(&spec));
    assert!(log.iter().any(|line| line == "bfree effect_path"));
    assert_eq!(log.last().map(String::as_str), Some("bfree effect"));
}

#[test]
fn test_effect_destroy_destroy_null_does_nothing() {
    effect_destroy::<Model>(None);
}

#[test]
fn test_effect_destroy_cached_effect_survives_destroy() {
    let mut model = Model::new(&c_test_effect(true));
    model.reads = true;
    effect_destroy(Some(&mut model));
    effect_destroy(Some(&mut model));
    assert_eq!(model.log(), lines(&["read cached", "read cached"]));
}

/// `gs_destroy` frees each cached effect with `gs_effect_actually_destroy`,
/// which does not look at the flag.
#[test]
fn test_effect_destroy_gs_destroy_frees_cached_effect() {
    let spec = c_test_effect(true);
    let mut model = Model::new(&spec);
    model.reads = true;
    effect_actually_destroy(&mut model);
    let log = model.log();
    assert!(!log.iter().any(|line| line == "read cached"));
    assert_eq!(freed(&log), allocations(&spec));
    assert_eq!(log.last().map(String::as_str), Some("bfree effect"));
}

#[test]
fn empty_effect_frees_its_arrays_and_paths() {
    let spec = Spec {
        cached: false,
        params: vec![],
        techniques: vec![],
    };
    assert_eq!(
        destroy(&spec),
        lines(&[
            "da_free params",
            "da_free techniques",
            "bfree effect_path",
            "bfree effect_dir",
            "clear effect_path effect_dir",
            "bfree effect",
        ])
    );
}

/// `effect_param_free` recurses: an annotation's own annotations are freed
/// before its annotation array, depth first.
#[test]
fn nested_annotations_free_depth_first() {
    let spec = Spec {
        cached: false,
        params: vec![pa(
            "a",
            vec![pa("a.0", vec![p("a.0.0"), p("a.0.1")]), p("a.1")],
        )],
        techniques: vec![],
    };
    let mut model = Model::new(&spec);
    let first = model.params[0];
    effect_param_free(&mut model, first);
    assert_eq!(
        model.log(),
        lines(&[
            "bfree a.name",
            "da_free a.cur_val",
            "da_free a.default_val",
            "bfree a.0.name",
            "da_free a.0.cur_val",
            "da_free a.0.default_val",
            "bfree a.0.0.name",
            "da_free a.0.0.cur_val",
            "da_free a.0.0.default_val",
            "da_free a.0.0.annotations",
            "bfree a.0.1.name",
            "da_free a.0.1.cur_val",
            "da_free a.0.1.default_val",
            "da_free a.0.1.annotations",
            "da_free a.0.annotations",
            "bfree a.1.name",
            "da_free a.1.cur_val",
            "da_free a.1.default_val",
            "da_free a.1.annotations",
            "da_free a.annotations",
        ])
    );
}

/// Both shaders of a pass are destroyed, a missing one too
/// (`gs_shader_destroy(NULL)`), after the name and the shader params.
#[test]
fn pass_destroys_both_shaders_null_included() {
    let spec = Spec {
        cached: false,
        params: vec![],
        techniques: vec![(
            "T",
            vec![S("T.none", false, false), S("T.pixel", false, true)],
        )],
    };
    let mut model = Model::new(&spec);
    effect_pass_free(&mut model, 0);
    effect_pass_free(&mut model, 1);
    assert_eq!(
        model.log(),
        lines(&[
            "bfree T.none.name",
            "da_free T.none.vertshader_params",
            "da_free T.none.pixelshader_params",
            "gs_shader_destroy NULL (T.none.vertshader)",
            "gs_shader_destroy NULL (T.none.pixelshader)",
            "bfree T.pixel.name",
            "da_free T.pixel.vertshader_params",
            "da_free T.pixel.pixelshader_params",
            "gs_shader_destroy NULL (T.pixel.vertshader)",
            "gs_shader_destroy T.pixel.pixelshader",
        ])
    );
}

/// A technique frees its passes, then the pass array, then its name; with
/// no pass, just the two.
#[test]
fn technique_frees_passes_then_array_then_name() {
    let spec = Spec {
        cached: false,
        params: vec![],
        techniques: vec![("Empty", vec![]), ("One", vec![s("One.p")])],
    };
    let mut model = Model::new(&spec);
    effect_technique_free(&mut model, 0);
    effect_technique_free(&mut model, 1);
    assert_eq!(
        model.log(),
        lines(&[
            "da_free Empty.passes",
            "bfree Empty.name",
            "bfree One.p.name",
            "da_free One.p.vertshader_params",
            "da_free One.p.pixelshader_params",
            "gs_shader_destroy One.p.vertshader",
            "gs_shader_destroy One.p.pixelshader",
            "da_free One.passes",
            "bfree One.name",
        ])
    );
}

/// `effect_free` leaves the effect itself: `gs_effect_actually_destroy`
/// frees it after.
#[test]
fn effect_free_leaves_the_effect() {
    let spec = c_test_effect(false);
    let mut model = Model::new(&spec);
    effect_free(&mut model);
    let log = model.log();
    assert!(!log.iter().any(|line| line == "bfree effect"));
    assert_eq!(
        log.last().map(String::as_str),
        Some("clear effect_path effect_dir")
    );
}

/// Each count is read before each item and once after the last, as the C's
/// `for` loops read `num`; the cached flag first, and only by
/// `gs_effect_destroy`.
#[test]
fn reads_follow_the_c_loops() {
    let spec = Spec {
        cached: false,
        params: vec![pa("a", vec![p("a.0")])],
        techniques: vec![("T", vec![s("T.p")])],
    };
    let mut model = Model::new(&spec);
    model.reads = true;
    effect_destroy(Some(&mut model));
    assert_eq!(
        model.log(),
        lines(&[
            "read cached",
            "read params.num",
            "read params[0]",
            "bfree a.name",
            "da_free a.cur_val",
            "da_free a.default_val",
            "read a.annotations.num",
            "read a.annotations[0]",
            "bfree a.0.name",
            "da_free a.0.cur_val",
            "da_free a.0.default_val",
            "read a.0.annotations.num",
            "da_free a.0.annotations",
            "read a.annotations.num",
            "da_free a.annotations",
            "read params.num",
            "read techniques.num",
            "read techniques[0]",
            "read T.passes.num",
            "read T.passes[0]",
            "bfree T.p.name",
            "da_free T.p.vertshader_params",
            "da_free T.p.pixelshader_params",
            "gs_shader_destroy T.p.vertshader",
            "gs_shader_destroy T.p.pixelshader",
            "read T.passes.num",
            "da_free T.passes",
            "bfree T.name",
            "read techniques.num",
            "da_free params",
            "da_free techniques",
            "bfree effect_path",
            "bfree effect_dir",
            "clear effect_path effect_dir",
            "bfree effect",
        ])
    );
}
