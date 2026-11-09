//! Tier 3: the shims against the original `libobs/graphics/effect.c.h`.
//!
//! The oracle (`rust/obs-c-oracle/oracle/effect_destroy.c`) is the C file,
//! unmodified, with `gs_effect_destroy` and `gs_effect_actually_destroy`
//! renamed, `effect_free()` of `graphics/effect.h` instantiated as
//! `oracle_effect_free`, and `bfree()` and `gs_shader_destroy()` renamed to
//! a recorder. The shim's parity [`Calls`] call the same recorder, so both
//! sides write one log of every free and shader destroy, in order. Each
//! side gets its own copy of the same random effect, built on the real
//! bmalloc; each logged pointer is named by the block or shader it is, so
//! the two logs compare by name.
//!
//! Runs: the C test's effect, NULL, and random effects (0 to 4 params with
//! up to 3 annotations each, nested to depth 2; 0 to 3 techniques with 0 to
//! 3 passes; names, paths, shaders and value arrays each there or NULL;
//! spare capacity; cached or not) through `gs_effect_destroy`,
//! `gs_effect_actually_destroy` and `effect_free`. Compared: the log, the
//! blocks left (none after a destroy that frees, all after one that
//! doesn't), and after `effect_free` every field of the effect.
//!
//! No intentional differences.

use core::ffi::{c_char, c_void};
use core::ptr;
use std::collections::HashMap;
use std::sync::Mutex;

use obs_c_oracle::effect_destroy as c;
use obs_graphics::ffi::effect_destroy::{self as r, Calls};
use obs_graphics::ffi::effect_lookups::{
    gs_effect, gs_effect_param, gs_effect_pass, gs_effect_technique,
};
use obs_util::ffi::darray::{bfree, bmalloc, darray};
use proptest::prelude::*;

/// The recorder is one process-wide log.
static RECORDER: Mutex<()> = Mutex::new(());

/// The shim's calls, into the oracle's recorder.
struct Recorder;

impl Calls for Recorder {
    unsafe fn bfree(&mut self, ptr: *mut c_void) {
        // SAFETY: the shim's contract; the recorder frees with bfree.
        unsafe { c::oracle_destroy_bfree(ptr) }
    }

    unsafe fn shader_destroy(&mut self, shader: *mut c_void) {
        // SAFETY: the recorder only records the pointer.
        unsafe { c::oracle_destroy_shader_destroy(shader) }
    }
}

/// Frees for real, without recording; shaders are fake and left.
struct Cleanup;

impl Calls for Cleanup {
    unsafe fn bfree(&mut self, ptr: *mut c_void) {
        // SAFETY: a block of the effect, freed once.
        unsafe { bfree(ptr) }
    }

    unsafe fn shader_destroy(&mut self, _shader: *mut c_void) {}
}

/// A value array: `None` a NULL array (num and capacity 0), else its count
/// and its spare capacity.
type ArraySpec = Option<(usize, usize)>;

#[derive(Debug, Clone)]
struct ParamSpec {
    name: bool,
    cur_val: ArraySpec,
    default_val: ArraySpec,
    /// A NULL annotation array when there are none.
    annotations_null: bool,
    annotations_spare: usize,
    annotations: Vec<ParamSpec>,
}

#[derive(Debug, Clone)]
struct PassSpec {
    name: bool,
    vertshader_params: ArraySpec,
    pixelshader_params: ArraySpec,
    vertshader: bool,
    pixelshader: bool,
}

#[derive(Debug, Clone)]
struct TechniqueSpec {
    name: bool,
    passes_null: bool,
    passes_spare: usize,
    passes: Vec<PassSpec>,
}

#[derive(Debug, Clone)]
struct Spec {
    cached: bool,
    processing: bool,
    looping: bool,
    loop_pass: usize,
    effect_path: bool,
    effect_dir: bool,
    params_null: bool,
    params_spare: usize,
    params: Vec<ParamSpec>,
    techniques_null: bool,
    techniques_spare: usize,
    techniques: Vec<TechniqueSpec>,
}

