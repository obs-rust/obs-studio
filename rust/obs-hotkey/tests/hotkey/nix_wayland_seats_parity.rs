//! Tier 3 parity: the shims against the original
//! `libobs/obs-nix-wayland-seats.c.h` (oracle/nix_wayland_seats.c).
//!
//! `test_nix_wayland_seats_steps` compiles the C into itself, so it never
//! sees the Rust swap (a variant linked against `libobs_rust-static` does
//! not link on its own: see
//! `test/cmocka/tests.d/test_nix_wayland_seats_steps.cmake`). This file is
//! what compares the two.
//!
//! - the two Wayland values and the seat's size, alignment and offsets;
//! - the bind version and the release rule of every version from 0 to
//!   0xFFFF and the 32 below `u32::MAX`, the keyboard bit test of every
//!   capability set from 0 to 0xFFF, and proptests over
//!   any `u32`;
//! - the seat searches on generated seat arrays (null and empty ones,
//!   shared names, every prefix length), both sides reading the same
//!   memory;
//! - whole platform runs: random sequences of seat announcements (any
//!   version), capability changes (of the keyboard's seat, another seat
//!   and an unknown one) and removals (known and unknown names), stepped
//!   the way `obs-nix-wayland.c` steps them, once with the shims and once
//!   with the C, with every bind, keyboard take and release, and seat
//!   release or destroy logged and compared, and the seats freed at the
//!   end.

use std::ffi::c_void;

use obs_c_oracle::nix_wayland_seats::{self as oracle, Seat as OracleSeat, VALUE_NAMES};
use obs_hotkey::ffi::nix_wayland_seats::{
    obs_nix_wayland_seat as Seat, obs_nix_wayland_seats_bind_version,
    obs_nix_wayland_seats_find_name, obs_nix_wayland_seats_lacks_keyboard,
    obs_nix_wayland_seats_pick, obs_nix_wayland_seats_uses_release,
};
use obs_hotkey::nix_wayland_seats::{CAPABILITY_KEYBOARD, RELEASE_SINCE_VERSION};
use proptest::prelude::*;

fn value(name: &str) -> i64 {
    let which = VALUE_NAMES.iter().position(|n| *n == name).unwrap();
    // SAFETY: the oracle getter takes any index.
    unsafe { oracle::oracle_wayland_seats_value(which) }
}

/// A made-up `wl_seat` pointer: never dereferenced by either side.
fn handle(id: usize) -> *mut c_void {
    std::ptr::without_provenance_mut(id.wrapping_mul(16).wrapping_add(16))
}

fn seat(name: u32, capabilities: u32) -> Seat {
    Seat {
        seat: handle(usize::try_from(name).unwrap()),
        name,
        capabilities,
    }
}

/// The shim's and the C's answer for one search over `num` seats at `ptr`.
fn both_pick(ptr: *const Seat, num: usize) -> (usize, usize) {
    // SAFETY: `ptr` is null with `num` 0, or holds `num` seats.
    let rust = unsafe { obs_nix_wayland_seats_pick(ptr, num) };
    // SAFETY: the same memory, with the oracle's identical layout.
    let c = unsafe { oracle::oracle_obs_nix_wayland_seats_pick(ptr.cast::<OracleSeat>(), num) };
    (rust, c)
}

fn both_find(ptr: *const Seat, num: usize, name: u32) -> (usize, usize) {
    // SAFETY: `ptr` is null with `num` 0, or holds `num` seats.
    let rust = unsafe { obs_nix_wayland_seats_find_name(ptr, num, name) };
    // SAFETY: the same memory, with the oracle's identical layout.
    let c = unsafe {
        oracle::oracle_obs_nix_wayland_seats_find_name(ptr.cast::<OracleSeat>(), num, name)
    };
    (rust, c)
}

fn c_bind(offered: u32) -> u32 {
    // SAFETY: a plain value.
    unsafe { oracle::oracle_obs_nix_wayland_seats_bind_version(offered) }
}

fn c_lacks(capabilities: u32) -> bool {
    // SAFETY: a plain value.
    unsafe { oracle::oracle_obs_nix_wayland_seats_lacks_keyboard(capabilities) }
}

fn c_uses_release(version: u32) -> bool {
    // SAFETY: a plain value.
    unsafe { oracle::oracle_obs_nix_wayland_seats_uses_release(version) }
}

fn check_searches(seats: &[Seat], names: &[u32]) {
    for num in 0..=seats.len() {
        let (rust, c) = both_pick(seats.as_ptr(), num);
        assert_eq!(rust, c, "pick of {num} seats {seats:?}");
        for &name in names {
            let (rust, c) = both_find(seats.as_ptr(), num, name);
            assert_eq!(rust, c, "find {name} in {num} seats {seats:?}");
        }
    }
}

