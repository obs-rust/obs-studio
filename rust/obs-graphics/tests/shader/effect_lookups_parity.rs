//! Tier 3: the Rust shims answer exactly like the original
//! `libobs/graphics/effect-lookups.c.h`, compiled as `oracle_*`.
//!
//! Annotation and pass index == num is one past the array. That return is
//! characterized, not endorsed: the test compares the pointer and does not
//! dereference it. A null array plus a zero offset is null, which is what
//! an empty annotation list and an empty pass list return.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::{CString, c_char, c_void};
use std::ptr;

use obs_c_oracle::effect_lookups as c;
use obs_graphics::ffi::effect_lookups as rs;
use obs_graphics::ffi::effect_lookups::{
    gs_effect, gs_effect_param, gs_effect_param_info, gs_effect_pass, gs_effect_technique,
};
use obs_graphics::shader_param_names::param;
use obs_util::ffi::darray::darray;

fn empty_da() -> darray {
    darray {
        array: ptr::null_mut(),
        num: 0,
        capacity: 0,
    }
}

/// A null pointer when `items` is empty, matching `da_init`. An empty Rust
/// slice's `as_ptr` is non-null, and the C empty array is not.
fn da_of<T>(items: &[T]) -> darray {
    darray {
        array: if items.is_empty() {
            ptr::null_mut()
        } else {
            items.as_ptr() as *mut c_void
        },
        num: items.len(),
        capacity: items.len(),
    }
}

fn cstr(bytes: &[u8]) -> CString {
    CString::new(bytes).expect("name has no interior NUL")
}

fn name_ptr(s: &CString) -> *mut c_char {
    s.as_ptr() as *mut c_char
}

fn param_of(name: *mut c_char, type_: i32, annotations: darray) -> gs_effect_param {
    gs_effect_param {
        name,
        section: 0,
        type_,
        changed: false,
        cur_val: empty_da(),
        default_val: empty_da(),
        effect: ptr::null_mut(),
        next_sampler: ptr::null_mut(),
        annotations,
    }
}

fn pass_of(name: *mut c_char) -> gs_effect_pass {
    gs_effect_pass {
        name,
        section: 0,
        vertshader: ptr::null_mut(),
        pixelshader: ptr::null_mut(),
        vertshader_params: empty_da(),
        pixelshader_params: empty_da(),
    }
}

fn technique_of(name: *mut c_char, passes: darray) -> gs_effect_technique {
    gs_effect_technique {
        name,
        section: 0,
        effect: ptr::null_mut(),
        passes,
    }
}

fn effect_of(params: &[gs_effect_param], techniques: &[gs_effect_technique]) -> gs_effect {
    gs_effect {
        processing: false,
        cached: false,
        effect_path: ptr::null_mut(),
        effect_dir: ptr::null_mut(),
        params: da_of(params),
        techniques: da_of(techniques),
        cur_technique: ptr::null_mut(),
        cur_pass: ptr::null_mut(),
        view_proj: ptr::null_mut(),
        world: ptr::null_mut(),
        scale: ptr::null_mut(),
        graphics: ptr::null_mut(),
        next: ptr::null_mut(),
        loop_pass: 0,
        looping: false,
    }
}

fn same(label: &str, rust_p: *mut c_void, oracle_p: *mut c_void) {
    assert_eq!(rust_p, oracle_p, "{label}");
}

