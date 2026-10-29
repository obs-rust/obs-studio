#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include "obs-cocoa-keys.h"

/*
 * Characterization of the macOS key map and pressed check of
 * libobs/obs-cocoa.m, moved to libobs/obs-cocoa-keys.c before their Rust
 * port:
 * - obs_cocoa_keys_modifier(): KeyboardEventProc()'s modifier flag map;
 * - obs_cocoa_keys_to_virtual_key(): obs_key_to_virtual_key();
 * - obs_cocoa_keys_from_virtual_key(): obs_key_from_virtual_key();
 * - obs_cocoa_keys_is_mouse() and obs_cocoa_keys_in_range():
 *   obs_hotkeys_platform_is_pressed()'s mouse range and key range gate.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. No macOS header is needed: the tables
 * are made up here, so it runs on every platform.
 *
 * Not characterized here:
 * - obs_cocoa_keys_mouse_pressed(), the button bit: C shifts an int by
 *   the key's enum value less one, past the int's width, which is
 *   undefined (an optimizing compiler may return anything);
 * - obs_cocoa_keys_to_virtual_key() for a key outside the table, which C
 *   reads past its end.
 * The Rust port fixes both (bit key - OBS_KEY_MOUSE1, and INVALID for a
 * key outside the table); its Tier 3 tests list them as intentional
 * differences.
 */

#define SHIFT OBS_COCOA_KEYS_FLAG_SHIFT
#define CONTROL OBS_COCOA_KEYS_FLAG_CONTROL
#define ALTERNATE OBS_COCOA_KEYS_FLAG_ALTERNATE
#define COMMAND OBS_COCOA_KEYS_FLAG_COMMAND

/* Some macOS virtual key codes (HIToolbox/Events.h). */
#define kVK_ANSI_A 0x00
#define kVK_ANSI_S 0x01
#define kVK_Return 0x24
#define kVK_Escape 0x35
#define kVK_F1 0x7A

static obs_key_code_t table[OBS_KEY_LAST_VALUE];

static int setup(void **state)
{
	(void)state;
	memset(table, 0, sizeof(table));
	return 0;
}

static void set_code(obs_key_t key, int code, bool is_valid)
{
	table[key].code = code;
	table[key].is_valid = is_valid;
}

static void modifier_each_flag(void **state)
{
	(void)state;

	/* Each flag alone sets only its key. */
	assert_true(obs_cocoa_keys_modifier(SHIFT, OBS_KEY_SHIFT));
	assert_false(obs_cocoa_keys_modifier(SHIFT, OBS_KEY_ALT));
	assert_false(obs_cocoa_keys_modifier(SHIFT, OBS_KEY_META));
	assert_false(obs_cocoa_keys_modifier(SHIFT, OBS_KEY_CONTROL));

	assert_true(obs_cocoa_keys_modifier(ALTERNATE, OBS_KEY_ALT));
	assert_false(obs_cocoa_keys_modifier(ALTERNATE, OBS_KEY_SHIFT));
	assert_false(obs_cocoa_keys_modifier(ALTERNATE, OBS_KEY_META));
	assert_false(obs_cocoa_keys_modifier(ALTERNATE, OBS_KEY_CONTROL));

	assert_true(obs_cocoa_keys_modifier(COMMAND, OBS_KEY_META));
	assert_false(obs_cocoa_keys_modifier(COMMAND, OBS_KEY_SHIFT));
	assert_false(obs_cocoa_keys_modifier(COMMAND, OBS_KEY_ALT));
	assert_false(obs_cocoa_keys_modifier(COMMAND, OBS_KEY_CONTROL));

	assert_true(obs_cocoa_keys_modifier(CONTROL, OBS_KEY_CONTROL));
	assert_false(obs_cocoa_keys_modifier(CONTROL, OBS_KEY_SHIFT));
	assert_false(obs_cocoa_keys_modifier(CONTROL, OBS_KEY_ALT));
	assert_false(obs_cocoa_keys_modifier(CONTROL, OBS_KEY_META));
}

