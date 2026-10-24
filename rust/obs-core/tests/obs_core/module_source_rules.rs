//! Tier 1: the safe core of `libobs/obs-module-source-rules.c.h`.
//!
//! The first ten tests mirror the cases of
//! `test/cmocka/test_source_registration.c` one to one, in the same order,
//! with the same ids, sizes and expected messages. [`Registry::register`]
//! stands for `obs_register_source_s()`: it picks the list with
//! [`list_of`], looks for a duplicate, checks the size with
//! [`size_supported`], copies the fields wholly inside the size,
//! normalizes the flags with [`output_flags`], finds the callbacks inside
//! the size (`HAS_VAL()`), asks [`check`], and names the type with
//! [`versioned_id`], logging what the C logs. The field offsets are those of
//! `struct obs_source_info` on the 64-bit targets libobs builds for.
//!
//! Cases marked "characterized, not endorsed" are odd but real (#1876) and
//! kept as the C has them. The tests after those pin the edges of each
//! function, and proptests cover any type, flags, size, callbacks and
//! version.

use obs_core::module_source_rules::{
    ASYNC, AUDIO, Broken, COMPOSITE, CUSTOM_DRAW, Callbacks, HAS_AUDIO_RENDER, HAS_GET_HEIGHT,
    HAS_GET_NAME, HAS_GET_WIDTH, List, NULL_ID, TYPE_FILTER, TYPE_INPUT, TYPE_SCENE,
    TYPE_TRANSITION, VIDEO, check, list_of, output_flags, size_supported, versioned_id,
};
use proptest::prelude::*;

const DEPRECATED: u32 = 1 << 8;
const ASYNC_VIDEO: u32 = ASYNC | VIDEO;

// `offsetof(struct obs_source_info, ...)` on 64-bit targets.
const OFF_ID: usize = 0;
const OFF_TYPE: usize = 8;
const OFF_OUTPUT_FLAGS: usize = 12;
const OFF_GET_NAME: usize = 16;
const OFF_GET_WIDTH: usize = 40;
const OFF_GET_HEIGHT: usize = 48;
const OFF_TYPE_DATA: usize = 216;
const OFF_FREE_TYPE_DATA: usize = 224;
const OFF_AUDIO_RENDER: usize = 232;
const OFF_VERSION: usize = 368;
/// `sizeof(struct obs_source_info)`.
const SIZE: usize = 424;
const PTR: usize = 8;

const W: Level = Level::Warning;
const E: Level = Level::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Warning,
    Error,
}

/// The fields of `struct obs_source_info` the registration reads; a
/// callback is set or NULL.
#[derive(Debug, Clone)]
struct Info {
    id: &'static str,
    source_type: u32,
    output_flags: u32,
    get_name: bool,
    get_width: bool,
    get_height: bool,
    audio_render: bool,
    type_data: bool,
    free_type_data: bool,
    version: u32,
}

/// `full_info()` of the C test.
fn full_info(id: &'static str, source_type: u32, output_flags: u32) -> Info {
    Info {
        id,
        source_type,
        output_flags,
        get_name: true,
        get_width: true,
        get_height: true,
        audio_render: true,
        type_data: true,
        free_type_data: true,
        version: 0,
    }
}

/// A registered type.
#[derive(Debug, Clone)]
struct Entry {
    id: Vec<u8>,
    unversioned_id: Vec<u8>,
    version: u32,
    list: List,
    flags: u32,
    type_data: bool,
}

#[derive(Default)]
struct Registry {
    types: Vec<Entry>,
    logged: Vec<(Level, String)>,
    frees: usize,
}

/// A field of `len` bytes at `offset` is whole inside `size`.
fn inside(offset: usize, len: usize, size: usize) -> bool {
    offset + len <= size
}

/// A `u32` at `offset` as the copy keeps it: whole inside the size, or
/// zero. The copy ends at the last field wholly within the size, so a field
/// the size cuts is not partly copied (#2203).
fn copied_u32(value: u32, offset: usize, size: usize) -> u32 {
    if inside(offset, 4, size) { value } else { 0 }
}

impl Registry {
    fn log(&mut self, level: Level, msg: String) {
        self.logged.push((level, msg));
    }

    fn find(&self, id: &str) -> Option<&Entry> {
        self.types.iter().find(|e| e.id == id.as_bytes())
    }

    fn count(&self, list: Option<List>) -> usize {
        self.types
            .iter()
            .filter(|e| list.is_none_or(|l| e.list == l))
            .count()
    }

    /// `obs_register_source_s(info, size)`.
    fn register(&mut self, info: &Info, size: usize) {
        self.logged.clear();
        self.frees = 0;
        if self.try_register(info, size).is_err() {
            // HANDLE_ERROR: nothing for size 0; else the type data is freed
            // when the copied part of the structure holds both.
            if size != 0
                && info.type_data
                && inside(OFF_TYPE_DATA, PTR, size)
                && info.free_type_data
                && inside(OFF_FREE_TYPE_DATA, PTR, size)
            {
                self.frees += 1;
            }
        }
    }

