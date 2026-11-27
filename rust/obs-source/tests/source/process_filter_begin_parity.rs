//! Tier 3: the original C `obs_source_process_filter_begin()` and
//! `obs_source_process_filter_begin_with_color_space()` and the Rust
//! exports, on noise-filled real-header sources
//! (oracle/source_process_filter_begin.c). Both reach the same recording
//! stand-ins for the lookups, the base size, the current color space, the
//! skip, the texrender calls, the blend state, clear and ortho and the two
//! draws of the target; the Rust side reaches them through the production
//! adapters, which read and write the real fields. Compares the result,
//! every call in order with its arguments, the log line, each bypass flag,
//! `allow_direct` and texrender and every other byte of the sources, over a
//! grid of the inputs, plus a proptest; and rejects mutants. The field
//! reads are not traced, so their order is checked by Tier 1 alone. No
//! intentional differences.
use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::NonNull;
use obs_source::ffi::source_process_filter_begin::{
    Libobs, Source, obs_source_process_filter_begin,
    obs_source_process_filter_begin_with_color_space,
};
use obs_source::process_filter_begin::FilterBegin;
use obs_source::render_route::{
    ALLOW_DIRECT_RENDERING, FLAG_ASYNC, FLAG_CUSTOM_DRAW, FLAG_SRGB, FLAG_VIDEO,
    GS_CS_709_EXTENDED, GS_CS_SRGB, NO_DIRECT_RENDERING,
};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

const NODES: usize = 3;
const EVENTS: usize = 24;
const NULL: i64 = 0xfe;
/// `GS_RGBA` and `GS_R8`.
const RGBA: i32 = 3;
const R8: i32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Case {
    filter: i8,
    target: i8,
    parent: i8,
    noise: u8,
    plain: bool,
    creates: bool,
    begins: bool,
    has_name: bool,
    format: i32,
    space: i32,
    allow_direct: i32,
    current: i32,
    texrender_format: i32,
    width: u32,
    height: u32,
    flags: [u32; NODES],
    bypass: [bool; NODES],
    texrender: [bool; NODES],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    result: bool,
    events: u32,
    trace: [[i64; 5]; EVENTS],
    log_count: u32,
    log_level: i32,
    log: [c_char; 128],
    bypass: [bool; NODES],
    allow_direct: [i32; NODES],
    texrender: [i64; NODES],
    untouched: bool,
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            result: false,
            events: 0,
            trace: [[0; 5]; EVENTS],
            log_count: 0,
            log_level: 0,
            log: [0; 128],
            bypass: [false; NODES],
            allow_direct: [0; NODES],
            texrender: [0; NODES],
            untouched: false,
        }
    }
}

fn float(bits: i64) -> f32 {
    f32::from_bits(u32::try_from(bits & 0xffff_ffff).unwrap())
}

impl Outcome {
    fn log(&self) -> &str {
        // SAFETY: the oracle wrote a NUL-terminated string (or left zeros).
        unsafe { CStr::from_ptr(self.log.as_ptr()) }
            .to_str()
            .unwrap()
    }

    /// The calls in order: "T0 P0 W1 H1 N3/0 G3/16/9/0 U J4/5/1/5 K1/1
    /// O16x9 E1 Q Z3" (kind, then its arguments as the oracle recorded
    /// them).
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
                    'C' | 'U' | 'Q' => kind.to_string(),
                    'N' => format!("N{}/{}", e[1], e[2]),
                    'G' => format!("G{}/{}/{}/{}", node(e[1]), e[2], e[3], e[4]),
                    'J' => format!("J{}/{}/{}/{}", e[1], e[2], e[3], e[4]),
                    'K' => format!("K{}/{}/{}/{}", e[1], e[2], float(e[3]), e[4]),
                    'O' => format!(
                        "O{}/{}x{}/{}/{}/{}",
                        float(e[3] >> 32),
                        float(e[1]),
                        float(e[3]),
                        float(e[2]),
                        float(e[4] >> 32),
                        float(e[4])
                    ),
                    _ => format!("{kind}{}", node(e[1])),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

type BeginFn = unsafe extern "C" fn(*mut c_void, c_int, c_int) -> bool;
type BeginSpaceFn = unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int) -> bool;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_process_filter_begin_original() -> BeginFn;
    fn oracle_source_process_filter_begin_with_color_space_original() -> BeginSpaceFn;
    fn oracle_source_process_filter_begin_run(
        begin: BeginFn,
        begin_space: BeginSpaceFn,
        get: GetLog,
        set: SetLog,
        case: *const Case,
        out: *mut Outcome,
    ) -> bool;
}

