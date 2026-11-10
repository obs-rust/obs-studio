//! Tier 1 for `obs_core::display`, the safe core of `libobs/obs-display.c.h`.
//!
//! A fake host keeps displays as indices, list links as [`Link`], and logs
//! every read, write, lock, link and call in order, with the display's locks
//! where the C holds them. The tests pin each function's full call order
//! (the assertions of `test_display_lifecycle` and `test_display_render`
//! among them): init with and without graphics data and each failure; the
//! list link at the front and the unlink from the front, middle and end;
//! a failed create destroyed inside the graphics context; free with and
//! without a swap chain; the resize, colour-space and callback calls; and
//! the render: a disabled display, the size and flag taken under
//! `draw_info_mutex`, the resize or colour-space update, the present-ready
//! answer, the clear colour per host answer and colour space, the clear
//! flags and solid workaround, and the callbacks under
//! `draw_callbacks_mutex`.

use core::ffi::{CStr, c_int};

use obs_core::canvas_draw::{CLEAR_COLOR, CLEAR_DEPTH, CLEAR_STENCIL, GS_CS_SRGB, GS_CS_SRGB_16F};
use obs_core::display::{
    self, Host, MUTEX_FAILED, SWAP_CHAIN_FAILED, Vec4, srgb_nonlinear_to_linear, to_float, to_int,
    vec4_from_rgba, vec4_from_rgba_srgb,
};

/// A link of the display list: the core's head, or a display's `next`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Link {
    Head,
    Next(usize),
}

#[derive(Debug, Clone, Default)]
struct Disp {
    cx: u32,
    cy: u32,
    next_cx: u32,
    next_cy: u32,
    update_color_space: bool,
    enabled: bool,
    background_color: u32,
    use_clear_workaround: bool,
    swap: Option<u32>,
    callbacks: Vec<(u32, u32)>,
    next: Option<usize>,
    prev_next: Option<Link>,
    info_locked: bool,
    callbacks_locked: bool,
    dealloced: bool,
}

#[derive(Default)]
struct Fake {
    displays: Vec<Disp>,
    first: Option<usize>,
    displays_locked: bool,
    log: Vec<String>,

    swap_ok: bool,
    next_swap: u32,
    callbacks_rc: c_int,
    info_rc: c_int,
    workaround: bool,
    always_srgb: bool,
    space: c_int,
    present_ready: bool,
    effect_loops: u32,
    data_size: (u32, u32),
}

impl Fake {
    fn new() -> Self {
        Self {
            swap_ok: true,
            next_swap: 100,
            present_ready: true,
            space: GS_CS_SRGB,
            data_size: (1920, 1080),
            ..Self::default()
        }
    }

    fn push(&mut self, op: String) {
        self.log.push(op);
    }

    fn d(&mut self, idx: usize) -> &mut Disp {
        &mut self.displays[idx]
    }

    /// A display made by hand, as create leaves it with no graphics data.
    fn add(&mut self) -> usize {
        self.displays.push(Disp {
            enabled: true,
            ..Disp::default()
        });
        self.displays.len() - 1
    }

    fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    /// The list from the head, following `next`.
    fn order(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut at = self.first;
        while let Some(i) = at {
            out.push(i);
            at = self.displays[i].next;
        }
        out
    }
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_string()).collect()
}

impl Host for Fake {
    type Display = usize;
    type Data = u32;
    type Swap = u32;
    type Link = Link;
    type Callback = (u32, u32);
    type Graphics = u32;
    type Effect = u32;
    type Param = u32;

    fn alloc(&mut self) -> usize {
        self.displays.push(Disp::default());
        let i = self.displays.len() - 1;
        self.push(format!("alloc {i}"));
        i
    }
    fn dealloc(&mut self, d: usize) {
        self.push(format!("dealloc {d}"));
        self.d(d).dealloced = true;
    }
    fn log_error(&mut self, message: &'static CStr) {
        self.push(format!("log {}", message.to_str().unwrap()));
    }

    fn init_mutex_values(&mut self, d: usize) {
        self.push(format!("init_mutex_values {d}"));
    }
    fn callbacks_mutex_init(&mut self, d: usize) -> c_int {
        self.push(format!("callbacks_mutex_init {d}"));
        self.callbacks_rc
    }
    fn info_mutex_init(&mut self, d: usize) -> c_int {
        self.push(format!("info_mutex_init {d}"));
        self.info_rc
    }
    fn callbacks_mutex_destroy(&mut self, d: usize) {
        self.push(format!("callbacks_mutex_destroy {d}"));
    }
    fn info_mutex_destroy(&mut self, d: usize) {
        self.push(format!("info_mutex_destroy {d}"));
    }
    fn lock_callbacks(&mut self, d: usize) {
        assert!(
            !self.d(d).callbacks_locked,
            "draw_callbacks_mutex taken twice"
        );
        self.d(d).callbacks_locked = true;
        self.push(format!("lock_callbacks {d}"));
    }
    fn unlock_callbacks(&mut self, d: usize) {
        assert!(self.d(d).callbacks_locked);
        self.d(d).callbacks_locked = false;
        self.push(format!("unlock_callbacks {d}"));
    }
    fn lock_info(&mut self, d: usize) {
        assert!(!self.d(d).info_locked, "draw_info_mutex taken twice");
        self.d(d).info_locked = true;
        self.push(format!("lock_info {d}"));
    }
    fn unlock_info(&mut self, d: usize) {
        assert!(self.d(d).info_locked);
        self.d(d).info_locked = false;
        self.push(format!("unlock_info {d}"));
    }

