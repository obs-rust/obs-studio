//! Tier 1: 1:1 port of `test/cmocka/test_obs_properties.c`, through the C
//! ABI shims.
#![allow(clippy::missing_safety_doc)]

use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_void};
use std::ptr;

use obs_c_oracle as _; // blog, obs_obj_get_data
use obs_properties::ffi::obs_properties::*;
use obs_properties::props::Fps;
use obs_util as _; // bmem, base log handler

// enum values from obs-properties.h
const PROPERTY_INVALID: i32 = 0;
const PROPERTY_BOOL: i32 = 1;
const PROPERTY_INT: i32 = 2;
const PROPERTY_COLOR: i32 = 7;
const PROPERTY_FONT: i32 = 9;
const PROPERTY_GROUP: i32 = 12;
const PROPERTY_COLOR_ALPHA: i32 = 13;
const COMBO_FORMAT_INVALID: i32 = 0;
const COMBO_FORMAT_INT: i32 = 1;
const COMBO_FORMAT_FLOAT: i32 = 2;
const COMBO_FORMAT_STRING: i32 = 3;
const COMBO_FORMAT_BOOL: i32 = 4;
const COMBO_TYPE_INVALID: i32 = 0;
const COMBO_TYPE_EDITABLE: i32 = 1;
const COMBO_TYPE_LIST: i32 = 2;
const COMBO_TYPE_RADIO: i32 = 3;
const EDITABLE_LIST_TYPE_STRINGS: i32 = 0;
const EDITABLE_LIST_TYPE_FILES: i32 = 1;
const PATH_FILE: i32 = 0;
const PATH_FILE_SAVE: i32 = 1;
const PATH_DIRECTORY: i32 = 2;
const TEXT_DEFAULT: i32 = 0;
const TEXT_MULTILINE: i32 = 2;
const TEXT_INFO_NORMAL: i32 = 0;
const TEXT_INFO_ERROR: i32 = 2;
const NUMBER_SCROLLER: i32 = 0;
const NUMBER_SLIDER: i32 = 1;
const COMBO_INVALID: i32 = 0;
const GROUP_NORMAL: i32 = 1;
const GROUP_CHECKABLE: i32 = 2;
const BUTTON_DEFAULT: i32 = 0;
const BUTTON_URL: i32 = 1;
const DEFER_UPDATE: u32 = 1 << 0;
const DARRAY_INVALID: usize = usize::MAX;

/* ------------------------------------------------------------------------- */
/* callback recorder */

struct Call {
    name: String,
    props: *mut c_void,
    priv_: *mut c_void,
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static DESTROYED: RefCell<i32> = const { RefCell::new(0) };
}

fn reset_calls() {
    CALLS.with_borrow_mut(Vec::clear);
}

fn calls() -> Vec<(String, *mut c_void, *mut c_void)> {
    CALLS.with_borrow(|c| {
        c.iter()
            .map(|c| (c.name.clone(), c.props, c.priv_))
            .collect()
    })
}

unsafe fn record(props: *mut c_void, p: *mut c_void, priv_: *mut c_void) {
    let name = s(unsafe { obs_property_name(p) }).unwrap().to_owned();
    CALLS.with_borrow_mut(|c| c.push(Call { name, props, priv_ }));
}

unsafe extern "C" fn modified_cb(props: *mut c_void, p: *mut c_void, _s: *mut c_void) -> bool {
    unsafe { record(props, p, ptr::null_mut()) };
    true
}

unsafe extern "C" fn modified2_cb(
    priv_: *mut c_void,
    props: *mut c_void,
    p: *mut c_void,
    _s: *mut c_void,
) -> bool {
    unsafe { record(props, p, priv_) };
    false
}

unsafe extern "C" fn clicked_cb(props: *mut c_void, p: *mut c_void, data: *mut c_void) -> bool {
    unsafe { record(props, p, data) };
    true
}