#[test]
fn values_and_layout() {
    assert_eq!(
        value("OBS_NIX_WAYLAND_SEATS_CAPABILITY_KEYBOARD"),
        i64::from(CAPABILITY_KEYBOARD)
    );
    assert_eq!(
        value("OBS_NIX_WAYLAND_SEATS_RELEASE_SINCE_VERSION"),
        i64::from(RELEASE_SINCE_VERSION)
    );
    let n = |v: usize| i64::try_from(v).unwrap();
    assert_eq!(
        value("sizeof(struct obs_nix_wayland_seat)"),
        n(size_of::<Seat>())
    );
    assert_eq!(
        value("alignof(struct obs_nix_wayland_seat)"),
        n(align_of::<Seat>())
    );
    assert_eq!(
        value("offsetof(struct obs_nix_wayland_seat, seat)"),
        n(std::mem::offset_of!(Seat, seat))
    );
    assert_eq!(
        value("offsetof(struct obs_nix_wayland_seat, name)"),
        n(std::mem::offset_of!(Seat, name))
    );
    assert_eq!(
        value("offsetof(struct obs_nix_wayland_seat, capabilities)"),
        n(std::mem::offset_of!(Seat, capabilities))
    );
    assert_eq!(size_of::<OracleSeat>(), size_of::<Seat>());
    // SAFETY: the getter takes any index.
    assert_eq!(
        unsafe { oracle::oracle_wayland_seats_value(VALUE_NAMES.len()) },
        -1
    );
}

#[test]
fn versions() {
    let edges = (u32::MAX - 31..=u32::MAX).chain([0x7fff_ffff, 0x8000_0000]);
    for v in (0..=0xffff).chain(edges) {
        assert_eq!(obs_nix_wayland_seats_bind_version(v), c_bind(v), "bind {v}");
        assert_eq!(
            obs_nix_wayland_seats_uses_release(v),
            c_uses_release(v),
            "release {v}"
        );
    }
}

#[test]
fn capability_changes() {
    for caps in (0..=0xfff).chain([!CAPABILITY_KEYBOARD, u32::MAX, 0x8000_0002]) {
        assert_eq!(
            obs_nix_wayland_seats_lacks_keyboard(caps),
            c_lacks(caps),
            "caps {caps:#x}"
        );
    }
}

#[test]
fn searches_on_fixed_arrays() {
    // A DARRAY that never grew: NULL and 0.
    assert_eq!(both_pick(std::ptr::null(), 0), (usize::MAX, usize::MAX));
    assert_eq!(both_find(std::ptr::null(), 0, 0), (usize::MAX, usize::MAX));

    let names = [0, 1, 2, 3, 5, 7, 9, 21, 22, u32::MAX];
    check_searches(&[], &names);
    check_searches(&[seat(1, CAPABILITY_KEYBOARD)], &names);
    check_searches(
        &[
            seat(1, 1),
            seat(2, 4),
            seat(3, 5),
            seat(5, !CAPABILITY_KEYBOARD),
        ],
        &names,
    );
    check_searches(&[seat(9, 1), seat(7, 2), seat(3, 3)], &names);
    check_searches(&[seat(5, 0), seat(5, 2), seat(5, u32::MAX)], &names);
    check_searches(
        &[seat(u32::MAX, 0), seat(0, 6), seat(21, 2), seat(22, 3)],
        &names,
    );
}

/// One step of the platform, as `obs-nix-wayland.c` takes it.
#[derive(Debug, Clone)]
enum Event {
    /// `platform_registry_handler()` for a `wl_seat` (or another global).
    Global { name: u32, version: u32, seat: bool },
    /// `platform_seat_capabilities()` for the seat at this index of the
    /// array (modulo its length), or for a seat not in it.
    Capabilities { which: Option<usize>, caps: u32 },
    /// `platform_registry_remover()`.
    Remove { name: u32 },
}

/// The seat decisions, from the shims or from the C.
trait Decide {
    fn bind_version(&self, offered: u32) -> u32;
    fn pick(&self, seats: &[Seat]) -> usize;
    fn lacks_keyboard(&self, caps: u32) -> bool;
    fn find_name(&self, seats: &[Seat], name: u32) -> usize;
    fn uses_release(&self, version: u32) -> bool;
}

struct Shims;
struct Oracle;

/// A DARRAY's array pointer: null when empty, as one that never grew.
fn darray(seats: &[Seat]) -> *const Seat {
    if seats.is_empty() {
        std::ptr::null()
    } else {
        seats.as_ptr()
    }
}

