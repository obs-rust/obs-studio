//! Tier 3: the `obs_canvas_rules_*` shims against the original C
//! (`obs-c-oracle`, `oracle/canvas_rules.c`: the unmodified
//! `libobs/obs-canvas-rules.c.h` with its functions renamed), and the safe
//! core against both.
//!
//! Every function takes plain values (the name check a C string) and
//! returns a value, so a run compares the bytes of the results: the C's
//! `bool` as the byte it returns, the flags as the `uint32_t`.
//!
//! Runs:
//! - the header's copies (flags, `REMOVED`, the public mask,
//!   `VIDEO_FORMAT_NONE`, `MAX_CHANNELS`), `enum obs_canvas_flags`,
//!   `VIDEO_FORMAT_NONE` and `MAX_CHANNELS` from the C headers and the type
//!   sizes, against the Rust constants;
//! - exhaustive: every flag rule for every low 16-bit flags word under 16
//!   high words (every combination of bits 16, 17, 30 and 31), and for every
//!   single bit and every combination of the known flags, `REMOVED` and an
//!   unknown bit; the load for every flags word above cast from 64 bits with
//!   0, 1, -1 and `i32::MIN` in the high word; the size and channel for
//!   0 to 70,000, every 65,521st `u32` and the ends; the format for -70,000
//!   to 70,000 and the ends; the name for NULL and every first byte;
//! - the decisions of each caller in their order (the constructors, the
//!   save, the reset, the set_channel, the set_name, the remove), one copy
//!   on each side, for every combination of the known flags, `REMOVED`, an
//!   unknown bit, the private flag and the other inputs;
//! - proptests (20,000 cases each): every decision over any flags, any
//!   saved `i64`, any size, format, channel and first byte, and the
//!   callers over all of it at once.
//!
//! No input reaches a division or a 128-bit multiply on either side, so no
//! input is skipped on MSVC x64. No intentional differences.

use std::ffi::{CStr, c_char};
use std::os::raw::c_int;

use obs_c_oracle::canvas_rules as oracle;
use obs_core::canvas_rules::{
    self as core_rules, ACTIVATE, DEVICE, EPHEMERAL, FORMAT_NONE, MAIN, MAX_CHANNELS, MIX_AUDIO,
    PREVIEW, PROGRAM, PUBLIC, REMOVED, SCENE_REF,
};
use obs_core::ffi::canvas_rules as shim;
use proptest::prelude::*;

const UNKNOWN: u32 = 1 << 5;
const BITS: [u32; 7] = [
    MAIN, ACTIVATE, MIX_AUDIO, SCENE_REF, EPHEMERAL, REMOVED, UNKNOWN,
];

fn combos() -> impl Iterator<Item = u32> {
    (0..(1_u32 << BITS.len())).map(|n| {
        BITS.iter()
            .enumerate()
            .filter(|(i, _)| n & (1 << i) != 0)
            .fold(0, |flags, (_, bit)| flags | bit)
    })
}

#[test]
fn values_match_the_c_headers() {
    let rust: [i64; 23] = [
        MAIN.into(),
        ACTIVATE.into(),
        MIX_AUDIO.into(),
        SCENE_REF.into(),
        EPHEMERAL.into(),
        REMOVED.into(),
        PUBLIC.into(),
        FORMAT_NONE.into(),
        MAX_CHANNELS.into(),
        MAIN.into(),
        ACTIVATE.into(),
        MIX_AUDIO.into(),
        SCENE_REF.into(),
        EPHEMERAL.into(),
        PROGRAM.into(),
        PREVIEW.into(),
        DEVICE.into(),
        FORMAT_NONE.into(),
        MAX_CHANNELS.into(),
        4,
        4,
        1,
        4,
    ];
    assert_eq!(oracle::VALUE_NAMES.len(), rust.len());
    for (which, (name, value)) in oracle::VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_canvas_rules_value(which) };
        assert_eq!(value, c, "{name}");
    }
    // SAFETY: a pure lookup, one past the end.
    let past = unsafe { oracle::oracle_canvas_rules_value(oracle::VALUE_NAMES.len()) };
    assert_eq!(past, i64::MIN);
    // SAFETY: no arguments.
    let c_main = unsafe { oracle::oracle_obs_canvas_rules_main_flags() };
    assert_eq!(shim::obs_canvas_rules_main_flags(), c_main);
    assert_eq!(core_rules::main_flags(), c_main);
}