    fn try_register(&mut self, info: &Info, size: usize) -> Result<(), ()> {
        let id = info.id;
        let list = list_of(info.source_type);
        if list == List::Unknown {
            let t = info.source_type;
            self.log(
                W,
                format!("obs_register_source: Tried to register unknown source type: {t}"),
            );
            return Err(());
        }
        if self
            .types
            .iter()
            .any(|e| e.unversioned_id == id.as_bytes() && e.version == info.version)
        {
            self.log(
                W,
                format!("obs_register_source: Source '{id}' already exists!  Duplicate library?"),
            );
            return Err(());
        }
        if !size_supported(size, SIZE) {
            self.log(
                W,
                format!(
                    "obs_register_source: Tried to register obs_source_info with size {size} \
                     which is more than libobs currently supports ({SIZE})"
                ),
            );
            return Err(());
        }

        // memcpy(&data, info, size), cut down to the last whole field
        let data_type = copied_u32(info.source_type, OFF_TYPE, size);
        let data_flags = copied_u32(info.output_flags, OFF_OUTPUT_FLAGS, size);
        let data_version = copied_u32(info.version, OFF_VERSION, size);
        let data_type_data = info.type_data && inside(OFF_TYPE_DATA, PTR, size);
        // The transition warnings print the copy's id ("%s" of NULL when
        // the size cuts it off).
        let data_id = if inside(OFF_ID, PTR, size) {
            id
        } else {
            "(null)"
        };

        if data_type == TYPE_TRANSITION {
            if info.get_width && inside(OFF_GET_WIDTH, PTR, size) {
                self.log(
                    W,
                    format!(
                        "obs_register_source: get_width ignored registering transition '{data_id}'"
                    ),
                );
            }
            if info.get_height && inside(OFF_GET_HEIGHT, PTR, size) {
                self.log(
                    W,
                    format!(
                        "obs_register_source: get_height ignored registering transition '{data_id}'"
                    ),
                );
            }
        }
        let flags = output_flags(data_type, data_flags);

        let callbacks = Callbacks {
            get_name: info.get_name && inside(OFF_GET_NAME, PTR, size),
            get_width: info.get_width && inside(OFF_GET_WIDTH, PTR, size),
            get_height: info.get_height && inside(OFF_GET_HEIGHT, PTR, size),
            audio_render: info.audio_render && inside(OFF_AUDIO_RENDER, PTR, size),
        };
        let required = |val: &str| {
            format!("Required value '{val}' for '{id}' not found.  obs_register_source failed.")
        };
        match check(info.source_type, info.output_flags, flags, callbacks) {
            Ok(()) => {}
            Err(broken) => {
                let (level, msg) = match broken {
                    Broken::CompositeAudio => (
                        W,
                        format!(
                            "obs_register_source: Source '{id}': Composite sources cannot be \
                             audio sources"
                        ),
                    ),
                    Broken::CompositeAsync => (
                        W,
                        format!(
                            "obs_register_source: Source '{id}': Composite sources cannot be \
                             async sources"
                        ),
                    ),
                    Broken::NoGetName => (E, required("get_name")),
                    Broken::NoGetWidth => (E, required("get_width")),
                    Broken::NoGetHeight => (E, required("get_height")),
                    Broken::NoAudioRender => (E, required("audio_render")),
                };
                self.log(level, msg);
                return Err(());
            }
        }

        let versioned = versioned_id(Some(id.as_bytes()), data_version).expect("an id");
        self.types.push(Entry {
            id: versioned,
            unversioned_id: id.as_bytes().to_vec(),
            version: data_version,
            list,
            flags,
            type_data: data_type_data,
        });
        Ok(())
    }

    fn expect_logged(&self, expected: &[(Level, &str)]) {
        let got: Vec<(Level, &str)> = self.logged.iter().map(|(l, m)| (*l, m.as_str())).collect();
        assert_eq!(got, expected);
    }

    /// `expect_registered()` of the C test.
    fn expect_registered(&self, id: &str, list: List, flags: u32) {
        let e = self
            .find(id)
            .unwrap_or_else(|| panic!("{id} not registered"));
        assert_eq!(e.list, list, "{id}");
        assert_eq!(e.flags, flags, "{id}");
        assert!(e.type_data, "{id}");
    }

    /// `expect_rejected()` of the C test.
    fn expect_rejected(&self, id: &str, types: usize, freed: bool) {
        assert!(self.find(id).is_none(), "{id}");
        assert_eq!(self.count(None), types, "{id}");
        assert_eq!(self.frees, usize::from(freed), "{id}");
    }
}

fn required(val: &str, id: &str) -> String {
    format!("Required value '{val}' for '{id}' not found.  obs_register_source failed.")
}

fn composite_audio(id: &str) -> String {
    format!("obs_register_source: Source '{id}': Composite sources cannot be audio sources")
}

fn composite_async(id: &str) -> String {
    format!("obs_register_source: Source '{id}': Composite sources cannot be async sources")
}

