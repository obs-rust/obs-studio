//! Tier 3: the original C `obs_source_process_filter_end()` and
//! `obs_source_process_filter_tech_end()` and the Rust exports, on
//! noise-filled real-header sources
//! (oracle/source_process_filter_end.c). Both reach the same recording
//! stand-ins for the target and parent lookups, `gs_set_linear_srgb()`, the
//! texrender's texture and the two draws; the Rust side reaches them
//! through the production adapters, which read and write the real fields.
//! Compares every call in order with its arguments, the log line, each
//! bypass flag, linear sRGB and every other byte of the sources, over a
//! grid of every input, plus a proptest; and rejects mutants. The field
//! reads are not traced, so their order is checked by Tier 1 alone. No
//! intentional differences.
use core::ffi::{CStr, c_char, c_void};
use core::ptr::NonNull;
use obs_source::ffi::source_process_filter_end::{
    Effect, Libobs, Source, Tech, obs_source_process_filter_end, obs_source_process_filter_tech_end,
};
use obs_source::process_filter_end::FilterEnd;
use obs_source::render_route::{FLAG_SRGB, FLAG_VIDEO};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

const NODES: usize = 3;
const EVENTS: usize = 12;
const NULL: i64 = 0xfe;
/// The technique names the oracle can pass (`spfe_techs`).
const TECHS: u8 = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Case {
    filter: i8,
    target: i8,
    parent: i8,
    noise: u8,
    end: bool,
    effect: u8,
    tech: u8,
    texture: bool,
    linear: bool,
    width: u32,
    height: u32,
    flags: [u32; NODES],
    bypass: [bool; NODES],
    texrender: [bool; NODES],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    events: u32,
    trace: [[i64; 5]; EVENTS],
    log_count: u32,
    log_level: i32,
    log: [c_char; 128],
    bypass: [bool; NODES],
    linear: bool,
    untouched: bool,
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            events: 0,
            trace: [[0; 5]; EVENTS],
            log_count: 0,
            log_level: 0,
            log: [0; 128],
            bypass: [false; NODES],
            linear: false,
            untouched: false,
        }
    }
}

impl Outcome {
    fn log(&self) -> &str {
        // SAFETY: the oracle wrote a NUL-terminated string (or left zeros).
        unsafe { CStr::from_ptr(self.log.as_ptr()) }
            .to_str()
            .unwrap()
    }

