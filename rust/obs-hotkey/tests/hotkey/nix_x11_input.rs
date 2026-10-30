//! Tier 1: the safe core of `libobs/obs-nix-x11-input.c.h`.
//!
//! The first eleven tests port `test/cmocka/test_nix_x11_input_steps.c` one
//! to one, with the same names, inputs and expectations, in the same order.
//! Each starts from the C test's `setup()`: [`Arrays::new`].
//!
//! The cases after those pin the edges of each function, and the proptests
//! check each one against a model written from the C.

use obs_hotkey::name_map::{
    self as k, OBS_KEY_LAST_VALUE, OBS_KEY_MOUSE1, OBS_KEY_MOUSE2, OBS_KEY_MOUSE3, OBS_KEY_MOUSE4,
    OBS_KEY_MOUSE5, OBS_KEY_MOUSE6, OBS_KEY_MOUSE28, OBS_KEY_MOUSE29,
};
use obs_hotkey::nix_x11_input::{
    self as xi, BUTTON_MASK_1, BUTTON_MASK_2, BUTTON_MASK_3, MOUSE_LEN,
};
use proptest::prelude::*;

const XK_RETURN: i32 = 0xff0d;
const XK_SPACE: i32 = 0x20;
const XK_1: i32 = 0x31;
const XK_A: i32 = 0x41;
const XK_GRAVE: i32 = 0x60;
const XK_CEDILLA: i32 = 0xb8;
const XK_SHIFT_L: i32 = 0xffe1;
const XK_CONTROL_L: i32 = 0xffe3;
const XK_ALT_L: i32 = 0xffe9;
const XK_F1: i32 = 0xffbe;
const XK_KP_0: i32 = 0xffb0;
const XK_KP_MULTIPLY: i32 = 0xffaa;
const XK_KANJI: i32 = 0xff21;
const XK_DEAD_GRAVE: i32 = 0xfe50;
const XF86XK_AUDIO_PLAY: i32 = 0x1008_ff14;
const XF86XK_AUDIO_RAISE_VOLUME: i32 = 0x1008_ff13;
const XF86XK_OPEN: i32 = 0x1008_ff6b;
const SUNXK_FRONT: i32 = 0x1005_ff71;

const LAST: i32 = OBS_KEY_LAST_VALUE as i32;

/// The C test's static arrays after `setup()`.
struct Arrays {
    pressed: [bool; MOUSE_LEN],
    update: [bool; MOUSE_LEN],
    held: [bool; MOUSE_LEN],
}

impl Arrays {
    fn new() -> Self {
        Self {
            pressed: [false; MOUSE_LEN],
            update: [false; MOUSE_LEN],
            held: [false; MOUSE_LEN],
        }
    }

    /// The start of a poll: obs-nix-x11.c clears pressed and update.
    fn new_poll(&mut self) {
        self.pressed = [false; MOUSE_LEN];
        self.update = [false; MOUSE_LEN];
    }

    /// What the drain stores for a raw button press of button `n` (1-based).
    fn press(&mut self, n: usize) {
        self.pressed[n - 1] = true;
        self.update[n - 1] = true;
    }

    fn release(&mut self, n: usize) {
        self.update[n - 1] = true;
    }

    fn answer(&self, key: i32) -> bool {
        xi::mouse_pressed(key, &self.pressed, &self.held)
    }

    fn commit(&mut self) {
        xi::commit(&self.update, &self.pressed, &mut self.held);
    }
}