fn dup(id: &str) -> String {
    format!("obs_register_source: Source '{id}' already exists!  Duplicate library?")
}

// ------------------------------------------------- the C test, 1:1

#[test]
fn type_lists() {
    let mut r = Registry::default();
    let info = full_info("sr_input", TYPE_INPUT, VIDEO);
    r.register(&info, SIZE);
    r.expect_logged(&[]);
    r.expect_registered("sr_input", List::Input, VIDEO);
    assert_eq!(r.frees, 0);

    r.register(&full_info("sr_filter", TYPE_FILTER, VIDEO), SIZE);
    r.expect_logged(&[]);
    r.expect_registered("sr_filter", List::Filter, VIDEO);

    let mut info = full_info("sr_transition", TYPE_TRANSITION, 0);
    info.get_width = false;
    info.get_height = false;
    r.register(&info, SIZE);
    r.expect_logged(&[]);
    r.expect_registered(
        "sr_transition",
        List::Transition,
        COMPOSITE | VIDEO | CUSTOM_DRAW,
    );

    // A scene type goes in no list of its own.
    let (inputs, filters, transitions) = (
        r.count(Some(List::Input)),
        r.count(Some(List::Filter)),
        r.count(Some(List::Transition)),
    );
    r.register(&full_info("sr_scene", TYPE_SCENE, VIDEO | COMPOSITE), SIZE);
    r.expect_logged(&[]);
    r.expect_registered("sr_scene", List::None, VIDEO | COMPOSITE);
    assert_eq!(r.count(Some(List::Input)), inputs);
    assert_eq!(r.count(Some(List::Filter)), filters);
    assert_eq!(r.count(Some(List::Transition)), transitions);
}

#[test]
fn unknown_type() {
    let mut r = Registry::default();
    r.register(&full_info("sr_input", TYPE_INPUT, AUDIO), SIZE);
    let types = r.count(None);

    r.register(&full_info("sr_type4", 4, VIDEO), SIZE);
    r.expect_logged(&[(
        W,
        "obs_register_source: Tried to register unknown source type: 4",
    )]);
    r.expect_rejected("sr_type4", types, true);

    r.register(&full_info("sr_type_max", u32::MAX, VIDEO), SIZE);
    r.expect_logged(&[(
        W,
        "obs_register_source: Tried to register unknown source type: 4294967295",
    )]);
    r.expect_rejected("sr_type_max", types, true);

    // The type is checked first.
    let mut info = full_info("sr_input", 7, COMPOSITE | AUDIO);
    info.get_name = false;
    r.register(&info, SIZE + 8);
    r.expect_logged(&[(
        W,
        "obs_register_source: Tried to register unknown source type: 7",
    )]);
    assert_eq!(r.count(None), types);
    assert_eq!(r.frees, 1);

    // Size 0: nothing copied, nothing freed.
    r.register(&full_info("sr_type9", 9, VIDEO), 0);
    r.expect_logged(&[(
        W,
        "obs_register_source: Tried to register unknown source type: 9",
    )]);
    r.expect_rejected("sr_type9", types, false);
}

#[test]
fn duplicate() {
    let mut r = Registry::default();
    r.register(&full_info("sr_dup", TYPE_INPUT, AUDIO), SIZE);
    r.expect_logged(&[]);
    let types = r.count(None);

    let mut info = full_info("sr_dup", TYPE_FILTER, VIDEO);
    r.register(&info, SIZE);
    r.expect_logged(&[(W, &dup("sr_dup"))]);
    assert_eq!(r.count(None), types);
    assert_eq!(r.frees, 1);
    assert_eq!(r.find("sr_dup").map(|e| e.flags), Some(AUDIO));

    // Before the size and the callbacks.
    info.get_name = false;
    r.register(&info, SIZE + 8);
    r.expect_logged(&[(W, &dup("sr_dup"))]);
    assert_eq!(r.count(None), types);

    // Another version is not a duplicate.
    let mut info = full_info("sr_dup", TYPE_INPUT, AUDIO);
    info.version = 2;
    r.register(&info, SIZE);
    r.expect_logged(&[]);
    r.expect_registered("sr_dup_v2", List::Input, AUDIO);
    r.register(&info, SIZE);
    r.expect_logged(&[(W, &dup("sr_dup"))]);
}

#[test]
fn size() {
    let mut r = Registry::default();
    let types = r.count(None);
    let msg = format!(
        "obs_register_source: Tried to register obs_source_info with size {} which is more than \
         libobs currently supports ({SIZE})",
        SIZE + 1
    );

    r.register(&full_info("sr_size_over", TYPE_INPUT, VIDEO), SIZE + 1);
    r.expect_logged(&[(W, &msg)]);
    r.expect_rejected("sr_size_over", types, true);

    let mut info = full_info("sr_size_over2", TYPE_INPUT, COMPOSITE | AUDIO);
    info.get_name = false;
    r.register(&info, SIZE + 1);
    r.expect_logged(&[(W, &msg)]);
    r.expect_rejected("sr_size_over2", types, true);

    r.register(&full_info("sr_size_exact", TYPE_INPUT, VIDEO), SIZE);
    r.expect_logged(&[]);
    r.expect_registered("sr_size_exact", List::Input, VIDEO);
}

