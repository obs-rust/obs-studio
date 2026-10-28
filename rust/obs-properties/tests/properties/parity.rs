//! Tier 3: random sequences of `obs_properties_*` / `obs_property_*` calls
//! run on the Rust C ABI and on the original `libobs/obs-properties.c`
//! (compiled as `oracle_*`), comparing every pointer result, return value,
//! callback (name, set, priv) and a full snapshot of the tree after each
//! call. The snapshot calls every getter on every property, so wrong-type
//! defaults are compared too.
//!
//! The sequences avoid the inputs where C crashes or leaks (null names,
//! inserts past the end, a file-save path's filter, a second button URL);
//! those are the intentional differences documented in the shims.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.
#![allow(clippy::missing_safety_doc, clippy::new_without_default)]

use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_void};
use std::ptr;

use obs_util as _; // bmem, base log handler
use proptest::prelude::*;

mod c {
    pub use obs_c_oracle::obs_properties::media_frames_per_second as Fps;
    pub use obs_c_oracle::obs_properties::{
        oracle_obs_properties_add_bool as obs_properties_add_bool,
        oracle_obs_properties_add_button as obs_properties_add_button,
        oracle_obs_properties_add_button2 as obs_properties_add_button2,
        oracle_obs_properties_add_color as obs_properties_add_color,
        oracle_obs_properties_add_color_alpha as obs_properties_add_color_alpha,
        oracle_obs_properties_add_editable_list as obs_properties_add_editable_list,
        oracle_obs_properties_add_float as obs_properties_add_float,
        oracle_obs_properties_add_float_slider as obs_properties_add_float_slider,
        oracle_obs_properties_add_font as obs_properties_add_font,
        oracle_obs_properties_add_frame_rate as obs_properties_add_frame_rate,
        oracle_obs_properties_add_group as obs_properties_add_group,
        oracle_obs_properties_add_int as obs_properties_add_int,
        oracle_obs_properties_add_int_slider as obs_properties_add_int_slider,
        oracle_obs_properties_add_list as obs_properties_add_list,
        oracle_obs_properties_add_path as obs_properties_add_path,
        oracle_obs_properties_add_text as obs_properties_add_text,
        oracle_obs_properties_apply_settings as obs_properties_apply_settings,
        oracle_obs_properties_create as obs_properties_create,
        oracle_obs_properties_create_param as obs_properties_create_param,
        oracle_obs_properties_destroy as obs_properties_destroy,
        oracle_obs_properties_first as obs_properties_first,
        oracle_obs_properties_get as obs_properties_get,
        oracle_obs_properties_get_flags as obs_properties_get_flags,
        oracle_obs_properties_get_param as obs_properties_get_param,
        oracle_obs_properties_get_parent as obs_properties_get_parent,
        oracle_obs_properties_remove_by_name as obs_properties_remove_by_name,
        oracle_obs_properties_set_flags as obs_properties_set_flags,
        oracle_obs_properties_set_param as obs_properties_set_param,
        oracle_obs_property_button_clicked as obs_property_button_clicked,
        oracle_obs_property_button_set_type as obs_property_button_set_type,
        oracle_obs_property_button_set_url as obs_property_button_set_url,
        oracle_obs_property_button_type as obs_property_button_type,
        oracle_obs_property_button_url as obs_property_button_url,
        oracle_obs_property_description as obs_property_description,
        oracle_obs_property_editable_list_default_path as obs_property_editable_list_default_path,
        oracle_obs_property_editable_list_filter as obs_property_editable_list_filter,
        oracle_obs_property_editable_list_type as obs_property_editable_list_type,
        oracle_obs_property_enabled as obs_property_enabled,
        oracle_obs_property_float_max as obs_property_float_max,
        oracle_obs_property_float_min as obs_property_float_min,
        oracle_obs_property_float_set_limits as obs_property_float_set_limits,
        oracle_obs_property_float_set_suffix as obs_property_float_set_suffix,
        oracle_obs_property_float_step as obs_property_float_step,
        oracle_obs_property_float_suffix as obs_property_float_suffix,
        oracle_obs_property_float_type as obs_property_float_type,
        oracle_obs_property_frame_rate_clear as obs_property_frame_rate_clear,
        oracle_obs_property_frame_rate_fps_range_add as obs_property_frame_rate_fps_range_add,
        oracle_obs_property_frame_rate_fps_range_insert as obs_property_frame_rate_fps_range_insert,
        oracle_obs_property_frame_rate_fps_range_max as obs_property_frame_rate_fps_range_max,
        oracle_obs_property_frame_rate_fps_range_min as obs_property_frame_rate_fps_range_min,
        oracle_obs_property_frame_rate_fps_ranges_clear as obs_property_frame_rate_fps_ranges_clear,
        oracle_obs_property_frame_rate_fps_ranges_count as obs_property_frame_rate_fps_ranges_count,
        oracle_obs_property_frame_rate_option_add as obs_property_frame_rate_option_add,
        oracle_obs_property_frame_rate_option_description as obs_property_frame_rate_option_description,
        oracle_obs_property_frame_rate_option_insert as obs_property_frame_rate_option_insert,
        oracle_obs_property_frame_rate_option_name as obs_property_frame_rate_option_name,
        oracle_obs_property_frame_rate_options_clear as obs_property_frame_rate_options_clear,
        oracle_obs_property_frame_rate_options_count as obs_property_frame_rate_options_count,
        oracle_obs_property_get_type as obs_property_get_type,
        oracle_obs_property_group_content as obs_property_group_content,
        oracle_obs_property_group_type as obs_property_group_type,
        oracle_obs_property_int_max as obs_property_int_max,
        oracle_obs_property_int_min as obs_property_int_min,
        oracle_obs_property_int_set_limits as obs_property_int_set_limits,
        oracle_obs_property_int_set_suffix as obs_property_int_set_suffix,
        oracle_obs_property_int_step as obs_property_int_step,
        oracle_obs_property_int_suffix as obs_property_int_suffix,
        oracle_obs_property_int_type as obs_property_int_type,
        oracle_obs_property_list_add_bool as obs_property_list_add_bool,
        oracle_obs_property_list_add_float as obs_property_list_add_float,
        oracle_obs_property_list_add_int as obs_property_list_add_int,
        oracle_obs_property_list_add_string as obs_property_list_add_string,
        oracle_obs_property_list_clear as obs_property_list_clear,
        oracle_obs_property_list_format as obs_property_list_format,
        oracle_obs_property_list_insert_bool as obs_property_list_insert_bool,
        oracle_obs_property_list_insert_float as obs_property_list_insert_float,
        oracle_obs_property_list_insert_int as obs_property_list_insert_int,
        oracle_obs_property_list_insert_string as obs_property_list_insert_string,
        oracle_obs_property_list_item_bool as obs_property_list_item_bool,
        oracle_obs_property_list_item_count as obs_property_list_item_count,
        oracle_obs_property_list_item_disable as obs_property_list_item_disable,
        oracle_obs_property_list_item_disabled as obs_property_list_item_disabled,
        oracle_obs_property_list_item_float as obs_property_list_item_float,
        oracle_obs_property_list_item_int as obs_property_list_item_int,
        oracle_obs_property_list_item_name as obs_property_list_item_name,
        oracle_obs_property_list_item_remove as obs_property_list_item_remove,
        oracle_obs_property_list_item_string as obs_property_list_item_string,
        oracle_obs_property_list_type as obs_property_list_type,
        oracle_obs_property_long_description as obs_property_long_description,
        oracle_obs_property_modified as obs_property_modified,
        oracle_obs_property_name as obs_property_name,
        oracle_obs_property_next as obs_property_next,
        oracle_obs_property_path_default_path as obs_property_path_default_path,
        oracle_obs_property_path_filter as obs_property_path_filter,
        oracle_obs_property_path_type as obs_property_path_type,
        oracle_obs_property_set_description as obs_property_set_description,
        oracle_obs_property_set_enabled as obs_property_set_enabled,
        oracle_obs_property_set_long_description as obs_property_set_long_description,
        oracle_obs_property_set_modified_callback as obs_property_set_modified_callback,
        oracle_obs_property_set_modified_callback2 as obs_property_set_modified_callback2,
        oracle_obs_property_set_visible as obs_property_set_visible,
        oracle_obs_property_text_info_type as obs_property_text_info_type,
        oracle_obs_property_text_info_word_wrap as obs_property_text_info_word_wrap,
        oracle_obs_property_text_monospace as obs_property_text_monospace,
        oracle_obs_property_text_set_info_type as obs_property_text_set_info_type,
        oracle_obs_property_text_set_info_word_wrap as obs_property_text_set_info_word_wrap,
        oracle_obs_property_text_set_monospace as obs_property_text_set_monospace,
        oracle_obs_property_text_type as obs_property_text_type,
        oracle_obs_property_visible as obs_property_visible,
    };
}