    fn host_clear_workaround(&mut self) -> bool {
        self.push("host_clear_workaround".into());
        self.workaround
    }
    fn host_clear_color_always_srgb(&mut self) -> bool {
        self.push("host_clear_color_always_srgb".into());
        self.always_srgb
    }

    fn use_clear_workaround(&mut self, d: usize) -> bool {
        self.push(format!("use_clear_workaround {d}"));
        self.d(d).use_clear_workaround
    }
    fn set_use_clear_workaround(&mut self, d: usize, v: bool) {
        self.push(format!("set_use_clear_workaround {d} {v}"));
        self.d(d).use_clear_workaround = v;
    }
    fn swap(&mut self, d: usize) -> Option<u32> {
        self.push(format!("swap {d}"));
        self.d(d).swap
    }
    fn set_swap(&mut self, d: usize, v: Option<u32>) {
        self.push(format!("set_swap {d} {v:?}"));
        self.d(d).swap = v;
    }
    fn cx(&mut self, d: usize) -> u32 {
        self.push(format!("cx {d}"));
        self.d(d).cx
    }
    fn cy(&mut self, d: usize) -> u32 {
        self.push(format!("cy {d}"));
        self.d(d).cy
    }
    fn set_cx(&mut self, d: usize, v: u32) {
        self.push(format!("set_cx {d} {v}"));
        self.d(d).cx = v;
    }
    fn set_cy(&mut self, d: usize, v: u32) {
        self.push(format!("set_cy {d} {v}"));
        self.d(d).cy = v;
    }
    fn next_cx(&mut self, d: usize) -> u32 {
        self.push(format!(
            "next_cx {d} locked={}",
            self.displays[d].info_locked
        ));
        self.d(d).next_cx
    }
    fn next_cy(&mut self, d: usize) -> u32 {
        self.push(format!(
            "next_cy {d} locked={}",
            self.displays[d].info_locked
        ));
        self.d(d).next_cy
    }
    fn set_next_cx(&mut self, d: usize, v: u32) {
        self.push(format!(
            "set_next_cx {d} {v} locked={}",
            self.displays[d].info_locked
        ));
        self.d(d).next_cx = v;
    }
    fn set_next_cy(&mut self, d: usize, v: u32) {
        self.push(format!(
            "set_next_cy {d} {v} locked={}",
            self.displays[d].info_locked
        ));
        self.d(d).next_cy = v;
    }
    fn update_color_space(&mut self, d: usize) -> bool {
        self.push(format!(
            "update_color_space {d} locked={}",
            self.displays[d].info_locked
        ));
        self.d(d).update_color_space
    }
    fn set_update_color_space(&mut self, d: usize, v: bool) {
        self.push(format!(
            "set_update_color_space {d} {v} locked={}",
            self.displays[d].info_locked
        ));
        self.d(d).update_color_space = v;
    }
    fn enabled(&mut self, d: usize) -> bool {
        self.push(format!("enabled {d}"));
        self.d(d).enabled
    }
    fn set_enabled(&mut self, d: usize, v: bool) {
        self.push(format!("set_enabled {d} {v}"));
        self.d(d).enabled = v;
    }
    fn background_color(&mut self, d: usize) -> u32 {
        self.push(format!("background_color {d}"));
        self.d(d).background_color
    }
    fn set_background_color(&mut self, d: usize, v: u32) {
        self.push(format!("set_background_color {d} {v:#x}"));
        self.d(d).background_color = v;
    }