#[test]
fn short_size() {
    let mut r = Registry::default();
    let mut types = r.count(None);

    // Up to and with get_name: an audio input needs nothing more.
    let info = full_info("sr_short_name", TYPE_INPUT, AUDIO);
    r.register(&info, OFF_GET_NAME + PTR);
    r.expect_logged(&[]);
    let e = r.find("sr_short_name").expect("registered");
    assert_eq!((e.list, e.flags), (List::Input, AUDIO));
    // The type data was not copied.
    assert!(!e.type_data);
    types += 1;

    // get_name cut off, whole or in part.
    let info = full_info("sr_short_noname", TYPE_INPUT, AUDIO);
    for size in [OFF_GET_NAME, OFF_GET_NAME + PTR - 1] {
        r.register(&info, size);
        r.expect_logged(&[(E, &required("get_name", "sr_short_noname"))]);
        r.expect_rejected("sr_short_noname", types, false);
    }

    // A sync video input cut before get_width, or before get_height.
    r.register(
        &full_info("sr_short_width", TYPE_INPUT, VIDEO),
        OFF_GET_WIDTH,
    );
    r.expect_logged(&[(E, &required("get_width", "sr_short_width"))]);
    r.expect_rejected("sr_short_width", types, false);
    r.register(
        &full_info("sr_short_height", TYPE_INPUT, VIDEO),
        OFF_GET_HEIGHT,
    );
    r.expect_logged(&[(E, &required("get_height", "sr_short_height"))]);
    r.expect_rejected("sr_short_height", types, false);

    // type_data inside the size but free_type_data not: not freed.
    let mut info = full_info("sr_short_free", TYPE_INPUT, VIDEO);
    info.get_name = false;
    r.register(&info, OFF_FREE_TYPE_DATA);
    r.expect_logged(&[(E, &required("get_name", "sr_short_free"))]);
    r.expect_rejected("sr_short_free", types, false);
    r.register(&info, OFF_FREE_TYPE_DATA + PTR);
    r.expect_logged(&[(E, &required("get_name", "sr_short_free"))]);
    r.expect_rejected("sr_short_free", types, true);

    // A composite source cut before audio_render.
    r.register(
        &full_info("sr_short_render", TYPE_INPUT, VIDEO | COMPOSITE),
        OFF_AUDIO_RENDER,
    );
    r.expect_logged(&[(E, &required("audio_render", "sr_short_render"))]);
    r.expect_rejected("sr_short_render", types, true);

    // Size 0.
    r.register(&full_info("sr_short_zero", TYPE_FILTER, VIDEO), 0);
    r.expect_logged(&[(E, &required("get_name", "sr_short_zero"))]);
    r.expect_rejected("sr_short_zero", types, false);

    // The composite checks see the flags only when they are inside the
    // size, and run before get_name is required.
    let info = full_info("sr_short_flags", TYPE_INPUT, COMPOSITE | AUDIO);
    r.register(&info, OFF_OUTPUT_FLAGS);
    r.expect_logged(&[(E, &required("get_name", "sr_short_flags"))]);
    r.expect_rejected("sr_short_flags", types, false);
    r.register(&info, OFF_OUTPUT_FLAGS + 4);
    r.expect_logged(&[(W, &composite_audio("sr_short_flags"))]);
    r.expect_rejected("sr_short_flags", types, false);

    // The list is picked by the caller's type, the flags are normalized by
    // the copy's (characterized, not endorsed).
    let info = full_info("sr_short_type", TYPE_FILTER, COMPOSITE);
    r.register(&info, OFF_TYPE);
    r.expect_logged(&[(E, &required("get_name", "sr_short_type"))]);
    r.expect_rejected("sr_short_type", types, false);
    r.register(&info, OFF_GET_NAME);
    r.expect_logged(&[(W, &composite_async("sr_short_type"))]);
    r.expect_rejected("sr_short_type", types, false);

    // Cut before version: not versioned, though the duplicate lookup used
    // the caller's version (characterized, not endorsed).
    let mut info = full_info("sr_short_version", TYPE_INPUT, VIDEO);
    info.version = 3;
    r.register(&info, OFF_VERSION);
    r.expect_logged(&[]);
    r.expect_registered("sr_short_version", List::Input, VIDEO);
    assert!(r.find("sr_short_version_v3").is_none());
    r.register(&info, SIZE);
    r.expect_logged(&[]);
    r.expect_registered("sr_short_version_v3", List::Input, VIDEO);
    info.version = 0;
    r.register(&info, SIZE);
    r.expect_logged(&[(W, &dup("sr_short_version"))]);
}

