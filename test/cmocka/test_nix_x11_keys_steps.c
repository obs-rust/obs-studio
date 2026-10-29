#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include "obs-nix-x11-keys.h"

/*
 * Characterization of the X11 key map lookups of libobs/obs-nix-x11.c,
 * moved to libobs/obs-nix-x11-keys.c before their Rust port:
 * - obs_nix_x11_keys_from_base_keysym(): key_from_base_keysym();
 * - obs_nix_x11_keys_row(): fill_keycodes()'s rule for one keycode's row;
 * - pressed_of(): key_pressed()'s decision;
 * - obs_nix_x11_keys_from_keycode(): key_from_keycode();
 * - obs_nix_x11_keys_from_virtual_key() and
 *   obs_nix_x11_keys_to_virtual_key(): the virtual key vtable entries.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. No X server is needed: the key maps
 * are made up here. test_x11_hotkey_layout covers the callers through the
 * real libobs on an X server.
 *
 * The platform fields the original read only under a condition are passed
 * by pointer, or through a getter, and reads_under_their_guards pins that
 * they are read only under it.
 *
 * Not characterized here: fill_keycodes() returns true when the keyboard
 * mapping request fails (characterized, not endorsed: the caller ignores
 * it). It stays in obs-nix-x11.c with the X calls.
 */

#define SHIFT_R OBS_NIX_X11_KEYS_SHIFT_R
#define CONTROL_R OBS_NIX_X11_KEYS_CONTROL_R
#define ALT_R OBS_NIX_X11_KEYS_ALT_R
#define SUPER_L OBS_NIX_X11_KEYS_SUPER_L
#define SUPER_R OBS_NIX_X11_KEYS_SUPER_R

/* Some X11 keysyms (X11/keysymdef.h). */
#define XK_a 0x61
#define XK_b 0x62
#define XK_A 0x41
#define XK_1 0x31
#define XK_exclam 0x21
#define XK_bracketleft 0x5b
#define XK_8 0x38
#define XK_asterisk 0x2a
#define XK_Shift_L 0xffe1
#define XK_unknown 0x10ffff

static uint32_t base[OBS_KEY_LAST_VALUE];
static struct keycode_list keycodes[OBS_KEY_LAST_VALUE];

/* Every key's keycode list points into this. */
static uint8_t code_store[OBS_KEY_LAST_VALUE][4];

static int setup(void **state)
{
	(void)state;
	memset(base, 0, sizeof(base));
	memset(keycodes, 0, sizeof(keycodes));
	base[OBS_KEY_A] = XK_a;
	base[OBS_KEY_B] = XK_b;
	base[OBS_KEY_1] = XK_1;
	base[OBS_KEY_8] = XK_8;
	base[OBS_KEY_BRACKETLEFT] = XK_bracketleft;
	base[OBS_KEY_SHIFT] = XK_Shift_L;
	return 0;
}

static void set_codes(obs_key_t key, const uint8_t *codes, size_t num)
{
	memcpy(code_store[key], codes, num);
	keycodes[key].list.array = code_store[key];
	keycodes[key].list.num = num;
	keycodes[key].list.capacity = num;
}

static void set_down(uint8_t *keys, uint8_t code)
{
	keys[code / 8] |= (uint8_t)(1 << (code % 8));
}

/* The calls with the platform fields by value, as the cases below give
 * them; the functions read them through pointers. */
static bool pressed_of(const uint8_t *keys, obs_key_t key, const struct keycode_list *codes, uint8_t super_l,
		       uint8_t super_r)
{
	return obs_nix_x11_keys_pressed(keys, key, codes, &super_l, &super_r);
}

static obs_key_t virtual_of(int sym, const uint32_t *keysyms, int num_keysyms, int syms_per_code, uint8_t min_keycode)
{
	return obs_nix_x11_keys_from_virtual_key(sym, keysyms, syms_per_code, &num_keysyms, &min_keycode, keycodes);
}

static const uint32_t *base_table(void)
{
	return base;
}

