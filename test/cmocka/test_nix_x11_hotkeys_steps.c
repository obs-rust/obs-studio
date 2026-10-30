#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include "obs-nix-x11-hotkeys.h"

/*
 * Characterization of the X11 key names and raw button decode of
 * libobs/obs-nix-x11.c, moved to libobs/obs-nix-x11-hotkeys.c before their
 * Rust port:
 * - obs_nix_x11_hotkeys_route(): obs_nix_x11_key_to_str()'s choice of how to
 *   name a key, with the default names of the fixed keys;
 * - obs_nix_x11_hotkeys_raw_button(): mouse_button_pressed()'s decode of one
 *   polled event with XInput.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. No X server is needed: the events are
 * their three fields, set by hand. test_x11_hotkey_layout and
 * test_hotkey_translations cover the key names through the real libobs.
 */

/* Not a value the functions write: shows what they left alone. */
#define UNSET_NUMBER (-12345)
#define UNSET_SLOT 0xdeadbeefu

static const char unset_name[] = "unset";

struct route {
	enum obs_nix_x11_hotkeys_route route;
	int number;
	const char *name;
};

static struct route route_of(obs_key_t key)
{
	struct route r = {OBS_NIX_X11_HOTKEYS_ROUTE_NONE, UNSET_NUMBER, unset_name};
	r.route = obs_nix_x11_hotkeys_route(key, &r.number, &r.name);
	return r;
}

static void assert_numbered(obs_key_t key, enum obs_nix_x11_hotkeys_route route, int number)
{
	struct route r = route_of(key);
	assert_int_equal(r.route, route);
	assert_int_equal(r.number, number);
	assert_ptr_equal(r.name, unset_name);
}

static void assert_fixed(obs_key_t key, const char *name)
{
	struct route r = route_of(key);
	assert_int_equal(r.route, OBS_NIX_X11_HOTKEYS_ROUTE_FIXED);
	assert_int_equal(r.number, UNSET_NUMBER);
	assert_string_equal(r.name, name);
}

static void assert_keycodes(obs_key_t key)
{
	struct route r = route_of(key);
	assert_int_equal(r.route, OBS_NIX_X11_HOTKEYS_ROUTE_KEYCODES);
	assert_int_equal(r.number, UNSET_NUMBER);
	assert_ptr_equal(r.name, unset_name);
}

/* Mouse 1 to 29, numbered from 1. */
static void route_mouse(void **state)
{
	(void)state;
	for (int n = 1; n <= 29; n++)
		assert_numbered((obs_key_t)(OBS_KEY_MOUSE1 + n - 1), OBS_NIX_X11_HOTKEYS_ROUTE_MOUSE, n);
	assert_numbered(OBS_KEY_MOUSE29, OBS_NIX_X11_HOTKEYS_ROUTE_MOUSE, 29);
}

/* Numpad 0 to 9, numbered from 0. */
static void route_numpad(void **state)
{
	(void)state;
	for (int n = 0; n <= 9; n++)
		assert_numbered((obs_key_t)(OBS_KEY_NUM0 + n), OBS_NIX_X11_HOTKEYS_ROUTE_NUMPAD, n);
	assert_numbered(OBS_KEY_NUM9, OBS_NIX_X11_HOTKEYS_ROUTE_NUMPAD, 9);
}

/* The 30 keys with a default name, Meta named "Super". */
static void route_fixed(void **state)
{
	(void)state;
	assert_fixed(OBS_KEY_INSERT, "Insert");
	assert_fixed(OBS_KEY_DELETE, "Delete");
	assert_fixed(OBS_KEY_HOME, "Home");
	assert_fixed(OBS_KEY_END, "End");
	assert_fixed(OBS_KEY_PAGEUP, "Page Up");
	assert_fixed(OBS_KEY_PAGEDOWN, "Page Down");
	assert_fixed(OBS_KEY_NUMLOCK, "Num Lock");
	assert_fixed(OBS_KEY_SCROLLLOCK, "Scroll Lock");
	assert_fixed(OBS_KEY_CAPSLOCK, "Caps Lock");
	assert_fixed(OBS_KEY_BACKSPACE, "Backspace");
	assert_fixed(OBS_KEY_TAB, "Tab");
	assert_fixed(OBS_KEY_PRINT, "Print");
	assert_fixed(OBS_KEY_PAUSE, "Pause");
	assert_fixed(OBS_KEY_LEFT, "Left");
	assert_fixed(OBS_KEY_RIGHT, "Right");
	assert_fixed(OBS_KEY_UP, "Up");
	assert_fixed(OBS_KEY_DOWN, "Down");
	assert_fixed(OBS_KEY_SHIFT, "Shift");
	assert_fixed(OBS_KEY_ALT, "Alt");
	assert_fixed(OBS_KEY_CONTROL, "Control");
	assert_fixed(OBS_KEY_META, "Super");
	assert_fixed(OBS_KEY_MENU, "Menu");
	assert_fixed(OBS_KEY_NUMASTERISK, "Numpad *");
	assert_fixed(OBS_KEY_NUMPLUS, "Numpad +");
	assert_fixed(OBS_KEY_NUMMINUS, "Numpad -");
	assert_fixed(OBS_KEY_NUMCOMMA, "Numpad ,");
	assert_fixed(OBS_KEY_NUMPERIOD, "Numpad .");
	assert_fixed(OBS_KEY_NUMSLASH, "Numpad /");
	assert_fixed(OBS_KEY_SPACE, "Space");
	assert_fixed(OBS_KEY_ESCAPE, "Escape");
}