#[test]
fn filter_flags() {
    let mut r = Registry::default();
    r.register(&full_info("sr_filter_audio", TYPE_FILTER, AUDIO), SIZE);
    r.expect_logged(&[]);
    r.expect_registered("sr_filter_audio", List::Filter, AUDIO | ASYNC);

    r.register(&full_info("sr_filter_none", TYPE_FILTER, 0), SIZE);
    r.expect_logged(&[]);
    r.expect_registered("sr_filter_none", List::Filter, ASYNC);

    let bits = AUDIO | DEPRECATED | (1 << 31);
    r.register(&full_info("sr_filter_bits", TYPE_FILTER, bits), SIZE);
    r.expect_logged(&[]);
    r.expect_registered("sr_filter_bits", List::Filter, bits | ASYNC);

    let mut info = full_info("sr_filter_async_video", TYPE_FILTER, ASYNC_VIDEO);
    info.get_width = false;
    info.get_height = false;
    r.register(&info, SIZE);
    r.expect_logged(&[]);
    r.expect_registered("sr_filter_async_video", List::Filter, ASYNC_VIDEO);

    r.register(&full_info("sr_input_audio", TYPE_INPUT, AUDIO), SIZE);
    r.expect_registered("sr_input_audio", List::Input, AUDIO);
    r.register(&full_info("sr_scene_none", TYPE_SCENE, 0), SIZE);
    r.expect_registered("sr_scene_none", List::None, 0);
}

#[test]
fn transition_flags() {
    let mut r = Registry::default();

    r.register(
        &full_info("sr_trans_bits", TYPE_TRANSITION, DEPRECATED | VIDEO),
        SIZE,
    );
    r.expect_logged(&[
        (
            W,
            "obs_register_source: get_width ignored registering transition 'sr_trans_bits'",
        ),
        (
            W,
            "obs_register_source: get_height ignored registering transition 'sr_trans_bits'",
        ),
    ]);
    r.expect_registered(
        "sr_trans_bits",
        List::Transition,
        DEPRECATED | COMPOSITE | VIDEO | CUSTOM_DRAW,
    );

    let mut info = full_info("sr_trans_height", TYPE_TRANSITION, 0);
    info.get_width = false;
    r.register(&info, SIZE);
    r.expect_logged(&[(
        W,
        "obs_register_source: get_height ignored registering transition 'sr_trans_height'",
    )]);
    let types = r.count(None);

    let mut info = full_info("sr_trans_norender", TYPE_TRANSITION, 0);
    info.get_width = false;
    info.get_height = false;
    info.audio_render = false;
    r.register(&info, SIZE);
    r.expect_logged(&[(E, &required("audio_render", "sr_trans_norender"))]);
    r.expect_rejected("sr_trans_norender", types, true);

    r.register(&full_info("sr_trans_audio", TYPE_TRANSITION, AUDIO), SIZE);
    r.expect_logged(&[
        (
            W,
            "obs_register_source: get_width ignored registering transition 'sr_trans_audio'",
        ),
        (
            W,
            "obs_register_source: get_height ignored registering transition 'sr_trans_audio'",
        ),
        (W, &composite_audio("sr_trans_audio")),
    ]);
    r.expect_rejected("sr_trans_audio", types, true);

    let mut info = full_info("sr_trans_async", TYPE_TRANSITION, ASYNC);
    info.get_width = false;
    info.get_height = false;
    r.register(&info, SIZE);
    r.expect_logged(&[(W, &composite_async("sr_trans_async"))]);
    r.expect_rejected("sr_trans_async", types, true);
}

#[test]
fn composite() {
    let mut r = Registry::default();
    let types = r.count(None);

    r.register(
        &full_info("sr_comp_audio", TYPE_INPUT, COMPOSITE | AUDIO),
        SIZE,
    );
    r.expect_logged(&[(W, &composite_audio("sr_comp_audio"))]);
    r.expect_rejected("sr_comp_audio", types, true);

    r.register(
        &full_info("sr_comp_async", TYPE_INPUT, COMPOSITE | ASYNC_VIDEO),
        SIZE,
    );
    r.expect_logged(&[(W, &composite_async("sr_comp_async"))]);
    r.expect_rejected("sr_comp_async", types, true);

    let mut info = full_info("sr_comp_both", TYPE_INPUT, COMPOSITE | AUDIO | ASYNC);
    info.get_name = false;
    info.audio_render = false;
    r.register(&info, SIZE);
    r.expect_logged(&[(W, &composite_audio("sr_comp_both"))]);
    r.expect_rejected("sr_comp_both", types, true);

    r.register(&full_info("sr_comp_filter", TYPE_FILTER, COMPOSITE), SIZE);
    r.expect_logged(&[(W, &composite_async("sr_comp_filter"))]);
    r.expect_rejected("sr_comp_filter", types, true);

    let mut info = full_info("sr_comp_norender", TYPE_INPUT, COMPOSITE | VIDEO);
    info.audio_render = false;
    r.register(&info, SIZE);
    r.expect_logged(&[(E, &required("audio_render", "sr_comp_norender"))]);
    r.expect_rejected("sr_comp_norender", types, true);

    r.register(
        &full_info("sr_comp_filter_video", TYPE_FILTER, COMPOSITE | VIDEO),
        SIZE,
    );
    r.expect_logged(&[]);
    r.expect_registered("sr_comp_filter_video", List::Filter, COMPOSITE | VIDEO);

    let mut info = full_info("sr_plain_norender", TYPE_INPUT, AUDIO);
    info.audio_render = false;
    r.register(&info, SIZE);
    r.expect_logged(&[]);
    r.expect_registered("sr_plain_norender", List::Input, AUDIO);
}