static void modifier_all_and_none(void **state)
{
	const uint64_t all = SHIFT | CONTROL | ALTERNATE | COMMAND;
	(void)state;

	assert_true(obs_cocoa_keys_modifier(all, OBS_KEY_SHIFT));
	assert_true(obs_cocoa_keys_modifier(all, OBS_KEY_ALT));
	assert_true(obs_cocoa_keys_modifier(all, OBS_KEY_META));
	assert_true(obs_cocoa_keys_modifier(all, OBS_KEY_CONTROL));

	assert_false(obs_cocoa_keys_modifier(0, OBS_KEY_SHIFT));
	assert_false(obs_cocoa_keys_modifier(0, OBS_KEY_ALT));
	assert_false(obs_cocoa_keys_modifier(0, OBS_KEY_META));
	assert_false(obs_cocoa_keys_modifier(0, OBS_KEY_CONTROL));

	/* Every other bit (caps lock, the numeric pad, the device bits, the
	 * high word) sets none of them. */
	assert_false(obs_cocoa_keys_modifier(~all, OBS_KEY_SHIFT));
	assert_false(obs_cocoa_keys_modifier(~all, OBS_KEY_ALT));
	assert_false(obs_cocoa_keys_modifier(~all, OBS_KEY_META));
	assert_false(obs_cocoa_keys_modifier(~all, OBS_KEY_CONTROL));
}

static void modifier_other_keys(void **state)
{
	(void)state;

	/* Only the four modifier keys read the flags. */
	assert_false(obs_cocoa_keys_modifier(UINT64_MAX, OBS_KEY_NONE));
	assert_false(obs_cocoa_keys_modifier(UINT64_MAX, OBS_KEY_A));
	assert_false(obs_cocoa_keys_modifier(UINT64_MAX, OBS_KEY_CAPSLOCK));
	assert_false(obs_cocoa_keys_modifier(UINT64_MAX, OBS_KEY_MOUSE1));
	assert_false(obs_cocoa_keys_modifier(UINT64_MAX, (obs_key_t)(OBS_KEY_LAST_VALUE - 1)));
}

static void to_virtual(void **state)
{
	(void)state;

	set_code(OBS_KEY_A, kVK_ANSI_A, true);
	set_code(OBS_KEY_RETURN, kVK_Return, true);
	set_code(OBS_KEY_ESCAPE, kVK_Escape, false);
	set_code(OBS_KEY_NONE, kVK_F1, true);

	/* A valid entry gives its code, 0 included. */
	assert_int_equal(obs_cocoa_keys_to_virtual_key(table, OBS_KEY_A), kVK_ANSI_A);
	assert_int_equal(obs_cocoa_keys_to_virtual_key(table, OBS_KEY_RETURN), kVK_Return);
	/* An invalid entry gives INVALID_KEY, whatever its code. */
	assert_int_equal(obs_cocoa_keys_to_virtual_key(table, OBS_KEY_ESCAPE), OBS_COCOA_KEYS_INVALID);
	assert_int_equal(obs_cocoa_keys_to_virtual_key(table, OBS_KEY_B), OBS_COCOA_KEYS_INVALID);
	/* OBS_KEY_NONE is an entry like any other. */
	assert_int_equal(obs_cocoa_keys_to_virtual_key(table, OBS_KEY_NONE), kVK_F1);
	/* The last entry of the table is read. */
	set_code((obs_key_t)(OBS_KEY_LAST_VALUE - 1), 0x42, true);
	assert_int_equal(obs_cocoa_keys_to_virtual_key(table, (obs_key_t)(OBS_KEY_LAST_VALUE - 1)), 0x42);
	/* A code is returned as stored, even INVALID_KEY or a negative one. */
	set_code(OBS_KEY_C, OBS_COCOA_KEYS_INVALID, true);
	set_code(OBS_KEY_D, -7, true);
	assert_int_equal(obs_cocoa_keys_to_virtual_key(table, OBS_KEY_C), OBS_COCOA_KEYS_INVALID);
	assert_int_equal(obs_cocoa_keys_to_virtual_key(table, OBS_KEY_D), -7);
}