/* F1 to F35 by number, with no translation lookup (characterized, not
 * endorsed: every other named key has one). */
static void route_function(void **state)
{
	(void)state;
	assert_numbered(OBS_KEY_F1, OBS_NIX_X11_HOTKEYS_ROUTE_FUNCTION, 1);
	assert_numbered(OBS_KEY_F12, OBS_NIX_X11_HOTKEYS_ROUTE_FUNCTION, 12);
	assert_numbered(OBS_KEY_F24, OBS_NIX_X11_HOTKEYS_ROUTE_FUNCTION, 24);
	assert_numbered(OBS_KEY_F35, OBS_NIX_X11_HOTKEYS_ROUTE_FUNCTION, 35);
}

/* Every other key goes to its keycodes, OBS_KEY_NONE too. */
static void route_keycodes(void **state)
{
	(void)state;
	assert_keycodes(OBS_KEY_NONE);
	assert_keycodes(OBS_KEY_A);
	assert_keycodes(OBS_KEY_1);
	assert_keycodes(OBS_KEY_RETURN);
	assert_keycodes(OBS_KEY_ENTER);
	assert_keycodes(OBS_KEY_NUMEQUAL);
	assert_keycodes(OBS_KEY_VK_VOLUME_UP);
	assert_keycodes((obs_key_t)(OBS_KEY_LAST_VALUE - 1));
}

/* A value outside the key table also goes to the keycodes, which
 * obs_nix_x11_key_to_str() indexes with it (characterized, not endorsed). */
static void route_outside(void **state)
{
	(void)state;
	assert_keycodes((obs_key_t)-1);
	assert_keycodes(OBS_KEY_LAST_VALUE);
	assert_keycodes((obs_key_t)(OBS_KEY_LAST_VALUE + 1));
}

struct button {
	enum obs_nix_x11_hotkeys_button button;
	uint32_t slot;
};

/* NULL fields are ones the decode must not read. */
static struct button decode(uint8_t response_type, const uint16_t *event_type, const uint32_t *detail)
{
	struct button b = {OBS_NIX_X11_HOTKEYS_BUTTON_IGNORE, UNSET_SLOT};
	b.button = obs_nix_x11_hotkeys_raw_button(response_type, event_type, detail, &b.slot);
	return b;
}

static void assert_button(uint8_t response_type, uint16_t event_type, uint32_t detail,
			  enum obs_nix_x11_hotkeys_button button, uint32_t slot)
{
	struct button b = decode(response_type, &event_type, &detail);
	assert_int_equal(b.button, button);
	assert_int_equal(b.slot, slot);
}

/* A raw press or release of buttons 1 to 32 sets slots 0 to 31. */
static void raw_press_release(void **state)
{
	(void)state;
	for (uint32_t d = 1; d < XINPUT_MOUSE_LEN; d++) {
		assert_button(OBS_NIX_X11_HOTKEYS_GE_GENERIC, OBS_NIX_X11_HOTKEYS_RAW_BUTTON_PRESS, d,
			      OBS_NIX_X11_HOTKEYS_BUTTON_PRESS, d - 1);
		assert_button(OBS_NIX_X11_HOTKEYS_GE_GENERIC, OBS_NIX_X11_HOTKEYS_RAW_BUTTON_RELEASE, d,
			      OBS_NIX_X11_HOTKEYS_BUTTON_RELEASE, d - 1);
	}
	assert_button(OBS_NIX_X11_HOTKEYS_GE_GENERIC, OBS_NIX_X11_HOTKEYS_RAW_BUTTON_PRESS, 32,
		      OBS_NIX_X11_HOTKEYS_BUTTON_PRESS, 31);
}