mod rs {
    pub use obs_properties::ffi::obs_properties::*;
    pub use obs_properties::props::Fps;
}

/* ------------------------------------------------------------------------- */
/* operations */

const NAMES: [&CStr; 8] = [c"a", c"b", c"c", c"d", c"e", c"f", c"g", c"h"];
/// Where an operation runs: the top set, or the content of group `NAMES[i]`.
const TOP: usize = usize::MAX;

#[derive(Debug, Clone)]
pub enum Op {
    Bool(usize, usize),
    Int(usize, usize, i32, i32, i32, bool),
    Float(usize, usize, f64, f64, f64, bool),
    Text(usize, usize, i32),
    Path(usize, usize, i32, bool, bool),
    List(usize, usize, i32, i32),
    Color(usize, usize, bool),
    Font(usize, usize),
    Button(usize, usize, bool),
    Editable(usize, usize, i32, bool, bool),
    FrameRate(usize, usize),
    /// A group named `name` whose new content holds bools named `inner`.
    Group(usize, usize, i32, Vec<usize>, bool),
    OldButton(usize, usize),
    Flags(usize, u32),
    Param(usize, Option<u8>),
    Remove(usize, usize),
    Visible(usize, bool),
    Enabled(usize, bool),
    Desc(usize, Option<u8>),
    LongDesc(usize, Option<u8>),
    IntLimits(usize, i32, i32, i32),
    FloatLimits(usize, f64, f64, f64),
    IntSuffix(usize, Option<u8>),
    FloatSuffix(usize, Option<u8>),
    TextSet(usize, bool, i32, bool),
    ButtonSet(usize, i32, Option<u8>),
    ListAdd(usize, u8, i64),
    ListInsert(usize, usize, u8, i64),
    ListRemove(usize, usize),
    ListDisable(usize, usize, bool),
    ListClear(usize),
    FrAddOption(usize, u8),
    FrInsertOption(usize, usize, u8),
    FrAddRange(usize, u32, u32),
    FrInsertRange(usize, usize, u32, u32),
    FrClear(usize, u8),
    Callback(usize, u8),
    Modified(usize),
    Apply(usize),
    Click(usize),
}