static void from_virtual(void **state)
{
	(void)state;

	set_code(OBS_KEY_A, kVK_ANSI_A, true);
	set_code(OBS_KEY_S, kVK_ANSI_S, true);
	set_code(OBS_KEY_RETURN, kVK_Return, true);

	assert_int_equal(obs_cocoa_keys_from_virtual_key(table, kVK_ANSI_A), OBS_KEY_A);
	assert_int_equal(obs_cocoa_keys_from_virtual_key(table, kVK_ANSI_S), OBS_KEY_S);
	assert_int_equal(obs_cocoa_keys_from_virtual_key(table, kVK_Return), OBS_KEY_RETURN);
	/* A code no valid entry has gives none. */
	assert_int_equal(obs_cocoa_keys_from_virtual_key(table, kVK_Escape), OBS_KEY_NONE);
	assert_int_equal(obs_cocoa_keys_from_virtual_key(table, OBS_COCOA_KEYS_INVALID), OBS_KEY_NONE);
	assert_int_equal(obs_cocoa_keys_from_virtual_key(table, -1), OBS_KEY_NONE);
	assert_int_equal(obs_cocoa_keys_from_virtual_key(table, 0x1000), OBS_KEY_NONE);
}

static void from_virtual_first_valid_wins(void **state)
{
	(void)state;

	/* Two valid entries with the same code: the first wins. */
	set_code(OBS_KEY_RETURN, kVK_Return, true);
	set_code(OBS_KEY_ENTER, kVK_Return, true);
	assert_int_equal(obs_cocoa_keys_from_virtual_key(table, kVK_Return), OBS_KEY_RETURN);

	/* An invalid entry is skipped, even with the code. */
	set_code(OBS_KEY_B, kVK_Escape, false);
	set_code(OBS_KEY_ESCAPE, kVK_Escape, true);
	assert_int_equal(obs_cocoa_keys_from_virtual_key(table, kVK_Escape), OBS_KEY_ESCAPE);

	/* The zeroed table's invalid entries all have code 0: none is found. */
	assert_int_equal(obs_cocoa_keys_from_virtual_key(table, 0), OBS_KEY_NONE);

	/* OBS_KEY_NONE's own entry and the last entry are searched. */
	set_code(OBS_KEY_NONE, kVK_F1, true);
	set_code(OBS_KEY_F1, kVK_F1, true);
	assert_int_equal(obs_cocoa_keys_from_virtual_key(table, kVK_F1), OBS_KEY_NONE);
	set_code((obs_key_t)(OBS_KEY_LAST_VALUE - 1), 0x77, true);
	assert_int_equal(obs_cocoa_keys_from_virtual_key(table, 0x77), (obs_key_t)(OBS_KEY_LAST_VALUE - 1));
}

static void mouse_range(void **state)
{
	(void)state;

	assert_true(obs_cocoa_keys_is_mouse(OBS_KEY_MOUSE1));
	assert_true(obs_cocoa_keys_is_mouse(OBS_KEY_MOUSE2));
	assert_true(obs_cocoa_keys_is_mouse(OBS_KEY_MOUSE29));
	assert_false(obs_cocoa_keys_is_mouse((obs_key_t)(OBS_KEY_MOUSE1 - 1)));
	assert_false(obs_cocoa_keys_is_mouse((obs_key_t)(OBS_KEY_MOUSE29 + 1)));
	assert_false(obs_cocoa_keys_is_mouse(OBS_KEY_NONE));
	assert_false(obs_cocoa_keys_is_mouse(OBS_KEY_A));
	assert_false(obs_cocoa_keys_is_mouse(OBS_KEY_LAST_VALUE));
}

static void key_range(void **state)
{
	(void)state;

	assert_true(obs_cocoa_keys_in_range(OBS_KEY_NONE));
	assert_true(obs_cocoa_keys_in_range(OBS_KEY_A));
	assert_true(obs_cocoa_keys_in_range(OBS_KEY_MOUSE29));
	assert_true(obs_cocoa_keys_in_range((obs_key_t)(OBS_KEY_LAST_VALUE - 1)));
	assert_false(obs_cocoa_keys_in_range(OBS_KEY_LAST_VALUE));
	assert_false(obs_cocoa_keys_in_range((obs_key_t)(OBS_KEY_LAST_VALUE + 1)));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(modifier_each_flag),
		cmocka_unit_test(modifier_all_and_none),
		cmocka_unit_test(modifier_other_keys),
		cmocka_unit_test_setup(to_virtual, setup),
		cmocka_unit_test_setup(from_virtual, setup),
		cmocka_unit_test_setup(from_virtual_first_valid_wins, setup),
		cmocka_unit_test(mouse_range),
		cmocka_unit_test(key_range),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