/// The C test's `slots` map: each mouse key and the slot it reads.
const SLOTS: [(i32, usize); 29] = [
    (k::OBS_KEY_MOUSE1, 0),
    (k::OBS_KEY_MOUSE2, 2),
    (k::OBS_KEY_MOUSE3, 1),
    (k::OBS_KEY_MOUSE4, 7),
    (k::OBS_KEY_MOUSE5, 8),
    (k::OBS_KEY_MOUSE6, 9),
    (k::OBS_KEY_MOUSE7, 10),
    (k::OBS_KEY_MOUSE8, 11),
    (k::OBS_KEY_MOUSE9, 12),
    (k::OBS_KEY_MOUSE10, 13),
    (k::OBS_KEY_MOUSE11, 14),
    (k::OBS_KEY_MOUSE12, 15),
    (k::OBS_KEY_MOUSE13, 16),
    (k::OBS_KEY_MOUSE14, 17),
    (k::OBS_KEY_MOUSE15, 18),
    (k::OBS_KEY_MOUSE16, 19),
    (k::OBS_KEY_MOUSE17, 20),
    (k::OBS_KEY_MOUSE18, 21),
    (k::OBS_KEY_MOUSE19, 22),
    (k::OBS_KEY_MOUSE20, 23),
    (k::OBS_KEY_MOUSE21, 24),
    (k::OBS_KEY_MOUSE22, 25),
    (k::OBS_KEY_MOUSE23, 26),
    (k::OBS_KEY_MOUSE24, 27),
    (k::OBS_KEY_MOUSE25, 28),
    (k::OBS_KEY_MOUSE26, 29),
    (k::OBS_KEY_MOUSE27, 30),
    (k::OBS_KEY_MOUSE28, 31),
    (k::OBS_KEY_MOUSE29, 32),
];

// --- test_nix_x11_input_steps.c, one to one ---

/// keysym_table: the keysym of a sample of keys, one or two per part of
/// the table.
#[test]
fn keysym_table() {
    assert_eq!(xi::keysym(k::OBS_KEY_RETURN), XK_RETURN);
    assert_eq!(xi::keysym(k::OBS_KEY_SHIFT), XK_SHIFT_L);
    assert_eq!(xi::keysym(k::OBS_KEY_CONTROL), XK_CONTROL_L);
    assert_eq!(xi::keysym(k::OBS_KEY_ALT), XK_ALT_L);
    assert_eq!(xi::keysym(k::OBS_KEY_F1), XK_F1);
    assert_eq!(xi::keysym(k::OBS_KEY_SPACE), XK_SPACE);
    assert_eq!(xi::keysym(k::OBS_KEY_1), XK_1);
    // The upper-case keysym, though the key types lower case.
    assert_eq!(xi::keysym(k::OBS_KEY_A), XK_A);
    assert_eq!(xi::keysym(k::OBS_KEY_NUM0), XK_KP_0);
    assert_eq!(xi::keysym(k::OBS_KEY_NUMASTERISK), XK_KP_MULTIPLY);
    assert_eq!(xi::keysym(k::OBS_KEY_KANJI), XK_KANJI);
    assert_eq!(xi::keysym(k::OBS_KEY_DEAD_GRAVE), XK_DEAD_GRAVE);
    assert_eq!(xi::keysym(k::OBS_KEY_OPEN), XF86XK_OPEN);
    assert_eq!(xi::keysym(k::OBS_KEY_FRONT), SUNXK_FRONT);
    assert_eq!(
        xi::keysym(k::OBS_KEY_VK_MEDIA_PLAY_PAUSE),
        XF86XK_AUDIO_PLAY
    );
    assert_eq!(
        xi::keysym(k::OBS_KEY_VK_VOLUME_UP),
        XF86XK_AUDIO_RAISE_VOLUME
    );
}

/// keysym_shared: two keys that share a keysym (characterized, not
/// endorsed: a keysym lookup of the base table finds only the first).
#[test]
fn keysym_shared() {
    assert_eq!(xi::keysym(k::OBS_KEY_QUOTELEFT), XK_GRAVE);
    assert_eq!(xi::keysym(k::OBS_KEY_ASCIITILDE), XK_GRAVE);
    assert_eq!(xi::keysym(k::OBS_KEY_CEDILLA), XK_CEDILLA);
    assert_eq!(xi::keysym(k::OBS_KEY_CCEDILLA), XK_CEDILLA);
}