/* key_from_base_keysym(): the table first, then the right modifiers. */
static void base_keysym_lookup(void **state)
{
	(void)state;
	assert_int_equal(obs_nix_x11_keys_from_base_keysym(base, XK_a), OBS_KEY_A);
	assert_int_equal(obs_nix_x11_keys_from_base_keysym(base, XK_Shift_L), OBS_KEY_SHIFT);
	assert_int_equal(obs_nix_x11_keys_from_base_keysym(base, SHIFT_R), OBS_KEY_SHIFT);
	assert_int_equal(obs_nix_x11_keys_from_base_keysym(base, CONTROL_R), OBS_KEY_CONTROL);
	assert_int_equal(obs_nix_x11_keys_from_base_keysym(base, ALT_R), OBS_KEY_ALT);
	assert_int_equal(obs_nix_x11_keys_from_base_keysym(base, XK_unknown), OBS_KEY_NONE);
	/* No right modifier fallback for the super keys. */
	assert_int_equal(obs_nix_x11_keys_from_base_keysym(base, SUPER_L), OBS_KEY_NONE);
	assert_int_equal(obs_nix_x11_keys_from_base_keysym(base, SUPER_R), OBS_KEY_NONE);
	/* 0 is the base keysym of OBS_KEY_NONE (and of every key without
	 * one): the first, OBS_KEY_NONE. */
	assert_int_equal(obs_nix_x11_keys_from_base_keysym(base, 0), OBS_KEY_NONE);
}

/* The first key with the keysym wins; the table wins over the fallback. */
static void base_keysym_first_wins(void **state)
{
	(void)state;
	base[OBS_KEY_C] = XK_a;
	assert_int_equal(obs_nix_x11_keys_from_base_keysym(base, XK_a), OBS_KEY_A);
	base[OBS_KEY_ALTGR] = ALT_R;
	assert_int_equal(obs_nix_x11_keys_from_base_keysym(base, ALT_R), OBS_KEY_ALTGR);
	/* A key with base keysym 0 before OBS_KEY_NONE's: none can be. */
	base[OBS_KEY_NONE] = XK_b;
	assert_int_equal(obs_nix_x11_keys_from_base_keysym(base, XK_b), OBS_KEY_NONE);
	assert_int_equal(obs_nix_x11_keys_from_base_keysym(base, 0), OBS_KEY_RETURN);
}

static enum obs_nix_x11_keys_row row_of(const uint32_t *row, int per, obs_key_t *key)
{
	*key = (obs_key_t)-7;
	return obs_nix_x11_keys_row(row, per, base, key);
}

/* fill_keycodes(): the first non-zero symbol that is wanted, up to a 0. */
static void row_rule(void **state)
{
	obs_key_t key;
	(void)state;

	const uint32_t plain[] = {XK_a, XK_A, 0, 0};
	assert_int_equal(row_of(plain, 4, &key), OBS_NIX_X11_KEYS_ROW_KEY);
	assert_int_equal(key, OBS_KEY_A);

	/* A first symbol of 0 ends the row: the rest is not looked at. */
	const uint32_t first_zero[] = {0, XK_a, SUPER_L, XK_b};
	assert_int_equal(row_of(first_zero, 4, &key), OBS_NIX_X11_KEYS_ROW_NONE);
	assert_int_equal(key, (obs_key_t)-7);

	/* Super_L in the second column, after a symbol with no key. */
	const uint32_t super_l[] = {XK_unknown, SUPER_L, XK_a, 0};
	assert_int_equal(row_of(super_l, 4, &key), OBS_NIX_X11_KEYS_ROW_SUPER_L);
	assert_int_equal(key, (obs_key_t)-7);

	const uint32_t super_r[] = {SUPER_R, XK_a};
	assert_int_equal(row_of(super_r, 2, &key), OBS_NIX_X11_KEYS_ROW_SUPER_R);

	/* A symbol known only at the second level still maps the keycode. */
	const uint32_t level_two[] = {XK_exclam, XK_1};
	assert_int_equal(row_of(level_two, 2, &key), OBS_NIX_X11_KEYS_ROW_KEY);
	assert_int_equal(key, OBS_KEY_1);

	/* A key before a super symbol wins, and the reverse. */
	const uint32_t key_then_super[] = {XK_b, SUPER_R};
	assert_int_equal(row_of(key_then_super, 2, &key), OBS_NIX_X11_KEYS_ROW_KEY);
	assert_int_equal(key, OBS_KEY_B);
	const uint32_t super_then_key[] = {SUPER_L, XK_b};
	assert_int_equal(row_of(super_then_key, 2, &key), OBS_NIX_X11_KEYS_ROW_SUPER_L);

	/* A 0 after unknown symbols ends the row before a known one. */
	const uint32_t zero_later[] = {XK_unknown, 0, XK_a};
	assert_int_equal(row_of(zero_later, 3, &key), OBS_NIX_X11_KEYS_ROW_NONE);

	/* A row of unknown symbols. */
	const uint32_t unknown[] = {XK_unknown, XK_unknown + 1, XK_unknown + 2};
	assert_int_equal(row_of(unknown, 3, &key), OBS_NIX_X11_KEYS_ROW_NONE);

	/* The right modifiers map through the fallback. */
	const uint32_t right_ctrl[] = {CONTROL_R};
	assert_int_equal(row_of(right_ctrl, 1, &key), OBS_NIX_X11_KEYS_ROW_KEY);
	assert_int_equal(key, OBS_KEY_CONTROL);

	/* Only syms_per_code symbols are read. */
	const uint32_t short_row[] = {XK_unknown, XK_a};
	assert_int_equal(row_of(short_row, 1, &key), OBS_NIX_X11_KEYS_ROW_NONE);
	assert_int_equal(row_of(plain, 0, &key), OBS_NIX_X11_KEYS_ROW_NONE);
	assert_int_equal(row_of(plain, -1, &key), OBS_NIX_X11_KEYS_ROW_NONE);
}