/// One built effect: its pointer and the name of every block and shader.
struct Built {
    effect: *mut gs_effect,
    names: HashMap<usize, String>,
    blocks: Vec<usize>,
}

struct Builder {
    names: HashMap<usize, String>,
    blocks: Vec<usize>,
    shaders: usize,
}

impl Builder {
    fn alloc(&mut self, size: usize, name: String) -> *mut c_void {
        // SAFETY: a plain allocation, never empty.
        let block = unsafe { bmalloc(size.max(1)) };
        // SAFETY: `block` holds `size.max(1)` bytes.
        unsafe { ptr::write_bytes(block.cast::<u8>(), 0, size.max(1)) };
        self.names.insert(block as usize, name);
        self.blocks.push(block as usize);
        block
    }

    fn string(&mut self, present: bool, name: String) -> *mut c_char {
        if present {
            self.alloc(4, name).cast()
        } else {
            ptr::null_mut()
        }
    }

    fn values(&mut self, spec: ArraySpec, elem: usize, name: String) -> darray {
        match spec {
            None => darray {
                array: ptr::null_mut(),
                num: 0,
                capacity: 0,
            },
            Some((num, spare)) => darray {
                array: self.alloc(elem * (num + spare), name),
                num,
                capacity: num + spare,
            },
        }
    }

    /// An array of `len` items of `T`: NULL when empty and `null`.
    fn items<T>(&mut self, len: usize, spare: usize, null: bool, name: String) -> darray {
        if len == 0 && null {
            return darray {
                array: ptr::null_mut(),
                num: 0,
                capacity: 0,
            };
        }
        let capacity = len + spare;
        darray {
            array: self.alloc(size_of::<T>() * capacity.max(1), name),
            num: len,
            capacity,
        }
    }

    fn shader(&mut self, present: bool, name: String) -> *mut c_void {
        if !present {
            return ptr::null_mut();
        }
        self.shaders += 1;
        let tag = 0x5000 + self.shaders * 16;
        self.names.insert(tag, name);
        tag as *mut c_void
    }

    fn param(&mut self, spec: &ParamSpec, at: *mut gs_effect_param, label: &str) {
        let annotations = self.items::<gs_effect_param>(
            spec.annotations.len(),
            spec.annotations_spare,
            spec.annotations_null,
            format!("{label}.annotations"),
        );
        let annotations_array = annotations.array;
        let param = gs_effect_param {
            name: self.string(spec.name, format!("{label}.name")),
            section: 1,
            type_: 2,
            changed: true,
            cur_val: self.values(spec.cur_val, 1, format!("{label}.cur_val")),
            default_val: self.values(spec.default_val, 1, format!("{label}.default_val")),
            effect: ptr::null_mut(),
            next_sampler: 0x77 as *mut c_void,
            annotations,
        };
        // SAFETY: `at` is a fresh slot of the parent's array.
        unsafe { at.write(param) };
        for (i, annotation) in spec.annotations.iter().enumerate() {
            let slot = (annotations_array as *mut gs_effect_param).wrapping_add(i);
            self.param(annotation, slot, &format!("{label}.a{i}"));
        }
    }