#[test]
fn null_owners_match_oracle() {
    let draw = cstr(b"Draw");
    let color = cstr(b"color");
    let widget = cstr(b"widget");
    let p0 = cstr(b"p0");
    let effect = ptr::null();
    // SAFETY: null owners. The name pointers are C strings and are not read.
    unsafe {
        same(
            "technique",
            rs::gs_effect_get_technique(effect, draw.as_ptr()) as *mut c_void,
            c::oracle_gs_effect_get_technique(effect as *const c_void, draw.as_ptr()),
        );
        same(
            "current",
            rs::gs_effect_get_current_technique(effect) as *mut c_void,
            c::oracle_gs_effect_get_current_technique(effect as *const c_void),
        );
        assert_eq!(
            rs::gs_effect_get_num_params(effect),
            c::oracle_gs_effect_get_num_params(effect as *const c_void)
        );
        same(
            "param idx",
            rs::gs_effect_get_param_by_idx(effect, 0) as *mut c_void,
            c::oracle_gs_effect_get_param_by_idx(effect as *const c_void, 0),
        );
        same(
            "param name",
            rs::gs_effect_get_param_by_name(effect, color.as_ptr()) as *mut c_void,
            c::oracle_gs_effect_get_param_by_name(effect as *const c_void, color.as_ptr()),
        );
        same(
            "view",
            rs::gs_effect_get_viewproj_matrix(effect) as *mut c_void,
            c::oracle_gs_effect_get_viewproj_matrix(effect as *const c_void),
        );
        same(
            "world",
            rs::gs_effect_get_world_matrix(effect) as *mut c_void,
            c::oracle_gs_effect_get_world_matrix(effect as *const c_void),
        );
        assert_eq!(
            rs::gs_param_get_num_annotations(ptr::null()),
            c::oracle_gs_param_get_num_annotations(ptr::null())
        );
        same(
            "annotation idx",
            rs::gs_param_get_annotation_by_idx(ptr::null(), 0) as *mut c_void,
            c::oracle_gs_param_get_annotation_by_idx(ptr::null(), 0),
        );
        same(
            "annotation name",
            rs::gs_param_get_annotation_by_name(ptr::null(), widget.as_ptr()) as *mut c_void,
            c::oracle_gs_param_get_annotation_by_name(ptr::null(), widget.as_ptr()),
        );
        same(
            "pass idx",
            rs::gs_technique_get_pass_by_idx(ptr::null(), 0) as *mut c_void,
            c::oracle_gs_technique_get_pass_by_idx(ptr::null(), 0),
        );
        same(
            "pass name",
            rs::gs_technique_get_pass_by_name(ptr::null(), p0.as_ptr()) as *mut c_void,
            c::oracle_gs_technique_get_pass_by_name(ptr::null(), p0.as_ptr()),
        );
    }

    let sentinel = cstr(b"sentinel");
    let mut info_r = gs_effect_param_info {
        name: sentinel.as_ptr(),
        type_: param::TEXTURE,
    };
    let mut info_c = info_r;
    // SAFETY: a null param does not read `info`.
    unsafe {
        rs::gs_effect_get_param_info(ptr::null(), &mut info_r);
        c::oracle_gs_effect_get_param_info(ptr::null(), &mut info_c as *mut _ as *mut c_void);
    }
    assert_eq!(info_r.name, info_c.name);
    assert_eq!(info_r.type_, info_c.type_);
    assert_eq!(info_r.name, sentinel.as_ptr());
    assert_eq!(info_r.type_, param::TEXTURE);
}

