//! Tier 1: the effect parser's compile step of
//! `libobs/graphics/effect-parser-compile.c.h`, in
//! `obs_graphics::effect_parser::compile`.
//!
//! `params`, `techniques` and `shaders` port
//! `test/cmocka/test_effect_parser_compile.c` one to one: the same cases, in
//! the same groups, with the same expected dump. That test parses each
//! effect text with `ep_parse()`, which calls the compile step, through
//! graphics stand-ins that hand out fake shaders and shader params; these
//! start from the parser `ep_parse()` hands the compile step for that text
//! (its params with their types, names, default bytes and annotations, its
//! techniques and passes with the text and used params the shader writer
//! builds for each call, and the errors already in its list), written out
//! below as the C keeps them, and run the core over [`Fake`], the C test's
//! stand-ins and dump in Rust. The two texts that never reach the compile
//! step (an empty text, a parse error) are `ep_parse()`'s and are left out,
//! and so are the `destroyed` and `leaked` lines, which are
//! `effect_free()`'s and the allocator's.
//!
//! The `edge_*` tests add what the C test leaves out.

use obs_graphics::effect_parser::compile::{
    CompileLog, CompileModel, EFFECT_ANNOTATION, EFFECT_PARAM, EFFECT_PASS, EFFECT_TECHNIQUE,
    ParamSlot, ShaderSlot, ShaderType, compile, compile_pass, compile_pass_shader,
    compile_technique, location,
};

/// A parser param or annotation as `ep_parse()` leaves it.
struct P {
    ty: &'static str,
    name: &'static str,
    default: &'static [u8],
    annotations: &'static [P],
}

/// What the writer builds for one call: its text (`None` when it wrote
/// nothing, so the string stays `NULL`) and the params it used.
#[derive(Clone, Copy)]
struct C {
    text: Option<&'static str>,
    used: &'static [&'static str],
}

/// A parser pass.
struct Ps {
    name: Option<&'static str>,
    vertex: C,
    pixel: C,
}

/// A parser technique.
struct T {
    name: Option<&'static str>,
    passes: &'static [Ps],
}

/// One case of the C test, with the parser it reaches the compile step
/// with.
struct Fx {
    /// The C test's effect text (for the failure message).
    text: &'static str,
    file: Option<&'static str>,
    params: &'static [P],
    techniques: &'static [T],
    /// The error list's lines before the compile step.
    errors: &'static [&'static str],
    /// Where `cf_adderror()` puts a shader's error (the parser's token).
    error_at: (u32, u32),
    null_mask: u32,
    error_mask: u32,
    empty_error_mask: u32,
    missing: Option<&'static str>,
    missing_mask: u32,
    expected: &'static str,
}

const MAX_SHADERS: usize = 16;
const MAX_SPARAMS: usize = 64;
const LOG_ERROR: i32 = 100;

/// `struct gs_effect_param`, the fields the compile step writes.
#[derive(Clone, Default)]
struct EffParam {
    name: Option<Vec<u8>>,
    section: i32,
    type_: i32,
    default: Vec<u8>,
    effect_self: bool,
    annotations: Vec<EffParam>,
}

/// `struct gs_effect_pass`.
#[derive(Clone, Default)]
struct EffPass {
    name: Option<Vec<u8>>,
    section: i32,
    vert: Option<usize>,
    pixel: Option<usize>,
    vparams: Vec<(Option<usize>, Option<usize>)>,
    pparams: Vec<(Option<usize>, Option<usize>)>,
}

/// `struct gs_effect_technique`.
#[derive(Clone, Default)]
struct EffTech {
    name: Option<Vec<u8>>,
    section: i32,
    effect_self: bool,
    passes: Vec<EffPass>,
}

/// A parser param's state: the bytes of its default value still in it, and
/// the effect param it points at.
struct ParserParam {
    left: usize,
    link: Option<usize>,
    annotations_left: Vec<usize>,
}

/// The C test's graphics stand-ins and dump, over a parser from a [`Fx`].
struct Fake {
    fx: &'static Fx,
    params: Vec<EffParam>,
    techniques: Vec<EffTech>,
    view_proj: Option<usize>,
    world: Option<usize>,
    parser: Vec<ParserParam>,
    create_count: usize,
    sparam_names: Vec<String>,
    errors: String,
    log: String,
    dump: String,
    /// Every model call, for the edge tests.
    trace: Vec<String>,
    /// Built texts and error texts not yet freed.
    live_built: usize,
    live_errors: usize,
}

impl Fake {
    fn new(fx: &'static Fx) -> Self {
        Self {
            fx,
            params: Vec::new(),
            techniques: Vec::new(),
            view_proj: None,
            world: None,
            parser: fx
                .params
                .iter()
                .map(|p| ParserParam {
                    left: p.default.len(),
                    link: None,
                    annotations_left: p.annotations.iter().map(|a| a.default.len()).collect(),
                })
                .collect(),
            create_count: 0,
            sparam_names: Vec::new(),
            errors: fx.errors.concat(),
            log: String::new(),
            dump: String::new(),
            trace: Vec::new(),
            live_built: 0,
            live_errors: 0,
        }
    }

    fn parser_param(&self, slot: ParamSlot) -> &'static P {
        match slot {
            ParamSlot::Param(i) => &self.fx.params[i],
            ParamSlot::Annotation { param, index } => &self.fx.params[param].annotations[index],
        }
    }

    fn pass(&mut self, slot: ShaderSlot) -> &mut EffPass {
        &mut self.techniques[slot.tech].passes[slot.pass]
    }

    fn pass_in(&self, slot: ShaderSlot) -> &'static Ps {
        &self.fx.techniques[slot.tech].passes[slot.pass]
    }

    /// `dump_text`: each line behind `| `, a tab as `\t`.
    fn dump_text(&mut self, text: Option<&str>) {
        let Some(text) = text else {
            self.dump.push_str("    (null text)\n");
            return;
        };
        let mut line_start = true;
        for c in text.chars() {
            if line_start {
                self.dump.push_str("    | ");
                line_start = false;
            }
            match c {
                '\n' => {
                    self.dump.push('\n');
                    line_start = true;
                }
                '\t' => self.dump.push_str("\\t"),
                c => self.dump.push(c),
            }
        }
        if !line_start {
            self.dump.push_str("\n    (no final newline)\n");
        }
    }

    fn eparam_name(&self, param: Option<usize>) -> String {
        match param {
            None => "-".into(),
            Some(i) => format!("{i}:{}", name_or_null(self.params[i].name.as_deref())),
        }
    }

    fn dump_param(&mut self, param: &EffParam, indent: &str, kind: &str, idx: usize) {
        let mut line = format!(
            "{indent}{kind} {idx} {} section={} type={} changed=0 cur=0 default=",
            name_or_null(param.name.as_deref()),
            param.section,
            param.type_
        );
        for b in &param.default {
            line.push_str(&format!("{b:02x}"));
        }
        line.push_str(&format!(
            " effect={} sampler=null\n",
            if param.effect_self { "self" } else { "other" }
        ));
        self.dump.push_str(&line);
    }

    fn dump_shaderparams(&mut self, label: &str, params: &[(Option<usize>, Option<usize>)]) {
        let mut line = format!("      {label}");
        for (i, (e, s)) in params.iter().enumerate() {
            let sname = s.map_or_else(|| "NULL".to_string(), |s| self.sparam_names[s].clone());
            line.push_str(&format!(
                "{}{}/{}",
                if i == 0 { " " } else { ", " },
                self.eparam_name(*e),
                sname
            ));
        }
        line.push('\n');
        self.dump.push_str(&line);
    }

    /// The C test's dump after `ep_parse()`, less `destroyed` and `leaked`.
    fn finish(mut self, ok: bool) -> String {
        self.dump.push_str(&format!("result {}\n", i32::from(ok)));
        let params = self.params.clone();
        for (i, param) in params.iter().enumerate() {
            self.dump_param(param, "", "param", i);
            for (j, ann) in param.annotations.iter().enumerate() {
                self.dump_param(ann, "  ", "annotation", j);
            }
        }
        let line = format!(
            "view_proj={} world={} scale=-\n",
            self.eparam_name(self.view_proj),
            self.eparam_name(self.world)
        );
        self.dump.push_str(&line);
        let techniques = self.techniques.clone();
        for (i, tech) in techniques.iter().enumerate() {
            self.dump.push_str(&format!(
                "technique {i} {} section={} effect={} passes={}\n",
                name_or_null(tech.name.as_deref()),
                tech.section,
                if tech.effect_self { "self" } else { "other" },
                tech.passes.len()
            ));
            for (j, pass) in tech.passes.iter().enumerate() {
                let shader = |s: Option<usize>| s.map_or_else(|| "NULL".into(), |s| s.to_string());
                self.dump.push_str(&format!(
                    "  pass {j} {} section={} vert={} pixel={}\n",
                    name_or_null(pass.name.as_deref()),
                    pass.section,
                    shader(pass.vert),
                    shader(pass.pixel)
                ));
                self.dump_shaderparams("vparams", &pass.vparams);
                self.dump_shaderparams("pparams", &pass.pparams);
            }
        }
        let compiled = self.params.len() == self.fx.params.len();
        for (i, p) in self.fx.params.iter().enumerate() {
            let link = if compiled {
                self.eparam_name(self.parser[i].link)
            } else {
                "uncompiled".into()
            };
            self.dump.push_str(&format!(
                "parser {i} {} left={} param={link}\n",
                p.name, self.parser[i].left
            ));
            for (j, a) in p.annotations.iter().enumerate() {
                self.dump.push_str(&format!(
                    "  parser annotation {j} {} left={}\n",
                    a.name, self.parser[i].annotations_left[j]
                ));
            }
        }
        let errors = core::mem::take(&mut self.errors);
        self.dump.push_str(&errors);
        let log = core::mem::take(&mut self.log);
        self.dump.push_str(&log);
        assert_eq!(self.live_built, 0, "every built text is freed");
        assert_eq!(self.live_errors, 0, "every error text is freed");
        self.dump
    }
}