unsafe extern "C" fn destroy_param(_param: *mut c_void) {
    DESTROYED.with_borrow_mut(|d| *d += 1);
}

fn destroyed() -> i32 {
    DESTROYED.with_borrow(|d| *d)
}

fn s<'a>(p: *const c_char) -> Option<&'a str> {
    // SAFETY: the shims return null or a NUL-terminated string.
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_str().unwrap())
}

macro_rules! c {
    ($s:literal) => {
        concat!($s, "\0").as_ptr().cast::<c_char>()
    };
}

/* ------------------------------------------------------------------------- */

#[test]
fn test_create_param_flags() {
    unsafe {
        let mut a = 1;
        let mut b = 2;
        let pa = (&raw mut a).cast::<c_void>();
        let pb = (&raw mut b).cast::<c_void>();
        DESTROYED.with_borrow_mut(|d| *d = 0);

        let props = obs_properties_create_param(pa, Some(destroy_param));
        assert_eq!(obs_properties_get_param(props), pa);
        assert_eq!(obs_properties_get_flags(props), 0);
        assert!(obs_properties_first(props).is_null());
        assert!(obs_properties_get_parent(props).is_null());

        obs_properties_set_flags(props, DEFER_UPDATE);
        assert_eq!(obs_properties_get_flags(props), DEFER_UPDATE);

        // replacing the param destroys the old one
        obs_properties_set_param(props, pb, Some(destroy_param));
        assert_eq!(destroyed(), 1);
        assert_eq!(obs_properties_get_param(props), pb);

        obs_properties_destroy(props);
        assert_eq!(destroyed(), 2);

        // NULL handles
        assert!(obs_properties_get_param(ptr::null_mut()).is_null());
        assert_eq!(obs_properties_get_flags(ptr::null_mut()), 0);
        assert!(obs_properties_first(ptr::null_mut()).is_null());
        assert!(obs_properties_get(ptr::null_mut(), c!("x")).is_null());
        obs_properties_set_flags(ptr::null_mut(), 1);
        obs_properties_destroy(ptr::null_mut());
    }
}

#[test]
fn test_common_fields() {
    unsafe {
        let props = obs_properties_create();
        let p = obs_properties_add_bool(props, c!("b"), c!("Bool"));

        assert_eq!(s(obs_property_name(p)), Some("b"));
        assert_eq!(s(obs_property_description(p)), Some("Bool"));
        assert!(obs_property_long_description(p).is_null());
        assert_eq!(obs_property_get_type(p), PROPERTY_BOOL);
        assert!(obs_property_enabled(p));
        assert!(obs_property_visible(p));

        obs_property_set_enabled(p, false);
        obs_property_set_visible(p, false);
        assert!(!obs_property_enabled(p));
        assert!(!obs_property_visible(p));

        obs_property_set_description(p, c!("New"));
        assert_eq!(s(obs_property_description(p)), Some("New"));
        // an empty description becomes NULL
        obs_property_set_description(p, c!(""));
        assert!(obs_property_description(p).is_null());
        obs_property_set_long_description(p, c!("Long"));
        assert_eq!(s(obs_property_long_description(p)), Some("Long"));
        obs_property_set_long_description(p, c!(""));
        assert!(obs_property_long_description(p).is_null());

        // NULL property
        assert!(obs_property_name(ptr::null_mut()).is_null());
        assert!(obs_property_description(ptr::null_mut()).is_null());
        assert_eq!(obs_property_get_type(ptr::null_mut()), PROPERTY_INVALID);
        assert!(!obs_property_enabled(ptr::null_mut()));
        assert!(!obs_property_visible(ptr::null_mut()));

        obs_properties_destroy(props);
    }
}