    /// The calls in order: "T0 P0 L0 X0 R1/1/1/0x0 L1" (kind, then its
    /// arguments as the oracle recorded them).
    fn calls(&self) -> String {
        self.trace
            .iter()
            .take(usize::try_from(self.events).unwrap())
            .map(|e| {
                let kind = char::from(u8::try_from(e[0]).unwrap());
                let node = |v: i64| {
                    if v == NULL {
                        "-".to_owned()
                    } else {
                        v.to_string()
                    }
                };
                match kind {
                    'B' => format!("B{}/{}/{}", node(e[1]), e[2], e[3]),
                    'R' => format!(
                        "R{}/{}/{}/{}x{}",
                        node(e[1]),
                        e[2],
                        e[3],
                        e[4] >> 32,
                        e[4] & 0xffff_ffff
                    ),
                    _ => format!("{kind}{}", node(e[1])),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

type TechEndFn = unsafe extern "C" fn(*mut c_void, *mut c_void, u32, u32, *const c_char);
type EndFn = unsafe extern "C" fn(*mut c_void, *mut c_void, u32, u32);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_process_filter_tech_end_original() -> TechEndFn;
    fn oracle_source_process_filter_end_original() -> EndFn;
    fn oracle_source_process_filter_end_run(
        tech_end: TechEndFn,
        end: EndFn,
        get: GetLog,
        set: SetLog,
        case: *const Case,
        out: *mut Outcome,
    ) -> bool;
}

/// Runs one side; panics when the oracle could not build the fixture or an
/// adapter does not reach its field.
fn run(tech_end: TechEndFn, end: EndFn, case: &Case) -> Outcome {
    let mut out = Outcome::default();
    // SAFETY: the isolated child owns the log handler; the case and the
    // outcome are valid for the call.
    let valid = unsafe {
        oracle_source_process_filter_end_run(
            tech_end,
            end,
            base_get_log_handler,
            base_set_log_handler,
            case,
            &mut out,
        )
    };
    assert!(valid, "the oracle fixture or an adapter failed: {case:?}");
    out
}

/// The original's outcome, when the pair agrees with it.
fn check_with(tech_end: TechEndFn, end: EndFn, case: &Case) -> Option<Outcome> {
    // SAFETY: return the oracle's original functions.
    let original = unsafe {
        run(
            oracle_source_process_filter_tech_end_original(),
            oracle_source_process_filter_end_original(),
            case,
        )
    };
    let ported = run(tech_end, end, case);
    (ported == original).then_some(original)
}

fn check(case: &Case) -> Outcome {
    check_with(
        obs_source_process_filter_tech_end,
        obs_source_process_filter_end,
        case,
    )
    .unwrap_or_else(|| panic!("the Rust exports disagree with the original C: {case:?}"))
}

/// Node 0 is a filter attached straight to node 1, a plain video source,
/// with a texrender holding a texture; node 2 is another filter. The
/// fixture's effect, "Draw", a 16x9 draw, linear sRGB off.
fn base() -> Case {
    Case {
        filter: 0,
        target: 1,
        parent: 1,
        noise: 0xa5,
        end: false,
        effect: 1,
        tech: 1,
        texture: true,
        linear: false,
        width: 16,
        height: 9,
        flags: [FLAG_VIDEO; NODES],
        bypass: [false; NODES],
        texrender: [true; NODES],
    }
}

// --- mutants ------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Change {
    NoLog,
    LogOnTechEnd,
    KeepBypass,
    ClearAfterCheck,
    ParentFirst,
    TargetOnly,
    ParentFlags,
    NoRestore,
    NullNameKept,
    DrawParent,
    TextureAlways,
}

/// The core's steps with one change, through the real adapters.
fn mutant(
    change: Change,
    logs: bool,
    filter: *mut c_void,
    effect: *mut c_void,
    width: u32,
    height: u32,
    tech_name: *const c_char,
) {
    let mut d = Libobs;
    let effect = Effect::from_raw(effect);
    let Some(filter) = NonNull::new(filter) else {
        if (logs && change != Change::NoLog) || (!logs && change == Change::LogOnTechEnd) {
            d.log_null_filter();
        }
        return;
    };
    let filter = Source::from_raw(filter);
    let mut bypass = false;
    if change != Change::ClearAfterCheck {
        bypass = d.bypass_active(filter);
        if change != Change::KeepBypass {
            d.clear_bypass(filter);
        }
    }
    let (target, parent) = if change == Change::ParentFirst {
        let parent = d.parent(filter);
        (d.target(filter), parent)
    } else {
        (d.target(filter), d.parent(filter))
    };
    let Some(target) = target else { return };
    let parent = match (parent, change) {
        (Some(parent), _) => parent,
        (None, Change::TargetOnly) => target,
        (None, _) => return,
    };
    if change == Change::ClearAfterCheck {
        bypass = d.bypass_active(filter);
        d.clear_bypass(filter);
    }
    let flags = d.output_flags(if change == Change::ParentFlags {
        parent
    } else {
        filter
    });
    let previous = d.set_linear_srgb(flags & FLAG_SRGB != 0);
    let tech = match NonNull::new(tech_name.cast_mut()) {
        Some(name) => Tech::from_raw(name),
        None if change == Change::NullNameKept => Tech::from_raw(NonNull::from(c"").cast()),
        None => d.draw_tech(),
    };
    if bypass && change != Change::TextureAlways {
        let drawn = if change == Change::DrawParent {
            parent
        } else {
            target
        };
        d.render_bypass(drawn, effect, tech);
    } else if let Some(texture) = d.texture(filter) {
        d.render_tex(texture, effect, width, height, tech);
    }
    if change != Change::NoRestore {
        d.set_linear_srgb(previous);
    }
}

macro_rules! mutant {
    ($tech_end:ident, $end:ident, $change:expr) => {
        unsafe extern "C" fn $tech_end(
            filter: *mut c_void,
            effect: *mut c_void,
            width: u32,
            height: u32,
            tech_name: *const c_char,
        ) {
            mutant($change, false, filter, effect, width, height, tech_name);
        }
        unsafe extern "C" fn $end(
            filter: *mut c_void,
            effect: *mut c_void,
            width: u32,
            height: u32,
        ) {
            mutant(
                $change,
                true,
                filter,
                effect,
                width,
                height,
                c"Draw".as_ptr(),
            );
        }
    };
}

mutant!(t_no_log, e_no_log, Change::NoLog);
mutant!(t_log_tech_end, e_log_tech_end, Change::LogOnTechEnd);
mutant!(t_keep_bypass, e_keep_bypass, Change::KeepBypass);
mutant!(t_clear_late, e_clear_late, Change::ClearAfterCheck);
mutant!(t_parent_first, e_parent_first, Change::ParentFirst);
mutant!(t_target_only, e_target_only, Change::TargetOnly);
mutant!(t_parent_flags, e_parent_flags, Change::ParentFlags);
mutant!(t_no_restore, e_no_restore, Change::NoRestore);
mutant!(t_null_name, e_null_name, Change::NullNameKept);
mutant!(t_draw_parent, e_draw_parent, Change::DrawParent);
mutant!(t_texture_always, e_texture_always, Change::TextureAlways);

// --- cases --------------------------------------------------------------

fn pinned() {
    // The texture path: bypass read and cleared (untraced), target, parent,
    // linear sRGB off, the texture of node 0's texrender drawn with the
    // fixture's effect, "Draw" and the size, linear sRGB back.
    let out = check(&base());
    assert_eq!(out.calls(), "T0 P0 L0 X0 R1/1/1/16x9 L0");
    assert_eq!(out.log_count, 0);
    assert!(out.untouched);

    // _end passes "Draw" too, and the same for a NULL name.
    let mut c = base();
    c.end = true;
    c.tech = 2;
    assert_eq!(check(&c).calls(), "T0 P0 L0 X0 R1/1/1/16x9 L0");
    let mut c = base();
    c.tech = 0;
    assert_eq!(check(&c).calls(), "T0 P0 L0 X0 R1/1/1/16x9 L0");
    // A given name is passed through, and a NULL effect.
    let mut c = base();
    c.tech = 2;
    c.effect = 0;
    assert_eq!(check(&c).calls(), "T0 P0 L0 X0 R1/0/2/16x9 L0");

    // The bypass: the target drawn, the flag cleared.
    let mut c = base();
    c.bypass[0] = true;
    let out = check(&c);
    assert_eq!(out.calls(), "T0 P0 L0 B1/1/1 L0");
    assert_eq!(out.bypass, [false; NODES]);
    // Another filter below: that one.
    c.target = 2;
    assert_eq!(check(&c).calls(), "T0 P0 L0 B2/1/1 L0");

    // An sRGB-aware filter turns linear sRGB on, and the caller's value
    // comes back; the parent's flag is not read.
    let mut c = base();
    c.flags[0] = FLAG_VIDEO | FLAG_SRGB;
    c.flags[1] = FLAG_VIDEO;
    let out = check(&c);
    assert_eq!(out.calls(), "T0 P0 L1 X0 R1/1/1/16x9 L0");
    assert!(!out.linear);
    let mut c = base();
    c.linear = true;
    c.flags[1] = FLAG_VIDEO | FLAG_SRGB;
    let out = check(&c);
    assert_eq!(out.calls(), "T0 P0 L0 X0 R1/1/1/16x9 L1");
    assert!(out.linear);

    // No texrender, or one without a texture: nothing drawn.
    let mut c = base();
    c.texrender[0] = false;
    assert_eq!(check(&c).calls(), "T0 P0 L0 X- L0");
    let mut c = base();
    c.texture = false;
    assert_eq!(check(&c).calls(), "T0 P0 L0 X0 L0");

    // NULL: _end logs one LOG_DEBUG line, _tech_end nothing.
    let mut c = base();
    c.filter = -1;
    c.end = true;
    let out = check(&c);
    assert_eq!((out.events, out.log_count, out.log_level), (0, 1, 400));
    assert_eq!(
        out.log(),
        "obs_source_process_filter_end: Null 'filter' parameter"
    );
    c.end = false;
    let out = check(&c);
    assert_eq!((out.events, out.log_count), (0, 0));

    // Removed: the flag is cleared, both looked up, nothing more.
    for (target, parent) in [(-1, -1), (-1, 1), (1, -1)] {
        let mut c = base();
        c.target = target;
        c.parent = parent;
        c.bypass[0] = true;
        c.linear = true;
        let out = check(&c);
        assert_eq!(out.calls(), "T0 P0");
        assert_eq!(out.bypass, [false; NODES]);
        assert!(out.linear);
    }
}

fn grid() -> Vec<Case> {
    let mut cases = Vec::new();
    for filter in [-1i8, 0] {
        for end in [false, true] {
            for (target, parent) in [(-1i8, -1i8), (-1, 1), (1, -1), (1, 1), (2, 1)] {
                for flags in [0, FLAG_VIDEO, FLAG_SRGB, !FLAG_SRGB, u32::MAX] {
                    for bypass in [false, true] {
                        for (texrender, texture) in [(false, false), (true, false), (true, true)] {
                            for tech in 0..TECHS {
                                for linear in [false, true] {
                                    let mut c = base();
                                    c.filter = filter;
                                    c.end = end;
                                    c.target = target;
                                    c.parent = parent;
                                    c.flags = [flags, !flags, flags];
                                    c.bypass = [bypass, !bypass, bypass];
                                    c.texrender = [texrender, true, !texrender];
                                    c.texture = texture;
                                    c.tech = tech;
                                    c.linear = linear;
                                    cases.push(c);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    cases
}

fn grid_and_mutants() {
    let cases = grid();
    for c in &cases {
        let out = check(c);
        assert!(out.untouched);
        assert!(out.log_count <= 1);
        assert_eq!(out.linear, c.linear);
    }
    let mutants: [(&str, TechEndFn, EndFn); 11] = [
        ("no NULL line", t_no_log, e_no_log),
        ("NULL line from _tech_end", t_log_tech_end, e_log_tech_end),
        ("bypass flag kept", t_keep_bypass, e_keep_bypass),
        ("bypass cleared after the check", t_clear_late, e_clear_late),
        ("parent looked up first", t_parent_first, e_parent_first),
        ("no parent check", t_target_only, e_target_only),
        ("parent's flags", t_parent_flags, e_parent_flags),
        ("linear sRGB not restored", t_no_restore, e_no_restore),
        ("NULL name not Draw", t_null_name, e_null_name),
        ("bypass draws the parent", t_draw_parent, e_draw_parent),
        ("texture always", t_texture_always, e_texture_always),
    ];
    for (name, tech_end, end) in mutants {
        assert!(
            cases.iter().any(|c| check_with(tech_end, end, c).is_none()),
            "the mutant '{name}' agreed with the original C"
        );
    }
}

fn random_cases() {
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(512),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                (-1i8..1, -1i8..3, -1i8..3, any::<u8>(), any::<bool>()),
                (0u8..2, 0u8..TECHS, any::<bool>(), any::<bool>()),
                (any::<u32>(), any::<u32>()),
                proptest::array::uniform3(any::<u32>()),
                proptest::array::uniform3(any::<bool>()),
                proptest::array::uniform3(any::<bool>()),
            ),
            |(
                (filter, target, parent, noise, end),
                (effect, tech, texture, linear),
                (width, height),
                flags,
                bypass,
                texrender,
            )| {
                let c = Case {
                    filter,
                    target,
                    parent,
                    noise,
                    end,
                    effect,
                    tech,
                    texture,
                    linear,
                    width,
                    height,
                    flags,
                    bypass,
                    texrender,
                };
                let out = check(&c);
                prop_assert!(out.untouched);
                prop_assert!(out.log_count <= 1);
                prop_assert_eq!(out.linear, linear);
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn process_filter_end_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::process_filter_end_matches_c")) {
        return;
    }
    pinned();
    grid_and_mutants();
    random_cases();
}