fn short_str() -> impl Strategy<Value = Option<u8>> {
    prop_oneof![Just(None), (0u8..4).prop_map(Some)]
}

fn op() -> impl Strategy<Value = Op> {
    let set = prop_oneof![3 => Just(TOP), 1 => 0usize..8];
    let n = 0usize..8;
    let f = -4.0f64..4.0;
    prop_oneof![
        (set.clone(), n.clone()).prop_map(|(s, n)| Op::Bool(s, n)),
        (
            set.clone(),
            n.clone(),
            -5i32..5,
            -5i32..5,
            -2i32..3,
            any::<bool>()
        )
            .prop_map(|(s, n, a, b, c, sl)| Op::Int(s, n, a, b, c, sl)),
        (
            set.clone(),
            n.clone(),
            f.clone(),
            f.clone(),
            f.clone(),
            any::<bool>()
        )
            .prop_map(|(s, n, a, b, c, sl)| Op::Float(s, n, a, b, c, sl)),
        (set.clone(), n.clone(), 0i32..4).prop_map(|(s, n, t)| Op::Text(s, n, t)),
        (
            set.clone(),
            n.clone(),
            0i32..3,
            any::<bool>(),
            any::<bool>()
        )
            .prop_map(|(s, n, t, a, b)| Op::Path(s, n, t, a, b)),
        (set.clone(), n.clone(), 0i32..4, 0i32..5).prop_map(|(s, n, t, f)| Op::List(s, n, t, f)),
        (set.clone(), n.clone(), any::<bool>()).prop_map(|(s, n, a)| Op::Color(s, n, a)),
        (set.clone(), n.clone()).prop_map(|(s, n)| Op::Font(s, n)),
        (set.clone(), n.clone(), any::<bool>()).prop_map(|(s, n, p)| Op::Button(s, n, p)),
        (
            set.clone(),
            n.clone(),
            0i32..3,
            any::<bool>(),
            any::<bool>()
        )
            .prop_map(|(s, n, t, a, b)| Op::Editable(s, n, t, a, b)),
        (set.clone(), n.clone()).prop_map(|(s, n)| Op::FrameRate(s, n)),
        (
            set.clone(),
            n.clone(),
            0i32..3,
            proptest::collection::vec(0usize..8, 0..3),
            any::<bool>()
        )
            .prop_map(|(s, n, t, inner, param)| Op::Group(s, n, t, inner, param)),
        (set.clone(), n.clone()).prop_map(|(s, n)| Op::OldButton(s, n)),
        (set.clone(), 0u32..4).prop_map(|(s, f)| Op::Flags(s, f)),
        (
            set.clone(),
            prop_oneof![Just(None), (0u8..3).prop_map(Some)]
        )
            .prop_map(|(s, v)| Op::Param(s, v)),
        (set.clone(), n.clone()).prop_map(|(s, n)| Op::Remove(s, n)),
        (n.clone(), any::<bool>()).prop_map(|(n, v)| Op::Visible(n, v)),
        (n.clone(), any::<bool>()).prop_map(|(n, v)| Op::Enabled(n, v)),
        (n.clone(), short_str()).prop_map(|(n, v)| Op::Desc(n, v)),
        (n.clone(), short_str()).prop_map(|(n, v)| Op::LongDesc(n, v)),
        (n.clone(), -5i32..5, -5i32..5, -2i32..3)
            .prop_map(|(n, a, b, c)| Op::IntLimits(n, a, b, c)),
        (n.clone(), f.clone(), f.clone(), f.clone())
            .prop_map(|(n, a, b, c)| Op::FloatLimits(n, a, b, c)),
        (n.clone(), short_str()).prop_map(|(n, v)| Op::IntSuffix(n, v)),
        (n.clone(), short_str()).prop_map(|(n, v)| Op::FloatSuffix(n, v)),
        (n.clone(), any::<bool>(), 0i32..3, any::<bool>())
            .prop_map(|(n, m, i, w)| Op::TextSet(n, m, i, w)),
        (n.clone(), 0i32..2, short_str()).prop_map(|(n, t, u)| Op::ButtonSet(n, t, u)),
        (n.clone(), 0u8..5, -3i64..3).prop_map(|(n, k, v)| Op::ListAdd(n, k, v)),
        (n.clone(), 0usize..4, 0u8..5, -3i64..3)
            .prop_map(|(n, i, k, v)| Op::ListInsert(n, i, k, v)),
        (n.clone(), 0usize..4).prop_map(|(n, i)| Op::ListRemove(n, i)),
        (n.clone(), 0usize..4, any::<bool>()).prop_map(|(n, i, d)| Op::ListDisable(n, i, d)),
        n.clone().prop_map(Op::ListClear),
        (n.clone(), 0u8..4).prop_map(|(n, v)| Op::FrAddOption(n, v)),
        (n.clone(), 0usize..3, 0u8..4).prop_map(|(n, i, v)| Op::FrInsertOption(n, i, v)),
        (n.clone(), 0u32..4, 0u32..4).prop_map(|(n, a, b)| Op::FrAddRange(n, a, b)),
        (n.clone(), 0usize..3, 0u32..4, 0u32..4)
            .prop_map(|(n, i, a, b)| Op::FrInsertRange(n, i, a, b)),
        (n.clone(), 0u8..3).prop_map(|(n, w)| Op::FrClear(n, w)),
        (n.clone(), 0u8..3).prop_map(|(n, w)| Op::Callback(n, w)),
        n.clone().prop_map(Op::Modified),
        set.clone().prop_map(Op::Apply),
        n.prop_map(Op::Click),
    ]
}