#[test]
fn required_callbacks() {
    let mut r = Registry::default();
    let types = r.count(None);

    let mut info = full_info("sr_req_noname", TYPE_INPUT, AUDIO);
    info.get_name = false;
    r.register(&info, SIZE);
    r.expect_logged(&[(E, &required("get_name", "sr_req_noname"))]);
    r.expect_rejected("sr_req_noname", types, true);

    let mut info = full_info("sr_req_none", TYPE_INPUT, VIDEO);
    info.get_name = false;
    info.get_width = false;
    info.get_height = false;
    r.register(&info, SIZE);
    r.expect_logged(&[(E, &required("get_name", "sr_req_none"))]);
    info.get_name = true;
    r.register(&info, SIZE);
    r.expect_logged(&[(E, &required("get_width", "sr_req_none"))]);
    info.get_width = true;
    r.register(&info, SIZE);
    r.expect_logged(&[(E, &required("get_height", "sr_req_none"))]);
    r.expect_rejected("sr_req_none", types, true);
    info.get_width = false;
    info.get_height = true;
    r.register(&info, SIZE);
    r.expect_logged(&[(E, &required("get_width", "sr_req_none"))]);

    let mut info = full_info("sr_req_comp", TYPE_INPUT, VIDEO | COMPOSITE);
    info.get_height = false;
    info.audio_render = false;
    r.register(&info, SIZE);
    r.expect_logged(&[(E, &required("get_height", "sr_req_comp"))]);

    let mut info = full_info("sr_req_scene", TYPE_SCENE, VIDEO);
    info.get_width = false;
    r.register(&info, SIZE);
    r.expect_logged(&[(E, &required("get_width", "sr_req_scene"))]);
    r.expect_rejected("sr_req_scene", types, true);

    for (id, t, flags, list) in [
        ("sr_req_async", TYPE_INPUT, ASYNC_VIDEO, List::Input),
        ("sr_req_audio", TYPE_INPUT, AUDIO, List::Input),
        ("sr_req_filter", TYPE_FILTER, VIDEO, List::Filter),
    ] {
        let mut info = full_info(id, t, flags);
        info.get_width = false;
        info.get_height = false;
        r.register(&info, SIZE);
        r.expect_logged(&[]);
        r.expect_registered(id, list, flags);
    }

    let types = r.count(None);
    let mut info = full_info("sr_req_nofree", TYPE_INPUT, AUDIO);
    info.get_name = false;
    info.free_type_data = false;
    r.register(&info, SIZE);
    r.expect_logged(&[(E, &required("get_name", "sr_req_nofree"))]);
    r.expect_rejected("sr_req_nofree", types, false);
    info.free_type_data = true;
    info.type_data = false;
    r.register(&info, SIZE);
    r.expect_rejected("sr_req_nofree", types, false);
}

#[test]
fn versioned_ids() {
    let mut r = Registry::default();
    let mut expect = |id: &'static str, version: u32, expected: &str| {
        let mut info = full_info(id, TYPE_INPUT, AUDIO);
        info.version = version;
        r.register(&info, SIZE);
        r.expect_logged(&[]);
        r.expect_registered(expected, List::Input, AUDIO);
        assert_eq!(
            r.find(expected).map(|e| e.unversioned_id.clone()),
            Some(id.as_bytes().to_vec())
        );
    };
    expect("sr_ver", 0, "sr_ver");
    expect("sr_ver", 1, "sr_ver_v1");
    expect("sr_ver", 10, "sr_ver_v10");
    expect("sr_ver", 2_147_483_647, "sr_ver_v2147483647");
    // The version is printed as an int (characterized, not endorsed).
    expect("sr_ver", 2_147_483_648, "sr_ver_v-2147483648");
    expect("sr_ver", u32::MAX, "sr_ver_v-1");
    // The id is not a format.
    expect("sr_%s%d", 4, "sr_%s%d_v4");
    expect("", 5, "_v5");
    expect("sr_ver_v1", 1, "sr_ver_v1_v1");
}

// ------------------------------------------------- edges of each function

#[test]
fn list_of_edges() {
    assert_eq!(list_of(TYPE_INPUT), List::Input);
    assert_eq!(list_of(TYPE_FILTER), List::Filter);
    assert_eq!(list_of(TYPE_TRANSITION), List::Transition);
    assert_eq!(list_of(TYPE_SCENE), List::None);
    for t in [4, 5, 0x100, 0x7fff_ffff, 0x8000_0000, u32::MAX] {
        assert_eq!(list_of(t), List::Unknown, "{t}");
    }
}