/// Every rule that takes the flags: shim, C and core agree.
fn check_flags(f: u32) {
    type Pair = (
        extern "C" fn(u32) -> bool,
        unsafe extern "C" fn(u32) -> u8,
        fn(u32) -> bool,
    );
    let rules: [(&str, Pair); 9] = [
        (
            "main_view",
            (
                shim::obs_canvas_rules_main_view,
                oracle::oracle_obs_canvas_rules_main_view,
                core_rules::main_view,
            ),
        ),
        (
            "mix_audio",
            (
                shim::obs_canvas_rules_mix_audio,
                oracle::oracle_obs_canvas_rules_mix_audio,
                core_rules::mix_audio,
            ),
        ),
        (
            "holds_scenes",
            (
                shim::obs_canvas_rules_holds_scenes,
                oracle::oracle_obs_canvas_rules_holds_scenes,
                core_rules::holds_scenes,
            ),
        ),
        (
            "saves",
            (
                shim::obs_canvas_rules_saves,
                oracle::oracle_obs_canvas_rules_saves,
                core_rules::saves,
            ),
        ),
        (
            "resets",
            (
                shim::obs_canvas_rules_resets,
                oracle::oracle_obs_canvas_rules_resets,
                core_rules::resets,
            ),
        ),
        (
            "global_signals",
            (
                shim::obs_canvas_rules_global_signals,
                oracle::oracle_obs_canvas_rules_global_signals,
                core_rules::global_signals,
            ),
        ),
        (
            "renames",
            (
                shim::obs_canvas_rules_renames,
                oracle::oracle_obs_canvas_rules_renames,
                core_rules::renames,
            ),
        ),
        (
            "removable",
            (
                shim::obs_canvas_rules_removable,
                oracle::oracle_obs_canvas_rules_removable,
                core_rules::removable,
            ),
        ),
        (
            "removed",
            (
                shim::obs_canvas_rules_removed,
                oracle::oracle_obs_canvas_rules_removed,
                core_rules::removed,
            ),
        ),
    ];
    for (name, (rs, c, safe)) in rules {
        // SAFETY: a pure function of a value.
        let want = unsafe { c(f) };
        assert_eq!(u8::from(rs(f)), want, "shim {name}({f:#x})");
        assert_eq!(u8::from(safe(f)), want, "core {name}({f:#x})");
    }
    // SAFETY: pure functions of a value.
    let (public, removed) = unsafe {
        (
            oracle::oracle_obs_canvas_rules_public_flags(f),
            oracle::oracle_obs_canvas_rules_mark_removed(f),
        )
    };
    assert_eq!(
        shim::obs_canvas_rules_public_flags(f),
        public,
        "public_flags({f:#x})"
    );
    assert_eq!(
        core_rules::public_flags(f),
        public,
        "core public_flags({f:#x})"
    );
    assert_eq!(
        shim::obs_canvas_rules_mark_removed(f),
        removed,
        "mark_removed({f:#x})"
    );
    assert_eq!(
        core_rules::mark_removed(f),
        removed,
        "core mark_removed({f:#x})"
    );
}

/// `obs_load_canvas()`'s `(uint32_t)` cast of the saved value, then the
/// mask: the cast is C's, so both sides get the same truncated word.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn check_load(saved: i64) {
    let word = saved as u32;
    // SAFETY: a pure function of a value.
    let c = unsafe { oracle::oracle_obs_canvas_rules_public_flags(word) };
    assert_eq!(
        shim::obs_canvas_rules_public_flags(word),
        c,
        "load {saved:#x}"
    );
    assert_eq!(core_rules::public_flags(word), c, "core load {saved:#x}");
}

fn check_size(size: u32) {
    // SAFETY: a pure function of a value.
    let c = unsafe { oracle::oracle_obs_canvas_rules_size_set(size) };
    assert_eq!(
        u8::from(shim::obs_canvas_rules_size_set(size)),
        c,
        "size_set({size})"
    );
    assert_eq!(
        u8::from(core_rules::size_set(size)),
        c,
        "core size_set({size})"
    );
}