const STRS: [&CStr; 4] = [c"", c"x", c"long text", c"*.png"];

fn opt(v: Option<u8>) -> *const c_char {
    v.map_or(ptr::null(), |i| STRS[i as usize].as_ptr())
}

/* ------------------------------------------------------------------------- */
/* callback log, per side */

thread_local! {
    static LOG: RefCell<Vec<(String, usize, usize)>> = const { RefCell::new(Vec::new()) };
}

unsafe fn name_of<F: Fn(*mut c_void) -> *const c_char>(f: F, p: *mut c_void) -> String {
    let n = f(p);
    if n.is_null() {
        "<null>".into()
    } else {
        unsafe { CStr::from_ptr(n) }.to_string_lossy().into_owned()
    }
}

macro_rules! side {
    ($m:ident, $api:ident) => {
        pub mod $m {
            use super::*;

            unsafe extern "C" fn modified_cb(
                props: *mut c_void,
                p: *mut c_void,
                _s: *mut c_void,
            ) -> bool {
                let n = unsafe { name_of(|p| $api::obs_property_name(p), p) };
                LOG.with_borrow_mut(|l| l.push((n, props as usize, 0)));
                true
            }

            unsafe extern "C" fn modified2_cb(
                priv_: *mut c_void,
                props: *mut c_void,
                p: *mut c_void,
                _s: *mut c_void,
            ) -> bool {
                let n = unsafe { name_of(|p| $api::obs_property_name(p), p) };
                LOG.with_borrow_mut(|l| l.push((n, props as usize, priv_ as usize)));
                false
            }

            unsafe extern "C" fn destroy_cb(param: *mut c_void) {
                LOG.with_borrow_mut(|l| l.push(("destroy".into(), 0, param as usize)));
            }

            unsafe extern "C" fn clicked_cb(
                props: *mut c_void,
                p: *mut c_void,
                data: *mut c_void,
            ) -> bool {
                let n = unsafe { name_of(|p| $api::obs_property_name(p), p) };
                LOG.with_borrow_mut(|l| l.push((n, props as usize, data as usize)));
                true
            }

            pub struct World {
                pub top: *mut c_void,
            }

            impl World {
                pub fn new() -> Self {
                    Self {
                        top: unsafe { $api::obs_properties_create() },
                    }
                }

                fn set(&self, s: usize) -> *mut c_void {
                    if s == TOP {
                        return self.top;
                    }
                    unsafe {
                        let g = $api::obs_properties_get(self.top, NAMES[s].as_ptr());
                        $api::obs_property_group_content(g)
                    }
                }

                fn prop(&self, n: usize) -> *mut c_void {
                    unsafe { $api::obs_properties_get(self.top, NAMES[n].as_ptr()) }
                }

                /// Applies `op`; returns the pointer results as "non-null" flags and
                /// the callback log with set pointers made side-independent.
                #[expect(
                    clippy::too_many_lines,
                    reason = "parity table: one block per ported C case"
                )]
                pub fn apply(&mut self, op: &Op) -> Vec<String> {
                    use $api::*;
                    LOG.with_borrow_mut(Vec::clear);
                    let mut out = Vec::new();
                    let nn = |p: *mut c_void| (!p.is_null()).to_string();
                    unsafe {
                        match op.clone() {
                            Op::Bool(s, n) => out.push(nn(obs_properties_add_bool(
                                self.set(s),
                                NAMES[n].as_ptr(),
                                c"B".as_ptr(),
                            ))),
                            Op::Int(s, n, a, b, c, sl) => out.push(nn(if sl {
                                obs_properties_add_int_slider(
                                    self.set(s),
                                    NAMES[n].as_ptr(),
                                    c"I".as_ptr(),
                                    a,
                                    b,
                                    c,
                                )
                            } else {
                                obs_properties_add_int(
                                    self.set(s),
                                    NAMES[n].as_ptr(),
                                    c"I".as_ptr(),
                                    a,
                                    b,
                                    c,
                                )
                            })),
                            Op::Float(s, n, a, b, c, sl) => out.push(nn(if sl {
                                obs_properties_add_float_slider(
                                    self.set(s),
                                    NAMES[n].as_ptr(),
                                    c"F".as_ptr(),
                                    a,
                                    b,
                                    c,
                                )
                            } else {
                                obs_properties_add_float(
                                    self.set(s),
                                    NAMES[n].as_ptr(),
                                    c"F".as_ptr(),
                                    a,
                                    b,
                                    c,
                                )
                            })),
                            Op::Text(s, n, t) => out.push(nn(obs_properties_add_text(
                                self.set(s),
                                NAMES[n].as_ptr(),
                                c"T".as_ptr(),
                                t,
                            ))),
                            // A file-save path keeps no filter here: C leaks it (#831).
                            Op::Path(s, n, t, f, d) => out.push(nn(obs_properties_add_path(
                                self.set(s),
                                NAMES[n].as_ptr(),
                                c"P".as_ptr(),
                                t,
                                if f && t != 1 {
                                    c"*.png".as_ptr()
                                } else {
                                    ptr::null()
                                },
                                if d { c"/tmp".as_ptr() } else { ptr::null() },
                            ))),
                            Op::List(s, n, t, f) => out.push(nn(obs_properties_add_list(
                                self.set(s),
                                NAMES[n].as_ptr(),
                                c"L".as_ptr(),
                                t,
                                f,
                            ))),
                            Op::Color(s, n, a) => out.push(nn(if a {
                                obs_properties_add_color_alpha(
                                    self.set(s),
                                    NAMES[n].as_ptr(),
                                    c"C".as_ptr(),
                                )
                            } else {
                                obs_properties_add_color(
                                    self.set(s),
                                    NAMES[n].as_ptr(),
                                    c"C".as_ptr(),
                                )
                            })),
                            Op::Font(s, n) => out.push(nn(obs_properties_add_font(
                                self.set(s),
                                NAMES[n].as_ptr(),
                                ptr::null(),
                            ))),
                            Op::Button(s, n, p) => out.push(nn(if p {
                                obs_properties_add_button2(
                                    self.set(s),
                                    NAMES[n].as_ptr(),
                                    c"Btn".as_ptr(),
                                    Some(clicked_cb),
                                    7 as *mut c_void,
                                )
                            } else {
                                obs_properties_add_button2(
                                    self.set(s),
                                    NAMES[n].as_ptr(),
                                    c"Btn".as_ptr(),
                                    Some(clicked_cb),
                                    ptr::null_mut(),
                                )
                            })),
                            Op::Editable(s, n, t, f, d) => {
                                out.push(nn(obs_properties_add_editable_list(
                                    self.set(s),
                                    NAMES[n].as_ptr(),
                                    c"E".as_ptr(),
                                    t,
                                    if f { c"*.mp4".as_ptr() } else { ptr::null() },
                                    if d { c"/v".as_ptr() } else { ptr::null() },
                                )))
                            }
                            Op::FrameRate(s, n) => out.push(nn(obs_properties_add_frame_rate(
                                self.set(s),
                                NAMES[n].as_ptr(),
                                c"FR".as_ptr(),
                            ))),
                            Op::Group(s, n, t, inner, param) => {
                                let content = if param {
                                    obs_properties_create_param(
                                        (100 + n) as *mut c_void,
                                        Some(destroy_cb),
                                    )
                                } else {
                                    obs_properties_create()
                                };
                                for i in inner {
                                    obs_properties_add_bool(
                                        content,
                                        NAMES[i].as_ptr(),
                                        c"In".as_ptr(),
                                    );
                                }
                                let p = obs_properties_add_group(
                                    self.set(s),
                                    NAMES[n].as_ptr(),
                                    c"G".as_ptr(),
                                    t,
                                    content,
                                );
                                if p.is_null() {
                                    obs_properties_destroy(content);
                                }
                                out.push(nn(p));
                            }
                            #[allow(deprecated)]
                            Op::OldButton(s, n) => out.push(nn(obs_properties_add_button(
                                self.set(s),
                                NAMES[n].as_ptr(),
                                c"Old".as_ptr(),
                                Some(clicked_cb),
                            ))),
                            Op::Flags(s, f) => obs_properties_set_flags(self.set(s), f),
                            Op::Param(s, v) => {
                                match v {
                                    Some(v) => obs_properties_set_param(
                                        self.set(s),
                                        (10 + v as usize) as *mut c_void,
                                        Some(destroy_cb),
                                    ),
                                    None => {
                                        obs_properties_set_param(self.set(s), ptr::null_mut(), None)
                                    }
                                }
                                out.push(
                                    (obs_properties_get_param(self.set(s)) as usize).to_string(),
                                );
                            }
                            Op::Remove(s, n) => {
                                obs_properties_remove_by_name(self.set(s), NAMES[n].as_ptr())
                            }
                            Op::Visible(n, v) => obs_property_set_visible(self.prop(n), v),
                            Op::Enabled(n, v) => obs_property_set_enabled(self.prop(n), v),
                            Op::Desc(n, v) => obs_property_set_description(self.prop(n), opt(v)),
                            Op::LongDesc(n, v) => {
                                obs_property_set_long_description(self.prop(n), opt(v))
                            }
                            Op::IntLimits(n, a, b, c) => {
                                obs_property_int_set_limits(self.prop(n), a, b, c)
                            }
                            Op::FloatLimits(n, a, b, c) => {
                                obs_property_float_set_limits(self.prop(n), a, b, c)
                            }
                            Op::IntSuffix(n, v) => {
                                obs_property_int_set_suffix(self.prop(n), opt(v))
                            }
                            Op::FloatSuffix(n, v) => {
                                obs_property_float_set_suffix(self.prop(n), opt(v))
                            }
                            Op::TextSet(n, m, i, w) => {
                                obs_property_text_set_monospace(self.prop(n), m);
                                obs_property_text_set_info_type(self.prop(n), i);
                                obs_property_text_set_info_word_wrap(self.prop(n), w);
                            }
                            // Only a first URL: C leaks a replaced one (#831).
                            Op::ButtonSet(n, t, u) => {
                                obs_property_button_set_type(self.prop(n), t);
                                if u.is_some() && obs_property_button_url(self.prop(n)).is_null() {
                                    obs_property_button_set_url(self.prop(n), opt(u).cast_mut());
                                }
                            }
                            Op::ListAdd(n, k, v) => out.push(
                                match k {
                                    0 => obs_property_list_add_string(
                                        self.prop(n),
                                        c"S".as_ptr(),
                                        STRS[(v.rem_euclid(4)) as usize].as_ptr(),
                                    ),
                                    1 => obs_property_list_add_int(self.prop(n), c"I".as_ptr(), v),
                                    2 => obs_property_list_add_float(
                                        self.prop(n),
                                        c"F".as_ptr(),
                                        v as f64 / 2.0,
                                    ),
                                    3 => obs_property_list_add_bool(
                                        self.prop(n),
                                        c"B".as_ptr(),
                                        v > 0,
                                    ),
                                    _ => obs_property_list_add_string(
                                        self.prop(n),
                                        ptr::null(),
                                        ptr::null(),
                                    ),
                                }
                                .to_string(),
                            ),
                            Op::ListInsert(n, i, k, v) => {
                                // C's da_insert asserts idx <= num; stay in range.
                                if i <= obs_property_list_item_count(self.prop(n)) {
                                    match k {
                                        0 => obs_property_list_insert_string(
                                            self.prop(n),
                                            i,
                                            c"S".as_ptr(),
                                            STRS[(v.rem_euclid(4)) as usize].as_ptr(),
                                        ),
                                        1 => obs_property_list_insert_int(
                                            self.prop(n),
                                            i,
                                            c"I".as_ptr(),
                                            v,
                                        ),
                                        2 => obs_property_list_insert_float(
                                            self.prop(n),
                                            i,
                                            c"F".as_ptr(),
                                            v as f64 / 2.0,
                                        ),
                                        3 => obs_property_list_insert_bool(
                                            self.prop(n),
                                            i,
                                            c"B".as_ptr(),
                                            v > 0,
                                        ),
                                        _ => obs_property_list_insert_string(
                                            self.prop(n),
                                            i,
                                            ptr::null(),
                                            ptr::null(),
                                        ),
                                    }
                                }
                            }
                            Op::ListRemove(n, i) => obs_property_list_item_remove(self.prop(n), i),
                            Op::ListDisable(n, i, d) => {
                                obs_property_list_item_disable(self.prop(n), i, d)
                            }
                            Op::ListClear(n) => obs_property_list_clear(self.prop(n)),
                            Op::FrAddOption(n, v) => out.push(
                                obs_property_frame_rate_option_add(
                                    self.prop(n),
                                    opt(Some(v)),
                                    opt(Some(3 - v)),
                                )
                                .to_string(),
                            ),
                            Op::FrInsertOption(n, i, v) => {
                                if i <= obs_property_frame_rate_options_count(self.prop(n)) {
                                    obs_property_frame_rate_option_insert(
                                        self.prop(n),
                                        i,
                                        opt(Some(v)),
                                        ptr::null(),
                                    );
                                }
                            }
                            Op::FrAddRange(n, a, b) => out.push(
                                obs_property_frame_rate_fps_range_add(
                                    self.prop(n),
                                    Fps {
                                        numerator: a,
                                        denominator: 1,
                                    },
                                    Fps {
                                        numerator: b,
                                        denominator: 2,
                                    },
                                )
                                .to_string(),
                            ),
                            Op::FrInsertRange(n, i, a, b) => {
                                if i <= obs_property_frame_rate_fps_ranges_count(self.prop(n)) {
                                    obs_property_frame_rate_fps_range_insert(
                                        self.prop(n),
                                        i,
                                        Fps {
                                            numerator: a,
                                            denominator: 3,
                                        },
                                        Fps {
                                            numerator: b,
                                            denominator: 4,
                                        },
                                    );
                                }
                            }
                            Op::FrClear(n, w) => match w {
                                0 => obs_property_frame_rate_clear(self.prop(n)),
                                1 => obs_property_frame_rate_options_clear(self.prop(n)),
                                _ => obs_property_frame_rate_fps_ranges_clear(self.prop(n)),
                            },
                            Op::Callback(n, w) => match w {
                                0 => obs_property_set_modified_callback(
                                    self.prop(n),
                                    Some(modified_cb),
                                ),
                                1 => obs_property_set_modified_callback2(
                                    self.prop(n),
                                    Some(modified2_cb),
                                    9 as *mut c_void,
                                ),
                                _ => obs_property_set_modified_callback(self.prop(n), None),
                            },
                            Op::Modified(n) => out.push(
                                obs_property_modified(self.prop(n), ptr::null_mut()).to_string(),
                            ),
                            Op::Apply(s) => {
                                obs_properties_apply_settings(self.set(s), ptr::null_mut())
                            }
                            Op::Click(n) => out.push(
                                obs_property_button_clicked(self.prop(n), ptr::null_mut())
                                    .to_string(),
                            ),
                        }
                    }
                    let top = self.top as usize;
                    LOG.with_borrow(|l| {
                        for (name, props, priv_) in l {
                            let which = if *props == top {
                                "top".to_string()
                            } else {
                                format!("set@{}", self.set_name(*props))
                            };
                            out.push(format!("cb {name} {which} {priv_}"));
                        }
                    });
                    out
                }

                /// The group name whose content is `props`.
                fn set_name(&self, props: usize) -> String {
                    for (i, n) in NAMES.iter().enumerate() {
                        let g = unsafe { $api::obs_properties_get(self.top, n.as_ptr()) };
                        if unsafe { $api::obs_property_group_content(g) } as usize == props {
                            return i.to_string();
                        }
                    }
                    "?".into()
                }

                /// Every observable value of the whole tree.
                pub fn snapshot(&self) -> Vec<String> {
                    let mut out = Vec::new();
                    unsafe { self.walk(self.top, 0, &mut out) };
                    out
                }

                unsafe fn walk(&self, props: *mut c_void, depth: usize, out: &mut Vec<String>) {
                    use $api::*;
                    let s = |p: *const c_char| -> String {
                        if p.is_null() {
                            "<null>".into()
                        } else {
                            unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
                        }
                    };
                    unsafe {
                        let parent = obs_properties_get_parent(props);
                        out.push(format!(
                            "set depth={depth} flags={} param={} parent_is_top={}",
                            obs_properties_get_flags(props),
                            obs_properties_get_param(props) as usize,
                            parent as usize == self.top as usize
                        ));
                        let mut p = obs_properties_first(props);
                        while !p.is_null() {
                            out.push(format!(
                                "prop {} desc={} long={} type={} en={} vis={}",
                                s(obs_property_name(p)),
                                s(obs_property_description(p)),
                                s(obs_property_long_description(p)),
                                obs_property_get_type(p),
                                obs_property_enabled(p),
                                obs_property_visible(p),
                            ));
                            out.push(format!(
                                " int {} {} {} {} {} float {:?} {:?} {:?} {} {}",
                                obs_property_int_min(p),
                                obs_property_int_max(p),
                                obs_property_int_step(p),
                                obs_property_int_type(p),
                                s(obs_property_int_suffix(p)),
                                obs_property_float_min(p).to_bits(),
                                obs_property_float_max(p).to_bits(),
                                obs_property_float_step(p).to_bits(),
                                obs_property_float_type(p),
                                s(obs_property_float_suffix(p)),
                            ));
                            out.push(format!(
                                " text {} {} {} {} path {} {} {} edit {} {} {}",
                                obs_property_text_type(p),
                                obs_property_text_monospace(p),
                                obs_property_text_info_type(p),
                                obs_property_text_info_word_wrap(p),
                                obs_property_path_type(p),
                                s(obs_property_path_filter(p)),
                                s(obs_property_path_default_path(p)),
                                obs_property_editable_list_type(p),
                                s(obs_property_editable_list_filter(p)),
                                s(obs_property_editable_list_default_path(p)),
                            ));
                            out.push(format!(
                                " button {} {} list {} {} n={} group {}",
                                obs_property_button_type(p),
                                s(obs_property_button_url(p)),
                                obs_property_list_type(p),
                                obs_property_list_format(p),
                                obs_property_list_item_count(p),
                                obs_property_group_type(p),
                            ));
                            for i in 0..obs_property_list_item_count(p) + 1 {
                                out.push(format!(
                                    "  item {i} {} dis={} s={} i={} f={:?} b={}",
                                    s(obs_property_list_item_name(p, i)),
                                    obs_property_list_item_disabled(p, i),
                                    s(obs_property_list_item_string(p, i)),
                                    obs_property_list_item_int(p, i),
                                    obs_property_list_item_float(p, i).to_bits(),
                                    obs_property_list_item_bool(p, i),
                                ));
                            }
                            for i in 0..obs_property_frame_rate_options_count(p) + 1 {
                                out.push(format!(
                                    "  opt {i} {} {}",
                                    s(obs_property_frame_rate_option_name(p, i)),
                                    s(obs_property_frame_rate_option_description(p, i)),
                                ));
                            }
                            for i in 0..obs_property_frame_rate_fps_ranges_count(p) + 1 {
                                let a = obs_property_frame_rate_fps_range_min(p, i);
                                let b = obs_property_frame_rate_fps_range_max(p, i);
                                out.push(format!(
                                    "  rng {i} {}/{} {}/{}",
                                    a.numerator, a.denominator, b.numerator, b.denominator
                                ));
                            }
                            let content = obs_property_group_content(p);
                            if !content.is_null() {
                                self.walk(content, depth + 1, out);
                            }
                            obs_property_next(&mut p);
                        }
                    }
                }
            }

            impl Drop for World {
                fn drop(&mut self) {
                    unsafe { $api::obs_properties_destroy(self.top) };
                }
            }
        }
    };
}