    fn callbacks_push(&mut self, d: usize, cb: (u32, u32)) {
        self.push(format!(
            "callbacks_push {d} {cb:?} locked={}",
            self.displays[d].callbacks_locked
        ));
        self.d(d).callbacks.push(cb);
    }
    fn callbacks_erase_item(&mut self, d: usize, cb: (u32, u32)) {
        self.push(format!(
            "callbacks_erase_item {d} {cb:?} locked={}",
            self.displays[d].callbacks_locked
        ));
        if let Some(i) = self.d(d).callbacks.iter().position(|c| *c == cb) {
            self.d(d).callbacks.remove(i);
        }
    }
    fn callbacks_num(&mut self, d: usize) -> usize {
        self.push(format!("callbacks_num {d}"));
        self.d(d).callbacks.len()
    }
    fn callbacks_get(&mut self, d: usize, idx: usize) -> (u32, u32) {
        self.push(format!("callbacks_get {d} {idx}"));
        self.d(d).callbacks[idx]
    }
    fn callbacks_free(&mut self, d: usize) {
        self.push(format!("callbacks_free {d}"));
        self.d(d).callbacks.clear();
    }
    fn call(&mut self, cb: (u32, u32), cx: u32, cy: u32) {
        let locks: Vec<String> = self
            .displays
            .iter()
            .enumerate()
            .filter(|(_, d)| d.callbacks_locked || d.info_locked)
            .map(|(i, d)| {
                format!(
                    "{i}:{}{}",
                    u8::from(d.callbacks_locked),
                    u8::from(d.info_locked)
                )
            })
            .collect();
        self.push(format!("call {cb:?} {cx}x{cy} locks={}", locks.join(",")));
    }

    fn lock_displays(&mut self) {
        assert!(!self.displays_locked);
        self.displays_locked = true;
        self.push("lock_displays".into());
    }
    fn unlock_displays(&mut self) {
        assert!(self.displays_locked);
        self.displays_locked = false;
        self.push("unlock_displays".into());
    }
    fn first_display(&mut self) -> Option<usize> {
        assert!(self.displays_locked);
        self.push("first_display".into());
        self.first
    }
    fn set_first_display(&mut self, v: Option<usize>) {
        assert!(self.displays_locked);
        self.push(format!("set_first_display {v:?}"));
        self.first = v;
    }
    fn first_display_ref(&mut self) -> Link {
        self.push("first_display_ref".into());
        Link::Head
    }
    fn next(&mut self, d: usize) -> Option<usize> {
        assert!(self.displays_locked);
        self.push(format!("next {d}"));
        self.d(d).next
    }
    fn set_next(&mut self, d: usize, v: Option<usize>) {
        assert!(self.displays_locked);
        self.push(format!("set_next {d} {v:?}"));
        self.d(d).next = v;
    }
    fn next_ref(&mut self, d: usize) -> Link {
        self.push(format!("next_ref {d}"));
        Link::Next(d)
    }
    fn prev_next(&mut self, d: usize) -> Option<Link> {
        assert!(self.displays_locked);
        self.push(format!("prev_next {d}"));
        self.d(d).prev_next
    }
    fn set_prev_next(&mut self, d: usize, v: Option<Link>) {
        assert!(self.displays_locked);
        self.push(format!("set_prev_next {d} {v:?}"));
        self.d(d).prev_next = v;
    }
    fn store_link(&mut self, link: Link, v: Option<usize>) {
        assert!(self.displays_locked);
        self.push(format!("store_link {link:?} {v:?}"));
        match link {
            Link::Head => self.first = v,
            Link::Next(d) => self.d(d).next = v,
        }
    }

    fn graphics(&mut self) -> u32 {
        self.push("graphics".into());
        7
    }
    fn solid_effect(&mut self) -> u32 {
        self.push("solid_effect".into());
        9
    }
    fn data_cx(&mut self, data: u32) -> u32 {
        self.push(format!("data_cx {data}"));
        self.data_size.0
    }
    fn data_cy(&mut self, data: u32) -> u32 {
        self.push(format!("data_cy {data}"));
        self.data_size.1
    }