/// Runs one side; panics when the oracle could not build the fixture or an
/// adapter does not reach its field.
fn run(begin: BeginFn, begin_space: BeginSpaceFn, case: &Case) -> Outcome {
    let mut out = Outcome::default();
    // SAFETY: the isolated child owns the log handler; the case and the
    // outcome are valid for the call.
    let valid = unsafe {
        oracle_source_process_filter_begin_run(
            begin,
            begin_space,
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
fn check_with(begin: BeginFn, begin_space: BeginSpaceFn, case: &Case) -> Option<Outcome> {
    // SAFETY: return the oracle's original functions.
    let original = unsafe {
        run(
            oracle_source_process_filter_begin_original(),
            oracle_source_process_filter_begin_with_color_space_original(),
            case,
        )
    };
    let ported = run(begin, begin_space, case);
    (ported == original).then_some(original)
}

fn check(case: &Case) -> Outcome {
    check_with(
        obs_source_process_filter_begin,
        obs_source_process_filter_begin_with_color_space,
        case,
    )
    .unwrap_or_else(|| panic!("the Rust exports disagree with the original C: {case:?}"))
}

/// Node 0 is a filter attached straight to node 1, a plain 16x9 video
/// source; node 2 is another filter. No texrender yet, one created and
/// begun; RGBA in sRGB, in an sRGB canvas, no direct rendering.
fn base() -> Case {
    Case {
        filter: 0,
        target: 1,
        parent: 1,
        noise: 0xa5,
        plain: false,
        creates: true,
        begins: true,
        has_name: true,
        format: RGBA,
        space: GS_CS_SRGB,
        allow_direct: NO_DIRECT_RENDERING,
        current: GS_CS_SRGB,
        texrender_format: RGBA,
        width: 16,
        height: 9,
        flags: [FLAG_VIDEO; NODES],
        bypass: [false; NODES],
        texrender: [false, true, true],
    }
}

// --- mutants ------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Change {
    NoLog,
    KeepBypass,
    ClearAfterLookup,
    ParentFirst,
    TargetLineForParent,
    FilterFlagsTwice,
    SpaceAlways,
    NoAllowDirect,
    SkipFirst,
    KeepStale,
    AlwaysCreate,
    ResultIsBegun,
    AlwaysDefault,
    UnsignedSize,
    PlainIs709,
}

/// The core's steps with one change, through the real adapters.
fn mutant(
    change: Change,
    filter: *mut c_void,
    format: c_int,
    space: c_int,
    allow_direct: c_int,
) -> bool {
    let mut d = Libobs;
    let Some(filter) = NonNull::new(filter) else {
        if change != Change::NoLog {
            d.log_null_filter();
        }
        return false;
    };
    let filter = Source::from_raw(filter);
    if !matches!(change, Change::KeepBypass | Change::ClearAfterLookup) {
        d.clear_bypass(filter);
    }
    let (target, parent) = if change == Change::ParentFirst {
        let parent = d.parent(filter);
        (d.target(filter), parent)
    } else {
        (d.target(filter), d.parent(filter))
    };
    if change == Change::ClearAfterLookup {
        d.clear_bypass(filter);
    }
    let Some(target) = target else {
        d.log_no_target(filter);
        return false;
    };
    let Some(parent) = parent else {
        if change == Change::TargetLineForParent {
            d.log_no_target(filter);
        } else {
            d.log_no_parent(filter);
        }
        return false;
    };
    let filter_flags = d.output_flags(filter);
    let parent_flags = d.output_flags(if change == Change::FilterFlagsTwice {
        filter
    } else {
        parent
    });
    let width = d.base_width(target);
    let height = d.base_height(target);
    let (cx, cy) = (width.cast_signed(), height.cast_signed());
    if change != Change::NoAllowDirect {
        d.set_allow_direct(filter, allow_direct);
    }
    if change == Change::SkipFirst && (cx == 0 || cy == 0) {
        d.skip_video_filter(filter);
        return false;
    }
    let is_parent = target == parent;
    let reads = is_parent
        && allow_direct == ALLOW_DIRECT_RENDERING
        && parent_flags & (FLAG_CUSTOM_DRAW | FLAG_ASYNC) == 0
        && filter_flags & FLAG_SRGB == parent_flags & FLAG_SRGB;
    let current = if reads || change == Change::SpaceAlways {
        d.color_space()
    } else {
        GS_CS_SRGB
    };
    if reads && space == current {
        d.set_bypass(filter);
        return true;
    }
    if cx == 0 || cy == 0 {
        d.skip_video_filter(filter);
        return false;
    }
    if change != Change::KeepStale
        && d.has_texrender(filter)
        && d.texrender_format(filter) != format
    {
        d.destroy_texrender(filter);
    }
    if change == Change::AlwaysCreate || !d.has_texrender(filter) {
        d.create_texrender(filter, format);
    }
    let begun = d.texrender_begin(filter, width, height, space);
    if begun {
        if change == Change::UnsignedSize {
            // The size as unsigned: the ortho differs past INT_MAX.
            d.draw_setup(
                i32::try_from(width).unwrap_or(i32::MAX),
                i32::try_from(height).unwrap_or(i32::MAX),
            );
        } else {
            d.draw_setup(cx, cy);
        }
        if change == Change::AlwaysDefault
            || (is_parent && parent_flags & (FLAG_CUSTOM_DRAW | FLAG_ASYNC) == 0)
        {
            d.default_render(target);
        } else {
            d.video_render(target);
        }
        d.draw_finish(filter);
    }
    change != Change::ResultIsBegun || begun
}

macro_rules! mutant {
    ($begin:ident, $begin_space:ident, $change:expr) => {
        unsafe extern "C" fn $begin(
            filter: *mut c_void,
            format: c_int,
            allow_direct: c_int,
        ) -> bool {
            let space = if $change == Change::PlainIs709 {
                GS_CS_709_EXTENDED
            } else {
                GS_CS_SRGB
            };
            mutant($change, filter, format, space, allow_direct)
        }
        unsafe extern "C" fn $begin_space(
            filter: *mut c_void,
            format: c_int,
            space: c_int,
            allow_direct: c_int,
        ) -> bool {
            mutant($change, filter, format, space, allow_direct)
        }
    };
}

mutant!(b_no_log, s_no_log, Change::NoLog);
mutant!(b_keep_bypass, s_keep_bypass, Change::KeepBypass);
mutant!(b_clear_late, s_clear_late, Change::ClearAfterLookup);
mutant!(b_parent_first, s_parent_first, Change::ParentFirst);
mutant!(b_target_line, s_target_line, Change::TargetLineForParent);
mutant!(b_filter_flags, s_filter_flags, Change::FilterFlagsTwice);
mutant!(b_space_always, s_space_always, Change::SpaceAlways);
mutant!(b_no_allow, s_no_allow, Change::NoAllowDirect);
mutant!(b_skip_first, s_skip_first, Change::SkipFirst);
mutant!(b_keep_stale, s_keep_stale, Change::KeepStale);
mutant!(b_always_create, s_always_create, Change::AlwaysCreate);
mutant!(b_result_begun, s_result_begun, Change::ResultIsBegun);
mutant!(b_always_default, s_always_default, Change::AlwaysDefault);
mutant!(b_unsigned, s_unsigned, Change::UnsignedSize);
mutant!(b_plain_709, s_plain_709, Change::PlainIs709);

// --- cases --------------------------------------------------------------

fn pinned() {
    // The texture path: target, parent, size, no texrender: one created
    // (RGBA, GS_ZS_NONE), begun 16x9 in sRGB, the blend state pushed, the
    // separate blend function, the clear to transparent, the ortho, the
    // parent's default render, popped, ended.
    let out = check(&base());
    assert_eq!(
        out.calls(),
        "T0 P0 W1 H1 N3/0 G3/16/9/0 U J4/5/1/5 K1/1/0/0 O0/16x0/9/-100/100 E1 Q Z3"
    );
    assert!(out.result);
    assert_eq!(out.texrender, [3, 1, 2]);
    assert_eq!(out.allow_direct[0], NO_DIRECT_RENDERING);
    assert_eq!(out.log_count, 0);
    assert!(out.untouched);

    // A texrender of the format asked for is kept; another is replaced.
    let mut c = base();
    c.texrender[0] = true;
    let out = check(&c);
    assert_eq!(
        out.calls(),
        "T0 P0 W1 H1 F0 G0/16/9/0 U J4/5/1/5 K1/1/0/0 O0/16x0/9/-100/100 E1 Q Z0"
    );
    assert_eq!(out.texrender[0], 0);
    c.texrender_format = R8;
    let out = check(&c);
    assert_eq!(
        out.calls(),
        "T0 P0 W1 H1 F0 D0 N3/0 G3/16/9/0 U J4/5/1/5 K1/1/0/0 O0/16x0/9/-100/100 E1 Q Z3"
    );
    assert_eq!(out.texrender[0], 3);
    // Not created, or not begun: nothing drawn, still true.
    let mut c = base();
    c.creates = false;
    let out = check(&c);
    assert_eq!(out.calls(), "T0 P0 W1 H1 N3/0 G-/16/9/0");
    assert!(out.result);
    assert_eq!(out.texrender[0], NULL);
    let mut c = base();
    c.begins = false;
    assert_eq!(check(&c).calls(), "T0 P0 W1 H1 N3/0 G3/16/9/0");
}

fn pinned_routes() {
    // Another filter below, or a custom-draw parent: its video render.
    let mut c = base();
    c.target = 2;
    assert!(
        check(&c)
            .calls()
            .ends_with("W2 H2 N3/0 G3/16/9/0 U J4/5/1/5 K1/1/0/0 O0/16x0/9/-100/100 V2 Q Z3")
    );
    let mut c = base();
    c.flags[1] = FLAG_VIDEO | FLAG_CUSTOM_DRAW;
    assert!(check(&c).calls().ends_with(" V1 Q Z3"));

    // The bypass: the space read, the flag set, nothing drawn.
    let mut c = base();
    c.allow_direct = ALLOW_DIRECT_RENDERING;
    let out = check(&c);
    assert_eq!(out.calls(), "T0 P0 W1 H1 C");
    assert!(out.result);
    assert!(out.bypass[0]);
    assert_eq!(out.allow_direct[0], ALLOW_DIRECT_RENDERING);
    // Another space than the canvas's: the texture, in that space.
    c.space = GS_CS_709_EXTENDED;
    assert!(
        check(&c)
            .calls()
            .starts_with("T0 P0 W1 H1 C N3/0 G3/16/9/2 ")
    );
    // The plain begin asks for sRGB.
    c.plain = true;
    c.current = GS_CS_709_EXTENDED;
    assert!(
        check(&c)
            .calls()
            .starts_with("T0 P0 W1 H1 C N3/0 G3/16/9/0 ")
    );
}

fn pinned_failures() {
    // Zero size: skipped, false.
    let mut c = base();
    c.width = 0;
    c.texrender[0] = true;
    let out = check(&c);
    assert_eq!(out.calls(), "T0 P0 W1 H1 S0");
    assert!(!out.result);
    assert_eq!(out.texrender[0], 0);

    // NULL: one LOG_DEBUG line, false, from either.
    for plain in [false, true] {
        let mut c = base();
        c.filter = -1;
        c.plain = plain;
        let out = check(&c);
        assert_eq!(
            (out.result, out.events, out.log_count, out.log_level),
            (false, 0, 1, 400)
        );
        assert_eq!(
            out.log(),
            "obs_source_process_filter_begin_with_color_space: Null 'filter' parameter"
        );
    }

    // Removed: the flag cleared, the line with the name, false.
    for (target, parent, line) in [
        (-1, -1, "filter 'filter' being processed with no target!"),
        (-1, 1, "filter 'filter' being processed with no target!"),
        (1, -1, "filter 'filter' being processed with no parent!"),
    ] {
        let mut c = base();
        c.target = target;
        c.parent = parent;
        c.bypass[0] = true;
        let out = check(&c);
        assert_eq!(out.calls(), "T0 P0");
        assert_eq!((out.result, out.log_count, out.log_level), (false, 1, 300));
        assert_eq!(out.log(), line);
        assert_eq!(out.bypass, [false, false, false]);
    }

    // Past INT_MAX the size is negative in the ortho, the same for the
    // texrender.
    let mut c = base();
    c.width = u32::MAX;
    c.height = 0x8000_0000;
    let out = check(&c);
    assert!(out.calls().contains("G3/4294967295/2147483648/0 "));
    assert!(out.calls().contains(" O0/-1x0/-2147483600/-100/100 "));
}

fn grid() -> Vec<Case> {
    let mut cases = Vec::new();
    for filter in [-1i8, 0] {
        for plain in [false, true] {
            for (target, parent) in [(-1i8, -1i8), (-1, 1), (1, -1), (1, 1), (2, 1)] {
                for (filter_flags, parent_flags) in [
                    (FLAG_VIDEO, FLAG_VIDEO),
                    (FLAG_VIDEO | FLAG_SRGB, FLAG_VIDEO | FLAG_SRGB),
                    (FLAG_VIDEO | FLAG_SRGB, FLAG_VIDEO),
                    (FLAG_VIDEO, FLAG_VIDEO | FLAG_CUSTOM_DRAW),
                    (FLAG_VIDEO, FLAG_VIDEO | FLAG_ASYNC),
                    (u32::MAX, 0),
                ] {
                    for allow_direct in [NO_DIRECT_RENDERING, ALLOW_DIRECT_RENDERING, 7] {
                        for (space, current) in [
                            (GS_CS_SRGB, GS_CS_SRGB),
                            (GS_CS_709_EXTENDED, GS_CS_SRGB),
                            (GS_CS_SRGB, GS_CS_709_EXTENDED),
                        ] {
                            for (width, height) in [(16, 9), (0, 9), (16, 0), (u32::MAX, 1)] {
                                for (texrender, texrender_format) in
                                    [(false, RGBA), (true, RGBA), (true, R8)]
                                {
                                    for (creates, begins) in
                                        [(true, true), (false, true), (true, false)]
                                    {
                                        let mut c = base();
                                        c.filter = filter;
                                        c.plain = plain;
                                        c.target = target;
                                        c.parent = parent;
                                        c.flags = [filter_flags, parent_flags, filter_flags];
                                        c.allow_direct = allow_direct;
                                        c.space = space;
                                        c.current = current;
                                        c.width = width;
                                        c.height = height;
                                        c.texrender[0] = texrender;
                                        c.texrender_format = texrender_format;
                                        c.creates = creates;
                                        c.begins = begins;
                                        c.bypass = [true, false, true];
                                        cases.push(c);
                                    }
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
        assert!(out.events <= u32::try_from(EVENTS).unwrap());
    }
    let mutants: [(&str, BeginFn, BeginSpaceFn); 15] = [
        ("no NULL line", b_no_log, s_no_log),
        ("bypass flag kept", b_keep_bypass, s_keep_bypass),
        (
            "bypass cleared after the lookups",
            b_clear_late,
            s_clear_late,
        ),
        ("parent looked up first", b_parent_first, s_parent_first),
        ("no target line for no parent", b_target_line, s_target_line),
        (
            "filter's flags as the parent's",
            b_filter_flags,
            s_filter_flags,
        ),
        ("space always read", b_space_always, s_space_always),
        ("allow_direct not stored", b_no_allow, s_no_allow),
        ("size skip before the bypass", b_skip_first, s_skip_first),
        ("stale texrender kept", b_keep_stale, s_keep_stale),
        ("texrender always created", b_always_create, s_always_create),
        ("false when not begun", b_result_begun, s_result_begun),
        (
            "always the default render",
            b_always_default,
            s_always_default,
        ),
        ("size as unsigned", b_unsigned, s_unsigned),
        ("plain begin in 709 extended", b_plain_709, s_plain_709),
    ];
    for (name, begin, begin_space) in mutants {
        assert!(
            cases
                .iter()
                .any(|c| check_with(begin, begin_space, c).is_none()),
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
                (any::<bool>(), any::<bool>(), any::<bool>()),
                (0i32..12, 0i32..4, 0i32..3, 0i32..4, 0i32..12),
                (
                    prop_oneof![Just(0u32), any::<u32>()],
                    prop_oneof![Just(0u32), any::<u32>()],
                ),
                proptest::array::uniform3(any::<u32>()),
                proptest::array::uniform3(any::<bool>()),
                proptest::array::uniform3(any::<bool>()),
            ),
            |(
                (filter, target, parent, noise, plain),
                (creates, begins, has_name),
                (format, space, allow_direct, current, texrender_format),
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
                    plain,
                    creates,
                    begins,
                    has_name,
                    format,
                    space,
                    allow_direct,
                    current,
                    texrender_format,
                    width,
                    height,
                    flags,
                    bypass,
                    texrender,
                };
                let out = check(&c);
                prop_assert!(out.untouched);
                prop_assert!(out.log_count <= 1);
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn process_filter_begin_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::process_filter_begin_matches_c")) {
        return;
    }
    pinned();
    pinned_routes();
    pinned_failures();
    grid_and_mutants();
    random_cases();
}