impl Decide for Shims {
    fn bind_version(&self, offered: u32) -> u32 {
        obs_nix_wayland_seats_bind_version(offered)
    }
    fn pick(&self, seats: &[Seat]) -> usize {
        // SAFETY: the darray's pointer and length.
        unsafe { obs_nix_wayland_seats_pick(darray(seats), seats.len()) }
    }
    fn lacks_keyboard(&self, caps: u32) -> bool {
        obs_nix_wayland_seats_lacks_keyboard(caps)
    }
    fn find_name(&self, seats: &[Seat], name: u32) -> usize {
        // SAFETY: the darray's pointer and length.
        unsafe { obs_nix_wayland_seats_find_name(darray(seats), seats.len(), name) }
    }
    fn uses_release(&self, version: u32) -> bool {
        obs_nix_wayland_seats_uses_release(version)
    }
}

impl Decide for Oracle {
    fn bind_version(&self, offered: u32) -> u32 {
        c_bind(offered)
    }
    fn pick(&self, seats: &[Seat]) -> usize {
        both_pick(darray(seats), seats.len()).1
    }
    fn lacks_keyboard(&self, caps: u32) -> bool {
        c_lacks(caps)
    }
    fn find_name(&self, seats: &[Seat], name: u32) -> usize {
        both_find(darray(seats), seats.len(), name).1
    }
    fn uses_release(&self, version: u32) -> bool {
        c_uses_release(version)
    }
}

/// The Wayland hotkey platform's seat state and the requests it makes, with
/// obs-nix-wayland.c's handlers around the decisions.
struct Platform<'d, D: Decide> {
    decide: &'d D,
    seats: Vec<Seat>,
    /// Bound version of each made-up `wl_seat`, by handle id.
    versions: Vec<u32>,
    keyboard: Option<*mut c_void>,
    keyboard_seat: Option<*mut c_void>,
    log: Vec<String>,
}

impl<'d, D: Decide> Platform<'d, D> {
    fn new(decide: &'d D) -> Self {
        Platform {
            decide,
            seats: Vec::new(),
            versions: Vec::new(),
            keyboard: None,
            keyboard_seat: None,
            log: Vec::new(),
        }
    }

    fn id(seat: *mut c_void) -> usize {
        (seat.addr() - 16) / 16
    }

    fn release_keyboard(&mut self) {
        self.log.push(format!(
            "wl_keyboard_release {:?}",
            self.keyboard.map(Self::id)
        ));
        self.keyboard = None;
        self.keyboard_seat = None;
        self.log.push("clear keys".into());
    }

    fn take_keyboard(&mut self) {
        if self.keyboard.is_some() {
            return;
        }
        let i = self.decide.pick(&self.seats);
        if i == usize::MAX {
            return;
        }
        let seat = self.seats[i].seat;
        self.log
            .push(format!("wl_seat_get_keyboard {}", Self::id(seat)));
        self.keyboard = Some(seat);
        self.keyboard_seat = Some(seat);
    }

    fn release_seat(&mut self, seat: *mut c_void) {
        let version = self.versions[Self::id(seat)];
        if self.decide.uses_release(version) {
            self.log.push(format!("wl_seat_release {}", Self::id(seat)));
        } else {
            self.log.push(format!("wl_seat_destroy {}", Self::id(seat)));
        }
    }

    fn step(&mut self, event: &Event) {
        match *event {
            Event::Global {
                name,
                version,
                seat,
            } => {
                if !seat {
                    return;
                }
                let bind = self.decide.bind_version(version);
                if bind == 0 {
                    self.log
                        .push("hotkeys disabled, compositor is too old".into());
                    return;
                }
                let handle = handle(self.versions.len());
                self.versions.push(bind);
                self.log.push(format!(
                    "wl_registry_bind {name} v{bind} -> {}",
                    Self::id(handle)
                ));
                self.seats.push(Seat {
                    seat: handle,
                    name,
                    capabilities: 0,
                });
            }
            Event::Capabilities { which, caps } => {
                let seat = match which {
                    Some(i) if !self.seats.is_empty() => self.seats[i % self.seats.len()].seat,
                    // find_seat() finds nothing: the event is ignored.
                    _ => return,
                };
                let entry = self.seats.iter().position(|s| s.seat == seat).unwrap();
                self.seats[entry].capabilities = caps;
                // The keyboard's seat is compared only for a seat without
                // a keyboard, as in C.
                if self.decide.lacks_keyboard(caps) && Some(seat) == self.keyboard_seat {
                    self.release_keyboard();
                }
                self.take_keyboard();
            }
            Event::Remove { name } => {
                let i = self.decide.find_name(&self.seats, name);
                if i == usize::MAX {
                    return;
                }
                let seat = self.seats[i].seat;
                if Some(seat) == self.keyboard_seat {
                    self.release_keyboard();
                }
                self.release_seat(seat);
                self.seats.remove(i);
                self.take_keyboard();
            }
        }
        let state: Vec<_> = self
            .seats
            .iter()
            .map(|s| (Self::id(s.seat), s.name, s.capabilities))
            .collect();
        self.log.push(format!(
            "state {state:?} keyboard {:?}",
            self.keyboard_seat.map(Self::id)
        ));
    }