fn check_channel(channel: u32) {
    // SAFETY: a pure function of a value.
    let c = unsafe { oracle::oracle_obs_canvas_rules_channel_ok(channel) };
    assert_eq!(
        u8::from(shim::obs_canvas_rules_channel_ok(channel)),
        c,
        "channel_ok({channel})"
    );
    assert_eq!(
        u8::from(core_rules::channel_ok(channel)),
        c,
        "core channel_ok({channel})"
    );
}

fn check_format(format: c_int) {
    // SAFETY: a pure function of a value.
    let c = unsafe { oracle::oracle_obs_canvas_rules_format_set(format) };
    assert_eq!(
        u8::from(shim::obs_canvas_rules_format_set(format)),
        c,
        "format_set({format})"
    );
    assert_eq!(
        u8::from(core_rules::format_set(format)),
        c,
        "core format_set({format})"
    );
}

fn check_private(private: bool) {
    // SAFETY: a pure function of a value.
    let c = unsafe { oracle::oracle_obs_canvas_rules_public(private) };
    assert_eq!(u8::from(shim::obs_canvas_rules_public(private)), c);
    assert_eq!(u8::from(core_rules::is_public(private)), c);
}

/// The name check through a C string: NULL, or `[first, b'x', 0]` (so a
/// first byte of 0 is an empty name with more bytes after it).
fn check_name(first: Option<u8>) {
    let buf = first.map(|b| [b, b'x', 0]);
    let ptr: *const c_char = buf.as_ref().map_or(std::ptr::null(), |b| b.as_ptr().cast());
    // SAFETY: NULL or a NUL-terminated buffer that outlives the calls.
    let (c, rs) = unsafe {
        (
            oracle::oracle_obs_canvas_rules_name_empty(ptr),
            shim::obs_canvas_rules_name_empty(ptr),
        )
    };
    assert_eq!(u8::from(rs), c, "name_empty({first:?})");
    assert_eq!(
        u8::from(core_rules::name_empty(first)),
        c,
        "core name_empty({first:?})"
    );
    if let Some(b) = &buf {
        // the name is still a C string
        // SAFETY: NUL-terminated.
        assert!(
            unsafe { CStr::from_ptr(b.as_ptr().cast()) }
                .to_bytes()
                .len()
                <= 2
        );
    }
}

/// The decisions of the callers in their order, as `obs-canvas.c` asks
/// them, on either side. `rules` picks the side: the C oracle or the shims.
#[derive(Debug, PartialEq, Eq)]
struct Plan {
    created: [u32; 3],
    view_main: [bool; 3],
    mix_audio: [bool; 3],
    public: bool,
    saved: bool,
    resets: bool,
    channel: (bool, bool),
    name: (bool, bool, bool),
    removed: (bool, u32, bool, bool),
    valid: bool,
}

struct Side {
    main_flags: fn() -> u32,
    public_flags: fn(u32) -> u32,
    flag: fn(&str, u32) -> bool,
    public: fn(bool) -> bool,
    size_set: fn(u32) -> bool,
    format_set: fn(c_int) -> bool,
    channel_ok: fn(u32) -> bool,
    name_empty: fn(Option<u8>) -> bool,
    mark_removed: fn(u32) -> u32,
}

fn c_flag(name: &str, f: u32) -> bool {
    // SAFETY: pure functions of a value.
    let byte = unsafe {
        match name {
            "main_view" => oracle::oracle_obs_canvas_rules_main_view(f),
            "mix_audio" => oracle::oracle_obs_canvas_rules_mix_audio(f),
            "saves" => oracle::oracle_obs_canvas_rules_saves(f),
            "resets" => oracle::oracle_obs_canvas_rules_resets(f),
            "global_signals" => oracle::oracle_obs_canvas_rules_global_signals(f),
            "renames" => oracle::oracle_obs_canvas_rules_renames(f),
            "removable" => oracle::oracle_obs_canvas_rules_removable(f),
            "removed" => oracle::oracle_obs_canvas_rules_removed(f),
            _ => panic!("{name}"),
        }
    };
    byte != 0
}