    fn enter_graphics(&mut self) {
        self.push("enter_graphics".into());
    }
    fn leave_graphics(&mut self) {
        self.push("leave_graphics".into());
    }
    fn enter_context(&mut self, g: u32) {
        self.push(format!("enter_context {g}"));
    }
    fn leave_context(&mut self) {
        self.push("leave_context".into());
    }
    fn swapchain_create(&mut self, data: u32) -> Option<u32> {
        self.push(format!("swapchain_create {data}"));
        self.swap_ok.then(|| {
            self.next_swap += 1;
            self.next_swap
        })
    }
    fn swapchain_destroy(&mut self, swap: u32) {
        self.push(format!("swapchain_destroy {swap}"));
    }
    fn load_swapchain(&mut self, swap: Option<u32>) {
        self.push(format!("load_swapchain {swap:?}"));
    }
    fn resize(&mut self, cx: u32, cy: u32) {
        self.push(format!("resize {cx}x{cy}"));
    }
    fn gs_update_color_space(&mut self) {
        self.push("gs_update_color_space".into());
    }
    fn is_present_ready(&mut self) -> bool {
        self.push("is_present_ready".into());
        self.present_ready
    }
    fn begin_scene(&mut self) {
        self.push("begin_scene".into());
    }
    fn color_space(&mut self) -> c_int {
        self.push("color_space".into());
        self.space
    }
    fn clear(&mut self, flags: u32, color: Vec4, depth: f32, stencil: u8) {
        self.push(format!(
            "clear {flags} {:?} {depth} {stencil}",
            [color.x, color.y, color.z, color.w]
        ));
    }
    fn enable_depth_test(&mut self, enable: bool) {
        self.push(format!("enable_depth_test {enable}"));
    }
    fn set_cull_mode(&mut self, mode: c_int) {
        self.push(format!("set_cull_mode {mode}"));
    }
    fn ortho(&mut self, l: f32, r: f32, t: f32, b: f32, n: f32, f: f32) {
        self.push(format!("ortho {l} {r} {t} {b} {n} {f}"));
    }
    fn set_viewport(&mut self, x: c_int, y: c_int, w: c_int, h: c_int) {
        self.push(format!("set_viewport {x} {y} {w} {h}"));
    }
    fn param_by_name(&mut self, effect: u32, name: &'static CStr) -> u32 {
        self.push(format!("param_by_name {effect} {}", name.to_str().unwrap()));
        11
    }
    fn effect_set_vec4(&mut self, param: u32, v: Vec4) {
        self.push(format!(
            "effect_set_vec4 {param} {:?}",
            [v.x, v.y, v.z, v.w]
        ));
    }
    fn effect_loop(&mut self, effect: u32, technique: &'static CStr) -> bool {
        self.push(format!(
            "effect_loop {effect} {}",
            technique.to_str().unwrap()
        ));
        if self.effect_loops > 0 {
            self.effect_loops -= 1;
            true
        } else {
            false
        }
    }
    fn draw_sprite(&mut self, cx: u32, cy: u32) {
        self.push(format!("draw_sprite {cx}x{cy}"));
    }
    fn end_scene(&mut self) {
        self.push("end_scene".into());
    }
    fn present(&mut self) {
        self.push("present".into());
    }
}

// --- init -------------------------------------------------------------

#[test]
fn init_without_graphics_data_makes_the_mutexes_only() {
    let mut h = Fake::new();
    h.workaround = true;
    h.displays.push(Disp::default());
    assert!(display::init(&mut h, 0, None));
    assert_eq!(
        h.take_log(),
        s(&[
            "init_mutex_values 0",
            "host_clear_workaround",
            "set_use_clear_workaround 0 true",
            "callbacks_mutex_init 0",
            "info_mutex_init 0",
            "set_enabled 0 true",
        ])
    );
    assert!(h.displays[0].use_clear_workaround);
    assert!(h.displays[0].enabled);
    assert_eq!(h.displays[0].swap, None);
    assert_eq!((h.displays[0].cx, h.displays[0].next_cx), (0, 0));
}

#[test]
fn init_with_graphics_data_makes_the_swap_chain_and_size() {
    let mut h = Fake::new();
    h.displays.push(Disp::default());
    assert!(display::init(&mut h, 0, Some(5)));
    assert_eq!(
        h.take_log(),
        s(&[
            "init_mutex_values 0",
            "host_clear_workaround",
            "set_use_clear_workaround 0 false",
            "swapchain_create 5",
            "set_swap 0 Some(101)",
            "swap 0",
            "data_cx 5",
            "data_cy 5",
            "set_cx 0 1920",
            "set_cy 0 1080",
            "set_next_cx 0 1920 locked=false",
            "set_next_cy 0 1080 locked=false",
            "callbacks_mutex_init 0",
            "info_mutex_init 0",
            "set_enabled 0 true",
        ])
    );
    let d = &h.displays[0];
    assert_eq!((d.cx, d.cy, d.next_cx, d.next_cy), (1920, 1080, 1920, 1080));
}

#[test]
fn init_fails_when_the_swap_chain_cannot_be_made() {
    let mut h = Fake::new();
    h.swap_ok = false;
    h.displays.push(Disp::default());
    assert!(!display::init(&mut h, 0, Some(5)));
    let mut want = s(&[
        "init_mutex_values 0",
        "host_clear_workaround",
        "set_use_clear_workaround 0 false",
        "swapchain_create 5",
        "set_swap 0 None",
        "swap 0",
    ]);
    want.push(format!("log {}", SWAP_CHAIN_FAILED.to_str().unwrap()));
    assert_eq!(h.take_log(), want);
    assert!(!h.displays[0].enabled);
}

#[test]
fn init_fails_on_each_mutex() {
    let mutex = format!("log {}", MUTEX_FAILED.to_str().unwrap());

    let mut h = Fake::new();
    h.callbacks_rc = 22;
    h.displays.push(Disp::default());
    assert!(!display::init(&mut h, 0, None));
    let log = h.take_log();
    assert_eq!(log.last(), Some(&mutex));
    assert!(!log.iter().any(|op| op.starts_with("info_mutex_init")));

    let mut h = Fake::new();
    h.info_rc = 12;
    h.displays.push(Disp::default());
    assert!(!display::init(&mut h, 0, None));
    let log = h.take_log();
    let mut want = s(&["callbacks_mutex_init 0", "info_mutex_init 0"]);
    want.push(mutex);
    assert_eq!(log[log.len() - 3..].to_vec(), want);
    assert!(!h.displays[0].enabled);
}