fn name_or_null(name: Option<&[u8]>) -> String {
    name.map_or_else(
        || "(null)".into(),
        |n| String::from_utf8_lossy(n).into_owned(),
    )
}

fn kind_name(kind: ShaderType) -> &'static str {
    match kind {
        ShaderType::Vertex => "vertex",
        ShaderType::Pixel => "pixel",
    }
}

impl CompileModel for Fake {
    type Shader = usize;
    type Built = C;
    type Errors = Option<String>;
    type EParam = Option<usize>;
    type SParam = usize;

    fn param_count(&self) -> usize {
        self.fx.params.len()
    }
    fn technique_count(&self) -> usize {
        self.fx.techniques.len()
    }
    fn resize_params(&mut self, num: usize) {
        self.trace.push(format!("resize_params {num}"));
        self.params.resize(num, EffParam::default());
    }
    fn resize_techniques(&mut self, num: usize) {
        self.trace.push(format!("resize_techniques {num}"));
        self.techniques.resize(num, EffTech::default());
    }
    fn link_param(&mut self, i: usize) {
        self.trace.push(format!("link {i}"));
        self.parser[i].link = Some(i);
    }
    fn param_type_name(&self, slot: ParamSlot) -> &[u8] {
        self.parser_param(slot).ty.as_bytes()
    }
    fn fill_param(&mut self, slot: ParamSlot, section: i32, type_: i32) {
        self.trace.push(format!("fill {slot:?} {section} {type_}"));
        let src = self.parser_param(slot);
        let param = EffParam {
            name: Some(src.name.as_bytes().to_vec()),
            section,
            type_,
            default: src.default.to_vec(),
            effect_self: true,
            annotations: Vec::new(),
        };
        match slot {
            ParamSlot::Param(i) => {
                let annotations = core::mem::take(&mut self.params[i].annotations);
                self.params[i] = EffParam {
                    annotations,
                    ..param
                };
                self.parser[i].left = 0;
            }
            ParamSlot::Annotation { param: i, index } => {
                self.params[i].annotations[index] = param;
                self.parser[i].annotations_left[index] = 0;
            }
        }
    }
    fn param_name(&self, i: usize) -> &[u8] {
        self.fx.params[i].name.as_bytes()
    }
    fn set_view_proj(&mut self, i: usize) {
        self.trace.push(format!("view_proj {i}"));
        self.view_proj = Some(i);
    }
    fn set_world(&mut self, i: usize) {
        self.trace.push(format!("world {i}"));
        self.world = Some(i);
    }
    fn annotation_count(&self, i: usize) -> usize {
        self.fx.params[i].annotations.len()
    }
    fn resize_annotations(&mut self, i: usize, num: usize) {
        self.trace.push(format!("resize_annotations {i} {num}"));
        self.params[i].annotations.resize(num, EffParam::default());
    }
    fn fill_technique(&mut self, t: usize) {
        self.trace.push(format!("fill_technique {t}"));
        let tech = &mut self.techniques[t];
        tech.name = self.fx.techniques[t].name.map(|n| n.as_bytes().to_vec());
        tech.section = EFFECT_TECHNIQUE;
        tech.effect_self = true;
    }
    fn parser_pass_count(&self, t: usize) -> usize {
        self.fx.techniques[t].passes.len()
    }
    fn resize_passes(&mut self, t: usize, num: usize) {
        self.trace.push(format!("resize_passes {t} {num}"));
        self.techniques[t].passes.resize(num, EffPass::default());
    }
    fn pass_count(&self, t: usize) -> usize {
        self.techniques[t].passes.len()
    }
    fn fill_pass(&mut self, t: usize, p: usize) {
        self.trace.push(format!("fill_pass {t} {p}"));
        let name = self.fx.techniques[t].passes[p]
            .name
            .map(|n| n.as_bytes().to_vec());
        let pass = &mut self.techniques[t].passes[p];
        pass.name = name;
        pass.section = EFFECT_PASS;
    }
    fn pass_name(&self, t: usize, p: usize) -> Option<&[u8]> {
        self.techniques[t].passes[p].name.as_deref()
    }
    fn technique_name(&self, t: usize) -> Option<&[u8]> {
        self.techniques[t].name.as_deref()
    }
    fn file(&self) -> Option<&[u8]> {
        self.fx.file.map(str::as_bytes)
    }
    fn make_shader(&mut self, slot: ShaderSlot) -> C {
        self.trace.push(format!("make {slot:?}"));
        self.live_built += 1;
        let pass = self.pass_in(slot);
        match slot.kind {
            ShaderType::Vertex => pass.vertex,
            ShaderType::Pixel => pass.pixel,
        }
    }
    fn create_shader(
        &mut self,
        slot: ShaderSlot,
        built: &C,
        location: &[u8],
    ) -> (Option<usize>, Option<String>) {
        self.trace.push(format!("create {slot:?}"));
        let n = self.create_count;
        self.create_count += 1;
        let bit = |mask: u32| n < 32 && mask & (1 << n) != 0;
        let fail = bit(self.fx.null_mask);
        self.dump.push_str(&format!(
            "create {} {n} {}\n",
            kind_name(slot.kind),
            String::from_utf8_lossy(location)
        ));
        self.dump_text(built.text);
        let errors = if bit(self.fx.error_mask) {
            Some(format!("fail {n}"))
        } else if bit(self.fx.empty_error_mask) {
            Some(String::new())
        } else {
            None
        };
        if errors.is_some() {
            self.live_errors += 1;
        }
        let err = errors
            .as_ref()
            .map_or_else(String::new, |e| format!(" error \"{e}\""));
        let shader = if fail || n >= MAX_SHADERS {
            self.dump.push_str(&format!("    -> NULL{err}\n"));
            None
        } else {
            self.dump.push_str(&format!("    -> shader {n}{err}\n"));
            Some(n)
        };
        let pass = self.pass(slot);
        match slot.kind {
            ShaderType::Vertex => pass.vert = shader,
            ShaderType::Pixel => pass.pixel = shader,
        }
        (shader, errors)
    }
    fn errors_nonempty(&self, errors: &Option<String>) -> bool {
        errors.as_ref().is_some_and(|e| !e.is_empty())
    }
    fn add_shader_error(&mut self, errors: &Option<String>) {
        self.trace.push("add_shader_error".into());
        let (row, col) = self.fx.error_at;
        self.errors.push_str(&format!(
            "error {row}:{col} 0 Error creating shader: {}\n",
            errors.as_deref().unwrap_or("")
        ));
    }
    fn free_errors(&mut self, errors: Option<String>) {
        self.trace.push("free_errors".into());
        if errors.is_some() {
            self.live_errors -= 1;
        }
    }
    fn used_count(&self, built: &C) -> usize {
        built.used.len()
    }
    fn resize_shader_params(&mut self, slot: ShaderSlot, num: usize) {
        self.trace
            .push(format!("resize_shader_params {slot:?} {num}"));
        let pass = self.pass(slot);
        let params = match slot.kind {
            ShaderType::Vertex => &mut pass.vparams,
            ShaderType::Pixel => &mut pass.pparams,
        };
        params.resize(num, (None, None));
    }
    fn shader_param_count(&self, slot: ShaderSlot) -> usize {
        let pass = &self.techniques[slot.tech].passes[slot.pass];
        match slot.kind {
            ShaderType::Vertex => pass.vparams.len(),
            ShaderType::Pixel => pass.pparams.len(),
        }
    }
    fn effect_param(&mut self, built: &C, i: usize) -> Option<usize> {
        // gs_effect_get_param_by_name: the first with that name.
        let name = built.used[i].as_bytes();
        self.params
            .iter()
            .position(|p| p.name.as_deref() == Some(name))
    }
    fn shader_param(&mut self, shader: usize, built: &C, i: usize) -> Option<usize> {
        let name = built.used[i];
        if shader < 32 && self.fx.missing == Some(name) && self.fx.missing_mask & (1 << shader) != 0
        {
            return None;
        }
        if self.sparam_names.len() >= MAX_SPARAMS {
            return None;
        }
        self.sparam_names.push(format!("{shader}:{name}"));
        Some(self.sparam_names.len() - 1)
    }
    fn set_shader_param(
        &mut self,
        slot: ShaderSlot,
        i: usize,
        eparam: Option<usize>,
        sparam: Option<usize>,
    ) {
        self.trace.push(format!("set_shader_param {slot:?} {i}"));
        let pass = self.pass(slot);
        let params = match slot.kind {
            ShaderType::Vertex => &mut pass.vparams,
            ShaderType::Pixel => &mut pass.pparams,
        };
        params[i] = (eparam, sparam);
    }
    fn free_built(&mut self, _built: C) {
        self.trace.push("free_built".into());
        self.live_built -= 1;
    }
    fn log(&mut self, line: CompileLog<'_>) {
        let text = match line {
            CompileLog::ShaderParamNotFound => "Effect shader parameter not found".to_string(),
            CompileLog::MissingVertexShader { idx, name } => format!(
                "Pass ({idx}) <{}> missing vertex shader!",
                String::from_utf8_lossy(name)
            ),
            CompileLog::MissingPixelShader { idx, name } => format!(
                "Pass ({idx}) <{}> missing pixel shader!",
                String::from_utf8_lossy(name)
            ),
        };
        self.trace.push(format!("log {text}"));
        self.log.push_str(&format!("log {LOG_ERROR} {text}\n"));
    }
}