/* key_pressed(): Meta checks the super keycodes, any other key its own. */
static void pressed_rule(void **state)
{
	uint8_t keys[OBS_NIX_X11_KEYS_KEYMAP_LEN] = {0};
	const uint8_t a_codes[] = {38, 140};
	(void)state;

	set_codes(OBS_KEY_A, a_codes, 2);
	assert_false(pressed_of(keys, OBS_KEY_A, &keycodes[OBS_KEY_A], 133, 134));

	/* The second keycode of a key with two. */
	set_down(keys, 140);
	assert_true(pressed_of(keys, OBS_KEY_A, &keycodes[OBS_KEY_A], 133, 134));
	/* A key with no keycode is never down. */
	assert_false(pressed_of(keys, OBS_KEY_B, &keycodes[OBS_KEY_B], 133, 134));

	/* Meta: either super keycode; its own list is not looked at. */
	assert_false(pressed_of(keys, OBS_KEY_META, &keycodes[OBS_KEY_A], 133, 134));
	set_down(keys, 134);
	assert_true(pressed_of(keys, OBS_KEY_META, &keycodes[OBS_KEY_B], 133, 134));
	memset(keys, 0, sizeof(keys));
	set_down(keys, 133);
	assert_true(pressed_of(keys, OBS_KEY_META, &keycodes[OBS_KEY_B], 133, 134));

	/* Meta with neither super keycode found: both are 0, which X never
	 * reports down. */
	memset(keys, 0, sizeof(keys));
	set_down(keys, 255);
	assert_false(pressed_of(keys, OBS_KEY_META, &keycodes[OBS_KEY_B], 0, 0));
	/* Keycode 255 is the last bit. */
	const uint8_t last_code[] = {255};
	set_codes(OBS_KEY_B, last_code, 1);
	assert_true(pressed_of(keys, OBS_KEY_B, &keycodes[OBS_KEY_B], 0, 0));

	/* No reply: nothing is down. */
	assert_false(pressed_of(NULL, OBS_KEY_B, &keycodes[OBS_KEY_B], 255, 255));
	assert_false(pressed_of(NULL, OBS_KEY_META, &keycodes[OBS_KEY_B], 255, 255));
}

/* key_from_keycode(): the first key whose list holds the keycode. */
static void keycode_lookup(void **state)
{
	const uint8_t a_codes[] = {38, 140};
	const uint8_t c_codes[] = {54, 140};
	(void)state;

	set_codes(OBS_KEY_A, a_codes, 2);
	set_codes(OBS_KEY_C, c_codes, 2);
	assert_int_equal(obs_nix_x11_keys_from_keycode(keycodes, 38), OBS_KEY_A);
	assert_int_equal(obs_nix_x11_keys_from_keycode(keycodes, 54), OBS_KEY_C);
	assert_int_equal(obs_nix_x11_keys_from_keycode(keycodes, 140), OBS_KEY_A);
	assert_int_equal(obs_nix_x11_keys_from_keycode(keycodes, 39), OBS_KEY_NONE);
	assert_int_equal(obs_nix_x11_keys_from_keycode(keycodes, 0), OBS_KEY_NONE);
}