// --- create and destroy -----------------------------------------------

#[test]
fn create_links_each_display_at_the_front() {
    let mut h = Fake::new();
    let a = display::create(&mut h, None, 0xff33_6699).unwrap();
    assert_eq!(
        h.take_log(),
        s(&[
            "alloc 0",
            "graphics",
            "enter_context 7",
            "set_background_color 0 0xff336699",
            "init_mutex_values 0",
            "host_clear_workaround",
            "set_use_clear_workaround 0 false",
            "callbacks_mutex_init 0",
            "info_mutex_init 0",
            "set_enabled 0 true",
            "lock_displays",
            "first_display_ref",
            "set_prev_next 0 Some(Head)",
            "first_display",
            "set_next 0 None",
            "set_first_display Some(0)",
            "next 0",
            "unlock_displays",
            "leave_context",
        ])
    );

    let b = display::create(&mut h, None, 2).unwrap();
    let log = h.take_log();
    let at = log.iter().position(|op| op == "lock_displays").unwrap();
    assert_eq!(
        log[at..].to_vec(),
        s(&[
            "lock_displays",
            "first_display_ref",
            "set_prev_next 1 Some(Head)",
            "first_display",
            "set_next 1 Some(0)",
            "set_first_display Some(1)",
            "next 1",
            "next_ref 1",
            "next 1",
            "set_prev_next 0 Some(Next(1))",
            "unlock_displays",
            "leave_context",
        ])
    );

    let c = display::create(&mut h, Some(3), 3).unwrap();
    assert_eq!(h.order(), vec![c, b, a]);
    assert_eq!(h.displays[c].prev_next, Some(Link::Head));
    assert_eq!(h.displays[b].prev_next, Some(Link::Next(c)));
    assert_eq!(h.displays[a].prev_next, Some(Link::Next(b)));
    assert_eq!(h.displays[a].next, None);
    assert_eq!(h.displays[c].swap, Some(101));
    assert_eq!(h.displays[a].background_color, 0xff33_6699);
}

#[test]
fn a_failed_create_is_destroyed_inside_the_context() {
    let mut h = Fake::new();
    let a = display::create(&mut h, None, 1).unwrap();
    h.take_log();
    h.swap_ok = false;
    assert_eq!(display::create(&mut h, Some(4), 9), None);
    let mut want = s(&[
        "alloc 1",
        "graphics",
        "enter_context 7",
        "set_background_color 1 0x9",
        "init_mutex_values 1",
        "host_clear_workaround",
        "set_use_clear_workaround 1 false",
        "swapchain_create 4",
        "set_swap 1 None",
        "swap 1",
    ]);
    want.push(format!("log {}", SWAP_CHAIN_FAILED.to_str().unwrap()));
    want.extend(s(&[
        "lock_displays",
        "prev_next 1",
        "next 1",
        "unlock_displays",
        "enter_graphics",
        "callbacks_mutex_destroy 1",
        "info_mutex_destroy 1",
        "callbacks_free 1",
        "swap 1",
        "leave_graphics",
        "dealloc 1",
        "leave_context",
    ]));
    assert_eq!(h.take_log(), want);
    assert!(h.displays[1].dealloced);
    assert_eq!(h.order(), vec![a]);
}

fn three() -> (Fake, usize, usize, usize) {
    let mut h = Fake::new();
    let a = display::create(&mut h, None, 1).unwrap();
    let b = display::create(&mut h, None, 2).unwrap();
    let c = display::create(&mut h, None, 3).unwrap();
    h.take_log();
    (h, a, b, c)
}

#[test]
fn destroy_unlinks_the_middle_display() {
    let (mut h, a, b, c) = three();
    display::destroy(&mut h, Some(b));
    assert_eq!(
        h.take_log(),
        s(&[
            "lock_displays",
            "prev_next 1",
            "prev_next 1",
            "next 1",
            "store_link Next(2) Some(0)",
            "next 1",
            "prev_next 1",
            "next 1",
            "set_prev_next 0 Some(Next(2))",
            "unlock_displays",
            "enter_graphics",
            "callbacks_mutex_destroy 1",
            "info_mutex_destroy 1",
            "callbacks_free 1",
            "swap 1",
            "leave_graphics",
            "dealloc 1",
        ])
    );
    assert_eq!(h.order(), vec![c, a]);
    assert_eq!(h.displays[a].prev_next, Some(Link::Next(c)));
}