    /// `obs_nix_wayland_hotkeys_platform_free()`'s seat steps.
    fn free(&mut self) {
        if self.keyboard.is_some() {
            self.log.push("wl_keyboard_release".into());
        }
        for seat in self.seats.clone() {
            self.release_seat(seat.seat);
        }
    }
}

fn run<D: Decide>(decide: &D, events: &[Event]) -> Vec<String> {
    let mut platform = Platform::new(decide);
    for event in events {
        platform.step(event);
    }
    platform.free();
    platform.log
}

#[test]
fn handover_runs() {
    // Two keyboard seats; the keyboard's seat loses its keyboard, gets it
    // back, and is removed; a seat that is too old; unknown names.
    let events = [
        Event::Global {
            name: 21,
            version: 7,
            seat: true,
        },
        Event::Global {
            name: 22,
            version: 4,
            seat: true,
        },
        Event::Global {
            name: 23,
            version: 3,
            seat: true,
        },
        Event::Global {
            name: 30,
            version: 4,
            seat: false,
        },
        Event::Capabilities {
            which: Some(0),
            caps: 2,
        },
        Event::Capabilities {
            which: Some(1),
            caps: 3,
        },
        Event::Capabilities {
            which: Some(1),
            caps: 1,
        },
        Event::Capabilities {
            which: Some(0),
            caps: 0,
        },
        Event::Capabilities {
            which: Some(1),
            caps: 7,
        },
        Event::Capabilities {
            which: None,
            caps: 0,
        },
        Event::Capabilities {
            which: Some(0),
            caps: 2,
        },
        Event::Remove { name: 30 },
        Event::Remove { name: 23 },
        Event::Remove { name: 22 },
        Event::Remove { name: 22 },
        Event::Global {
            name: 24,
            version: u32::MAX,
            seat: true,
        },
        Event::Capabilities {
            which: Some(1),
            caps: u32::MAX,
        },
        Event::Remove { name: 21 },
    ];
    let rust = run(&Shims, &events);
    let c = run(&Oracle, &events);
    assert_eq!(rust, c);
    assert!(rust.iter().any(|l| l == "wl_seat_get_keyboard 1"));
    assert!(rust.iter().any(|l| l == "wl_seat_destroy 1"));
    assert!(rust.iter().any(|l| l == "wl_seat_release 0"));
    assert!(rust.iter().any(|l| l.starts_with("hotkeys disabled")));
}

fn version() -> impl Strategy<Value = u32> {
    prop_oneof![0u32..10, Just(u32::MAX), any::<u32>()]
}

fn caps() -> impl Strategy<Value = u32> {
    prop_oneof![4 => 0u32..8, 1 => any::<u32>()]
}

fn event() -> impl Strategy<Value = Event> {
    prop_oneof![
        3 => (0u32..12, version(), prop::bool::weighted(0.85))
            .prop_map(|(name, version, seat)| Event::Global { name, version, seat }),
        4 => (prop::option::weighted(0.9, 0usize..8), caps())
            .prop_map(|(which, caps)| Event::Capabilities { which, caps }),
        2 => (0u32..14).prop_map(|name| Event::Remove { name }),
    ]
}

fn seats() -> impl Strategy<Value = Vec<Seat>> {
    prop::collection::vec((prop_oneof![0u32..6, any::<u32>()], caps()), 0..12)
        .prop_map(|v| v.into_iter().map(|(name, caps)| seat(name, caps)).collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn prop_values(v in any::<u32>()) {
        prop_assert_eq!(obs_nix_wayland_seats_bind_version(v), c_bind(v));
        prop_assert_eq!(obs_nix_wayland_seats_uses_release(v), c_uses_release(v));
        prop_assert_eq!(obs_nix_wayland_seats_lacks_keyboard(v), c_lacks(v));
    }

    #[test]
    fn prop_searches(s in seats(), name in prop_oneof![0u32..6, any::<u32>()]) {
        for num in 0..=s.len() {
            let (rust, c) = both_pick(s.as_ptr(), num);
            prop_assert_eq!(rust, c);
            let (rust, c) = both_find(s.as_ptr(), num, name);
            prop_assert_eq!(rust, c);
        }
    }

    #[test]
    fn prop_platform_runs(events in prop::collection::vec(event(), 1..48)) {
        prop_assert_eq!(run(&Shims, &events), run(&Oracle, &events));
    }
}