/// keysym_mouse: Mouse 1 to 5 are their number shifted left by 16; 6 to 29
/// have none.
#[test]
fn keysym_mouse() {
    assert_eq!(xi::keysym(OBS_KEY_MOUSE1), 1 << 16);
    assert_eq!(xi::keysym(OBS_KEY_MOUSE2), 2 << 16);
    assert_eq!(xi::keysym(OBS_KEY_MOUSE3), 3 << 16);
    assert_eq!(xi::keysym(OBS_KEY_MOUSE4), 4 << 16);
    assert_eq!(xi::keysym(OBS_KEY_MOUSE5), 5 << 16);
    assert_eq!(xi::keysym(OBS_KEY_MOUSE6), 0);
    assert_eq!(xi::keysym(OBS_KEY_MOUSE29), 0);
}

/// keysym_none: a key the table doesn't name has 0, and so does any value
/// that isn't a key.
#[test]
fn keysym_none() {
    assert_eq!(xi::keysym(k::OBS_KEY_NONE), 0);
    assert_eq!(xi::keysym(k::OBS_KEY_META), 0);
    assert_eq!(xi::keysym(LAST), 0);
    assert_eq!(xi::keysym(-1), 0);
}

/// slots: the slot of each obs mouse key, in the pressed array and in the
/// held one, against every slot.
#[test]
fn slots() {
    let mut a = Arrays::new();
    for (key, want) in SLOTS {
        for slot in 0..MOUSE_LEN {
            a.pressed = [false; MOUSE_LEN];
            a.held = [false; MOUSE_LEN];
            a.pressed[slot] = true;
            assert_eq!(a.answer(key), slot == want, "key {key} pressed slot {slot}");
            a.pressed[slot] = false;
            a.held[slot] = true;
            assert_eq!(a.answer(key), slot == want, "key {key} held slot {slot}");
        }
    }
}

/// mouse29_unfilled: Mouse 29 reads slot 32, which the drain never fills
/// (characterized, not endorsed: Mouse 29 never reads as pressed with
/// XInput).
#[test]
fn mouse29_unfilled() {
    let mut a = Arrays::new();
    for n in 1..MOUSE_LEN {
        a.press(n);
    }
    assert!(!a.answer(OBS_KEY_MOUSE29));
    a.commit();
    a.new_poll();
    assert!(!a.answer(OBS_KEY_MOUSE29));
    assert!(a.answer(OBS_KEY_MOUSE28));
}

/// other_keys: any other key is never pressed, whatever the arrays hold.
#[test]
fn other_keys() {
    let mut a = Arrays::new();
    a.pressed = [true; MOUSE_LEN];
    a.held = [true; MOUSE_LEN];
    assert!(!a.answer(k::OBS_KEY_NONE));
    assert!(!a.answer(k::OBS_KEY_A));
    assert!(!a.answer(LAST));
    assert!(!a.answer(-1));
}

/// press_then_release: pressed in the poll of the press, held in the polls
/// between, still pressed in the poll of the release (the answer comes
/// before the commit), released after it.
#[test]
fn press_then_release() {
    let mut a = Arrays::new();
    a.press(1);
    assert!(a.answer(OBS_KEY_MOUSE1));
    a.commit();
    assert!(a.held[0]);

    a.new_poll();
    assert!(a.answer(OBS_KEY_MOUSE1));
    a.commit();
    assert!(a.held[0]);

    a.new_poll();
    a.release(1);
    assert!(a.answer(OBS_KEY_MOUSE1));
    a.commit();
    assert!(!a.held[0]);

    a.new_poll();
    assert!(!a.answer(OBS_KEY_MOUSE1));
}

/// release_without_press: never pressed, nothing held.
#[test]
fn release_without_press() {
    let mut a = Arrays::new();
    a.release(3);
    assert!(!a.answer(OBS_KEY_MOUSE2));
    a.commit();
    assert!(!a.held[2]);
    a.new_poll();
    assert!(!a.answer(OBS_KEY_MOUSE2));
}