#[test]
fn size_supported_edges() {
    assert!(size_supported(0, SIZE));
    assert!(size_supported(SIZE, SIZE));
    assert!(!size_supported(SIZE + 1, SIZE));
    assert!(size_supported(0, 0));
    assert!(!size_supported(1, 0));
    assert!(size_supported(usize::MAX, usize::MAX));
    assert!(!size_supported(usize::MAX, usize::MAX - 1));
}

#[test]
fn output_flags_edges() {
    // A filter: async unless it has video.
    assert_eq!(output_flags(TYPE_FILTER, 0), ASYNC);
    assert_eq!(output_flags(TYPE_FILTER, AUDIO), AUDIO | ASYNC);
    assert_eq!(output_flags(TYPE_FILTER, ASYNC), ASYNC);
    assert_eq!(output_flags(TYPE_FILTER, VIDEO), VIDEO);
    assert_eq!(output_flags(TYPE_FILTER, !VIDEO), !VIDEO);
    assert_eq!(output_flags(TYPE_FILTER, u32::MAX), u32::MAX);
    // A transition: composite, video and custom draw added.
    let t = COMPOSITE | VIDEO | CUSTOM_DRAW;
    assert_eq!(output_flags(TYPE_TRANSITION, 0), t);
    assert_eq!(
        output_flags(TYPE_TRANSITION, AUDIO | ASYNC),
        t | AUDIO | ASYNC
    );
    assert_eq!(output_flags(TYPE_TRANSITION, u32::MAX), u32::MAX);
    // Anything else: unchanged.
    for ty in [TYPE_INPUT, TYPE_SCENE, 4, u32::MAX] {
        for f in [0, AUDIO, VIDEO, COMPOSITE, u32::MAX] {
            assert_eq!(output_flags(ty, f), f, "{ty} {f:#x}");
        }
    }
}

const ALL: Callbacks = Callbacks {
    get_name: true,
    get_width: true,
    get_height: true,
    audio_render: true,
};

#[test]
fn check_edges() {
    let none = Callbacks::default();
    // Composite audio before composite async before every callback.
    assert_eq!(
        check(TYPE_INPUT, VIDEO, COMPOSITE | AUDIO | ASYNC, none),
        Err(Broken::CompositeAudio)
    );
    assert_eq!(
        check(TYPE_INPUT, VIDEO, COMPOSITE | ASYNC, none),
        Err(Broken::CompositeAsync)
    );
    // Audio or async without composite is fine.
    assert_eq!(check(TYPE_INPUT, AUDIO | ASYNC, AUDIO | ASYNC, ALL), Ok(()));
    // get_name first.
    assert_eq!(
        check(TYPE_INPUT, VIDEO, COMPOSITE, none),
        Err(Broken::NoGetName)
    );
    // get_width, get_height, then audio_render.
    let name = Callbacks {
        get_name: true,
        ..none
    };
    assert_eq!(
        check(TYPE_INPUT, VIDEO, COMPOSITE, name),
        Err(Broken::NoGetWidth)
    );
    let wide = Callbacks {
        get_width: true,
        ..name
    };
    assert_eq!(
        check(TYPE_INPUT, VIDEO, COMPOSITE, wide),
        Err(Broken::NoGetHeight)
    );
    let sized = Callbacks {
        get_height: true,
        ..wide
    };
    assert_eq!(
        check(TYPE_INPUT, VIDEO, COMPOSITE, sized),
        Err(Broken::NoAudioRender)
    );
    assert_eq!(check(TYPE_INPUT, VIDEO, COMPOSITE, ALL), Ok(()));
    // The size rule reads the caller's flags and type, not the normalized
    // flags: a filter, a transition, async video or no video skip it.
    assert_eq!(check(TYPE_INPUT, VIDEO, 0, name), Err(Broken::NoGetWidth));
    assert_eq!(check(TYPE_INPUT, 0, VIDEO, name), Ok(()));
    assert_eq!(check(TYPE_INPUT, ASYNC_VIDEO, VIDEO, name), Ok(()));
    assert_eq!(check(TYPE_FILTER, VIDEO, VIDEO, name), Ok(()));
    assert_eq!(check(TYPE_TRANSITION, VIDEO, VIDEO, name), Ok(()));
    assert_eq!(
        check(TYPE_SCENE, VIDEO, VIDEO, name),
        Err(Broken::NoGetWidth)
    );
    assert_eq!(check(u32::MAX, VIDEO, VIDEO, name), Err(Broken::NoGetWidth));
    // The composite rules read the normalized flags, not the caller's.
    assert_eq!(check(TYPE_INPUT, COMPOSITE | AUDIO, 0, ALL), Ok(()));
    assert_eq!(
        check(TYPE_INPUT, 0, COMPOSITE | AUDIO, ALL),
        Err(Broken::CompositeAudio)
    );
    assert_eq!(check(TYPE_INPUT, COMPOSITE, 0, name), Ok(()));
}