#[test]
fn test_order_and_lookup() {
    unsafe {
        let props = obs_properties_create();
        obs_properties_add_bool(props, c!("one"), c!("1"));
        obs_properties_add_color(props, c!("two"), c!("2"));
        obs_properties_add_color_alpha(props, c!("three"), c!("3"));
        obs_properties_add_font(props, c!("four"), c!("4"));

        // iteration is insertion order
        let expected = ["one", "two", "three", "four"];
        let mut p = obs_properties_first(props);
        for (i, e) in expected.iter().enumerate() {
            assert!(!p.is_null());
            assert_eq!(s(obs_property_name(p)), Some(*e));
            let more = obs_property_next(&mut p);
            assert_eq!(more, i < 3);
        }
        assert!(p.is_null());
        assert!(!obs_property_next(ptr::null_mut()));

        assert_eq!(
            obs_property_get_type(obs_properties_get(props, c!("two"))),
            PROPERTY_COLOR
        );
        assert_eq!(
            obs_property_get_type(obs_properties_get(props, c!("three"))),
            PROPERTY_COLOR_ALPHA
        );
        assert_eq!(
            obs_property_get_type(obs_properties_get(props, c!("four"))),
            PROPERTY_FONT
        );
        assert!(obs_properties_get(props, c!("five")).is_null());

        // duplicate names are refused
        assert!(obs_properties_add_bool(props, c!("one"), c!("again")).is_null());
        assert!(obs_properties_add_int(props, c!("two"), c!("again"), 0, 1, 1).is_null());

        obs_properties_remove_by_name(props, c!("two"));
        assert!(obs_properties_get(props, c!("two")).is_null());
        let mut p = obs_properties_first(props);
        assert_eq!(s(obs_property_name(p)), Some("one"));
        obs_property_next(&mut p);
        assert_eq!(s(obs_property_name(p)), Some("three"));
        // the name is free again
        assert!(!obs_properties_add_bool(props, c!("two"), c!("back")).is_null());
        obs_properties_remove_by_name(props, c!("missing"));
        obs_properties_remove_by_name(ptr::null_mut(), c!("one"));

        obs_properties_destroy(props);
    }
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "scenario table: one block per ported step"
)]
fn test_numbers() {
    unsafe {
        let props = obs_properties_create();
        let i = obs_properties_add_int(props, c!("i"), c!("I"), -5, 10, 2);
        let is = obs_properties_add_int_slider(props, c!("is"), c!("IS"), 0, 100, 5);
        let f = obs_properties_add_float(props, c!("f"), c!("F"), -1.5, 2.5, 0.25);
        let fs = obs_properties_add_float_slider(props, c!("fs"), c!("FS"), 0.0, 1.0, 0.01);

        assert_eq!(obs_property_int_min(i), -5);
        assert_eq!(obs_property_int_max(i), 10);
        assert_eq!(obs_property_int_step(i), 2);
        assert_eq!(obs_property_int_type(i), NUMBER_SCROLLER);
        assert_eq!(obs_property_int_type(is), NUMBER_SLIDER);
        assert!(obs_property_int_suffix(i).is_null());

        assert_eq!(obs_property_float_min(f), -1.5);
        assert_eq!(obs_property_float_max(f), 2.5);
        assert_eq!(obs_property_float_step(f), 0.25);
        assert_eq!(obs_property_float_type(f), NUMBER_SCROLLER);
        assert_eq!(obs_property_float_type(fs), NUMBER_SLIDER);
        assert!(obs_property_float_suffix(f).is_null());

        obs_property_int_set_limits(i, 1, 2, 3);
        assert_eq!(obs_property_int_min(i), 1);
        assert_eq!(obs_property_int_max(i), 2);
        assert_eq!(obs_property_int_step(i), 3);
        obs_property_float_set_limits(f, 0.5, 0.75, 0.125);
        assert_eq!(obs_property_float_min(f), 0.5);
        assert_eq!(obs_property_float_max(f), 0.75);
        assert_eq!(obs_property_float_step(f), 0.125);

        obs_property_int_set_suffix(i, c!(" px"));
        assert_eq!(s(obs_property_int_suffix(i)), Some(" px"));
        obs_property_int_set_suffix(i, c!(" ms"));
        assert_eq!(s(obs_property_int_suffix(i)), Some(" ms"));
        obs_property_int_set_suffix(i, ptr::null());
        assert!(obs_property_int_suffix(i).is_null());
        obs_property_float_set_suffix(f, c!(" %"));
        assert_eq!(s(obs_property_float_suffix(f)), Some(" %"));

        // getters on the wrong type return defaults; setters do nothing
        assert_eq!(obs_property_int_min(f), 0);
        assert_eq!(obs_property_int_type(f), NUMBER_SCROLLER);
        assert_eq!(obs_property_float_max(i), 0.0);
        obs_property_int_set_limits(f, 7, 8, 9);
        assert_eq!(obs_property_float_min(f), 0.5);
        obs_property_float_set_suffix(i, c!("x"));
        assert!(obs_property_int_suffix(i).is_null());

        obs_properties_destroy(props);
    }
}