/// commit: an updated slot takes this poll's pressed state, any other slot
/// keeps its held state; pressed and update are left as they are.
#[test]
fn commit() {
    let mut a = Arrays::new();
    a.held[0] = true; // updated, released: false
    a.held[1] = true; // not updated: stays
    a.held[2] = false; // updated, pressed: true
    a.held[3] = false; // not updated, pressed without update: stays false
    a.update[0] = true;
    a.update[2] = true;
    a.pressed[2] = true;
    a.pressed[3] = true;
    a.held[MOUSE_LEN - 1] = true;
    a.update[MOUSE_LEN - 1] = true;

    a.commit();

    assert!(!a.held[0]);
    assert!(a.held[1]);
    assert!(a.held[2]);
    assert!(!a.held[3]);
    assert!(!a.held[MOUSE_LEN - 1]);
    assert!(a.update[0]);
    assert!(a.pressed[3]);
    assert!(!a.update[3]);
}

/// mask: Mouse 1 is button 1's mask bit, Mouse 2 button 3's and Mouse 3
/// button 2's; every other key, Mouse 4 included, is never pressed.
#[test]
fn mask() {
    assert!(xi::mask_pressed(OBS_KEY_MOUSE1, BUTTON_MASK_1));
    assert!(!xi::mask_pressed(OBS_KEY_MOUSE1, BUTTON_MASK_2));
    assert!(xi::mask_pressed(OBS_KEY_MOUSE2, BUTTON_MASK_3));
    assert!(!xi::mask_pressed(OBS_KEY_MOUSE2, BUTTON_MASK_2));
    assert!(xi::mask_pressed(OBS_KEY_MOUSE3, BUTTON_MASK_2));
    assert!(!xi::mask_pressed(OBS_KEY_MOUSE3, BUTTON_MASK_3));
    assert!(!xi::mask_pressed(OBS_KEY_MOUSE1, 0));
    assert!(!xi::mask_pressed(OBS_KEY_MOUSE1, 0xfeff));
    assert!(!xi::mask_pressed(OBS_KEY_MOUSE4, 0xffff));
    assert!(!xi::mask_pressed(k::OBS_KEY_A, 0xffff));
    assert!(!xi::mask_pressed(k::OBS_KEY_NONE, 0xffff));
    // The pointer masks are X's (xcb/xproto.h); obs-nix-x11.c checks them.
    assert_eq!(BUTTON_MASK_1, 0x100);
    assert_eq!(BUTTON_MASK_2, 0x200);
    assert_eq!(BUTTON_MASK_3, 0x400);
}

// --- Edges ---

/// The table names 285 keys; every other key, and every value outside the
/// keys, has 0.
#[test]
fn keysym_named_keys() {
    let named = (0..LAST).filter(|&key| xi::keysym(key) != 0).count();
    assert_eq!(named, 285);
    for key in [i32::MIN, -2, -1, LAST, LAST + 1, i32::MAX] {
        assert_eq!(xi::keysym(key), 0, "key {key}");
    }
}

/// Mouse 6 to 29 have no keysym; only Mouse 1 to 5 do.
#[test]
fn keysym_mouse_range() {
    for (i, (key, _)) in SLOTS.iter().enumerate() {
        let want = if i < 5 {
            i32::try_from(i + 1).unwrap() << 16
        } else {
            0
        };
        assert_eq!(xi::keysym(*key), want, "Mouse {}", i + 1);
    }
}

/// The slot of every mouse key, and none for the keys around the range and
/// for values outside the keys.
#[test]
fn slot_edges() {
    for (key, want) in SLOTS {
        assert_eq!(xi::slot(key), Some(want), "key {key}");
    }
    for key in [
        OBS_KEY_MOUSE1 - 1,
        OBS_KEY_MOUSE29 + 1,
        k::OBS_KEY_NONE,
        LAST,
        -1,
        i32::MIN,
        i32::MAX,
    ] {
        assert_eq!(xi::slot(key), None, "key {key}");
    }
    // Slots 3 to 6 (buttons 4 to 7, the wheel) have no key.
    let used: Vec<usize> = SLOTS.iter().map(|&(_, s)| s).collect();
    for wheel in 3..7 {
        assert!(!used.contains(&wheel), "slot {wheel}");
    }
}