/// `check_cases`: every case's dump, compared whole.
fn check_cases(cases: &'static [Fx]) {
    let mut failed = 0;
    for (i, fx) in cases.iter().enumerate() {
        let mut m = Fake::new(fx);
        let ok = compile(&mut m);
        let got = m.finish(ok);
        if got != fx.expected {
            eprintln!(
                "case {i}: {}\n--- expected\n{}--- got\n{got}",
                fx.text, fx.expected
            );
            failed += 1;
        }
    }
    assert_eq!(failed, 0, "{failed} of {} cases differ", cases.len());
}

/// `test_effect_parser_compile.c` `params`.
#[test]
fn params() {
    check_cases(PARAMS_CASES);
}

/// `test_effect_parser_compile.c` `techniques`.
#[test]
fn techniques() {
    check_cases(TECHNIQUES_CASES);
}

/// `test_effect_parser_compile.c` `shaders`.
#[test]
fn shaders() {
    check_cases(SHADERS_CASES);
}

// ---------------------------------------------------------------------------
// Edge cases the C test leaves out.

/// A parser with nothing but what each edge test sets.
const EMPTY: Fx = Fx {
    text: "",
    file: Some("e.effect"),
    params: &[],
    techniques: &[],
    errors: &[],
    error_at: (1, 1),
    null_mask: 0,
    error_mask: 0,
    empty_error_mask: 0,
    missing: None,
    missing_mask: 0,
    expected: "",
};

#[test]
fn edge_location_formats() {
    assert_eq!(
        location(Some(b"a.effect"), ShaderType::Vertex, Some(b"Draw"), 2),
        b"a.effect (Vertex shader, technique Draw, pass 2)"
    );
    assert_eq!(
        location(Some(b"a.effect"), ShaderType::Pixel, Some(b"Draw"), 0),
        b"a.effect (Pixel shader, technique Draw, pass 0)"
    );
    // A NULL file copies nothing (dstr_copy frees for it), and a NULL
    // technique name is printf's "(null)".
    assert_eq!(
        location(None, ShaderType::Vertex, None, 7),
        b" (Vertex shader, technique (null), pass 7)"
    );
    assert_eq!(
        location(Some(b""), ShaderType::Pixel, Some(b""), 1),
        b" (Pixel shader, technique , pass 1)"
    );
    // (unsigned)pass_idx: the index is cut to 32 bits.
    assert_eq!(
        location(Some(b"f"), ShaderType::Vertex, Some(b"T"), 0x1_0000_0003),
        b"f (Vertex shader, technique T, pass 3)"
    );
}

static EDGE_PASSES: Fx = Fx {
    params: &[P {
        ty: "float4",
        name: "a",
        default: &[1, 2],
        annotations: &[],
    }],
    techniques: &[T {
        name: Some("T"),
        passes: &[Ps {
            name: None,
            vertex: C {
                text: Some("v\n"),
                used: &["a", "b"],
            },
            pixel: C {
                text: None,
                used: &[],
            },
        }],
    }],
    null_mask: 0b11,
    error_mask: 0b10,
    ..EMPTY
};

#[test]
fn edge_failed_pass_calls_in_order() {
    // Both shaders fail: each is still built, created, its error added and
    // freed, its params left unsized; the vertex failure is logged before
    // the pixel shader is made.
    let mut m = Fake::new(&EDGE_PASSES);
    assert!(!compile(&mut m));
    assert_eq!(
        m.trace,
        [
            "resize_params 1",
            "resize_techniques 1",
            "link 0",
            "fill Param(0) 0 7",
            "resize_annotations 0 0",
            "fill_technique 0",
            "resize_passes 0 1",
            "fill_pass 0 0",
            "make ShaderSlot { tech: 0, pass: 0, kind: Vertex }",
            "create ShaderSlot { tech: 0, pass: 0, kind: Vertex }",
            "free_errors",
            "free_built",
            "log Pass (0) <> missing vertex shader!",
            "make ShaderSlot { tech: 0, pass: 0, kind: Pixel }",
            "create ShaderSlot { tech: 0, pass: 0, kind: Pixel }",
            "add_shader_error",
            "free_errors",
            "free_built",
            "log Pass (0) <> missing pixel shader!",
        ]
    );
    assert_eq!(m.parser[0].left, 0);
    let got = m.finish(false);
    assert!(got.contains("create pixel 1 e.effect (Pixel shader, technique T, pass 0)\n    (null text)\n    -> NULL error \"fail 1\"\n"), "{got}");
    assert!(
        got.contains("error 1:1 0 Error creating shader: fail 1\n"),
        "{got}"
    );
    assert!(got.contains("      vparams\n      pparams\n"), "{got}");
}

static EDGE_MISSING: Fx = Fx {
    params: &[P {
        ty: "float4",
        name: "a",
        default: &[],
        annotations: &[],
    }],
    techniques: &[T {
        name: Some("T"),
        passes: &[Ps {
            name: Some("p"),
            vertex: C {
                text: Some("v"),
                used: &["x", "a", "y"],
            },
            pixel: C {
                text: Some("p"),
                used: &[],
            },
        }],
    }],
    missing: Some("a"),
    missing_mask: 0b1,
    ..EMPTY
};

#[test]
fn edge_shader_params_stop_at_the_missing_one() {
    // A used name the effect lacks pairs with no effect param; the first
    // the shader lacks is stored with no shader param, logged, and ends the
    // list; the last stays zeroed. The pixel shader is still compiled.
    let mut m = Fake::new(&EDGE_MISSING);
    assert!(!compile(&mut m));
    let got = m.finish(false);
    assert!(
        got.contains("  pass 0 p section=3 vert=0 pixel=1\n      vparams -/0:x, 0:a/NULL, -/NULL\n      pparams\n"),
        "{got}"
    );
    assert!(got.ends_with("log 100 Effect shader parameter not found\nlog 100 Pass (0) <p> missing vertex shader!\n"), "{got}");
}

static EDGE_SHAPES: Fx = Fx {
    params: &[
        P {
            ty: "float4x4",
            name: "World",
            default: &[],
            annotations: &[],
        },
        P {
            ty: "int",
            name: "ViewProj",
            default: &[3, 0, 0, 0],
            annotations: &[
                P {
                    ty: "string",
                    name: "s",
                    default: b"x\0",
                    annotations: &[],
                },
                P {
                    ty: "nope",
                    name: "n",
                    default: &[],
                    annotations: &[],
                },
            ],
        },
    ],
    techniques: &[
        T {
            name: None,
            passes: &[],
        },
        T {
            name: Some("U"),
            passes: &[],
        },
    ],
    ..EMPTY
};

#[test]
fn edge_params_and_empty_techniques() {
    // The name decides view_proj and world, not the type; annotations get
    // their own section and type; techniques with no pass compile to true.
    let mut m = Fake::new(&EDGE_SHAPES);
    assert!(compile(&mut m));
    assert_eq!(
        m.trace,
        [
            "resize_params 2",
            "resize_techniques 2",
            "link 0",
            "fill Param(0) 0 11",
            "world 0",
            "resize_annotations 0 0",
            "link 1",
            "fill Param(1) 0 3",
            "view_proj 1",
            "resize_annotations 1 2",
            "fill Annotation { param: 1, index: 0 } 4 4",
            "fill Annotation { param: 1, index: 1 } 4 0",
            "fill_technique 0",
            "resize_passes 0 0",
            "fill_technique 1",
            "resize_passes 1 0",
        ]
    );
    let got = m.finish(true);
    assert!(
        got.contains("view_proj=1:ViewProj world=0:World scale=-\n"),
        "{got}"
    );
    assert!(
        got.contains("technique 0 (null) section=1 effect=self passes=0\n"),
        "{got}"
    );
    assert_eq!(EFFECT_PARAM, 0);
    assert_eq!(EFFECT_ANNOTATION, 4);
}