#[test]
fn test_text_path_editable() {
    unsafe {
        let props = obs_properties_create();
        let t = obs_properties_add_text(props, c!("t"), c!("T"), TEXT_MULTILINE);
        assert_eq!(obs_property_text_type(t), TEXT_MULTILINE);
        assert!(!obs_property_text_monospace(t));
        assert_eq!(obs_property_text_info_type(t), TEXT_INFO_NORMAL);
        assert!(obs_property_text_info_word_wrap(t));
        obs_property_text_set_monospace(t, true);
        obs_property_text_set_info_type(t, TEXT_INFO_ERROR);
        obs_property_text_set_info_word_wrap(t, false);
        assert!(obs_property_text_monospace(t));
        assert_eq!(obs_property_text_info_type(t), TEXT_INFO_ERROR);
        assert!(!obs_property_text_info_word_wrap(t));

        let file = obs_properties_add_path(
            props,
            c!("file"),
            c!("File"),
            PATH_FILE,
            c!("*.png"),
            c!("/tmp"),
        );
        assert_eq!(obs_property_path_type(file), PATH_FILE);
        assert_eq!(s(obs_property_path_filter(file)), Some("*.png"));
        assert_eq!(s(obs_property_path_default_path(file)), Some("/tmp"));

        // characterized, not endorsed: a directory path drops its filter
        let dir = obs_properties_add_path(
            props,
            c!("dir"),
            c!("Dir"),
            PATH_DIRECTORY,
            c!("*.png"),
            ptr::null(),
        );
        assert_eq!(obs_property_path_type(dir), PATH_DIRECTORY);
        assert!(obs_property_path_filter(dir).is_null());
        assert!(obs_property_path_default_path(dir).is_null());

        let save = obs_properties_add_path(
            props,
            c!("save"),
            c!("Save"),
            PATH_FILE_SAVE,
            ptr::null(),
            c!("/out"),
        );
        assert_eq!(obs_property_path_type(save), PATH_FILE_SAVE);
        assert!(obs_property_path_filter(save).is_null());
        assert_eq!(s(obs_property_path_default_path(save)), Some("/out"));

        let el = obs_properties_add_editable_list(
            props,
            c!("el"),
            c!("EL"),
            EDITABLE_LIST_TYPE_FILES,
            c!("*.mp4"),
            c!("/videos"),
        );
        assert_eq!(
            obs_property_editable_list_type(el),
            EDITABLE_LIST_TYPE_FILES
        );
        assert_eq!(s(obs_property_editable_list_filter(el)), Some("*.mp4"));
        assert_eq!(
            s(obs_property_editable_list_default_path(el)),
            Some("/videos")
        );

        // wrong-type defaults
        assert_eq!(obs_property_text_type(file), TEXT_DEFAULT);
        assert!(obs_property_text_info_word_wrap(file));
        assert_eq!(obs_property_path_type(t), PATH_DIRECTORY);
        assert_eq!(
            obs_property_editable_list_type(t),
            EDITABLE_LIST_TYPE_STRINGS
        );
        assert!(obs_property_editable_list_filter(t).is_null());

        obs_properties_destroy(props);
    }
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "scenario table: one block per ported step"
)]
fn test_list() {
    unsafe {
        let props = obs_properties_create();

        // an editable combo must use the string format
        assert!(
            obs_properties_add_list(
                props,
                c!("bad"),
                c!("Bad"),
                COMBO_TYPE_EDITABLE,
                COMBO_FORMAT_INT
            )
            .is_null()
        );
        assert!(obs_properties_get(props, c!("bad")).is_null());

        let sl = obs_properties_add_list(
            props,
            c!("s"),
            c!("S"),
            COMBO_TYPE_EDITABLE,
            COMBO_FORMAT_STRING,
        );
        assert_eq!(obs_property_list_type(sl), COMBO_TYPE_EDITABLE);
        assert_eq!(obs_property_list_format(sl), COMBO_FORMAT_STRING);

        assert_eq!(obs_property_list_add_string(sl, c!("A"), c!("a")), 0);
        assert_eq!(obs_property_list_add_string(sl, c!("C"), c!("c")), 1);
        obs_property_list_insert_string(sl, 1, c!("B"), c!("b"));
        assert_eq!(obs_property_list_item_count(sl), 3);
        assert_eq!(s(obs_property_list_item_name(sl, 1)), Some("B"));
        assert_eq!(s(obs_property_list_item_string(sl, 1)), Some("b"));
        assert_eq!(s(obs_property_list_item_string(sl, 2)), Some("c"));

        // wrong format: nothing added, 0 returned
        assert_eq!(obs_property_list_add_int(sl, c!("X"), 5), 0);
        assert_eq!(obs_property_list_item_count(sl), 3);
        assert_eq!(obs_property_list_item_int(sl, 0), 0);

        assert!(!obs_property_list_item_disabled(sl, 2));
        obs_property_list_item_disable(sl, 2, true);
        assert!(obs_property_list_item_disabled(sl, 2));
        obs_property_list_item_disable(sl, 9, true);
        assert!(!obs_property_list_item_disabled(sl, 9));

        obs_property_list_item_remove(sl, 0);
        obs_property_list_item_remove(sl, 9);
        assert_eq!(obs_property_list_item_count(sl), 2);
        assert_eq!(s(obs_property_list_item_name(sl, 0)), Some("B"));
        assert!(obs_property_list_item_disabled(sl, 1));
        assert!(obs_property_list_item_name(sl, 2).is_null());
        assert!(obs_property_list_item_string(sl, 2).is_null());

        obs_property_list_clear(sl);
        assert_eq!(obs_property_list_item_count(sl), 0);
        assert_eq!(obs_property_list_add_string(sl, c!("Z"), c!("z")), 0);

        let i = obs_properties_add_list(props, c!("i"), c!("I"), COMBO_TYPE_LIST, COMBO_FORMAT_INT);
        assert_eq!(obs_property_list_add_int(i, c!("big"), 1 << 40), 0);
        obs_property_list_insert_int(i, 0, c!("neg"), -7);
        assert_eq!(obs_property_list_item_int(i, 0), -7);
        assert_eq!(obs_property_list_item_int(i, 1), 1 << 40);
        assert!(obs_property_list_item_string(i, 0).is_null());

        let f = obs_properties_add_list(
            props,
            c!("f"),
            c!("F"),
            COMBO_TYPE_RADIO,
            COMBO_FORMAT_FLOAT,
        );
        assert_eq!(obs_property_list_add_float(f, c!("half"), 0.5), 0);
        obs_property_list_insert_float(f, 0, c!("quarter"), 0.25);
        assert_eq!(obs_property_list_item_float(f, 0), 0.25);
        assert_eq!(obs_property_list_item_float(f, 1), 0.5);

        // characterized, not endorsed: the bool getter reads the union's
        // double member, which is non-zero exactly when the stored bool is
        // true
        let b =
            obs_properties_add_list(props, c!("b"), c!("B"), COMBO_TYPE_LIST, COMBO_FORMAT_BOOL);
        assert_eq!(obs_property_list_add_bool(b, c!("yes"), true), 0);
        assert_eq!(obs_property_list_add_bool(b, c!("no"), false), 1);
        obs_property_list_insert_bool(b, 0, c!("first"), true);
        assert!(obs_property_list_item_bool(b, 0));
        assert!(obs_property_list_item_bool(b, 1));
        assert!(!obs_property_list_item_bool(b, 2));

        // not a list
        let nl = obs_properties_add_bool(props, c!("nl"), c!("NL"));
        assert_eq!(obs_property_list_type(nl), COMBO_TYPE_INVALID);
        assert_eq!(obs_property_list_format(nl), COMBO_FORMAT_INVALID);
        assert_eq!(obs_property_list_item_count(nl), 0);
        assert_eq!(obs_property_list_add_string(nl, c!("A"), c!("a")), 0);

        obs_properties_destroy(props);
    }
}