#[test]
fn destroy_unlinks_the_front_and_the_end() {
    let (mut h, a, b, c) = three();
    display::destroy(&mut h, Some(c));
    assert_eq!(h.order(), vec![b, a]);
    assert_eq!(h.displays[b].prev_next, Some(Link::Head));
    display::destroy(&mut h, Some(a));
    assert_eq!(h.order(), vec![b]);
    assert_eq!(h.displays[b].next, None);
    display::destroy(&mut h, Some(b));
    assert_eq!(h.order(), Vec::<usize>::new());
    assert!(h.displays.iter().all(|d| d.dealloced));
}

#[test]
fn destroy_of_an_unlinked_display_skips_the_list() {
    let mut h = Fake::new();
    let d = h.add();
    display::destroy(&mut h, Some(d));
    let log = h.take_log();
    assert_eq!(
        log[..4].to_vec(),
        s(&["lock_displays", "prev_next 0", "next 0", "unlock_displays"])
    );
}

#[test]
fn destroy_of_none_does_nothing() {
    let mut h = Fake::new();
    display::destroy(&mut h, None);
    assert!(h.take_log().is_empty());
}

#[test]
fn free_destroys_the_swap_chain_and_clears_it() {
    let mut h = Fake::new();
    let d = h.add();
    h.displays[d].swap = Some(55);
    h.displays[d].callbacks.push((1, 2));
    display::free(&mut h, d);
    assert_eq!(
        h.take_log(),
        s(&[
            "callbacks_mutex_destroy 0",
            "info_mutex_destroy 0",
            "callbacks_free 0",
            "swap 0",
            "swapchain_destroy 55",
            "set_swap 0 None",
        ])
    );
    assert!(h.displays[d].callbacks.is_empty());
    assert_eq!(h.displays[d].swap, None);
}

// --- resize, colour space and callbacks --------------------------------

#[test]
fn resize_sets_the_next_size_under_draw_info_mutex() {
    let mut h = Fake::new();
    let d = h.add();
    display::resize(&mut h, Some(d), 1280, 720);
    assert_eq!(
        h.take_log(),
        s(&[
            "lock_info 0",
            "set_next_cx 0 1280 locked=true",
            "set_next_cy 0 720 locked=true",
            "unlock_info 0",
        ])
    );
    assert_eq!((h.displays[d].cx, h.displays[d].cy), (0, 0));
    display::resize(&mut h, None, 1, 1);
    assert!(h.take_log().is_empty());
}

#[test]
fn update_color_space_sets_the_flag_under_draw_info_mutex() {
    let mut h = Fake::new();
    let d = h.add();
    display::update_color_space(&mut h, Some(d));
    assert_eq!(
        h.take_log(),
        s(&[
            "lock_info 0",
            "set_update_color_space 0 true locked=true",
            "unlock_info 0",
        ])
    );
    assert!(h.displays[d].update_color_space);
    display::update_color_space(&mut h, None);
    assert!(h.take_log().is_empty());
}

#[test]
fn callbacks_add_in_order_and_remove_the_first_match() {
    let mut h = Fake::new();
    let d = h.add();
    display::add_draw_callback(&mut h, Some(d), (1, 10));
    assert_eq!(
        h.take_log(),
        s(&[
            "lock_callbacks 0",
            "callbacks_push 0 (1, 10) locked=true",
            "unlock_callbacks 0",
        ])
    );
    display::add_draw_callback(&mut h, Some(d), (2, 20));
    display::add_draw_callback(&mut h, Some(d), (1, 10));
    assert_eq!(h.displays[d].callbacks, vec![(1, 10), (2, 20), (1, 10)]);

    h.take_log();
    display::remove_draw_callback(&mut h, Some(d), (1, 10));
    assert_eq!(
        h.take_log(),
        s(&[
            "lock_callbacks 0",
            "callbacks_erase_item 0 (1, 10) locked=true",
            "unlock_callbacks 0",
        ])
    );
    assert_eq!(h.displays[d].callbacks, vec![(2, 20), (1, 10)]);

    display::add_draw_callback(&mut h, None, (3, 3));
    display::remove_draw_callback(&mut h, None, (2, 20));
    assert!(h.take_log().is_empty());
}

// --- render ------------------------------------------------------------

fn ready(h: &mut Fake, cx: u32, cy: u32) -> usize {
    let d = h.add();
    h.displays[d].swap = Some(42);
    h.displays[d].cx = cx;
    h.displays[d].cy = cy;
    h.displays[d].next_cx = cx;
    h.displays[d].next_cy = cy;
    h.displays[d].background_color = 0xff00_8040;
    d
}