/// A commit with nothing updated changes nothing; with everything updated
/// the held array becomes the pressed one.
#[test]
fn commit_all_or_nothing() {
    let mut a = Arrays::new();
    a.held = [true; MOUSE_LEN];
    a.pressed[5] = true;
    a.commit();
    assert_eq!(a.held, [true; MOUSE_LEN]);

    a.update = [true; MOUSE_LEN];
    a.commit();
    let mut want = [false; MOUSE_LEN];
    want[5] = true;
    assert_eq!(a.held, want);
}

/// Without XInput, no mouse key reads the modifier bits (0xff) or the wheel
/// buttons' bits (buttons 4 and 5: 0x800 and 0x1000), and Mouse 5 reads none.
#[test]
fn mask_wheel_bits() {
    for key in SLOTS.iter().map(|&(key, _)| key) {
        assert!(!xi::mask_pressed(key, 0x800 | 0x1000 | 0xff));
    }
    assert!(xi::mask_pressed(OBS_KEY_MOUSE1, 0xffff));
    assert!(xi::mask_pressed(OBS_KEY_MOUSE2, 0xffff));
    assert!(xi::mask_pressed(OBS_KEY_MOUSE3, 0xffff));
    assert!(!xi::mask_pressed(OBS_KEY_MOUSE5, 0xffff));
}

// --- Models written from the C ---

fn model_slot(key: i32) -> Option<usize> {
    if key == OBS_KEY_MOUSE1 {
        Some(0)
    } else if key == OBS_KEY_MOUSE2 {
        Some(2)
    } else if key == OBS_KEY_MOUSE3 {
        Some(1)
    } else if (OBS_KEY_MOUSE4..=OBS_KEY_MOUSE29).contains(&key) {
        Some(usize::try_from(key - OBS_KEY_MOUSE4).unwrap() + 7)
    } else {
        None
    }
}

fn model_mask(key: i32, buttons: u16) -> bool {
    let bit = if key == OBS_KEY_MOUSE1 {
        BUTTON_MASK_1
    } else if key == OBS_KEY_MOUSE2 {
        BUTTON_MASK_3
    } else if key == OBS_KEY_MOUSE3 {
        BUTTON_MASK_2
    } else {
        0
    };
    buttons & bit != 0
}

fn any_key() -> impl Strategy<Value = i32> {
    prop_oneof![
        OBS_KEY_MOUSE1 - 2..=OBS_KEY_MOUSE29 + 2,
        -2..=LAST + 2,
        any::<i32>(),
    ]
}

fn arrays() -> impl Strategy<Value = [bool; MOUSE_LEN]> {
    proptest::collection::vec(any::<bool>(), MOUSE_LEN)
        .prop_map(|v| <[bool; MOUSE_LEN]>::try_from(v).unwrap())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn slot_matches_model(key in any_key()) {
        prop_assert_eq!(xi::slot(key), model_slot(key));
    }

    #[test]
    fn mouse_pressed_matches_model(key in any_key(), pressed in arrays(), held in arrays()) {
        let want = model_slot(key).is_some_and(|s| pressed[s] || held[s]);
        prop_assert_eq!(xi::mouse_pressed(key, &pressed, &held), want);
    }

    #[test]
    fn commit_matches_model(update in arrays(), pressed in arrays(), held in arrays()) {
        let mut got = held;
        xi::commit(&update, &pressed, &mut got);
        for i in 0..MOUSE_LEN {
            let want = if update[i] { pressed[i] } else { held[i] };
            prop_assert_eq!(got[i], want, "slot {}", i);
        }
    }

    #[test]
    fn mask_matches_model(key in any_key(), buttons in any::<u16>()) {
        prop_assert_eq!(xi::mask_pressed(key, buttons), model_mask(key, buttons));
    }

    #[test]
    fn keysym_zero_outside_the_keys(key in any::<i32>()) {
        prop_assume!(!(0..LAST).contains(&key));
        prop_assert_eq!(xi::keysym(key), 0);
    }
}