#[test]
fn test_frame_rate() {
    unsafe {
        let props = obs_properties_create();
        let fr = obs_properties_add_frame_rate(props, c!("fr"), c!("FR"));
        let f30 = Fps {
            numerator: 30,
            denominator: 1,
        };
        let f60 = Fps {
            numerator: 60,
            denominator: 1,
        };
        let f24 = Fps {
            numerator: 24000,
            denominator: 1001,
        };

        assert_eq!(
            obs_property_frame_rate_option_add(fr, c!("auto"), c!("Auto")),
            0
        );
        assert_eq!(
            obs_property_frame_rate_option_add(fr, c!("max"), c!("Max")),
            1
        );
        obs_property_frame_rate_option_insert(fr, 0, c!("first"), c!("First"));
        assert_eq!(obs_property_frame_rate_options_count(fr), 3);
        assert_eq!(s(obs_property_frame_rate_option_name(fr, 0)), Some("first"));
        assert_eq!(
            s(obs_property_frame_rate_option_description(fr, 2)),
            Some("Max")
        );
        assert!(obs_property_frame_rate_option_name(fr, 3).is_null());

        assert_eq!(obs_property_frame_rate_fps_range_add(fr, f30, f60), 0);
        obs_property_frame_rate_fps_range_insert(fr, 0, f24, f30);
        assert_eq!(obs_property_frame_rate_fps_ranges_count(fr), 2);
        assert_eq!(
            obs_property_frame_rate_fps_range_min(fr, 0).numerator,
            24000
        );
        assert_eq!(
            obs_property_frame_rate_fps_range_min(fr, 0).denominator,
            1001
        );
        assert_eq!(obs_property_frame_rate_fps_range_max(fr, 1).numerator, 60);
        assert_eq!(obs_property_frame_rate_fps_range_max(fr, 5).numerator, 0);
        assert_eq!(obs_property_frame_rate_fps_range_max(fr, 5).denominator, 0);

        obs_property_frame_rate_options_clear(fr);
        assert_eq!(obs_property_frame_rate_options_count(fr), 0);
        assert_eq!(obs_property_frame_rate_fps_ranges_count(fr), 2);
        obs_property_frame_rate_fps_ranges_clear(fr);
        assert_eq!(obs_property_frame_rate_fps_ranges_count(fr), 0);

        obs_property_frame_rate_option_add(fr, c!("again"), c!("Again"));
        obs_property_frame_rate_fps_range_add(fr, f30, f30);
        obs_property_frame_rate_clear(fr);
        assert_eq!(obs_property_frame_rate_options_count(fr), 0);
        assert_eq!(obs_property_frame_rate_fps_ranges_count(fr), 0);

        // not a frame rate
        let nf = obs_properties_add_bool(props, c!("nf"), c!("NF"));
        assert_eq!(
            obs_property_frame_rate_option_add(nf, c!("x"), c!("X")),
            DARRAY_INVALID
        );
        assert_eq!(
            obs_property_frame_rate_fps_range_add(nf, f30, f60),
            DARRAY_INVALID
        );
        assert_eq!(obs_property_frame_rate_options_count(nf), 0);

        obs_properties_destroy(props);
    }
}