fn rs_flag(name: &str, f: u32) -> bool {
    match name {
        "main_view" => shim::obs_canvas_rules_main_view(f),
        "mix_audio" => shim::obs_canvas_rules_mix_audio(f),
        "saves" => shim::obs_canvas_rules_saves(f),
        "resets" => shim::obs_canvas_rules_resets(f),
        "global_signals" => shim::obs_canvas_rules_global_signals(f),
        "renames" => shim::obs_canvas_rules_renames(f),
        "removable" => shim::obs_canvas_rules_removable(f),
        "removed" => shim::obs_canvas_rules_removed(f),
        _ => panic!("{name}"),
    }
}

fn name_ptr_call(first: Option<u8>, c: bool) -> bool {
    let buf = first.map(|b| [b, 0]);
    let ptr: *const c_char = buf.as_ref().map_or(std::ptr::null(), |b| b.as_ptr().cast());
    // SAFETY: NULL or a NUL-terminated buffer that outlives the call.
    unsafe {
        if c {
            oracle::oracle_obs_canvas_rules_name_empty(ptr) != 0
        } else {
            shim::obs_canvas_rules_name_empty(ptr)
        }
    }
}

fn c_main_flags() -> u32 {
    // SAFETY: a pure function.
    unsafe { oracle::oracle_obs_canvas_rules_main_flags() }
}

fn c_public_flags(f: u32) -> u32 {
    // SAFETY: a pure function of a value.
    unsafe { oracle::oracle_obs_canvas_rules_public_flags(f) }
}

fn c_public(private: bool) -> bool {
    // SAFETY: a pure function of a value.
    unsafe { oracle::oracle_obs_canvas_rules_public(private) != 0 }
}

fn c_size_set(size: u32) -> bool {
    // SAFETY: a pure function of a value.
    unsafe { oracle::oracle_obs_canvas_rules_size_set(size) != 0 }
}

fn c_format_set(format: c_int) -> bool {
    // SAFETY: a pure function of a value.
    unsafe { oracle::oracle_obs_canvas_rules_format_set(format) != 0 }
}

fn c_channel_ok(channel: u32) -> bool {
    // SAFETY: a pure function of a value.
    unsafe { oracle::oracle_obs_canvas_rules_channel_ok(channel) != 0 }
}

fn c_mark_removed(f: u32) -> u32 {
    // SAFETY: a pure function of a value.
    unsafe { oracle::oracle_obs_canvas_rules_mark_removed(f) }
}

const C_SIDE: Side = Side {
    main_flags: c_main_flags,
    public_flags: c_public_flags,
    flag: c_flag,
    public: c_public,
    size_set: c_size_set,
    format_set: c_format_set,
    channel_ok: c_channel_ok,
    name_empty: |b| name_ptr_call(b, true),
    mark_removed: c_mark_removed,
};

const RS_SIDE: Side = Side {
    main_flags: || shim::obs_canvas_rules_main_flags(),
    public_flags: |f| shim::obs_canvas_rules_public_flags(f),
    flag: rs_flag,
    public: |p| shim::obs_canvas_rules_public(p),
    size_set: |s| shim::obs_canvas_rules_size_set(s),
    format_set: |f| shim::obs_canvas_rules_format_set(f),
    channel_ok: |c| shim::obs_canvas_rules_channel_ok(c),
    name_empty: |b| name_ptr_call(b, false),
    mark_removed: |f| shim::obs_canvas_rules_mark_removed(f),
};

#[derive(Clone, Copy, Debug)]
struct Input {
    flags: u32,
    private: bool,
    channel: u32,
    name: Option<u8>,
    sizes: [u32; 4],
    format: c_int,
}

fn plan(side: &Side, i: Input) -> Plan {
    let f = i.flags;
    // the constructors: main, create (and create_private) and load
    let created = [
        (side.main_flags)(),
        (side.public_flags)(f),
        (side.public_flags)(f),
    ];
    let view_main = created.map(|c| (side.flag)("main_view", c));
    let mix_audio = created.map(|c| (side.flag)("mix_audio", c));
    let public = (side.public)(i.private);
    let saved = (side.flag)("saves", f);
    let resets = (side.flag)("resets", f);
    let channel = if (side.channel_ok)(i.channel) {
        (true, (side.flag)("global_signals", f))
    } else {
        (false, false)
    };
    let name = if (side.name_empty)(i.name) {
        (true, false, false)
    } else if !(side.flag)("renames", f) {
        (false, true, false)
    } else {
        (false, false, !(side.public)(i.private))
    };
    let removed = if (side.flag)("removable", f) {
        let n = (side.mark_removed)(f);
        (
            true,
            n,
            (side.flag)("removed", n),
            (side.flag)("removable", n),
        )
    } else {
        (false, f, (side.flag)("removed", f), false)
    };
    let valid = i.sizes.iter().all(|&s| (side.size_set)(s)) && (side.format_set)(i.format);
    Plan {
        created,
        view_main,
        mix_audio,
        public,
        saved,
        resets,
        channel,
        name,
        removed,
        valid,
    }
}