#[test]
fn callbacks_from_bits() {
    assert_eq!(Callbacks::from_bits(0), Callbacks::default());
    assert_eq!(
        Callbacks::from_bits(HAS_GET_NAME | HAS_GET_WIDTH | HAS_GET_HEIGHT | HAS_AUDIO_RENDER),
        ALL
    );
    assert_eq!(Callbacks::from_bits(u32::MAX), ALL);
    assert_eq!(Callbacks::from_bits(!0xf), Callbacks::default());
    assert_eq!(
        Callbacks::from_bits(HAS_GET_HEIGHT),
        Callbacks {
            get_height: true,
            ..Callbacks::default()
        }
    );
}

#[test]
fn versioned_id_edges() {
    let v = |id: Option<&[u8]>, version| versioned_id(id, version);
    assert_eq!(v(Some(b"x"), 0).as_deref(), Some(&b"x"[..]));
    assert_eq!(v(Some(b""), 0).as_deref(), Some(&b""[..]));
    assert_eq!(v(None, 0), None);
    assert_eq!(v(Some(b"x"), 1).as_deref(), Some(&b"x_v1"[..]));
    assert_eq!(v(Some(b""), 1).as_deref(), Some(&b"_v1"[..]));
    assert_eq!(
        v(Some(b"x"), 0x8000_0000).as_deref(),
        Some(&b"x_v-2147483648"[..])
    );
    assert_eq!(v(Some(b"x"), u32::MAX).as_deref(), Some(&b"x_v-1"[..]));
    assert_eq!(v(Some(b"\xff%n"), 7).as_deref(), Some(&b"\xff%n_v7"[..]));
    // A NULL id prints as the C's "%s" prints NULL.
    assert_eq!(v(None, 3), Some([NULL_ID, b"_v3"].concat()));
}

// ------------------------------------------------------------ proptests

fn any_type() -> impl Strategy<Value = u32> {
    prop_oneof![0u32..6, any::<u32>()]
}

fn any_flags() -> impl Strategy<Value = u32> {
    prop_oneof![0u32..0x80, 0u32..0x400, any::<u32>()]
}

fn any_callbacks() -> impl Strategy<Value = Callbacks> {
    any::<u32>().prop_map(Callbacks::from_bits)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn list_of_is_unknown_past_the_scene(t in any_type()) {
        prop_assert_eq!(list_of(t) == List::Unknown, t > TYPE_SCENE);
    }

    #[test]
    fn output_flags_only_add_bits(t in any_type(), f in any_flags()) {
        let out = output_flags(t, f);
        prop_assert_eq!(out & f, f);
        let added = out & !f;
        match t {
            TYPE_FILTER => prop_assert_eq!(added, if f & VIDEO == 0 { ASYNC & !f } else { 0 }),
            TYPE_TRANSITION => prop_assert_eq!(added, (COMPOSITE | VIDEO | CUSTOM_DRAW) & !f),
            _ => prop_assert_eq!(added, 0),
        }
        // Normalizing twice changes nothing more.
        prop_assert_eq!(output_flags(t, out), out);
    }

    #[test]
    fn check_reports_the_first_rule_broken(
        t in any_type(),
        info in any_flags(),
        flags in any_flags(),
        cb in any_callbacks(),
    ) {
        let composite = flags & COMPOSITE != 0;
        let sized = t != TYPE_FILTER && t != TYPE_TRANSITION && info & ASYNC_VIDEO == VIDEO;
        let rules = [
            (composite && flags & AUDIO != 0, Broken::CompositeAudio),
            (composite && flags & ASYNC != 0, Broken::CompositeAsync),
            (!cb.get_name, Broken::NoGetName),
            (sized && !cb.get_width, Broken::NoGetWidth),
            (sized && !cb.get_height, Broken::NoGetHeight),
            (composite && !cb.audio_render, Broken::NoAudioRender),
        ];
        let first = rules.iter().find(|(broken, _)| *broken).map(|(_, rule)| *rule);
        prop_assert_eq!(check(t, info, flags, cb), first.map_or(Ok(()), Err));
        // Every callback set never breaks a callback rule.
        let full = Callbacks { get_name: true, get_width: true, get_height: true, audio_render: true };
        prop_assert!(matches!(
            check(t, info, flags, full),
            Ok(()) | Err(Broken::CompositeAudio | Broken::CompositeAsync)
        ));
    }

    #[test]
    fn size_supported_is_at_most(size in any::<usize>(), supported in any::<usize>()) {
        prop_assert_eq!(size_supported(size, supported), size <= supported);
        prop_assert!(size_supported(supported, supported));
    }

    #[test]
    fn versioned_id_is_the_id_and_the_signed_version(
        id in prop::collection::vec(1u8..=255, 0..12),
        version in prop_oneof![Just(0u32), 1u32..100, any::<u32>()],
        null in any::<bool>(),
    ) {
        let id = (!null).then_some(id);
        let out = versioned_id(id.as_deref(), version);
        if version == 0 {
            prop_assert_eq!(out, id);
        } else {
            let mut want = id.unwrap_or_else(|| NULL_ID.to_vec());
            want.extend_from_slice(format!("_v{}", version.cast_signed()).as_bytes());
            prop_assert_eq!(out, Some(want));
        }
    }
}