side!(c_side, c);
side!(rs_side, rs);

fn run(ops: &[Op]) {
    let mut c = c_side::World::new();
    let mut r = rs_side::World::new();
    for (i, op) in ops.iter().enumerate() {
        let co = c.apply(op);
        let ro = r.apply(op);
        assert_eq!(co, ro, "step {i}: {op:?}");
        assert_eq!(c.snapshot(), r.snapshot(), "after step {i}: {op:?}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn matches_c_oracle(ops in proptest::collection::vec(op(), 1..40)) {
        run(&ops);
    }
}

/// The characterization test's group and callback scenario, as one fixed
/// sequence.
#[test]
fn groups_and_callbacks_match() {
    run(&[
        Op::Bool(TOP, 0),
        Op::Bool(TOP, 1),
        Op::Group(TOP, 6, 1, vec![2, 3], true),
        Op::Bool(TOP, 7),
        Op::Callback(0, 0),
        Op::Callback(2, 1),
        Op::Callback(6, 0),
        Op::Callback(7, 1),
        Op::Modified(0),
        Op::Modified(2),
        Op::Apply(TOP),
        Op::Apply(6),
        Op::Bool(6, 3),
        Op::Bool(6, 4),
        Op::Remove(TOP, 2),
        Op::Remove(TOP, 6),
        Op::Bool(TOP, 4),
    ]);
}