#[test]
fn render_of_none_or_a_disabled_display_does_nothing() {
    let mut h = Fake::new();
    display::render(&mut h, None);
    assert!(h.take_log().is_empty());

    let d = ready(&mut h, 64, 32);
    h.displays[d].enabled = false;
    h.displays[d].next_cx = 10;
    h.displays[d].update_color_space = true;
    h.displays[d].callbacks.push((1, 1));
    display::render(&mut h, Some(d));
    assert_eq!(h.take_log(), s(&["enabled 0"]));
    assert!(h.displays[d].update_color_space);
    assert_eq!(h.displays[d].cx, 64);
}

#[test]
fn render_at_a_new_size_resizes_draws_and_presents() {
    let mut h = Fake::new();
    let d = ready(&mut h, 64, 32);
    h.displays[d].next_cx = 200;
    h.displays[d].next_cy = 100;
    h.displays[d].update_color_space = true;
    h.displays[d].callbacks = vec![(1, 10), (2, 20), (1, 10)];
    display::render(&mut h, Some(d));
    let color = vec4_from_rgba(0xff00_8040);
    assert_eq!(
        h.take_log(),
        vec![
            "enabled 0".to_string(),
            "lock_info 0".into(),
            "next_cx 0 locked=true".into(),
            "next_cy 0 locked=true".into(),
            "update_color_space 0 locked=true".into(),
            "set_update_color_space 0 false locked=true".into(),
            "unlock_info 0".into(),
            "swap 0".into(),
            "load_swapchain Some(42)".into(),
            "cx 0".into(),
            "resize 200x100".into(),
            "set_cx 0 200".into(),
            "set_cy 0 100".into(),
            "is_present_ready".into(),
            "begin_scene".into(),
            "host_clear_color_always_srgb".into(),
            "color_space".into(),
            "background_color 0".into(),
            "use_clear_workaround 0".into(),
            format!(
                "clear {} {:?} 1 0",
                CLEAR_COLOR | CLEAR_DEPTH | CLEAR_STENCIL,
                [color.x, color.y, color.z, 1.0]
            ),
            "enable_depth_test false".into(),
            "set_cull_mode 2".into(),
            "ortho 0 200 0 100 -100 100".into(),
            "set_viewport 0 0 200 100".into(),
            "lock_callbacks 0".into(),
            "callbacks_num 0".into(),
            "callbacks_get 0 0".into(),
            "call (1, 10) 200x100 locks=0:10".into(),
            "callbacks_num 0".into(),
            "callbacks_get 0 1".into(),
            "call (2, 20) 200x100 locks=0:10".into(),
            "callbacks_num 0".into(),
            "callbacks_get 0 2".into(),
            "call (1, 10) 200x100 locks=0:10".into(),
            "callbacks_num 0".into(),
            "unlock_callbacks 0".into(),
            "end_scene".into(),
            "present".into(),
        ]
    );
    let disp = &h.displays[d];
    assert_eq!((disp.cx, disp.cy), (200, 100));
    assert!(!disp.update_color_space);
}

#[test]
fn render_checks_cy_when_cx_matches() {
    let mut h = Fake::new();
    let d = ready(&mut h, 64, 32);
    h.displays[d].next_cy = 48;
    display::render(&mut h, Some(d));
    let log = h.take_log();
    let at = log.iter().position(|op| op == "cx 0").unwrap();
    assert_eq!(
        log[at..at + 5].to_vec(),
        s(&["cx 0", "cy 0", "resize 64x48", "set_cx 0 64", "set_cy 0 48"])
    );
}

#[test]
fn render_at_the_same_size_updates_the_colour_space_only_when_asked() {
    let mut h = Fake::new();
    let d = ready(&mut h, 64, 32);
    h.displays[d].update_color_space = true;
    display::render(&mut h, Some(d));
    let log = h.take_log();
    let at = log.iter().position(|op| op == "cx 0").unwrap();
    assert_eq!(
        log[at..at + 4].to_vec(),
        s(&["cx 0", "cy 0", "gs_update_color_space", "is_present_ready"])
    );
    assert!(!h.displays[d].update_color_space);

    display::render(&mut h, Some(d));
    let log = h.take_log();
    let at = log.iter().position(|op| op == "cx 0").unwrap();
    assert_eq!(
        log[at..at + 3].to_vec(),
        s(&["cx 0", "cy 0", "is_present_ready"])
    );
}

#[test]
fn render_stores_the_size_before_the_present_check() {
    let mut h = Fake::new();
    let d = ready(&mut h, 64, 32);
    h.present_ready = false;
    h.displays[d].next_cx = 640;
    h.displays[d].next_cy = 480;
    h.displays[d].callbacks.push((1, 1));
    display::render(&mut h, Some(d));
    let log = h.take_log();
    assert_eq!(
        log[log.len() - 5..].to_vec(),
        s(&[
            "cx 0",
            "resize 640x480",
            "set_cx 0 640",
            "set_cy 0 480",
            "is_present_ready"
        ])
    );
    assert_eq!((h.displays[d].cx, h.displays[d].cy), (640, 480));
}