fn check_callers(i: Input) {
    assert_eq!(plan(&RS_SIDE, i), plan(&C_SIDE, i), "{i:?}");
}

#[test]
fn every_flags_word() {
    for high in 0..16_u32 {
        let high_bits = [16, 17, 30, 31]
            .iter()
            .enumerate()
            .filter(|(i, _)| high & (1 << i) != 0)
            .fold(0_u32, |acc, (_, bit)| acc | (1 << bit));
        for low in 0..=0xffff_u32 {
            check_flags(high_bits | low);
        }
    }
    for bit in 0..32 {
        check_flags(1 << bit);
    }
    combos().for_each(check_flags);
    check_flags(u32::MAX);
}

#[test]
fn every_load() {
    for high in [0_i64, 1, -1, i64::from(i32::MIN)] {
        for low in 0..=0xffff_i64 {
            for top in [0_i64, 0x8000, 0xffff] {
                check_load((high << 32) | (top << 16) | low);
            }
        }
    }
    for saved in [i64::MIN, i64::MAX, -1, -2, -32, 0x1_0000_001e] {
        check_load(saved);
    }
}

#[test]
fn every_value() {
    for v in 0..=70_000_u32 {
        check_size(v);
        check_channel(v);
    }
    for v in (0..=u32::MAX).step_by(65_521) {
        check_size(v);
        check_channel(v);
    }
    for v in [u32::MAX, u32::MAX - 1, 0x8000_0000, 63, 64, 65] {
        check_size(v);
        check_channel(v);
    }
    for f in -70_000..=70_000 {
        check_format(f);
    }
    for f in [c_int::MIN, c_int::MIN + 1, c_int::MAX, c_int::MAX - 1] {
        check_format(f);
    }
    check_private(false);
    check_private(true);
    check_name(None);
    for b in 0..=u8::MAX {
        check_name(Some(b));
    }
}

#[test]
fn callers_every_input() {
    let channels = [0, 1, 63, 64, 65, u32::MAX];
    let names = [None, Some(0), Some(b'a'), Some(0xff)];
    let sizes = [
        [1920, 1080, 1280, 720],
        [0, 1080, 1280, 720],
        [1920, 1080, 1280, 0],
    ];
    let formats = [0, 1, 2, -1, c_int::MAX];
    for flags in combos() {
        for private in [false, true] {
            for &channel in &channels {
                for &name in &names {
                    for &s in &sizes {
                        for &format in &formats {
                            check_callers(Input {
                                flags,
                                private,
                                channel,
                                name,
                                sizes: s,
                                format,
                            });
                        }
                    }
                }
            }
        }
    }
}

fn flags() -> impl Strategy<Value = u32> {
    prop_oneof![
        2 => any::<u32>(),
        1 => proptest::sample::select(combos().collect::<Vec<_>>()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn proptest_decisions(
        f in flags(),
        saved in any::<i64>(),
        size in any::<u32>(),
        format in any::<c_int>(),
        channel in any::<u32>(),
        first in any::<Option<u8>>(),
        private: bool,
    ) {
        check_flags(f);
        check_load(saved);
        check_size(size);
        check_format(format);
        check_channel(channel);
        check_name(first);
        check_private(private);
    }

    #[test]
    fn proptest_callers(
        f in flags(),
        private: bool,
        channel in prop_oneof![0..70_u32, any::<u32>()],
        name in any::<Option<u8>>(),
        sizes in any::<[u32; 4]>(),
        format in prop_oneof![-2..3_i32, any::<i32>()],
    ) {
        check_callers(Input { flags: f, private, channel, name, sizes, format });
    }
}
