#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include "obs-nix-wayland-key-names.h"

/*
 * Characterization of the Wayland key names of libobs/obs-nix-wayland.c,
 * moved to libobs/obs-nix-wayland-key-names.c before their Rust port:
 * - obs_nix_wayland_key_names_route(): obs_nix_wayland_key_to_str()'s choice
 *   of how to name a key, with the default names of the fixed keys.
 *
 * The function is hidden in libobs, so this test compiles the C source into
 * itself and calls only it. No compositor is needed. No test reaches the
 * caller, obs_nix_wayland_key_to_str(), through the real libobs: the Tier 3
 * parity test in rust/obs-hotkey compares the port with this C.
 */

/* Not a value the function writes: shows what it left alone. */
#define UNSET_NUMBER (-12345)

static const char unset_name[] = "unset";

struct route {
	enum obs_nix_wayland_key_names_route route;
	int number;
	const char *name;
};

static struct route route_of(obs_key_t key)
{
	struct route r = {OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_NONE, UNSET_NUMBER, unset_name};
	r.route = obs_nix_wayland_key_names_route(key, &r.number, &r.name);
	return r;
}

static void assert_numbered(obs_key_t key, enum obs_nix_wayland_key_names_route route, int number)
{
	struct route r = route_of(key);
	assert_int_equal(r.route, route);
	assert_int_equal(r.number, number);
	assert_ptr_equal(r.name, unset_name);
}

static void assert_fixed(obs_key_t key, const char *name)
{
	struct route r = route_of(key);
	assert_int_equal(r.route, OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_FIXED);
	assert_int_equal(r.number, UNSET_NUMBER);
	assert_string_equal(r.name, name);
}

static void assert_keysym(obs_key_t key)
{
	struct route r = route_of(key);
	assert_int_equal(r.route, OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_KEYSYM);
	assert_int_equal(r.number, UNSET_NUMBER);
	assert_ptr_equal(r.name, unset_name);
}

/* Mouse 1 to 29, numbered from 1. */
static void route_mouse(void **state)
{
	(void)state;
	for (int n = 1; n <= 29; n++)
		assert_numbered((obs_key_t)(OBS_KEY_MOUSE1 + n - 1), OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_MOUSE, n);
	assert_numbered(OBS_KEY_MOUSE29, OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_MOUSE, 29);
}

/* Numpad 0 to 9, numbered from 0. */
static void route_numpad(void **state)
{
	(void)state;
	for (int n = 0; n <= 9; n++)
		assert_numbered((obs_key_t)(OBS_KEY_NUM0 + n), OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_NUMPAD, n);
	assert_numbered(OBS_KEY_NUM9, OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_NUMPAD, 9);
}

/* The 30 keys with a default name, Meta named "Super". */
static const struct {
	obs_key_t key;
	const char *name;
} fixed_keys[] = {
	{OBS_KEY_INSERT, "Insert"},
	{OBS_KEY_DELETE, "Delete"},
	{OBS_KEY_HOME, "Home"},
	{OBS_KEY_END, "End"},
	{OBS_KEY_PAGEUP, "Page Up"},
	{OBS_KEY_PAGEDOWN, "Page Down"},
	{OBS_KEY_NUMLOCK, "Num Lock"},
	{OBS_KEY_SCROLLLOCK, "Scroll Lock"},
	{OBS_KEY_CAPSLOCK, "Caps Lock"},
	{OBS_KEY_BACKSPACE, "Backspace"},
	{OBS_KEY_TAB, "Tab"},
	{OBS_KEY_PRINT, "Print"},
	{OBS_KEY_PAUSE, "Pause"},
	{OBS_KEY_LEFT, "Left"},
	{OBS_KEY_RIGHT, "Right"},
	{OBS_KEY_UP, "Up"},
	{OBS_KEY_DOWN, "Down"},
	{OBS_KEY_SHIFT, "Shift"},
	{OBS_KEY_ALT, "Alt"},
	{OBS_KEY_CONTROL, "Control"},
	{OBS_KEY_META, "Super"},
	{OBS_KEY_MENU, "Menu"},
	{OBS_KEY_NUMASTERISK, "Numpad *"},
	{OBS_KEY_NUMPLUS, "Numpad +"},
	{OBS_KEY_NUMMINUS, "Numpad -"},
	{OBS_KEY_NUMCOMMA, "Numpad ,"},
	{OBS_KEY_NUMPERIOD, "Numpad ."},
	{OBS_KEY_NUMSLASH, "Numpad /"},
	{OBS_KEY_SPACE, "Space"},
	{OBS_KEY_ESCAPE, "Escape"},
};