/* The reporter's layout of test_x11_hotkey_layout, two symbols per
 * keycode, keycodes 16 to 19: 16 has nothing, 17 is [ 8 asterisk ], 18
 * is [ bracketleft A ] and 19 is [ 1 bracketleft ]. */
#define MIN_CODE 16
static const uint32_t map[] = {
	0, 0, XK_8, XK_asterisk, XK_bracketleft, XK_A, XK_1, XK_bracketleft,
};

static obs_key_t from_virtual(int sym, int per)
{
	return virtual_of(sym, map, (int)(sizeof(map) / sizeof(map[0])), per, MIN_CODE);
}

/* obs_nix_x11_key_from_virtual_key(): one column at a time. */
static void from_virtual_columns(void **state)
{
	const uint8_t c17[] = {17}, c18[] = {18}, c19[] = {19};
	(void)state;

	set_codes(OBS_KEY_8, c17, 1);
	set_codes(OBS_KEY_BRACKETLEFT, c18, 1);
	set_codes(OBS_KEY_1, c19, 1);

	assert_int_equal(from_virtual(XK_8, 2), OBS_KEY_8);
	/* Unshifted on 18, shifted on the lower 19: 18 wins. */
	assert_int_equal(from_virtual(XK_bracketleft, 2), OBS_KEY_BRACKETLEFT);
	/* Only on a shifted level: still its key. */
	assert_int_equal(from_virtual(XK_asterisk, 2), OBS_KEY_8);
	assert_int_equal(from_virtual(XK_A, 2), OBS_KEY_BRACKETLEFT);
	assert_int_equal(from_virtual(XK_unknown, 2), OBS_KEY_NONE);

	/* The same map read as one symbol per keycode: keycodes 16 to 23. */
	const uint8_t c20[] = {20};
	set_codes(OBS_KEY_A, c20, 1);
	assert_int_equal(from_virtual(XK_bracketleft, 1), OBS_KEY_A);
}

/* The search stops at the first keycode with the symbol, even when that
 * keycode has no key and a later column's keycode has one. */
static void from_virtual_first_hit_only(void **state)
{
	const uint8_t c19[] = {19};
	(void)state;

	set_codes(OBS_KEY_1, c19, 1);
	assert_int_equal(from_virtual(XK_bracketleft, 2), OBS_KEY_NONE);
	assert_int_equal(from_virtual(XK_1, 2), OBS_KEY_1);
}

/* Symbol 0 and an empty map give no key, before any search. */
static void from_virtual_gate(void **state)
{
	const uint8_t c16[] = {16};
	(void)state;

	set_codes(OBS_KEY_ESCAPE, c16, 1);
	assert_int_equal(from_virtual(0, 2), OBS_KEY_NONE);
	assert_int_equal(from_virtual(XK_8, 0), OBS_KEY_NONE);
	assert_int_equal(from_virtual(XK_8, -2), OBS_KEY_NONE);
	assert_int_equal(virtual_of(XK_8, NULL, 0, 2, MIN_CODE), OBS_KEY_NONE);
	/* A negative symbol is compared as its 32-bit value. */
	assert_int_equal(from_virtual((int)0xffffffffu, 2), OBS_KEY_NONE);
}

/* The keycode is the row number plus min_keycode, as a uint8_t. */
static void from_virtual_min_keycode(void **state)
{
	const uint8_t c255[] = {255}, c0[] = {0};
	static const uint32_t two_rows[] = {XK_a, XK_b};
	(void)state;

	set_codes(OBS_KEY_A, c255, 1);
	set_codes(OBS_KEY_B, c0, 1);
	assert_int_equal(virtual_of(XK_a, two_rows, 2, 1, 255), OBS_KEY_A);
	/* 255 + 1 wraps to keycode 0 (characterized, not endorsed: X never
	 * reports more keycodes than fit). */
	assert_int_equal(virtual_of(XK_b, two_rows, 2, 1, 255), OBS_KEY_B);
	/* num_keysyms not a multiple: the rows that fit. */
	assert_int_equal(virtual_of(XK_b, two_rows, 1, 1, 255), OBS_KEY_NONE);
}