#[test]
fn test_button() {
    unsafe {
        let props = obs_properties_create();
        let mut priv_val = 0;
        let priv_ = (&raw mut priv_val).cast::<c_void>();
        let b1 = obs_properties_add_button(props, c!("b1"), c!("Click"), Some(clicked_cb));
        let b2 =
            obs_properties_add_button2(props, c!("b2"), c!("Click 2"), Some(clicked_cb), priv_);

        assert_eq!(s(obs_property_description(b1)), Some("Click"));
        assert_eq!(obs_property_button_type(b1), BUTTON_DEFAULT);
        // a button without a URL returns NULL ...
        assert!(obs_property_button_url(b1).is_null());
        obs_property_button_set_type(b1, BUTTON_URL);
        obs_property_button_set_url(b1, c!("https://obsproject.com").cast_mut());
        assert_eq!(obs_property_button_type(b1), BUTTON_URL);
        assert_eq!(
            s(obs_property_button_url(b1)),
            Some("https://obsproject.com")
        );

        // ... but a non-button returns "" (characterized, not endorsed)
        let nb = obs_properties_add_bool(props, c!("nb"), c!("NB"));
        assert_eq!(s(obs_property_button_url(nb)), Some(""));
        assert_eq!(obs_property_button_type(nb), BUTTON_DEFAULT);
        assert!(!obs_property_button_clicked(nb, ptr::null_mut()));

        // without priv, the callback gets the context data; a NULL context
        // gives NULL
        reset_calls();
        assert!(obs_property_button_clicked(b1, ptr::null_mut()));
        let c = calls();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].0, "b1");
        assert_eq!(c[0].1, props);
        assert!(c[0].2.is_null());

        // with priv, the callback gets priv
        reset_calls();
        assert!(obs_property_button_clicked(b2, ptr::null_mut()));
        assert_eq!(calls()[0].2, priv_);

        // no callback
        let b3 = obs_properties_add_button(props, c!("b3"), c!("None"), None);
        assert!(!obs_property_button_clicked(b3, ptr::null_mut()));
        assert!(!obs_property_button_clicked(
            ptr::null_mut(),
            ptr::null_mut()
        ));

        obs_properties_destroy(props);
    }
}