#[test]
fn edge_pass_and_shader_alone() {
    // compile_technique, compile_pass and compile_pass_shader on their
    // own: a created shader with every param found is true.
    static FX: Fx = Fx {
        params: &[P {
            ty: "float",
            name: "a",
            default: &[],
            annotations: &[],
        }],
        techniques: &[T {
            name: Some("T"),
            passes: &[Ps {
                name: Some("p"),
                vertex: C {
                    text: Some("v"),
                    used: &["a"],
                },
                pixel: C {
                    text: Some("p"),
                    used: &["a"],
                },
            }],
        }],
        ..EMPTY
    };
    let mut m = Fake::new(&FX);
    m.resize_params(1);
    m.link_param(0);
    m.fill_param(ParamSlot::Param(0), EFFECT_PARAM, 2);
    m.resize_techniques(1);
    assert!(compile_technique(&mut m, 0));
    assert!(compile_pass(&mut m, 0, 0));
    let slot = ShaderSlot {
        tech: 0,
        pass: 0,
        kind: ShaderType::Pixel,
    };
    assert!(compile_pass_shader(&mut m, slot));
    assert_eq!(m.techniques[0].passes[0].pparams, [(Some(0), Some(4))]);
    assert_eq!(m.create_count, 5);
}

// ---------------------------------------------------------------------------
// The C test's cases, with the parser each reaches the compile step with.

/// The C test's `params_cases`, less the texts that never reach the compile step.
static PARAMS_CASES: &[Fx] = &[
    Fx {
        text: "uniform float4x4 ViewProj;\nuniform float4x4 World;\nuniform float4x4 viewproj;\nuniform float4 color = {1.0, 0.5, 0.25, 1.0};\nuniform float gain <string name = \"Gain\"; float minimum = 0.0; int step = 2;> = 1.5;\nuniform float2 size;\nuniform float3 axis;\nuniform int count = 3;\nuniform int2 i2;\nuniform int3 i3;\nuniform int4 i4;\nuniform bool flag = true;\nuniform texture2d image;\nuniform texture_rect rect;\nuniform string label = \"hi\";\nuniform half odd;\nfloat4 VSSolid(float4 pos : POSITION) : POSITION\n{\n\treturn mul(pos, ViewProj);\n}\nfloat4 PSSolid(float4 pos : POSITION) : TARGET\n{\n\treturn color;\n}\ntechnique Solid { pass { vertex_shader = VSSolid(pos); pixel_shader = PSSolid(pos); } }",
        file: Some("test.effect"),
        params: &[
            P {
                ty: "float4x4",
                name: "ViewProj",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4x4",
                name: "World",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4x4",
                name: "viewproj",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "color",
                default: &[
                    0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x3f, 0x00, 0x00, 0x80, 0x3e, 0x00,
                    0x00, 0x80, 0x3f,
                ],
                annotations: &[],
            },
            P {
                ty: "float",
                name: "gain",
                default: &[0x00, 0x00, 0xc0, 0x3f],
                annotations: &[
                    P {
                        ty: "string",
                        name: "name",
                        default: &[0x47, 0x61, 0x69, 0x6e, 0x00],
                        annotations: &[],
                    },
                    P {
                        ty: "float",
                        name: "minimum",
                        default: &[0x00, 0x00, 0x00, 0x00],
                        annotations: &[],
                    },
                    P {
                        ty: "int",
                        name: "step",
                        default: &[0x02, 0x00, 0x00, 0x00],
                        annotations: &[],
                    },
                ],
            },
            P {
                ty: "float2",
                name: "size",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float3",
                name: "axis",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "int",
                name: "count",
                default: &[0x03, 0x00, 0x00, 0x00],
                annotations: &[],
            },
            P {
                ty: "int2",
                name: "i2",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "int3",
                name: "i3",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "int4",
                name: "i4",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "bool",
                name: "flag",
                default: &[0x01, 0x00, 0x00, 0x00],
                annotations: &[],
            },
            P {
                ty: "texture2d",
                name: "image",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "texture_rect",
                name: "rect",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "string",
                name: "label",
                default: &[0x68, 0x69, 0x00],
                annotations: &[],
            },
            P {
                ty: "half",
                name: "odd",
                default: &[],
                annotations: &[],
            },
        ],
        techniques: &[T {
            name: Some("Solid"),
            passes: &[Ps {
                name: None,
                vertex: C {
                    text: Some(
                        "uniform float4x4 ViewProj;\n\n\nfloat4 VSSolid(float4 pos : POSITION)\n{\n\treturn mul(pos, ViewProj);\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSSolid(pos);\n}\n",
                    ),
                    used: &["ViewProj"],
                },
                pixel: C {
                    text: Some(
                        "uniform float4 color;\n\n\nfloat4 PSSolid(float4 pos : POSITION)\n{\n\treturn color;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSSolid(pos);\n}\n",
                    ),
                    used: &["color"],
                },
            }],
        }],
        errors: &[],
        error_at: (25, 88),
        null_mask: 0x0,
        error_mask: 0x0,
        empty_error_mask: 0x0,
        missing: None,
        missing_mask: 0x0,
        expected: concat!(
            "create vertex 0 test.effect (Vertex shader, technique Solid, pass 0)\n",
            "    | uniform float4x4 ViewProj;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSSolid(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn mul(pos, ViewProj);\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSSolid(pos);\n",
            "    | }\n",
            "    -> shader 0\n",
            "create pixel 1 test.effect (Pixel shader, technique Solid, pass 0)\n",
            "    | uniform float4 color;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSSolid(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn color;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSSolid(pos);\n",
            "    | }\n",
            "    -> shader 1\n",
            "result 1\n",
            "param 0 ViewProj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 1 World section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 2 viewproj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 3 color section=0 type=7 changed=0 cur=0 default=0000803f0000003f0000803e0000803f effect=self sampler=null\n",
            "param 4 gain section=0 type=2 changed=0 cur=0 default=0000c03f effect=self sampler=null\n",
            "  annotation 0 name section=4 type=4 changed=0 cur=0 default=4761696e00 effect=self sampler=null\n",
            "  annotation 1 minimum section=4 type=2 changed=0 cur=0 default=00000000 effect=self sampler=null\n",
            "  annotation 2 step section=4 type=3 changed=0 cur=0 default=02000000 effect=self sampler=null\n",
            "param 5 size section=0 type=5 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 6 axis section=0 type=6 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 7 count section=0 type=3 changed=0 cur=0 default=03000000 effect=self sampler=null\n",
            "param 8 i2 section=0 type=8 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 9 i3 section=0 type=9 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 10 i4 section=0 type=10 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 11 flag section=0 type=1 changed=0 cur=0 default=01000000 effect=self sampler=null\n",
            "param 12 image section=0 type=12 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 13 rect section=0 type=12 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 14 label section=0 type=4 changed=0 cur=0 default=686900 effect=self sampler=null\n",
            "param 15 odd section=0 type=0 changed=0 cur=0 default= effect=self sampler=null\n",
            "view_proj=0:ViewProj world=1:World scale=-\n",
            "technique 0 Solid section=1 effect=self passes=1\n",
            "  pass 0 (null) section=3 vert=0 pixel=1\n",
            "      vparams 0:ViewProj/0:ViewProj\n",
            "      pparams 3:color/1:color\n",
            "parser 0 ViewProj left=0 param=0:ViewProj\n",
            "parser 1 World left=0 param=1:World\n",
            "parser 2 viewproj left=0 param=2:viewproj\n",
            "parser 3 color left=0 param=3:color\n",
            "parser 4 gain left=0 param=4:gain\n",
            "  parser annotation 0 name left=0\n",
            "  parser annotation 1 minimum left=0\n",
            "  parser annotation 2 step left=0\n",
            "parser 5 size left=0 param=5:size\n",
            "parser 6 axis left=0 param=6:axis\n",
            "parser 7 count left=0 param=7:count\n",
            "parser 8 i2 left=0 param=8:i2\n",
            "parser 9 i3 left=0 param=9:i3\n",
            "parser 10 i4 left=0 param=10:i4\n",
            "parser 11 flag left=0 param=11:flag\n",
            "parser 12 image left=0 param=12:image\n",
            "parser 13 rect left=0 param=13:rect\n",
            "parser 14 label left=0 param=14:label\n",
            "parser 15 odd left=0 param=15:odd\n",
        ),
    },
    Fx {
        text: "uniform float4x4 ViewProj;\nuniform float4x4 World;\nuniform float4x4 ViewProj;\nuniform float4x4 World;\nuniform float4 color;\nfloat4 VSSolid(float4 pos : POSITION) : POSITION\n{\n\treturn mul(pos, ViewProj);\n}\nfloat4 PSSolid(float4 pos : POSITION) : TARGET\n{\n\treturn color;\n}\ntechnique Solid { pass { vertex_shader = VSSolid(pos); pixel_shader = PSSolid(pos); } }",
        file: Some("test.effect"),
        params: &[
            P {
                ty: "float4x4",
                name: "ViewProj",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4x4",
                name: "World",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4x4",
                name: "ViewProj",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4x4",
                name: "World",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "color",
                default: &[],
                annotations: &[],
            },
        ],
        techniques: &[T {
            name: Some("Solid"),
            passes: &[Ps {
                name: None,
                vertex: C {
                    text: Some(
                        "uniform float4x4 ViewProj;\n\n\nfloat4 VSSolid(float4 pos : POSITION)\n{\n\treturn mul(pos, ViewProj);\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSSolid(pos);\n}\n",
                    ),
                    used: &["ViewProj"],
                },
                pixel: C {
                    text: Some(
                        "uniform float4 color;\n\n\nfloat4 PSSolid(float4 pos : POSITION)\n{\n\treturn color;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSSolid(pos);\n}\n",
                    ),
                    used: &["color"],
                },
            }],
        }],
        errors: &[],
        error_at: (14, 88),
        null_mask: 0x0,
        error_mask: 0x0,
        empty_error_mask: 0x0,
        missing: None,
        missing_mask: 0x0,
        expected: concat!(
            "create vertex 0 test.effect (Vertex shader, technique Solid, pass 0)\n",
            "    | uniform float4x4 ViewProj;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSSolid(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn mul(pos, ViewProj);\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSSolid(pos);\n",
            "    | }\n",
            "    -> shader 0\n",
            "create pixel 1 test.effect (Pixel shader, technique Solid, pass 0)\n",
            "    | uniform float4 color;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSSolid(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn color;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSSolid(pos);\n",
            "    | }\n",
            "    -> shader 1\n",
            "result 1\n",
            "param 0 ViewProj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 1 World section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 2 ViewProj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 3 World section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 4 color section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "view_proj=2:ViewProj world=3:World scale=-\n",
            "technique 0 Solid section=1 effect=self passes=1\n",
            "  pass 0 (null) section=3 vert=0 pixel=1\n",
            "      vparams 0:ViewProj/0:ViewProj\n",
            "      pparams 4:color/1:color\n",
            "parser 0 ViewProj left=0 param=0:ViewProj\n",
            "parser 1 World left=0 param=1:World\n",
            "parser 2 ViewProj left=0 param=2:ViewProj\n",
            "parser 3 World left=0 param=3:World\n",
            "parser 4 color left=0 param=4:color\n",
        ),
    },
    Fx {
        text: "uniform float4x4 ViewProj;\nuniform float4 color <string name = \"Color\";> = {0.0, 0.0, 0.0, 1.0};\nuniform float level <float maximum = 4.0;>;\nfloat4 VSSolid(float4 pos : POSITION) : POSITION\n{\n\treturn mul(pos, ViewProj);\n}\nfloat4 PSSolid(float4 pos : POSITION) : TARGET\n{\n\treturn color;\n}\ntechnique Solid { pass { vertex_shader = VSSolid(pos); pixel_shader = PSSolid(pos); } }",
        file: Some("test.effect"),
        params: &[
            P {
                ty: "float4x4",
                name: "ViewProj",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "color",
                default: &[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x80, 0x3f,
                ],
                annotations: &[P {
                    ty: "string",
                    name: "name",
                    default: &[0x43, 0x6f, 0x6c, 0x6f, 0x72, 0x00],
                    annotations: &[],
                }],
            },
            P {
                ty: "float",
                name: "level",
                default: &[],
                annotations: &[P {
                    ty: "float",
                    name: "maximum",
                    default: &[0x00, 0x00, 0x80, 0x40],
                    annotations: &[],
                }],
            },
        ],
        techniques: &[T {
            name: Some("Solid"),
            passes: &[Ps {
                name: None,
                vertex: C {
                    text: Some(
                        "uniform float4x4 ViewProj;\n\n\nfloat4 VSSolid(float4 pos : POSITION)\n{\n\treturn mul(pos, ViewProj);\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSSolid(pos);\n}\n",
                    ),
                    used: &["ViewProj"],
                },
                pixel: C {
                    text: Some(
                        "uniform float4 color;\n\n\nfloat4 PSSolid(float4 pos : POSITION)\n{\n\treturn color;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSSolid(pos);\n}\n",
                    ),
                    used: &["color"],
                },
            }],
        }],
        errors: &[],
        error_at: (12, 88),
        null_mask: 0x0,
        error_mask: 0x0,
        empty_error_mask: 0x0,
        missing: None,
        missing_mask: 0x0,
        expected: concat!(
            "create vertex 0 test.effect (Vertex shader, technique Solid, pass 0)\n",
            "    | uniform float4x4 ViewProj;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSSolid(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn mul(pos, ViewProj);\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSSolid(pos);\n",
            "    | }\n",
            "    -> shader 0\n",
            "create pixel 1 test.effect (Pixel shader, technique Solid, pass 0)\n",
            "    | uniform float4 color;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSSolid(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn color;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSSolid(pos);\n",
            "    | }\n",
            "    -> shader 1\n",
            "result 1\n",
            "param 0 ViewProj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 1 color section=0 type=7 changed=0 cur=0 default=0000000000000000000000000000803f effect=self sampler=null\n",
            "  annotation 0 name section=4 type=4 changed=0 cur=0 default=436f6c6f7200 effect=self sampler=null\n",
            "param 2 level section=0 type=2 changed=0 cur=0 default= effect=self sampler=null\n",
            "  annotation 0 maximum section=4 type=2 changed=0 cur=0 default=00008040 effect=self sampler=null\n",
            "view_proj=0:ViewProj world=- scale=-\n",
            "technique 0 Solid section=1 effect=self passes=1\n",
            "  pass 0 (null) section=3 vert=0 pixel=1\n",
            "      vparams 0:ViewProj/0:ViewProj\n",
            "      pparams 1:color/1:color\n",
            "parser 0 ViewProj left=0 param=0:ViewProj\n",
            "parser 1 color left=0 param=1:color\n",
            "  parser annotation 0 name left=0\n",
            "parser 2 level left=0 param=2:level\n",
            "  parser annotation 0 maximum left=0\n",
        ),
    },
    Fx {
        text: "uniform float4x4 ViewProj;\nuniform float4 color;\n",
        file: Some("test.effect"),
        params: &[
            P {
                ty: "float4x4",
                name: "ViewProj",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "color",
                default: &[],
                annotations: &[],
            },
        ],
        techniques: &[],
        errors: &[],
        error_at: (3, 1),
        null_mask: 0x0,
        error_mask: 0x0,
        empty_error_mask: 0x0,
        missing: None,
        missing_mask: 0x0,
        expected: concat!(
            "result 1\n",
            "param 0 ViewProj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 1 color section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "view_proj=0:ViewProj world=- scale=-\n",
            "parser 0 ViewProj left=0 param=0:ViewProj\n",
            "parser 1 color left=0 param=1:color\n",
        ),
    },
];