#define FIXED_KEYS (sizeof(fixed_keys) / sizeof(fixed_keys[0]))

static void route_fixed(void **state)
{
	(void)state;
	assert_int_equal(FIXED_KEYS, 30);
	for (size_t i = 0; i < FIXED_KEYS; i++)
		assert_fixed(fixed_keys[i].key, fixed_keys[i].name);
}

/* F1 to F35 by number, with no translation lookup (characterized, not
 * endorsed: every other named key has one). */
static void route_function(void **state)
{
	(void)state;
	for (int n = 1; n <= 35; n++)
		assert_numbered((obs_key_t)(OBS_KEY_F1 + n - 1), OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_FUNCTION, n);
}

/* Every other key goes to its keysym, OBS_KEY_NONE too. */
static void route_keysym(void **state)
{
	(void)state;
	assert_keysym(OBS_KEY_NONE);
	assert_keysym(OBS_KEY_A);
	assert_keysym(OBS_KEY_1);
	assert_keysym(OBS_KEY_RETURN);
	assert_keysym(OBS_KEY_ENTER);
	assert_keysym(OBS_KEY_NUMEQUAL);
	assert_keysym(OBS_KEY_VK_VOLUME_UP);
	assert_keysym((obs_key_t)(OBS_KEY_LAST_VALUE - 1));
}

/* Every key of the table, OBS_KEY_NONE to OBS_KEY_LAST_VALUE - 1: the route
 * of each, by the ranges and the fixed list above, and how many take each. */
static void route_every_key(void **state)
{
	(void)state;
	int counts[OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_KEYSYM + 1] = {0};

	for (int k = 0; k < OBS_KEY_LAST_VALUE; k++) {
		obs_key_t key = (obs_key_t)k;
		const char *fixed = NULL;
		for (size_t i = 0; i < FIXED_KEYS; i++) {
			if (fixed_keys[i].key == key)
				fixed = fixed_keys[i].name;
		}

		if (key >= OBS_KEY_MOUSE1 && key <= OBS_KEY_MOUSE29)
			assert_numbered(key, OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_MOUSE, (int)(key - OBS_KEY_MOUSE1 + 1));
		else if (key >= OBS_KEY_NUM0 && key <= OBS_KEY_NUM9)
			assert_numbered(key, OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_NUMPAD, (int)(key - OBS_KEY_NUM0));
		else if (fixed)
			assert_fixed(key, fixed);
		else if (key >= OBS_KEY_F1 && key <= OBS_KEY_F35)
			assert_numbered(key, OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_FUNCTION, (int)(key - OBS_KEY_F1 + 1));
		else
			assert_keysym(key);

		counts[route_of(key).route]++;
	}

	assert_int_equal(counts[OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_NONE], 0);
	assert_int_equal(counts[OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_MOUSE], 29);
	assert_int_equal(counts[OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_NUMPAD], 10);
	assert_int_equal(counts[OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_FIXED], 30);
	assert_int_equal(counts[OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_FUNCTION], 35);
	assert_int_equal(counts[OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_KEYSYM], OBS_KEY_LAST_VALUE - 29 - 10 - 30 - 35);
}

/* The route values are those of enum obs_nix_x11_hotkeys_route, KEYSYM in the
 * place of KEYCODES. */
static void route_values(void **state)
{
	(void)state;
	assert_int_equal(OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_NONE, 0);
	assert_int_equal(OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_MOUSE, 1);
	assert_int_equal(OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_NUMPAD, 2);
	assert_int_equal(OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_FIXED, 3);
	assert_int_equal(OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_FUNCTION, 4);
	assert_int_equal(OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_KEYSYM, 5);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(route_mouse),    cmocka_unit_test(route_numpad), cmocka_unit_test(route_fixed),
		cmocka_unit_test(route_function), cmocka_unit_test(route_keysym), cmocka_unit_test(route_every_key),
		cmocka_unit_test(route_values),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