#[test]
fn test_groups() {
    unsafe {
        let props = obs_properties_create();
        let inner = obs_properties_create();
        obs_properties_add_bool(props, c!("top"), c!("Top"));
        obs_properties_add_int(inner, c!("in"), c!("In"), 0, 1, 1);

        let g = obs_properties_add_group(props, c!("g"), c!("G"), GROUP_CHECKABLE, inner);
        assert!(!g.is_null());
        assert_eq!(obs_property_get_type(g), PROPERTY_GROUP);
        assert_eq!(obs_property_group_type(g), GROUP_CHECKABLE);
        assert_eq!(obs_property_group_content(g), inner);
        assert_eq!(obs_properties_get_parent(inner), props);
        assert!(obs_properties_get_parent(props).is_null());

        // lookup reaches into groups
        let inp = obs_properties_get(props, c!("in"));
        assert!(!inp.is_null());
        assert_eq!(obs_property_get_type(inp), PROPERTY_INT);

        // names are unique across the whole tree, from either level
        assert!(obs_properties_add_bool(props, c!("in"), c!("dup")).is_null());
        assert!(obs_properties_add_bool(inner, c!("top"), c!("dup")).is_null());
        assert!(obs_properties_add_bool(inner, c!("g"), c!("dup")).is_null());
        assert!(!obs_properties_add_bool(inner, c!("in2"), c!("In 2")).is_null());
        assert!(!obs_properties_get(props, c!("in2")).is_null());

        // a group whose names clash with the parent is refused
        let clash = obs_properties_create();
        obs_properties_add_bool(clash, c!("top"), c!("Top"));
        assert!(obs_properties_add_group(props, c!("g2"), c!("G2"), GROUP_NORMAL, clash).is_null());
        obs_properties_destroy(clash);

        // recursion and self-groups are refused
        assert!(
            obs_properties_add_group(props, c!("self"), c!("Self"), GROUP_NORMAL, props).is_null()
        );
        assert!(
            obs_properties_add_group(props, c!("null"), c!("Null"), GROUP_NORMAL, ptr::null_mut())
                .is_null()
        );

        // characterized, not endorsed: a non-group's group type is
        // OBS_COMBO_INVALID, the list enum's 0
        assert_eq!(
            obs_property_group_type(obs_properties_get(props, c!("top"))),
            COMBO_INVALID
        );
        assert!(obs_property_group_content(obs_properties_get(props, c!("top"))).is_null());

        // removal reaches into groups
        obs_properties_remove_by_name(props, c!("in"));
        assert!(obs_properties_get(props, c!("in")).is_null());
        assert!(!obs_properties_get(inner, c!("in2")).is_null());

        // removing the group destroys its content and frees its names
        obs_properties_remove_by_name(props, c!("g"));
        assert!(obs_properties_get(props, c!("in2")).is_null());
        assert!(!obs_properties_add_bool(props, c!("in2"), c!("free again")).is_null());

        obs_properties_destroy(props);
    }
}