/* Button 33 and up are unsupported, and leave the slot alone. */
static void raw_unsupported(void **state)
{
	(void)state;
	const uint32_t details[] = {33, 34, 255, 256, 0x7fffffff, 0xffffffff};
	for (size_t i = 0; i < sizeof(details) / sizeof(details[0]); i++) {
		assert_button(OBS_NIX_X11_HOTKEYS_GE_GENERIC, OBS_NIX_X11_HOTKEYS_RAW_BUTTON_PRESS, details[i],
			      OBS_NIX_X11_HOTKEYS_BUTTON_UNSUPPORTED, UNSET_SLOT);
		assert_button(OBS_NIX_X11_HOTKEYS_GE_GENERIC, OBS_NIX_X11_HOTKEYS_RAW_BUTTON_RELEASE, details[i],
			      OBS_NIX_X11_HOTKEYS_BUTTON_UNSUPPORTED, UNSET_SLOT);
	}
}

/* Button 0 passes the bound and gives slot 0 - 1, which wraps to
 * UINT32_MAX (characterized, not endorsed: obs-nix-x11.c stores it there). */
static void raw_button_zero(void **state)
{
	(void)state;
	assert_button(OBS_NIX_X11_HOTKEYS_GE_GENERIC, OBS_NIX_X11_HOTKEYS_RAW_BUTTON_PRESS, 0,
		      OBS_NIX_X11_HOTKEYS_BUTTON_PRESS, UINT32_MAX);
	assert_button(OBS_NIX_X11_HOTKEYS_GE_GENERIC, OBS_NIX_X11_HOTKEYS_RAW_BUTTON_RELEASE, 0,
		      OBS_NIX_X11_HOTKEYS_BUTTON_RELEASE, UINT32_MAX);
}

/* Another XInput event type is ignored, and its detail not read. */
static void raw_other_types(void **state)
{
	(void)state;
	const uint16_t types[] = {0, 1, 13, 14, 17, 18, 31, 0x10f, 0xffff};
	for (size_t i = 0; i < sizeof(types) / sizeof(types[0]); i++) {
		struct button b = decode(OBS_NIX_X11_HOTKEYS_GE_GENERIC, &types[i], NULL);
		assert_int_equal(b.button, OBS_NIX_X11_HOTKEYS_BUTTON_IGNORE);
		assert_int_equal(b.slot, UNSET_SLOT);
	}
}

/* The generic event test masks the response type with ~80, decimal: 35,
 * 51, 99 and 115 are generic, and 35 with the send-event bit (0x80) is not
 * (characterized, not endorsed: the send-event bit is 0x80, not 80). */
static void raw_generic_mask(void **state)
{
	(void)state;
	const uint8_t generic[] = {35, 35 | 16, 35 | 64, 35 | 80};
	for (size_t i = 0; i < sizeof(generic) / sizeof(generic[0]); i++)
		assert_button(generic[i], OBS_NIX_X11_HOTKEYS_RAW_BUTTON_PRESS, 3, OBS_NIX_X11_HOTKEYS_BUTTON_PRESS, 2);
	assert_int_equal(35 | 16, 51);
	assert_int_equal(35 | 64, 99);
	assert_int_equal(35 | 80, 115);

	/* Not generic: the event type is not read. */
	const uint8_t other[] = {0, 2, 3, 4, 5, 34, 36, 0x80 | 35, 0xff, 0x80 | 51};
	for (size_t i = 0; i < sizeof(other) / sizeof(other[0]); i++) {
		struct button b = decode(other[i], NULL, NULL);
		assert_int_equal(b.button, OBS_NIX_X11_HOTKEYS_BUTTON_IGNORE);
		assert_int_equal(b.slot, UNSET_SLOT);
	}
}

/* The event codes are X's (xcb/xproto.h, xcb/xinput.h); obs-nix-x11.c checks
 * them. */
static void event_codes(void **state)
{
	(void)state;
	assert_int_equal(OBS_NIX_X11_HOTKEYS_GE_GENERIC, 35);
	assert_int_equal(OBS_NIX_X11_HOTKEYS_RAW_BUTTON_PRESS, 15);
	assert_int_equal(OBS_NIX_X11_HOTKEYS_RAW_BUTTON_RELEASE, 16);
	assert_int_equal(XINPUT_MOUSE_LEN, 33);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(route_mouse),       cmocka_unit_test(route_numpad),
		cmocka_unit_test(route_fixed),       cmocka_unit_test(route_function),
		cmocka_unit_test(route_keycodes),    cmocka_unit_test(route_outside),
		cmocka_unit_test(raw_press_release), cmocka_unit_test(raw_unsupported),
		cmocka_unit_test(raw_button_zero),   cmocka_unit_test(raw_other_types),
		cmocka_unit_test(raw_generic_mask),  cmocka_unit_test(event_codes),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