#[test]
fn render_with_no_swap_chain_loads_none() {
    let mut h = Fake::new();
    let d = h.add();
    display::render(&mut h, Some(d));
    assert!(h.take_log().contains(&"load_swapchain None".to_string()));
}

fn clear_line(h: &mut Fake) -> String {
    h.take_log()
        .into_iter()
        .find(|op| op.starts_with("clear "))
        .unwrap()
}

#[test]
fn render_clear_colour_follows_the_host_and_colour_space() {
    let bg = 0xff00_8040;
    let srgb = vec4_from_rgba_srgb(bg);
    let linear = vec4_from_rgba(bg);
    let flags = CLEAR_COLOR | CLEAR_DEPTH | CLEAR_STENCIL;

    // Always sRGB: the colour space is not asked.
    let mut h = Fake::new();
    h.always_srgb = true;
    let d = ready(&mut h, 8, 8);
    display::render(&mut h, Some(d));
    let log = h.take_log();
    assert!(!log.contains(&"color_space".to_string()));
    let line = log.into_iter().find(|op| op.starts_with("clear ")).unwrap();
    assert_eq!(
        line,
        format!("clear {flags} {:?} 1 0", [srgb.x, srgb.y, srgb.z, 1.0])
    );

    // GS_CS_SRGB: linear.
    let mut h = Fake::new();
    let d = ready(&mut h, 8, 8);
    display::render(&mut h, Some(d));
    assert_eq!(
        clear_line(&mut h),
        format!(
            "clear {flags} {:?} 1 0",
            [linear.x, linear.y, linear.z, 1.0]
        )
    );

    // Any other space: sRGB.
    let mut h = Fake::new();
    h.space = GS_CS_SRGB_16F;
    let d = ready(&mut h, 8, 8);
    display::render(&mut h, Some(d));
    assert_eq!(
        clear_line(&mut h),
        format!("clear {flags} {:?} 1 0", [srgb.x, srgb.y, srgb.z, 1.0])
    );
}

#[test]
fn render_with_the_clear_workaround_draws_the_solid_colour() {
    let mut h = Fake::new();
    h.effect_loops = 2;
    let d = ready(&mut h, 30, 20);
    h.displays[d].use_clear_workaround = true;
    h.displays[d].background_color = 0x0000_00ff;
    display::render(&mut h, Some(d));
    let c = vec4_from_rgba(0x0000_00ff);
    let log = h.take_log();
    let at = log.iter().position(|op| op.starts_with("clear ")).unwrap();
    assert_eq!(
        log[at..at + 13].to_vec(),
        vec![
            format!(
                "clear {} {:?} 1 0",
                CLEAR_DEPTH | CLEAR_STENCIL,
                [c.x, c.y, c.z, 1.0]
            ),
            "enable_depth_test false".into(),
            "set_cull_mode 2".into(),
            "ortho 0 30 0 20 -100 100".into(),
            "set_viewport 0 0 30 20".into(),
            "solid_effect".into(),
            "param_by_name 9 color".into(),
            format!("effect_set_vec4 11 {:?}", [c.x, c.y, c.z, 1.0]),
            "effect_loop 9 Solid".into(),
            "draw_sprite 30x20".into(),
            "effect_loop 9 Solid".into(),
            "draw_sprite 30x20".into(),
            "effect_loop 9 Solid".into(),
        ]
    );
}

// --- colour and number conversions ------------------------------------

#[test]
fn vec4_from_rgba_takes_the_bytes_in_memory_order() {
    let v = vec4_from_rgba(u32::from_ne_bytes([0, 51, 255, 128]));
    assert_eq!(
        v,
        Vec4 {
            x: 0.0,
            y: 51.0 / 255.0,
            z: 1.0,
            w: 128.0 / 255.0
        }
    );
}

#[test]
fn vec4_from_rgba_srgb_linearises_three_channels() {
    let v = vec4_from_rgba_srgb(u32::from_ne_bytes([0, 10, 255, 128]));
    assert_eq!(v.x, 0.0);
    assert_eq!(v.y, (10.0f32 / 255.0) / 12.92);
    assert_eq!(v.z, 1.0);
    assert_eq!(v.w, 128.0 / 255.0);
    assert_eq!(srgb_nonlinear_to_linear(0.04045), 0.04045 / 12.92);
    assert_eq!(srgb_nonlinear_to_linear(0.5), (0.555f32 / 1.055).powf(2.4));
}

#[test]
fn conversions_round_and_wrap_as_c_does() {
    assert_eq!(to_float(16_777_217), 16_777_216.0);
    assert_eq!(to_float(u32::MAX), 4_294_967_296.0);
    assert_eq!(to_int(u32::MAX), -1);
    assert_eq!(to_int(0x8000_0000), i32::MIN);
    assert_eq!(to_int(640), 640);
}