#[test]
#[expect(
    clippy::cognitive_complexity,
    reason = "parity table: one block per ported C case"
)]
#[expect(
    clippy::too_many_lines,
    reason = "parity table: one block per ported C case"
)]
fn lookups_match_oracle() {
    let param_names = [
        cstr(b"color"),
        cstr(b"Color"),
        cstr(b"color"),
        cstr(b""),
        cstr(b"c\xfflor"),
    ];
    let ann_names = [cstr(b"ui"), cstr(b"ui"), cstr(b"widget")];
    let tech_names = [cstr(b"Draw"), cstr(b"Draw"), cstr(b"Solid"), cstr(b"")];
    let pass_names = [cstr(b"p0"), cstr(b"P0"), cstr(b"p0")];

    let anns = vec![
        param_of(name_ptr(&ann_names[0]), param::STRING, empty_da()),
        param_of(name_ptr(&ann_names[1]), param::BOOL, empty_da()),
        param_of(name_ptr(&ann_names[2]), param::STRING, empty_da()),
    ];
    let mut params = vec![
        param_of(name_ptr(&param_names[0]), param::VEC4, da_of(&anns)),
        param_of(name_ptr(&param_names[1]), param::FLOAT, empty_da()),
        param_of(name_ptr(&param_names[2]), param::INT, empty_da()),
        param_of(name_ptr(&param_names[3]), param::STRING, empty_da()),
        param_of(name_ptr(&param_names[4]), param::STRING, empty_da()),
    ];
    let passes = vec![
        pass_of(name_ptr(&pass_names[0])),
        pass_of(name_ptr(&pass_names[1])),
        pass_of(name_ptr(&pass_names[2])),
    ];
    let techniques = vec![
        technique_of(name_ptr(&tech_names[0]), da_of(&passes)),
        technique_of(name_ptr(&tech_names[1]), empty_da()),
        technique_of(name_ptr(&tech_names[2]), empty_da()),
        technique_of(name_ptr(&tech_names[3]), empty_da()),
    ];
    let mut effect = effect_of(&params, &techniques);
    let effect_p = &effect as *const gs_effect;

    let query = |bytes: &[u8]| cstr(bytes);
    let color = query(b"color");
    let color_upper = query(b"Color");
    let empty = query(b"");
    let prefix = query(b"col");
    let missing = query(b"missing");
    let raw = query(b"c\xfflor");
    let ui = query(b"ui");
    let widget = query(b"widget");
    let ui_upper = query(b"UI");
    let draw = query(b"Draw");
    let solid = query(b"Solid");
    let draw_lower = query(b"draw");
    let p0 = query(b"p0");
    let p0_upper = query(b"P0");
    let p1 = query(b"p1");

    // SAFETY: the fixture is initialized, names are C strings, and the calls
    // do not write through the returned one-past pointers.
    unsafe {
        let param_name = |label: &str, name: &CString| {
            same(
                label,
                rs::gs_effect_get_param_by_name(effect_p, name.as_ptr()) as *mut c_void,
                c::oracle_gs_effect_get_param_by_name(effect_p as *const c_void, name.as_ptr()),
            );
        };
        param_name("color", &color);
        param_name("Color", &color_upper);
        param_name("empty param", &empty);
        param_name("prefix", &prefix);
        param_name("missing param", &missing);
        param_name("raw", &raw);
        assert_eq!(
            rs::gs_effect_get_param_by_name(effect_p, color.as_ptr()),
            params.as_mut_ptr()
        );
        assert_ne!(
            rs::gs_effect_get_param_by_name(effect_p, color.as_ptr()),
            params.as_mut_ptr().wrapping_add(2)
        );

        let ann = params.as_ptr();
        let ann_name = |label: &str, name: &CString| {
            same(
                label,
                rs::gs_param_get_annotation_by_name(ann, name.as_ptr()) as *mut c_void,
                c::oracle_gs_param_get_annotation_by_name(ann as *const c_void, name.as_ptr()),
            );
        };
        ann_name("ui", &ui);
        ann_name("widget", &widget);
        ann_name("UI", &ui_upper);
        ann_name("annotation color", &color);
        assert_eq!(
            rs::gs_param_get_annotation_by_name(ann, ui.as_ptr()),
            anns.as_ptr() as *mut gs_effect_param
        );
        same(
            "bare annotation",
            rs::gs_param_get_annotation_by_name(params.as_ptr().wrapping_add(1), ui.as_ptr())
                as *mut c_void,
            c::oracle_gs_param_get_annotation_by_name(
                params.as_ptr().wrapping_add(1) as *const c_void,
                ui.as_ptr(),
            ),
        );

        let tech_name = |label: &str, name: &CString| {
            same(
                label,
                rs::gs_effect_get_technique(effect_p, name.as_ptr()) as *mut c_void,
                c::oracle_gs_effect_get_technique(effect_p as *const c_void, name.as_ptr()),
            );
        };
        tech_name("Draw", &draw);
        tech_name("Solid", &solid);
        tech_name("empty tech", &empty);
        tech_name("draw", &draw_lower);
        tech_name("missing tech", &missing);
        assert_eq!(
            rs::gs_effect_get_technique(effect_p, draw.as_ptr()),
            techniques.as_ptr() as *mut gs_effect_technique
        );

        let draw_tech = techniques.as_ptr();
        let pass_name = |label: &str, name: &CString| {
            same(
                label,
                rs::gs_technique_get_pass_by_name(draw_tech, name.as_ptr()) as *mut c_void,
                c::oracle_gs_technique_get_pass_by_name(draw_tech as *const c_void, name.as_ptr()),
            );
        };
        pass_name("p0", &p0);
        pass_name("P0", &p0_upper);
        pass_name("p1", &p1);
        assert_eq!(
            rs::gs_technique_get_pass_by_name(draw_tech, p0.as_ptr()),
            passes.as_ptr() as *mut gs_effect_pass
        );
        same(
            "solid has no p0",
            rs::gs_technique_get_pass_by_name(techniques.as_ptr().wrapping_add(2), p0.as_ptr())
                as *mut c_void,
            c::oracle_gs_technique_get_pass_by_name(
                techniques.as_ptr().wrapping_add(2) as *const c_void,
                p0.as_ptr(),
            ),
        );

        assert_eq!(rs::gs_effect_get_num_params(effect_p), params.len());
        assert_eq!(
            rs::gs_effect_get_num_params(effect_p),
            c::oracle_gs_effect_get_num_params(effect_p as *const c_void)
        );
        for idx in [
            0,
            1,
            params.len() - 1,
            params.len(),
            params.len() + 1,
            usize::MAX,
        ] {
            same(
                "param idx",
                rs::gs_effect_get_param_by_idx(effect_p, idx) as *mut c_void,
                c::oracle_gs_effect_get_param_by_idx(effect_p as *const c_void, idx),
            );
        }
        assert!(rs::gs_effect_get_param_by_idx(effect_p, params.len()).is_null());

        assert_eq!(rs::gs_param_get_num_annotations(ann), anns.len());
        assert_eq!(
            rs::gs_param_get_num_annotations(ann),
            c::oracle_gs_param_get_num_annotations(ann as *const c_void)
        );
        for idx in [0, 1, anns.len(), anns.len() + 1, usize::MAX] {
            let got = rs::gs_param_get_annotation_by_idx(ann, idx);
            same(
                "annotation idx",
                got as *mut c_void,
                c::oracle_gs_param_get_annotation_by_idx(ann as *const c_void, idx),
            );
            if idx == anns.len() {
                // characterized, not endorsed
                assert_eq!(got, anns.as_ptr().wrapping_add(anns.len()) as *mut _);
            }
        }
        let bare = params.as_ptr().wrapping_add(1);
        assert_eq!(rs::gs_param_get_num_annotations(bare), 0);
        for idx in [0, 1] {
            let got = rs::gs_param_get_annotation_by_idx(bare, idx);
            same(
                "bare annotation idx",
                got as *mut c_void,
                c::oracle_gs_param_get_annotation_by_idx(bare as *const c_void, idx),
            );
            assert!(got.is_null());
        }

        for idx in [0, 1, passes.len(), passes.len() + 1, usize::MAX] {
            let got = rs::gs_technique_get_pass_by_idx(draw_tech, idx);
            same(
                "pass idx",
                got as *mut c_void,
                c::oracle_gs_technique_get_pass_by_idx(draw_tech as *const c_void, idx),
            );
            if idx == passes.len() {
                // characterized, not endorsed
                assert_eq!(got, passes.as_ptr().wrapping_add(passes.len()) as *mut _);
            }
        }
        let solid_tech = techniques.as_ptr().wrapping_add(2);
        for idx in [0, 1] {
            let got = rs::gs_technique_get_pass_by_idx(solid_tech, idx);
            same(
                "empty pass idx",
                got as *mut c_void,
                c::oracle_gs_technique_get_pass_by_idx(solid_tech as *const c_void, idx),
            );
            assert!(got.is_null());
        }

        effect.view_proj = params.as_mut_ptr();
        effect.world = params.as_mut_ptr().wrapping_add(1);
        same(
            "viewproj",
            rs::gs_effect_get_viewproj_matrix(effect_p) as *mut c_void,
            c::oracle_gs_effect_get_viewproj_matrix(effect_p as *const c_void),
        );
        same(
            "world",
            rs::gs_effect_get_world_matrix(effect_p) as *mut c_void,
            c::oracle_gs_effect_get_world_matrix(effect_p as *const c_void),
        );
        assert_eq!(
            rs::gs_effect_get_viewproj_matrix(effect_p),
            effect.view_proj
        );
        assert_eq!(rs::gs_effect_get_world_matrix(effect_p), effect.world);

        effect.cur_technique = techniques.as_ptr() as *mut gs_effect_technique;
        same(
            "current draw",
            rs::gs_effect_get_current_technique(effect_p) as *mut c_void,
            c::oracle_gs_effect_get_current_technique(effect_p as *const c_void),
        );
        assert_eq!(
            rs::gs_effect_get_current_technique(effect_p),
            effect.cur_technique
        );

        let outsider_name = cstr(b"outsider");
        let outsider = technique_of(name_ptr(&outsider_name), empty_da());
        effect.cur_technique = &outsider as *const gs_effect_technique as *mut _;
        same(
            "outsider",
            rs::gs_effect_get_current_technique(effect_p) as *mut c_void,
            c::oracle_gs_effect_get_current_technique(effect_p as *const c_void),
        );
        assert_eq!(
            rs::gs_effect_get_current_technique(effect_p),
            effect.cur_technique
        );

        effect.cur_technique = ptr::null_mut();
        same(
            "current null",
            rs::gs_effect_get_current_technique(effect_p) as *mut c_void,
            c::oracle_gs_effect_get_current_technique(effect_p as *const c_void),
        );
        assert!(effect.cur_technique.is_null());
        assert!(rs::gs_effect_get_current_technique(effect_p).is_null());

        let sentinel = cstr(b"sentinel");
        let mut info_r = gs_effect_param_info {
            name: sentinel.as_ptr(),
            type_: param::TEXTURE,
        };
        let mut info_c = info_r;
        rs::gs_effect_get_param_info(params.as_ptr(), &mut info_r);
        c::oracle_gs_effect_get_param_info(
            params.as_ptr() as *const c_void,
            &mut info_c as *mut _ as *mut c_void,
        );
        assert_eq!(info_r.name, info_c.name);
        assert_eq!(info_r.type_, info_c.type_);
        assert_eq!(info_r.name, params[0].name as *const c_char);
        assert_eq!(info_r.type_, param::VEC4);

        params[0].name = ptr::null_mut();
        params[0].type_ = param::INT;
        info_r.name = sentinel.as_ptr();
        info_r.type_ = param::TEXTURE;
        info_c = info_r;
        rs::gs_effect_get_param_info(params.as_ptr(), &mut info_r);
        c::oracle_gs_effect_get_param_info(
            params.as_ptr() as *const c_void,
            &mut info_c as *mut _ as *mut c_void,
        );
        assert!(info_r.name.is_null());
        assert_eq!(info_r.type_, param::INT);
        assert_eq!(info_r.name, info_c.name);
        assert_eq!(info_r.type_, info_c.type_);
    }
}