#[test]
fn test_modified_callbacks() {
    unsafe {
        let settings = obs_data::ffi::obs_data::obs_data_create();
        let props = obs_properties_create();
        let inner = obs_properties_create();
        let mut priv_val = 0;
        let priv_ = (&raw mut priv_val).cast::<c_void>();

        let a = obs_properties_add_bool(props, c!("a"), c!("A"));
        obs_properties_add_bool(props, c!("plain"), c!("Plain"));
        let inp = obs_properties_add_bool(inner, c!("in"), c!("In"));
        let g = obs_properties_add_group(props, c!("g"), c!("G"), GROUP_NORMAL, inner);
        let z = obs_properties_add_bool(props, c!("z"), c!("Z"));

        obs_property_set_modified_callback(a, Some(modified_cb));
        obs_property_set_modified_callback2(inp, Some(modified2_cb), priv_);
        obs_property_set_modified_callback(g, Some(modified_cb));
        obs_property_set_modified_callback2(z, Some(modified2_cb), priv_);

        // obs_property_modified returns the callback's result and passes the
        // topmost properties, also for a property inside a group
        reset_calls();
        assert!(obs_property_modified(a, settings));
        assert!(!obs_property_modified(inp, settings));
        assert!(!obs_property_modified(
            obs_properties_get(props, c!("plain")),
            settings
        ));
        assert!(!obs_property_modified(ptr::null_mut(), settings));
        let c = calls();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].1, props);
        assert_eq!(c[1].1, props);
        assert_eq!(c[1].2, priv_);

        // characterized, not endorsed: apply_settings collects a group's
        // content before the group itself, then calls back in reverse
        reset_calls();
        obs_properties_apply_settings(props, settings);
        let names: Vec<String> = calls().into_iter().map(|c| c.0).collect();
        assert_eq!(names, ["z", "g", "in", "a"]);

        // the callbacks get the properties apply_settings was called on, not
        // the topmost ones
        reset_calls();
        obs_properties_apply_settings(inner, settings);
        let c = calls();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].0, "in");
        assert_eq!(c[0].1, inner);

        // modified takes precedence over modified2
        obs_property_set_modified_callback(z, Some(modified_cb));
        reset_calls();
        assert!(obs_property_modified(z, settings));
        assert!(calls()[0].2.is_null());

        obs_properties_apply_settings(ptr::null_mut(), settings);

        obs_properties_destroy(props);
        obs_data::ffi::obs_data::obs_data_release(settings);
    }
}