    fn build(spec: &Spec) -> Built {
        let mut b = Self {
            names: HashMap::new(),
            blocks: Vec::new(),
            shaders: 0,
        };
        let effect = b
            .alloc(size_of::<gs_effect>(), "effect".into())
            .cast::<gs_effect>();
        let params = b.items::<gs_effect_param>(
            spec.params.len(),
            spec.params_spare,
            spec.params_null,
            "params".into(),
        );
        let techniques = b.items::<gs_effect_technique>(
            spec.techniques.len(),
            spec.techniques_spare,
            spec.techniques_null,
            "techniques".into(),
        );
        let (params_array, techniques_array) = (params.array, techniques.array);
        let value = gs_effect {
            processing: spec.processing,
            cached: spec.cached,
            effect_path: b.string(spec.effect_path, "effect_path".into()),
            effect_dir: b.string(spec.effect_dir, "effect_dir".into()),
            params,
            techniques,
            cur_technique: 0x11 as *mut gs_effect_technique,
            cur_pass: 0x22 as *mut gs_effect_pass,
            view_proj: 0x33 as *mut gs_effect_param,
            world: 0x44 as *mut gs_effect_param,
            scale: 0x55 as *mut gs_effect_param,
            graphics: 0x66 as *mut c_void,
            next: 0x88 as *mut gs_effect,
            loop_pass: spec.loop_pass,
            looping: spec.looping,
        };
        // SAFETY: `effect` is a fresh block of the struct's size.
        unsafe { effect.write(value) };
        for (i, param) in spec.params.iter().enumerate() {
            let slot = (params_array as *mut gs_effect_param).wrapping_add(i);
            b.param(param, slot, &format!("p{i}"));
        }
        for (i, technique) in spec.techniques.iter().enumerate() {
            let label = format!("t{i}");
            let passes = b.items::<gs_effect_pass>(
                technique.passes.len(),
                technique.passes_spare,
                technique.passes_null,
                format!("{label}.passes"),
            );
            let passes_array = passes.array;
            let slot = (techniques_array as *mut gs_effect_technique).wrapping_add(i);
            let value = gs_effect_technique {
                name: b.string(technique.name, format!("{label}.name")),
                section: 3,
                effect,
                passes,
            };
            // SAFETY: a fresh slot of the technique array.
            unsafe { slot.write(value) };
            for (k, pass) in technique.passes.iter().enumerate() {
                let label = format!("{label}.s{k}");
                let slot = (passes_array as *mut gs_effect_pass).wrapping_add(k);
                let value = gs_effect_pass {
                    name: b.string(pass.name, format!("{label}.name")),
                    section: 4,
                    vertshader: b.shader(pass.vertshader, format!("{label}.vertshader")),
                    pixelshader: b.shader(pass.pixelshader, format!("{label}.pixelshader")),
                    vertshader_params: b.values(
                        pass.vertshader_params,
                        16,
                        format!("{label}.vertshader_params"),
                    ),
                    pixelshader_params: b.values(
                        pass.pixelshader_params,
                        16,
                        format!("{label}.pixelshader_params"),
                    ),
                };
                // SAFETY: a fresh slot of the pass array.
                unsafe { slot.write(value) };
            }
        }
        Built {
            effect,
            names: b.names,
            blocks: b.blocks,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Op {
    Destroy,
    ActuallyDestroy,
    Free,
}

/// The recorded log, each pointer by name.
fn take_log(names: &HashMap<usize, String>) -> Vec<String> {
    // SAFETY: plain reads of the recorder.
    let (count, dropped) = unsafe { (c::oracle_destroy_count(), c::oracle_destroy_dropped()) };
    assert_eq!(dropped, 0, "the recorder's log overflowed");
    (0..count)
        .map(|i| {
            let mut ptr = ptr::null();
            // SAFETY: `i < count`.
            let kind = unsafe { c::oracle_destroy_event(i, &mut ptr) };
            let what = if ptr.is_null() {
                "NULL".to_owned()
            } else {
                names
                    .get(&(ptr as usize))
                    .cloned()
                    .unwrap_or_else(|| "(unknown)".into())
            };
            match kind {
                0x66 => format!("bfree {what}"),
                0x73 => format!("gs_shader_destroy {what}"),
                _ => format!("? {kind} {what}"),
            }
        })
        .collect()
}

/// Every field of the effect after `effect_free`, with the arrays it holds.
fn effect_state(effect: *const gs_effect) -> String {
    // SAFETY: the effect itself is still live after effect_free.
    let e = unsafe { &*effect };
    format!(
        "processing={} cached={} path={:?} dir={:?} params=({:?},{},{}) techniques=({:?},{},{}) \
         cur={:?}/{:?} vp={:?} w={:?} s={:?} g={:?} next={:?} loop={}/{}",
        e.processing,
        e.cached,
        e.effect_path,
        e.effect_dir,
        e.params.array,
        e.params.num,
        e.params.capacity,
        e.techniques.array,
        e.techniques.num,
        e.techniques.capacity,
        e.cur_technique,
        e.cur_pass,
        e.view_proj,
        e.world,
        e.scale,
        e.graphics,
        e.next,
        e.loop_pass,
        e.looping,
    )
}

/// What one side leaves: the log, which blocks the log did not free, and
/// the effect's fields after `effect_free`.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    log: Vec<String>,
    left: Vec<String>,
    state: Option<String>,
}

fn run(spec: &Spec, op: Op, rust: bool) -> Outcome {
    let built = Builder::build(spec);
    // SAFETY: the recorder lock is held by the caller.
    unsafe { c::oracle_destroy_reset() };
    let effect = built.effect;
    // SAFETY: `effect` is a valid effect built above; nothing else uses it.
    unsafe {
        match (op, rust) {
            (Op::Destroy, false) => c::oracle_gs_effect_destroy(effect.cast()),
            (Op::Destroy, true) => r::destroy_with(effect, &mut Recorder),
            (Op::ActuallyDestroy, false) => c::oracle_gs_effect_actually_destroy(effect.cast()),
            (Op::ActuallyDestroy, true) => r::actually_destroy_with(effect, &mut Recorder),
            (Op::Free, false) => c::oracle_effect_free(effect.cast()),
            (Op::Free, true) => r::free_with(effect, &mut Recorder),
        }
    }
    let log = take_log(&built.names);
    let freed: Vec<&str> = log
        .iter()
        .filter_map(|line| line.strip_prefix("bfree "))
        .collect();
    let left: Vec<String> = built
        .blocks
        .iter()
        .map(|block| built.names[block].clone())
        .filter(|name| !freed.contains(&name.as_str()))
        .collect();
    let mut state = None;
    if matches!(op, Op::Free) {
        state = Some(effect_state(effect));
        // SAFETY: effect_free left only the effect itself.
        unsafe { bfree(effect.cast()) };
    } else if !left.is_empty() {
        // A cached effect gs_effect_destroy left: free it all.
        // SAFETY: nothing was freed, so the whole effect is still live.
        unsafe { r::actually_destroy_with(effect, &mut Cleanup) };
    }
    Outcome { log, left, state }
}

fn compare(spec: &Spec, op: Op) -> Result<(), TestCaseError> {
    let _lock = RECORDER.lock().unwrap_or_else(|e| e.into_inner());
    let c_side = run(spec, op, false);
    let rust_side = run(spec, op, true);
    prop_assert_eq!(&rust_side, &c_side, "{:?}", op);
    // A destroy frees every block, or (a cached effect) none.
    let frees_all = !matches!(op, Op::Destroy) || !spec.cached;
    if matches!(op, Op::Free) {
        prop_assert_eq!(&c_side.left, &vec!["effect".to_owned()]);
    } else if frees_all {
        prop_assert!(c_side.left.is_empty(), "left: {:?}", c_side.left);
    } else {
        prop_assert!(c_side.log.is_empty(), "log: {:?}", c_side.log);
    }
    Ok(())
}

fn some(num: usize) -> ArraySpec {
    Some((num, 0))
}

fn plain_param(annotations: Vec<ParamSpec>) -> ParamSpec {
    ParamSpec {
        name: true,
        cur_val: some(16),
        default_val: some(16),
        annotations_null: true,
        annotations_spare: 0,
        annotations,
    }
}

fn plain_pass() -> PassSpec {
    PassSpec {
        name: true,
        vertshader_params: some(1),
        pixelshader_params: some(2),
        vertshader: true,
        pixelshader: true,
    }
}

/// The effect of test_effect_destroy: two techniques, one with two
/// passes; params with and without annotations; no path.
fn c_test_effect(cached: bool) -> Spec {
    Spec {
        cached,
        processing: false,
        looping: false,
        loop_pass: 0,
        effect_path: cached,
        effect_dir: false,
        params_null: false,
        params_spare: 0,
        params: vec![
            plain_param(vec![]),
            plain_param(vec![plain_param(vec![]), plain_param(vec![])]),
            plain_param(vec![plain_param(vec![])]),
            plain_param(vec![]),
        ],
        techniques_null: false,
        techniques_spare: 0,
        techniques: vec![
            TechniqueSpec {
                name: true,
                passes_null: false,
                passes_spare: 0,
                passes: vec![plain_pass(), plain_pass()],
            },
            TechniqueSpec {
                name: true,
                passes_null: false,
                passes_spare: 0,
                passes: vec![plain_pass()],
            },
        ],
    }
}

#[test]
fn c_test_effect_matches() {
    for cached in [false, true] {
        for op in [Op::Destroy, Op::ActuallyDestroy, Op::Free] {
            compare(&c_test_effect(cached), op).unwrap();
        }
    }
}

#[test]
fn null_matches() {
    let _lock = RECORDER.lock().unwrap_or_else(|e| e.into_inner());
    let names = HashMap::new();
    // SAFETY: NULL is a valid argument to both.
    unsafe {
        c::oracle_destroy_reset();
        c::oracle_gs_effect_destroy(ptr::null_mut());
    }
    let c_log = take_log(&names);
    // SAFETY: as above.
    unsafe {
        c::oracle_destroy_reset();
        r::destroy_with(ptr::null_mut(), &mut Recorder);
    }
    assert_eq!(take_log(&names), c_log);
    assert!(c_log.is_empty());
}

fn array_strategy() -> impl Strategy<Value = ArraySpec> {
    prop_oneof![Just(None), (0usize..4, 0usize..3).prop_map(Some)]
}

fn param_strategy() -> impl Strategy<Value = ParamSpec> {
    let leaf = (
        any::<bool>(),
        array_strategy(),
        array_strategy(),
        any::<bool>(),
        0usize..2,
    )
        .prop_map(|(name, cur_val, default_val, null, spare)| ParamSpec {
            name,
            cur_val,
            default_val,
            annotations_null: null,
            annotations_spare: spare,
            annotations: Vec::new(),
        });
    leaf.prop_recursive(2, 16, 3, |inner| {
        (
            any::<bool>(),
            array_strategy(),
            array_strategy(),
            any::<bool>(),
            0usize..2,
            prop::collection::vec(inner, 0..=3),
        )
            .prop_map(
                |(name, cur_val, default_val, null, spare, annotations)| ParamSpec {
                    name,
                    cur_val,
                    default_val,
                    annotations_null: null,
                    annotations_spare: spare,
                    annotations,
                },
            )
    })
}

fn pass_strategy() -> impl Strategy<Value = PassSpec> {
    (
        any::<bool>(),
        array_strategy(),
        array_strategy(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(name, vsp, psp, vertshader, pixelshader)| PassSpec {
            name,
            vertshader_params: vsp,
            pixelshader_params: psp,
            vertshader,
            pixelshader,
        })
}

fn technique_strategy() -> impl Strategy<Value = TechniqueSpec> {
    (
        any::<bool>(),
        any::<bool>(),
        0usize..2,
        prop::collection::vec(pass_strategy(), 0..=3),
    )
        .prop_map(|(name, passes_null, passes_spare, passes)| TechniqueSpec {
            name,
            passes_null,
            passes_spare,
            passes,
        })
}

fn spec_strategy() -> impl Strategy<Value = Spec> {
    (
        (any::<bool>(), any::<bool>(), any::<bool>(), 0usize..3),
        (any::<bool>(), any::<bool>()),
        (
            any::<bool>(),
            0usize..2,
            prop::collection::vec(param_strategy(), 0..=4),
        ),
        (
            any::<bool>(),
            0usize..2,
            prop::collection::vec(technique_strategy(), 0..=3),
        ),
    )
        .prop_map(
            |(
                (cached, processing, looping, loop_pass),
                (effect_path, effect_dir),
                (params_null, params_spare, params),
                (techniques_null, techniques_spare, techniques),
            )| Spec {
                cached,
                processing,
                looping,
                loop_pass,
                effect_path,
                effect_dir,
                params_null,
                params_spare,
                params,
                techniques_null,
                techniques_spare,
                techniques,
            },
        )
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![Just(Op::Destroy), Just(Op::ActuallyDestroy), Just(Op::Free)]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(2000)))]

    #[test]
    fn random_effects_match(spec in spec_strategy(), op in op_strategy()) {
        compare(&spec, op)?;
    }
}