/// The C test's `techniques_cases`, less the texts that never reach the compile step.
static TECHNIQUES_CASES: &[Fx] = &[
    Fx {
        text: "uniform float4x4 ViewProj;\nuniform float4 color;\nfloat4 VSSolid(float4 pos : POSITION) : POSITION\n{\n\treturn mul(pos, ViewProj);\n}\nfloat4 PSSolid(float4 pos : POSITION) : TARGET\n{\n\treturn color;\n}\ntechnique Draw { pass first { vertex_shader = VSSolid(pos); pixel_shader = PSSolid(pos); } pass { vertex_shader = VSSolid(pos); pixel_shader = PSSolid(pos); } }\ntechnique Empty { }\n",
        file: Some("test.effect"),
        params: &[
            P {
                ty: "float4x4",
                name: "ViewProj",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "color",
                default: &[],
                annotations: &[],
            },
        ],
        techniques: &[
            T {
                name: Some("Draw"),
                passes: &[
                    Ps {
                        name: Some("first"),
                        vertex: C {
                            text: Some(
                                "uniform float4x4 ViewProj;\n\n\nfloat4 VSSolid(float4 pos : POSITION)\n{\n\treturn mul(pos, ViewProj);\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSSolid(pos);\n}\n",
                            ),
                            used: &["ViewProj"],
                        },
                        pixel: C {
                            text: Some(
                                "uniform float4 color;\n\n\nfloat4 PSSolid(float4 pos : POSITION)\n{\n\treturn color;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSSolid(pos);\n}\n",
                            ),
                            used: &["color"],
                        },
                    },
                    Ps {
                        name: None,
                        vertex: C {
                            text: Some(
                                "uniform float4x4 ViewProj;\n\n\nfloat4 VSSolid(float4 pos : POSITION)\n{\n\treturn mul(pos, ViewProj);\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSSolid(pos);\n}\n",
                            ),
                            used: &["ViewProj"],
                        },
                        pixel: C {
                            text: Some(
                                "uniform float4 color;\n\n\nfloat4 PSSolid(float4 pos : POSITION)\n{\n\treturn color;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSSolid(pos);\n}\n",
                            ),
                            used: &["color"],
                        },
                    },
                ],
            },
            T {
                name: Some("Empty"),
                passes: &[],
            },
        ],
        errors: &[],
        error_at: (13, 1),
        null_mask: 0x0,
        error_mask: 0x0,
        empty_error_mask: 0x0,
        missing: None,
        missing_mask: 0x0,
        expected: concat!(
            "create vertex 0 test.effect (Vertex shader, technique Draw, pass 0)\n",
            "    | uniform float4x4 ViewProj;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSSolid(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn mul(pos, ViewProj);\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSSolid(pos);\n",
            "    | }\n",
            "    -> shader 0\n",
            "create pixel 1 test.effect (Pixel shader, technique Draw, pass 0)\n",
            "    | uniform float4 color;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSSolid(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn color;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSSolid(pos);\n",
            "    | }\n",
            "    -> shader 1\n",
            "create vertex 2 test.effect (Vertex shader, technique Draw, pass 1)\n",
            "    | uniform float4x4 ViewProj;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSSolid(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn mul(pos, ViewProj);\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSSolid(pos);\n",
            "    | }\n",
            "    -> shader 2\n",
            "create pixel 3 test.effect (Pixel shader, technique Draw, pass 1)\n",
            "    | uniform float4 color;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSSolid(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn color;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSSolid(pos);\n",
            "    | }\n",
            "    -> shader 3\n",
            "result 1\n",
            "param 0 ViewProj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 1 color section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "view_proj=0:ViewProj world=- scale=-\n",
            "technique 0 Draw section=1 effect=self passes=2\n",
            "  pass 0 first section=3 vert=0 pixel=1\n",
            "      vparams 0:ViewProj/0:ViewProj\n",
            "      pparams 1:color/1:color\n",
            "  pass 1 (null) section=3 vert=2 pixel=3\n",
            "      vparams 0:ViewProj/2:ViewProj\n",
            "      pparams 1:color/3:color\n",
            "technique 1 Empty section=1 effect=self passes=0\n",
            "parser 0 ViewProj left=0 param=0:ViewProj\n",
            "parser 1 color left=0 param=1:color\n",
        ),
    },
    Fx {
        text: "uniform float4x4 ViewProj;\nuniform float4 color;\nfloat4 VSSolid(float4 pos : POSITION) : POSITION\n{\n\treturn mul(pos, ViewProj);\n}\nfloat4 PSSolid(float4 pos : POSITION) : TARGET\n{\n\treturn color;\n}\ntechnique Solid { pass { vertex_shader = NULL; pixel_shader = PSSolid(pos); } }",
        file: Some("test.effect"),
        params: &[
            P {
                ty: "float4x4",
                name: "ViewProj",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "color",
                default: &[],
                annotations: &[],
            },
        ],
        techniques: &[T {
            name: Some("Solid"),
            passes: &[Ps {
                name: None,
                vertex: C {
                    text: None,
                    used: &[],
                },
                pixel: C {
                    text: Some(
                        "uniform float4 color;\n\n\nfloat4 PSSolid(float4 pos : POSITION)\n{\n\treturn color;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSSolid(pos);\n}\n",
                    ),
                    used: &["color"],
                },
            }],
        }],
        errors: &[],
        error_at: (11, 80),
        null_mask: 0x0,
        error_mask: 0x0,
        empty_error_mask: 0x0,
        missing: None,
        missing_mask: 0x0,
        expected: concat!(
            "create vertex 0 test.effect (Vertex shader, technique Solid, pass 0)\n",
            "    (null text)\n",
            "    -> shader 0\n",
            "create pixel 1 test.effect (Pixel shader, technique Solid, pass 0)\n",
            "    | uniform float4 color;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSSolid(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn color;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSSolid(pos);\n",
            "    | }\n",
            "    -> shader 1\n",
            "result 1\n",
            "param 0 ViewProj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 1 color section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "view_proj=0:ViewProj world=- scale=-\n",
            "technique 0 Solid section=1 effect=self passes=1\n",
            "  pass 0 (null) section=3 vert=0 pixel=1\n",
            "      vparams\n",
            "      pparams 1:color/1:color\n",
            "parser 0 ViewProj left=0 param=0:ViewProj\n",
            "parser 1 color left=0 param=1:color\n",
        ),
    },
    Fx {
        text: "uniform float4 color;\nfloat4 VSSolid(float4 pos : POSITION) : POSITION\n{\n\treturn mul(pos, ViewProj);\n}\nfloat4 PSSolid(float4 pos : POSITION) : TARGET\n{\n\treturn color;\n}\ntechnique Solid { pass { } }",
        file: Some("test.effect"),
        params: &[P {
            ty: "float4",
            name: "color",
            default: &[],
            annotations: &[],
        }],
        techniques: &[T {
            name: Some("Solid"),
            passes: &[Ps {
                name: None,
                vertex: C {
                    text: None,
                    used: &[],
                },
                pixel: C {
                    text: None,
                    used: &[],
                },
            }],
        }],
        errors: &[],
        error_at: (10, 29),
        null_mask: 0x0,
        error_mask: 0x0,
        empty_error_mask: 0x0,
        missing: None,
        missing_mask: 0x0,
        expected: concat!(
            "create vertex 0 test.effect (Vertex shader, technique Solid, pass 0)\n",
            "    (null text)\n",
            "    -> shader 0\n",
            "create pixel 1 test.effect (Pixel shader, technique Solid, pass 0)\n",
            "    (null text)\n",
            "    -> shader 1\n",
            "result 1\n",
            "param 0 color section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "view_proj=- world=- scale=-\n",
            "technique 0 Solid section=1 effect=self passes=1\n",
            "  pass 0 (null) section=3 vert=0 pixel=1\n",
            "      vparams\n",
            "      pparams\n",
            "parser 0 color left=0 param=0:color\n",
        ),
    },
    Fx {
        text: "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\nfloat4 VSAbc(float4 pos : POSITION) : POSITION\n{\n\treturn pos * c + b * a + c * b;\n}\ntechnique Abc { pass { vertex_shader = VSAbc(pos); pixel_shader = VSAbc(pos); } }",
        file: Some("test.effect"),
        params: &[
            P {
                ty: "float4",
                name: "a",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "b",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "c",
                default: &[],
                annotations: &[],
            },
        ],
        techniques: &[T {
            name: Some("Abc"),
            passes: &[Ps {
                name: None,
                vertex: C {
                    text: Some(
                        "uniform float4 c;\nuniform float4 b;\nuniform float4 a;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * c + b * a + c * b;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                    ),
                    used: &["c", "b", "a"],
                },
                pixel: C {
                    text: Some(
                        "uniform float4 c;\nuniform float4 b;\nuniform float4 a;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * c + b * a + c * b;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                    ),
                    used: &["c", "b", "a"],
                },
            }],
        }],
        errors: &[],
        error_at: (8, 82),
        null_mask: 0x0,
        error_mask: 0x0,
        empty_error_mask: 0x0,
        missing: None,
        missing_mask: 0x0,
        expected: concat!(
            "create vertex 0 test.effect (Vertex shader, technique Abc, pass 0)\n",
            "    | uniform float4 c;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 a;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * c + b * a + c * b;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> shader 0\n",
            "create pixel 1 test.effect (Pixel shader, technique Abc, pass 0)\n",
            "    | uniform float4 c;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 a;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * c + b * a + c * b;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> shader 1\n",
            "result 1\n",
            "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "view_proj=- world=- scale=-\n",
            "technique 0 Abc section=1 effect=self passes=1\n",
            "  pass 0 (null) section=3 vert=0 pixel=1\n",
            "      vparams 2:c/0:c, 1:b/0:b, 0:a/0:a\n",
            "      pparams 2:c/1:c, 1:b/1:b, 0:a/1:a\n",
            "parser 0 a left=0 param=0:a\n",
            "parser 1 b left=0 param=1:b\n",
            "parser 2 c left=0 param=2:c\n",
        ),
    },
];

/// The C test's `shaders_cases`, less the texts that never reach the compile step.
static SHADERS_CASES: &[Fx] = &[
    Fx {
        text: "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\nfloat4 VSAbc(float4 pos : POSITION) : POSITION\n{\n\treturn pos * a * b * c;\n}\nfloat4 PSAbc(float4 pos : POSITION) : TARGET\n{\n\treturn a + b + c;\n}\ntechnique One { pass p0 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } pass p1 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } }\n",
        file: Some("test.effect"),
        params: &[
            P {
                ty: "float4",
                name: "a",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "b",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "c",
                default: &[],
                annotations: &[],
            },
        ],
        techniques: &[T {
            name: Some("One"),
            passes: &[
                Ps {
                    name: Some("p0"),
                    vertex: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * a * b * c;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                    pixel: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 PSAbc(float4 pos : POSITION)\n{\n\treturn a + b + c;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                },
                Ps {
                    name: Some("p1"),
                    vertex: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * a * b * c;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                    pixel: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 PSAbc(float4 pos : POSITION)\n{\n\treturn a + b + c;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                },
            ],
        }],
        errors: &[],
        error_at: (13, 1),
        null_mask: 0x1,
        error_mask: 0x0,
        empty_error_mask: 0x0,
        missing: None,
        missing_mask: 0x0,
        expected: concat!(
            "create vertex 0 test.effect (Vertex shader, technique One, pass 0)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * a * b * c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> NULL\n",
            "create pixel 1 test.effect (Pixel shader, technique One, pass 0)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn a + b + c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSAbc(pos);\n",
            "    | }\n",
            "    -> shader 1\n",
            "create vertex 2 test.effect (Vertex shader, technique One, pass 1)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * a * b * c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> shader 2\n",
            "create pixel 3 test.effect (Pixel shader, technique One, pass 1)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn a + b + c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSAbc(pos);\n",
            "    | }\n",
            "    -> shader 3\n",
            "result 0\n",
            "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "view_proj=- world=- scale=-\n",
            "technique 0 One section=1 effect=self passes=2\n",
            "  pass 0 p0 section=3 vert=NULL pixel=1\n",
            "      vparams\n",
            "      pparams 0:a/1:a, 1:b/1:b, 2:c/1:c\n",
            "  pass 1 p1 section=3 vert=2 pixel=3\n",
            "      vparams 0:a/2:a, 1:b/2:b, 2:c/2:c\n",
            "      pparams 0:a/3:a, 1:b/3:b, 2:c/3:c\n",
            "parser 0 a left=0 param=0:a\n",
            "parser 1 b left=0 param=1:b\n",
            "parser 2 c left=0 param=2:c\n",
            "log 100 Pass (0) <p0> missing vertex shader!\n",
        ),
    },
    Fx {
        text: "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\nfloat4 VSAbc(float4 pos : POSITION) : POSITION\n{\n\treturn pos * a * b * c;\n}\nfloat4 PSAbc(float4 pos : POSITION) : TARGET\n{\n\treturn a + b + c;\n}\ntechnique One { pass p0 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } pass p1 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } }\n",
        file: Some("test.effect"),
        params: &[
            P {
                ty: "float4",
                name: "a",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "b",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "c",
                default: &[],
                annotations: &[],
            },
        ],
        techniques: &[T {
            name: Some("One"),
            passes: &[
                Ps {
                    name: Some("p0"),
                    vertex: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * a * b * c;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                    pixel: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 PSAbc(float4 pos : POSITION)\n{\n\treturn a + b + c;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                },
                Ps {
                    name: Some("p1"),
                    vertex: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * a * b * c;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                    pixel: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 PSAbc(float4 pos : POSITION)\n{\n\treturn a + b + c;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                },
            ],
        }],
        errors: &[],
        error_at: (13, 1),
        null_mask: 0x2,
        error_mask: 0x2,
        empty_error_mask: 0x0,
        missing: None,
        missing_mask: 0x0,
        expected: concat!(
            "create vertex 0 test.effect (Vertex shader, technique One, pass 0)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * a * b * c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> shader 0\n",
            "create pixel 1 test.effect (Pixel shader, technique One, pass 0)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn a + b + c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSAbc(pos);\n",
            "    | }\n",
            "    -> NULL error \"fail 1\"\n",
            "create vertex 2 test.effect (Vertex shader, technique One, pass 1)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * a * b * c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> shader 2\n",
            "create pixel 3 test.effect (Pixel shader, technique One, pass 1)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn a + b + c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSAbc(pos);\n",
            "    | }\n",
            "    -> shader 3\n",
            "result 0\n",
            "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "view_proj=- world=- scale=-\n",
            "technique 0 One section=1 effect=self passes=2\n",
            "  pass 0 p0 section=3 vert=0 pixel=NULL\n",
            "      vparams 0:a/0:a, 1:b/0:b, 2:c/0:c\n",
            "      pparams\n",
            "  pass 1 p1 section=3 vert=2 pixel=3\n",
            "      vparams 0:a/2:a, 1:b/2:b, 2:c/2:c\n",
            "      pparams 0:a/3:a, 1:b/3:b, 2:c/3:c\n",
            "parser 0 a left=0 param=0:a\n",
            "parser 1 b left=0 param=1:b\n",
            "parser 2 c left=0 param=2:c\n",
            "error 13:1 0 Error creating shader: fail 1\n",
            "log 100 Pass (0) <p0> missing pixel shader!\n",
        ),
    },
    Fx {
        text: "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\nfloat4 VSAbc(float4 pos : POSITION) : POSITION\n{\n\treturn pos * a * b * c;\n}\nfloat4 PSAbc(float4 pos : POSITION) : TARGET\n{\n\treturn a + b + c;\n}\ntechnique One { pass p0 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } pass p1 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } }\n",
        file: Some("test.effect"),
        params: &[
            P {
                ty: "float4",
                name: "a",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "b",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "c",
                default: &[],
                annotations: &[],
            },
        ],
        techniques: &[T {
            name: Some("One"),
            passes: &[
                Ps {
                    name: Some("p0"),
                    vertex: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * a * b * c;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                    pixel: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 PSAbc(float4 pos : POSITION)\n{\n\treturn a + b + c;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                },
                Ps {
                    name: Some("p1"),
                    vertex: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * a * b * c;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                    pixel: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 PSAbc(float4 pos : POSITION)\n{\n\treturn a + b + c;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                },
            ],
        }],
        errors: &[],
        error_at: (13, 1),
        null_mask: 0x0,
        error_mask: 0x4,
        empty_error_mask: 0x0,
        missing: None,
        missing_mask: 0x0,
        expected: concat!(
            "create vertex 0 test.effect (Vertex shader, technique One, pass 0)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * a * b * c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> shader 0\n",
            "create pixel 1 test.effect (Pixel shader, technique One, pass 0)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn a + b + c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSAbc(pos);\n",
            "    | }\n",
            "    -> shader 1\n",
            "create vertex 2 test.effect (Vertex shader, technique One, pass 1)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * a * b * c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> shader 2 error \"fail 2\"\n",
            "create pixel 3 test.effect (Pixel shader, technique One, pass 1)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn a + b + c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSAbc(pos);\n",
            "    | }\n",
            "    -> shader 3\n",
            "result 1\n",
            "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "view_proj=- world=- scale=-\n",
            "technique 0 One section=1 effect=self passes=2\n",
            "  pass 0 p0 section=3 vert=0 pixel=1\n",
            "      vparams 0:a/0:a, 1:b/0:b, 2:c/0:c\n",
            "      pparams 0:a/1:a, 1:b/1:b, 2:c/1:c\n",
            "  pass 1 p1 section=3 vert=2 pixel=3\n",
            "      vparams 0:a/2:a, 1:b/2:b, 2:c/2:c\n",
            "      pparams 0:a/3:a, 1:b/3:b, 2:c/3:c\n",
            "parser 0 a left=0 param=0:a\n",
            "parser 1 b left=0 param=1:b\n",
            "parser 2 c left=0 param=2:c\n",
            "error 13:1 0 Error creating shader: fail 2\n",
        ),
    },
    Fx {
        text: "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\nfloat4 VSAbc(float4 pos : POSITION) : POSITION\n{\n\treturn pos * a * b * c;\n}\nfloat4 PSAbc(float4 pos : POSITION) : TARGET\n{\n\treturn a + b + c;\n}\ntechnique One { pass p0 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } pass p1 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } }\n",
        file: Some("test.effect"),
        params: &[
            P {
                ty: "float4",
                name: "a",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "b",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "c",
                default: &[],
                annotations: &[],
            },
        ],
        techniques: &[T {
            name: Some("One"),
            passes: &[
                Ps {
                    name: Some("p0"),
                    vertex: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * a * b * c;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                    pixel: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 PSAbc(float4 pos : POSITION)\n{\n\treturn a + b + c;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                },
                Ps {
                    name: Some("p1"),
                    vertex: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * a * b * c;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                    pixel: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 PSAbc(float4 pos : POSITION)\n{\n\treturn a + b + c;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                },
            ],
        }],
        errors: &[],
        error_at: (13, 1),
        null_mask: 0x0,
        error_mask: 0x0,
        empty_error_mask: 0x9,
        missing: None,
        missing_mask: 0x0,
        expected: concat!(
            "create vertex 0 test.effect (Vertex shader, technique One, pass 0)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * a * b * c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> shader 0 error \"\"\n",
            "create pixel 1 test.effect (Pixel shader, technique One, pass 0)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn a + b + c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSAbc(pos);\n",
            "    | }\n",
            "    -> shader 1\n",
            "create vertex 2 test.effect (Vertex shader, technique One, pass 1)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * a * b * c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> shader 2\n",
            "create pixel 3 test.effect (Pixel shader, technique One, pass 1)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn a + b + c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSAbc(pos);\n",
            "    | }\n",
            "    -> shader 3 error \"\"\n",
            "result 1\n",
            "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "view_proj=- world=- scale=-\n",
            "technique 0 One section=1 effect=self passes=2\n",
            "  pass 0 p0 section=3 vert=0 pixel=1\n",
            "      vparams 0:a/0:a, 1:b/0:b, 2:c/0:c\n",
            "      pparams 0:a/1:a, 1:b/1:b, 2:c/1:c\n",
            "  pass 1 p1 section=3 vert=2 pixel=3\n",
            "      vparams 0:a/2:a, 1:b/2:b, 2:c/2:c\n",
            "      pparams 0:a/3:a, 1:b/3:b, 2:c/3:c\n",
            "parser 0 a left=0 param=0:a\n",
            "parser 1 b left=0 param=1:b\n",
            "parser 2 c left=0 param=2:c\n",
        ),
    },
    Fx {
        text: "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\nfloat4 VSAbc(float4 pos : POSITION) : POSITION\n{\n\treturn pos * a * b * c;\n}\nfloat4 PSAbc(float4 pos : POSITION) : TARGET\n{\n\treturn a + b + c;\n}\ntechnique One { pass p0 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } pass p1 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } }\n",
        file: Some("test.effect"),
        params: &[
            P {
                ty: "float4",
                name: "a",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "b",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "c",
                default: &[],
                annotations: &[],
            },
        ],
        techniques: &[T {
            name: Some("One"),
            passes: &[
                Ps {
                    name: Some("p0"),
                    vertex: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * a * b * c;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                    pixel: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 PSAbc(float4 pos : POSITION)\n{\n\treturn a + b + c;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                },
                Ps {
                    name: Some("p1"),
                    vertex: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * a * b * c;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                    pixel: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 PSAbc(float4 pos : POSITION)\n{\n\treturn a + b + c;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                },
            ],
        }],
        errors: &[],
        error_at: (13, 1),
        null_mask: 0x0,
        error_mask: 0x0,
        empty_error_mask: 0x0,
        missing: Some("b"),
        missing_mask: 0x1,
        expected: concat!(
            "create vertex 0 test.effect (Vertex shader, technique One, pass 0)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * a * b * c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> shader 0\n",
            "create pixel 1 test.effect (Pixel shader, technique One, pass 0)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn a + b + c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSAbc(pos);\n",
            "    | }\n",
            "    -> shader 1\n",
            "create vertex 2 test.effect (Vertex shader, technique One, pass 1)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * a * b * c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> shader 2\n",
            "create pixel 3 test.effect (Pixel shader, technique One, pass 1)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn a + b + c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSAbc(pos);\n",
            "    | }\n",
            "    -> shader 3\n",
            "result 0\n",
            "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "view_proj=- world=- scale=-\n",
            "technique 0 One section=1 effect=self passes=2\n",
            "  pass 0 p0 section=3 vert=0 pixel=1\n",
            "      vparams 0:a/0:a, 1:b/NULL, -/NULL\n",
            "      pparams 0:a/1:a, 1:b/1:b, 2:c/1:c\n",
            "  pass 1 p1 section=3 vert=2 pixel=3\n",
            "      vparams 0:a/2:a, 1:b/2:b, 2:c/2:c\n",
            "      pparams 0:a/3:a, 1:b/3:b, 2:c/3:c\n",
            "parser 0 a left=0 param=0:a\n",
            "parser 1 b left=0 param=1:b\n",
            "parser 2 c left=0 param=2:c\n",
            "log 100 Effect shader parameter not found\n",
            "log 100 Pass (0) <p0> missing vertex shader!\n",
        ),
    },
    Fx {
        text: "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\nfloat4 VSAbc(float4 pos : POSITION) : POSITION\n{\n\treturn pos * a * b * c;\n}\nfloat4 PSAbc(float4 pos : POSITION) : TARGET\n{\n\treturn a + b + c;\n}\ntechnique One { pass p0 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } pass p1 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } }\n",
        file: Some("test.effect"),
        params: &[
            P {
                ty: "float4",
                name: "a",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "b",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "c",
                default: &[],
                annotations: &[],
            },
        ],
        techniques: &[T {
            name: Some("One"),
            passes: &[
                Ps {
                    name: Some("p0"),
                    vertex: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * a * b * c;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                    pixel: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 PSAbc(float4 pos : POSITION)\n{\n\treturn a + b + c;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                },
                Ps {
                    name: Some("p1"),
                    vertex: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * a * b * c;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                    pixel: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 PSAbc(float4 pos : POSITION)\n{\n\treturn a + b + c;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                },
            ],
        }],
        errors: &[],
        error_at: (13, 1),
        null_mask: 0x0,
        error_mask: 0x0,
        empty_error_mask: 0x0,
        missing: Some("a"),
        missing_mask: 0x8,
        expected: concat!(
            "create vertex 0 test.effect (Vertex shader, technique One, pass 0)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * a * b * c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> shader 0\n",
            "create pixel 1 test.effect (Pixel shader, technique One, pass 0)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn a + b + c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSAbc(pos);\n",
            "    | }\n",
            "    -> shader 1\n",
            "create vertex 2 test.effect (Vertex shader, technique One, pass 1)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * a * b * c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> shader 2\n",
            "create pixel 3 test.effect (Pixel shader, technique One, pass 1)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn a + b + c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSAbc(pos);\n",
            "    | }\n",
            "    -> shader 3\n",
            "result 0\n",
            "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "view_proj=- world=- scale=-\n",
            "technique 0 One section=1 effect=self passes=2\n",
            "  pass 0 p0 section=3 vert=0 pixel=1\n",
            "      vparams 0:a/0:a, 1:b/0:b, 2:c/0:c\n",
            "      pparams 0:a/1:a, 1:b/1:b, 2:c/1:c\n",
            "  pass 1 p1 section=3 vert=2 pixel=3\n",
            "      vparams 0:a/2:a, 1:b/2:b, 2:c/2:c\n",
            "      pparams 0:a/NULL, -/NULL, -/NULL\n",
            "parser 0 a left=0 param=0:a\n",
            "parser 1 b left=0 param=1:b\n",
            "parser 2 c left=0 param=2:c\n",
            "log 100 Effect shader parameter not found\n",
            "log 100 Pass (1) <p1> missing pixel shader!\n",
        ),
    },
    Fx {
        text: "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\nfloat4 VSAbc(float4 pos : POSITION) : POSITION\n{\n\treturn pos * a * b * c;\n}\nfloat4 PSAbc(float4 pos : POSITION) : TARGET\n{\n\treturn a + b + c;\n}\ntechnique One { pass p0 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } pass p1 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } }\n",
        file: Some("test.effect"),
        params: &[
            P {
                ty: "float4",
                name: "a",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "b",
                default: &[],
                annotations: &[],
            },
            P {
                ty: "float4",
                name: "c",
                default: &[],
                annotations: &[],
            },
        ],
        techniques: &[T {
            name: Some("One"),
            passes: &[
                Ps {
                    name: Some("p0"),
                    vertex: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * a * b * c;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                    pixel: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 PSAbc(float4 pos : POSITION)\n{\n\treturn a + b + c;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                },
                Ps {
                    name: Some("p1"),
                    vertex: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 VSAbc(float4 pos : POSITION)\n{\n\treturn pos * a * b * c;\n}\n\nfloat4 main(float4 pos : POSITION) : POSITION\n{\n\treturn VSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                    pixel: C {
                        text: Some(
                            "uniform float4 a;\nuniform float4 b;\nuniform float4 c;\n\n\nfloat4 PSAbc(float4 pos : POSITION)\n{\n\treturn a + b + c;\n}\n\nfloat4 main(float4 pos : POSITION) : TARGET\n{\n\treturn PSAbc(pos);\n}\n",
                        ),
                        used: &["a", "b", "c"],
                    },
                },
            ],
        }],
        errors: &[],
        error_at: (13, 1),
        null_mask: 0xf,
        error_mask: 0x5,
        empty_error_mask: 0x0,
        missing: None,
        missing_mask: 0x0,
        expected: concat!(
            "create vertex 0 test.effect (Vertex shader, technique One, pass 0)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * a * b * c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> NULL error \"fail 0\"\n",
            "create pixel 1 test.effect (Pixel shader, technique One, pass 0)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn a + b + c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSAbc(pos);\n",
            "    | }\n",
            "    -> NULL\n",
            "create vertex 2 test.effect (Vertex shader, technique One, pass 1)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 VSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn pos * a * b * c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : POSITION\n",
            "    | {\n",
            "    | \\treturn VSAbc(pos);\n",
            "    | }\n",
            "    -> NULL error \"fail 2\"\n",
            "create pixel 3 test.effect (Pixel shader, technique One, pass 1)\n",
            "    | uniform float4 a;\n",
            "    | uniform float4 b;\n",
            "    | uniform float4 c;\n",
            "    | \n",
            "    | \n",
            "    | float4 PSAbc(float4 pos : POSITION)\n",
            "    | {\n",
            "    | \\treturn a + b + c;\n",
            "    | }\n",
            "    | \n",
            "    | float4 main(float4 pos : POSITION) : TARGET\n",
            "    | {\n",
            "    | \\treturn PSAbc(pos);\n",
            "    | }\n",
            "    -> NULL\n",
            "result 0\n",
            "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n",
            "view_proj=- world=- scale=-\n",
            "technique 0 One section=1 effect=self passes=2\n",
            "  pass 0 p0 section=3 vert=NULL pixel=NULL\n",
            "      vparams\n",
            "      pparams\n",
            "  pass 1 p1 section=3 vert=NULL pixel=NULL\n",
            "      vparams\n",
            "      pparams\n",
            "parser 0 a left=0 param=0:a\n",
            "parser 1 b left=0 param=1:b\n",
            "parser 2 c left=0 param=2:c\n",
            "error 13:1 0 Error creating shader: fail 0\n",
            "error 13:1 0 Error creating shader: fail 2\n",
            "log 100 Pass (0) <p0> missing vertex shader!\n",
            "log 100 Pass (0) <p0> missing pixel shader!\n",
            "log 100 Pass (1) <p1> missing vertex shader!\n",
            "log 100 Pass (1) <p1> missing pixel shader!\n",
        ),
    },
];