/* obs_nix_x11_key_to_virtual_key(): Meta is Super_L, others their base. */
static void to_virtual(void **state)
{
	(void)state;
	assert_int_equal(obs_nix_x11_keys_to_virtual_key(OBS_KEY_META, base_table), SUPER_L);
	base[OBS_KEY_META] = SUPER_R;
	assert_int_equal(obs_nix_x11_keys_to_virtual_key(OBS_KEY_META, base_table), SUPER_L);
	assert_int_equal(obs_nix_x11_keys_to_virtual_key(OBS_KEY_A, base_table), XK_a);
	assert_int_equal(obs_nix_x11_keys_to_virtual_key(OBS_KEY_SHIFT, base_table), XK_Shift_L);
	assert_int_equal(obs_nix_x11_keys_to_virtual_key(OBS_KEY_NONE, base_table), 0);
	assert_int_equal(obs_nix_x11_keys_to_virtual_key(OBS_KEY_C, base_table), 0);
	base[OBS_KEY_LAST_VALUE - 1] = 0xfffffffeu;
	assert_int_equal(obs_nix_x11_keys_to_virtual_key((obs_key_t)(OBS_KEY_LAST_VALUE - 1), base_table), -2);
}

/* Each platform field is read only where obs-nix-x11.c read it: NULL for
 * one that must not be read, and a counting getter for the table behind
 * obs->hotkeys.platform_context. */
static int table_asks;

static const uint32_t *counted_table(void)
{
	table_asks++;
	return base;
}

static void reads_under_their_guards(void **state)
{
	uint8_t keys[OBS_NIX_X11_KEYS_KEYMAP_LEN] = {0};
	const uint8_t a_codes[] = {38};
	const uint8_t super_l = 133;
	const int num = (int)(sizeof(map) / sizeof(map[0]));
	(void)state;

	/* key_pressed(): the super keycodes only for Meta, the right one only
	 * when the left one is not down; the key's list only for other keys. */
	set_codes(OBS_KEY_A, a_codes, 1);
	set_down(keys, 38);
	assert_true(obs_nix_x11_keys_pressed(keys, OBS_KEY_A, &keycodes[OBS_KEY_A], NULL, NULL));
	set_down(keys, 133);
	assert_true(obs_nix_x11_keys_pressed(keys, OBS_KEY_META, NULL, &super_l, NULL));

	/* The virtual key search: the map's size only past the gate, the
	 * minimum keycode and the lists only once the symbol is found. */
	assert_int_equal(obs_nix_x11_keys_from_virtual_key(0, NULL, 2, NULL, NULL, NULL), OBS_KEY_NONE);
	assert_int_equal(obs_nix_x11_keys_from_virtual_key(XK_8, NULL, 0, NULL, NULL, NULL), OBS_KEY_NONE);
	assert_int_equal(obs_nix_x11_keys_from_virtual_key(XK_unknown, map, 2, &num, NULL, NULL), OBS_KEY_NONE);

	/* The virtual key of a key: the table only for a key other than Meta. */
	table_asks = 0;
	assert_int_equal(obs_nix_x11_keys_to_virtual_key(OBS_KEY_META, counted_table), SUPER_L);
	assert_int_equal(table_asks, 0);
	assert_int_equal(obs_nix_x11_keys_to_virtual_key(OBS_KEY_A, counted_table), XK_a);
	assert_int_equal(table_asks, 1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(base_keysym_lookup, setup),
		cmocka_unit_test_setup(base_keysym_first_wins, setup),
		cmocka_unit_test_setup(row_rule, setup),
		cmocka_unit_test_setup(pressed_rule, setup),
		cmocka_unit_test_setup(keycode_lookup, setup),
		cmocka_unit_test_setup(from_virtual_columns, setup),
		cmocka_unit_test_setup(from_virtual_first_hit_only, setup),
		cmocka_unit_test_setup(from_virtual_gate, setup),
		cmocka_unit_test_setup(from_virtual_min_keycode, setup),
		cmocka_unit_test_setup(to_virtual, setup),
		cmocka_unit_test_setup(reads_under_their_guards, setup),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
