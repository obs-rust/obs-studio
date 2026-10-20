#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <util/bmem.h>
#include <util/dstr.h>

#include "obs-nix-portal-shortcuts.h"

/*
 * Characterization tests for the GlobalShortcuts portal shortcut id and
 * trigger codec in libobs/obs-nix-portal-shortcut-ids.c (#1517):
 * obs_nix_portal_shortcut_id, obs_nix_portal_shortcut_combo and
 * obs_nix_portal_shortcut_trigger, with the bindable() check they share with
 * the binding collection. They pin the current C behavior for the Rust port;
 * cases marked "characterized, not endorsed" are odd but real.
 *
 * The codec is compiled into this test, and needs no D-Bus, no GIO and no
 * libobs core: with no core, obs_key_from_name falls back to the key table.
 * test_portal_shortcuts covers the portal client that uses it.
 */

#define ALL_MODIFIERS (INTERACT_CONTROL_KEY | INTERACT_ALT_KEY | INTERACT_SHIFT_KEY | INTERACT_COMMAND_KEY)

/* The four modifiers the codec knows, in its id order. */
static const struct {
	uint32_t modifier;
	const char *id;
	const char *trigger;
} known[] = {
	{INTERACT_CONTROL_KEY, "ctrl", "CTRL"},
	{INTERACT_ALT_KEY, "alt", "ALT"},
	{INTERACT_SHIFT_KEY, "shift", "SHIFT"},
	{INTERACT_COMMAND_KEY, "super", "LOGO"},
};

#define NUM_KNOWN (sizeof(known) / sizeof(known[0]))

/* Every other interaction flag, which the codec ignores. */
static const uint32_t other_flags[] = {
	INTERACT_CAPS_KEY,    INTERACT_MOUSE_LEFT, INTERACT_MOUSE_MIDDLE, INTERACT_MOUSE_RIGHT,
	INTERACT_NUMLOCK_KEY, INTERACT_IS_KEY_PAD, INTERACT_IS_LEFT,      INTERACT_IS_RIGHT,
};

static long allocs_before;

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	allocs_before = bnum_allocs();
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	/* Every dstr the codec fills is freed by the test; the codec's own
	 * temporaries must be freed by the codec. */
	if (bnum_allocs() != allocs_before) {
		fprintf(stderr, "allocations: %ld before, %ld after\n", allocs_before, bnum_allocs());
		return -1;
	}
	return 0;
}

static obs_key_combination_t combo_of(uint32_t modifiers, obs_key_t key)
{
	obs_key_combination_t combo = {modifiers, key};
	return combo;
}

static bool is_mouse(obs_key_t key)
{
	return key >= OBS_KEY_MOUSE1 && key <= OBS_KEY_MOUSE29;
}

/* The expected prefix of a modifier set, in the codec's order. */
static void expected_prefix(uint32_t modifiers, bool trigger, struct dstr *out)
{
	dstr_free(out);
	dstr_copy(out, "");
	for (size_t i = 0; i < NUM_KNOWN; i++) {
		if (modifiers & known[i].modifier) {
			dstr_cat(out, trigger ? known[i].trigger : known[i].id);
			dstr_cat(out, "+");
		}
	}
}

static void assert_id(obs_key_combination_t combo, const char *expected)
{
	struct dstr id = {0};
	assert_true(obs_nix_portal_shortcut_id(combo, &id));
	assert_non_null(id.array);
	assert_string_equal(id.array, expected);
	assert_int_equal(id.len, strlen(expected));
	dstr_free(&id);
}

static void assert_no_id(obs_key_combination_t combo)
{
	struct dstr id = {0};
	dstr_copy(&id, "untouched");
	assert_false(obs_nix_portal_shortcut_id(combo, &id));
	assert_string_equal(id.array, "untouched");
	dstr_free(&id);
}

static void assert_combo(const char *id, uint32_t modifiers, obs_key_t key)
{
	obs_key_combination_t combo = {0xdeadbeef, OBS_KEY_LAST_VALUE};
	if (!obs_nix_portal_shortcut_combo(id, &combo)) {
		fprintf(stderr, "'%s' did not parse\n", id);
		fail();
	}
	assert_int_equal(combo.modifiers, modifiers);
	assert_int_equal(combo.key, key);
}

static void assert_no_combo(const char *id)
{
	obs_key_combination_t combo = {0xdeadbeef, OBS_KEY_LAST_VALUE};
	if (obs_nix_portal_shortcut_combo(id, &combo)) {
		fprintf(stderr, "'%s' parsed as %#x + %d\n", id, combo.modifiers, (int)combo.key);
		fail();
	}
	/* The output is written only on success. */
	assert_int_equal(combo.modifiers, 0xdeadbeef);
	assert_int_equal(combo.key, OBS_KEY_LAST_VALUE);
}

/* ------------------------------------------------------------------------ */
/* Key names for triggers                                                    */

static int name_calls;
static obs_key_t name_last_key;

/* What test_portal_shortcuts uses: the key name without OBS_KEY_, single
 * letters in lower case. */
static bool lower_name(obs_key_t key, struct dstr *name)
{
	name_calls++;
	name_last_key = key;
	const char *full = obs_key_to_name(key);
	if (strncmp(full, "OBS_KEY_", 8) != 0 || key == OBS_KEY_NONE)
		return false;
	dstr_copy(name, full + 8);
	if (name->len == 1)
		dstr_to_lower(name);
	return true;
}

static bool no_name(obs_key_t key, struct dstr *name)
{
	UNUSED_PARAMETER(name);
	name_calls++;
	name_last_key = key;
	return false;
}

static bool empty_name(obs_key_t key, struct dstr *name)
{
	name_calls++;
	name_last_key = key;
	dstr_copy(name, "");
	return true;
}

static bool unset_name(obs_key_t key, struct dstr *name)
{
	UNUSED_PARAMETER(name);
	name_calls++;
	name_last_key = key;
	return true;
}

/* Fills the name, then says there is none: the codec frees it. */
static bool filled_but_no_name(obs_key_t key, struct dstr *name)
{
	name_calls++;
	name_last_key = key;
	dstr_copy(name, "leaked?");
	return false;
}

/* Appends to whatever the name holds: shows the codec starts it empty. */
static bool appending_name(obs_key_t key, struct dstr *name)
{
	name_calls++;
	name_last_key = key;
	dstr_cat(name, "K");
	return true;
}

static void reset_name_calls(void)
{
	name_calls = 0;
	name_last_key = OBS_KEY_LAST_VALUE;
}

static void assert_trigger(obs_key_combination_t combo, obs_nix_portal_key_name_func key_name, const char *expected)
{
	struct dstr trigger = {0};
	reset_name_calls();
	assert_true(obs_nix_portal_shortcut_trigger(combo, key_name, &trigger));
	assert_int_equal(name_calls, 1);
	assert_int_equal(name_last_key, combo.key);
	assert_non_null(trigger.array);
	assert_string_equal(trigger.array, expected);
	assert_int_equal(trigger.len, strlen(expected));
	dstr_free(&trigger);
}

static void assert_no_trigger(obs_key_combination_t combo, obs_nix_portal_key_name_func key_name, int calls)
{
	struct dstr trigger = {0};
	dstr_copy(&trigger, "untouched");
	reset_name_calls();
	assert_false(obs_nix_portal_shortcut_trigger(combo, key_name, &trigger));
	assert_int_equal(name_calls, calls);
	assert_string_equal(trigger.array, "untouched");
	dstr_free(&trigger);
}

/* ------------------------------------------------------------------------ */
/* bindable                                                                  */

static void test_bindable_keys(void **state)
{
	UNUSED_PARAMETER(state);

	assert_false(bindable(combo_of(0, OBS_KEY_NONE)));
	assert_false(bindable(combo_of(ALL_MODIFIERS, OBS_KEY_NONE)));
	assert_false(bindable(combo_of(0, OBS_KEY_LAST_VALUE)));
	assert_false(bindable(combo_of(0, (obs_key_t)(OBS_KEY_LAST_VALUE + 1))));
	assert_false(bindable(combo_of(0, (obs_key_t)-1)));

	for (int key = OBS_KEY_NONE + 1; key < OBS_KEY_LAST_VALUE; key++) {
		bool expected = !is_mouse((obs_key_t)key);
		assert_int_equal(bindable(combo_of(0, (obs_key_t)key)), expected);
		/* Modifiers never matter. */
		assert_int_equal(bindable(combo_of(UINT32_MAX, (obs_key_t)key)), expected);
	}

	/* The mouse range ends exactly at MOUSE29: the key after it is a
	 * keyboard key again. */
	assert_false(bindable(combo_of(0, OBS_KEY_MOUSE1)));
	assert_false(bindable(combo_of(0, OBS_KEY_MOUSE29)));
	assert_true(bindable(combo_of(0, OBS_KEY_MOUSE1 - 1)));
	assert_true(bindable(combo_of(0, OBS_KEY_BACKSLASH_RT102)));
	assert_int_equal(OBS_KEY_BACKSLASH_RT102, OBS_KEY_MOUSE29 + 1);
}

/* ------------------------------------------------------------------------ */
/* Shortcut ids                                                              */

static void test_id_plain_keys(void **state)
{
	UNUSED_PARAMETER(state);

	assert_id(combo_of(0, OBS_KEY_A), "OBS_KEY_A");
	assert_id(combo_of(0, OBS_KEY_Z), "OBS_KEY_Z");
	assert_id(combo_of(0, OBS_KEY_0), "OBS_KEY_0");
	assert_id(combo_of(0, OBS_KEY_F1), "OBS_KEY_F1");
	assert_id(combo_of(0, OBS_KEY_F13), "OBS_KEY_F13");
	assert_id(combo_of(0, OBS_KEY_F35), "OBS_KEY_F35");
	assert_id(combo_of(0, OBS_KEY_SPACE), "OBS_KEY_SPACE");
	assert_id(combo_of(0, OBS_KEY_RETURN), "OBS_KEY_RETURN");
	assert_id(combo_of(0, OBS_KEY_NUMPLUS), "OBS_KEY_NUMPLUS");
	assert_id(combo_of(0, OBS_KEY_PLUS), "OBS_KEY_PLUS");
	assert_id(combo_of(0, OBS_KEY_BACKSLASH_RT102), "OBS_KEY_BACKSLASH_RT102");

	/* Modifier keys are keys: alone they get an id like any other. */
	assert_id(combo_of(0, OBS_KEY_SHIFT), "OBS_KEY_SHIFT");
	assert_id(combo_of(0, OBS_KEY_CONTROL), "OBS_KEY_CONTROL");
	assert_id(combo_of(INTERACT_CONTROL_KEY, OBS_KEY_CONTROL), "ctrl+OBS_KEY_CONTROL");
}

static void test_id_every_modifier_set(void **state)
{
	UNUSED_PARAMETER(state);

	assert_id(combo_of(INTERACT_CONTROL_KEY, OBS_KEY_A), "ctrl+OBS_KEY_A");
	assert_id(combo_of(INTERACT_ALT_KEY, OBS_KEY_A), "alt+OBS_KEY_A");
	assert_id(combo_of(INTERACT_SHIFT_KEY, OBS_KEY_A), "shift+OBS_KEY_A");
	assert_id(combo_of(INTERACT_COMMAND_KEY, OBS_KEY_A), "super+OBS_KEY_A");
	assert_id(combo_of(INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY, OBS_KEY_A), "ctrl+shift+OBS_KEY_A");
	assert_id(combo_of(INTERACT_ALT_KEY | INTERACT_COMMAND_KEY, OBS_KEY_F1), "alt+super+OBS_KEY_F1");
	assert_id(combo_of(ALL_MODIFIERS, OBS_KEY_SPACE), "ctrl+alt+shift+super+OBS_KEY_SPACE");

	/* All 16 sets, always in the order ctrl, alt, shift, super. */
	struct dstr expected = {0};
	for (uint32_t set = 0; set < (1u << NUM_KNOWN); set++) {
		uint32_t modifiers = 0;
		for (size_t i = 0; i < NUM_KNOWN; i++)
			if (set & (1u << i))
				modifiers |= known[i].modifier;
		expected_prefix(modifiers, false, &expected);
		dstr_cat(&expected, "OBS_KEY_F13");
		assert_id(combo_of(modifiers, OBS_KEY_F13), expected.array);
	}
	dstr_free(&expected);
}

/* Characterized, not endorsed: modifier bits other than the four known ones
 * are dropped from the id, so the id does not round-trip them and two
 * combinations that differ only in them share an id. Saved bindings carry
 * only the four (load_modifier in obs-hotkey.c), so this needs a binding set
 * through the API. */
static void test_id_ignores_other_modifier_bits(void **state)
{
	UNUSED_PARAMETER(state);

	for (size_t i = 0; i < sizeof(other_flags) / sizeof(other_flags[0]); i++) {
		assert_id(combo_of(other_flags[i], OBS_KEY_A), "OBS_KEY_A");
		assert_id(combo_of(other_flags[i] | INTERACT_CONTROL_KEY, OBS_KEY_A), "ctrl+OBS_KEY_A");
	}
	assert_id(combo_of(UINT32_MAX, OBS_KEY_A), "ctrl+alt+shift+super+OBS_KEY_A");
}

static void test_id_unbindable(void **state)
{
	UNUSED_PARAMETER(state);

	/* No key: modifier-only combinations stay focused-only. */
	assert_no_id(combo_of(0, OBS_KEY_NONE));
	assert_no_id(combo_of(INTERACT_CONTROL_KEY, OBS_KEY_NONE));
	assert_no_id(combo_of(ALL_MODIFIERS, OBS_KEY_NONE));

	/* Mouse buttons, with and without modifiers. */
	for (int key = OBS_KEY_MOUSE1; key <= OBS_KEY_MOUSE29; key++) {
		assert_no_id(combo_of(0, (obs_key_t)key));
		assert_no_id(combo_of(INTERACT_SHIFT_KEY, (obs_key_t)key));
	}

	/* Out of range. */
	assert_no_id(combo_of(0, OBS_KEY_LAST_VALUE));
	assert_no_id(combo_of(0, (obs_key_t)(OBS_KEY_LAST_VALUE + 100)));
	assert_no_id(combo_of(INTERACT_ALT_KEY, (obs_key_t)-1));
}

static void test_id_replaces_previous_contents(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr id = {0};
	dstr_copy(&id, "a much longer previous id that is replaced");
	assert_true(obs_nix_portal_shortcut_id(combo_of(INTERACT_SHIFT_KEY, OBS_KEY_B), &id));
	assert_string_equal(id.array, "shift+OBS_KEY_B");
	assert_int_equal(id.len, strlen("shift+OBS_KEY_B"));

	/* The same dstr again. */
	assert_true(obs_nix_portal_shortcut_id(combo_of(0, OBS_KEY_C), &id));
	assert_string_equal(id.array, "OBS_KEY_C");
	dstr_free(&id);
}

/* ------------------------------------------------------------------------ */
/* Ids back to combinations                                                  */

static void test_combo_valid_ids(void **state)
{
	UNUSED_PARAMETER(state);

	assert_combo("OBS_KEY_A", 0, OBS_KEY_A);
	assert_combo("OBS_KEY_F13", 0, OBS_KEY_F13);
	assert_combo("OBS_KEY_PLUS", 0, OBS_KEY_PLUS);
	assert_combo("OBS_KEY_BACKSLASH_RT102", 0, OBS_KEY_BACKSLASH_RT102);
	assert_combo("ctrl+OBS_KEY_A", INTERACT_CONTROL_KEY, OBS_KEY_A);
	assert_combo("alt+OBS_KEY_A", INTERACT_ALT_KEY, OBS_KEY_A);
	assert_combo("shift+OBS_KEY_A", INTERACT_SHIFT_KEY, OBS_KEY_A);
	assert_combo("super+OBS_KEY_A", INTERACT_COMMAND_KEY, OBS_KEY_A);
	assert_combo("ctrl+shift+OBS_KEY_A", INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY, OBS_KEY_A);
	assert_combo("ctrl+alt+shift+super+OBS_KEY_SPACE", ALL_MODIFIERS, OBS_KEY_SPACE);
}

/* Characterized, not endorsed: the parser accepts more than the ids it
 * makes. Modifiers come in any order and may repeat; the result is the same
 * combination as the canonical id. */
static void test_combo_non_canonical_ids(void **state)
{
	UNUSED_PARAMETER(state);

	assert_combo("shift+ctrl+OBS_KEY_A", INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY, OBS_KEY_A);
	assert_combo("super+shift+alt+ctrl+OBS_KEY_A", ALL_MODIFIERS, OBS_KEY_A);
	assert_combo("ctrl+ctrl+OBS_KEY_A", INTERACT_CONTROL_KEY, OBS_KEY_A);
	assert_combo("alt+shift+alt+OBS_KEY_A", INTERACT_ALT_KEY | INTERACT_SHIFT_KEY, OBS_KEY_A);

	/* A modifier key named as the key. */
	assert_combo("ctrl+OBS_KEY_CONTROL", INTERACT_CONTROL_KEY, OBS_KEY_CONTROL);
}

static void test_combo_rejects(void **state)
{
	UNUSED_PARAMETER(state);

	/* Empty, or modifiers with no key. */
	assert_no_combo("");
	assert_no_combo("ctrl+");
	assert_no_combo("ctrl");
	assert_no_combo("ctrl+shift+");
	assert_no_combo("+");
	assert_no_combo("++");

	/* Empty or unknown modifier segments. */
	assert_no_combo("+OBS_KEY_A");
	assert_no_combo("ctrl++OBS_KEY_A");
	assert_no_combo("hyper+OBS_KEY_A");
	assert_no_combo("meta+OBS_KEY_A");
	assert_no_combo("control+OBS_KEY_A");
	assert_no_combo("logo+OBS_KEY_A");
	assert_no_combo("cmd+OBS_KEY_A");
	assert_no_combo("ctr+OBS_KEY_A");
	assert_no_combo("ctrlx+OBS_KEY_A");

	/* Matching is exact and case-sensitive: the trigger names and other
	 * spellings are not ids. */
	assert_no_combo("CTRL+OBS_KEY_A");
	assert_no_combo("Ctrl+OBS_KEY_A");
	assert_no_combo("SHIFT+OBS_KEY_A");
	assert_no_combo("LOGO+OBS_KEY_A");
	assert_no_combo(" ctrl+OBS_KEY_A");
	assert_no_combo("ctrl +OBS_KEY_A");
	assert_no_combo("obs_key_a");
	assert_no_combo("OBS_KEY_a");
	assert_no_combo("OBS_KEY_A ");
	assert_no_combo(" OBS_KEY_A");

	/* Not an OBS key name. */
	assert_no_combo("A");
	assert_no_combo("F13");
	assert_no_combo("ctrl+a");
	assert_no_combo("OBS_KEY_");
	assert_no_combo("OBS_KEY");
	assert_no_combo("OBS_KEY_NOT_A_KEY");
	assert_no_combo("OBS_KEY_LAST_VALUE");
	assert_no_combo("some-other-app-shortcut");
	assert_no_combo("ctrl+shift+F13");

	/* Something after the key. */
	assert_no_combo("OBS_KEY_A+");
	assert_no_combo("OBS_KEY_A+ctrl");
	assert_no_combo("ctrl+OBS_KEY_A+OBS_KEY_B");

	/* Known names that are not bindable. */
	assert_no_combo("OBS_KEY_NONE");
	assert_no_combo("ctrl+OBS_KEY_NONE");
	assert_no_combo("OBS_KEY_MOUSE1");
	assert_no_combo("OBS_KEY_MOUSE29");
	assert_no_combo("shift+OBS_KEY_MOUSE3");
}

/* Every bindable key, with no modifiers, each one and all four, makes an id
 * that parses back to the same combination, and every key name parses to
 * its key. */
static void test_round_trip_every_key(void **state)
{
	UNUSED_PARAMETER(state);

	const uint32_t sets[] = {0,
				 INTERACT_CONTROL_KEY,
				 INTERACT_ALT_KEY,
				 INTERACT_SHIFT_KEY,
				 INTERACT_COMMAND_KEY,
				 INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY,
				 ALL_MODIFIERS};
	int bindable_keys = 0;

	for (int k = OBS_KEY_NONE + 1; k < OBS_KEY_LAST_VALUE; k++) {
		obs_key_t key = (obs_key_t)k;
		if (is_mouse(key)) {
			assert_no_id(combo_of(0, key));
			assert_no_combo(obs_key_to_name(key));
			continue;
		}
		bindable_keys++;

		for (size_t s = 0; s < sizeof(sets) / sizeof(sets[0]); s++) {
			struct dstr id = {0};
			struct dstr expected = {0};
			expected_prefix(sets[s], false, &expected);
			dstr_cat(&expected, obs_key_to_name(key));

			obs_key_combination_t back = {0xdeadbeef, OBS_KEY_LAST_VALUE};
			if (!obs_nix_portal_shortcut_id(combo_of(sets[s], key), &id) ||
			    strcmp(id.array, expected.array) != 0 || !obs_nix_portal_shortcut_combo(id.array, &back) ||
			    back.key != key || back.modifiers != sets[s]) {
				fprintf(stderr, "key %d (%s), modifiers %#x: id '%s', back %#x + %d\n", k,
					obs_key_to_name(key), sets[s], id.array ? id.array : "(none)", back.modifiers,
					(int)back.key);
				dstr_free(&id);
				dstr_free(&expected);
				fail();
			}
			dstr_free(&id);
			dstr_free(&expected);
		}
	}

	/* Every key in obs-hotkeys.h but OBS_KEY_NONE and the 29 mouse
	 * buttons. */
	assert_int_equal(bindable_keys, OBS_KEY_LAST_VALUE - 1 - 29);
}

/* ------------------------------------------------------------------------ */
/* Preferred triggers                                                        */

static void test_trigger_plain_and_modifiers(void **state)
{
	UNUSED_PARAMETER(state);

	assert_trigger(combo_of(0, OBS_KEY_A), lower_name, "a");
	assert_trigger(combo_of(0, OBS_KEY_F13), lower_name, "F13");
	assert_trigger(combo_of(0, OBS_KEY_SPACE), lower_name, "SPACE");
	assert_trigger(combo_of(INTERACT_CONTROL_KEY, OBS_KEY_A), lower_name, "CTRL+a");
	assert_trigger(combo_of(INTERACT_ALT_KEY, OBS_KEY_A), lower_name, "ALT+a");
	assert_trigger(combo_of(INTERACT_SHIFT_KEY, OBS_KEY_A), lower_name, "SHIFT+a");
	assert_trigger(combo_of(INTERACT_COMMAND_KEY, OBS_KEY_A), lower_name, "LOGO+a");
	assert_trigger(combo_of(INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY, OBS_KEY_A), lower_name, "CTRL+SHIFT+a");
	assert_trigger(combo_of(INTERACT_ALT_KEY | INTERACT_COMMAND_KEY, OBS_KEY_F13), lower_name, "ALT+LOGO+F13");
	assert_trigger(combo_of(ALL_MODIFIERS, OBS_KEY_F1), lower_name, "CTRL+ALT+SHIFT+LOGO+F1");

	/* All 16 sets, always in the order CTRL, ALT, SHIFT, LOGO. */
	struct dstr expected = {0};
	for (uint32_t set = 0; set < (1u << NUM_KNOWN); set++) {
		uint32_t modifiers = 0;
		for (size_t i = 0; i < NUM_KNOWN; i++)
			if (set & (1u << i))
				modifiers |= known[i].modifier;
		expected_prefix(modifiers, true, &expected);
		dstr_cat(&expected, "F13");
		assert_trigger(combo_of(modifiers, OBS_KEY_F13), lower_name, expected.array);
	}
	dstr_free(&expected);

	/* The key name is used as given, whatever it holds. */
	assert_trigger(combo_of(INTERACT_SHIFT_KEY, OBS_KEY_B), appending_name, "SHIFT+K");
}

/* Characterized, not endorsed: as for ids, other modifier bits are dropped. */
static void test_trigger_ignores_other_modifier_bits(void **state)
{
	UNUSED_PARAMETER(state);

	for (size_t i = 0; i < sizeof(other_flags) / sizeof(other_flags[0]); i++)
		assert_trigger(combo_of(other_flags[i] | INTERACT_ALT_KEY, OBS_KEY_A), lower_name, "ALT+a");
	assert_trigger(combo_of(UINT32_MAX, OBS_KEY_A), lower_name, "CTRL+ALT+SHIFT+LOGO+a");
}

static void test_trigger_without_name(void **state)
{
	UNUSED_PARAMETER(state);

	/* No key name function. */
	assert_no_trigger(combo_of(INTERACT_CONTROL_KEY, OBS_KEY_A), NULL, 0);

	/* The function has no name, an empty one, or leaves it unset. */
	assert_no_trigger(combo_of(INTERACT_CONTROL_KEY, OBS_KEY_A), no_name, 1);
	assert_int_equal(name_last_key, OBS_KEY_A);
	assert_no_trigger(combo_of(0, OBS_KEY_F13), empty_name, 1);
	assert_no_trigger(combo_of(0, OBS_KEY_F13), unset_name, 1);

	/* A name filled in before returning false is freed (teardown checks
	 * the allocation count). */
	assert_no_trigger(combo_of(INTERACT_SHIFT_KEY, OBS_KEY_B), filled_but_no_name, 1);
}

static void test_trigger_unbindable(void **state)
{
	UNUSED_PARAMETER(state);

	/* The key name function is not called for an unbindable combination. */
	assert_no_trigger(combo_of(0, OBS_KEY_NONE), lower_name, 0);
	assert_no_trigger(combo_of(ALL_MODIFIERS, OBS_KEY_NONE), lower_name, 0);
	assert_no_trigger(combo_of(0, OBS_KEY_MOUSE1), lower_name, 0);
	assert_no_trigger(combo_of(INTERACT_CONTROL_KEY, OBS_KEY_MOUSE29), lower_name, 0);
	assert_no_trigger(combo_of(0, OBS_KEY_LAST_VALUE), lower_name, 0);
	assert_no_trigger(combo_of(0, (obs_key_t)-1), lower_name, 0);
	assert_no_trigger(combo_of(0, OBS_KEY_MOUSE1), NULL, 0);
}

static void test_trigger_replaces_previous_contents(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr trigger = {0};
	dstr_copy(&trigger, "a much longer previous trigger that is replaced");
	assert_true(obs_nix_portal_shortcut_trigger(combo_of(INTERACT_ALT_KEY, OBS_KEY_C), lower_name, &trigger));
	assert_string_equal(trigger.array, "ALT+c");
	assert_int_equal(trigger.len, strlen("ALT+c"));

	assert_true(obs_nix_portal_shortcut_trigger(combo_of(0, OBS_KEY_F2), lower_name, &trigger));
	assert_string_equal(trigger.array, "F2");
	dstr_free(&trigger);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(test_bindable_keys, setup, teardown),
		cmocka_unit_test_setup_teardown(test_id_plain_keys, setup, teardown),
		cmocka_unit_test_setup_teardown(test_id_every_modifier_set, setup, teardown),
		cmocka_unit_test_setup_teardown(test_id_ignores_other_modifier_bits, setup, teardown),
		cmocka_unit_test_setup_teardown(test_id_unbindable, setup, teardown),
		cmocka_unit_test_setup_teardown(test_id_replaces_previous_contents, setup, teardown),
		cmocka_unit_test_setup_teardown(test_combo_valid_ids, setup, teardown),
		cmocka_unit_test_setup_teardown(test_combo_non_canonical_ids, setup, teardown),
		cmocka_unit_test_setup_teardown(test_combo_rejects, setup, teardown),
		cmocka_unit_test_setup_teardown(test_round_trip_every_key, setup, teardown),
		cmocka_unit_test_setup_teardown(test_trigger_plain_and_modifiers, setup, teardown),
		cmocka_unit_test_setup_teardown(test_trigger_ignores_other_modifier_bits, setup, teardown),
		cmocka_unit_test_setup_teardown(test_trigger_without_name, setup, teardown),
		cmocka_unit_test_setup_teardown(test_trigger_unbindable, setup, teardown),
		cmocka_unit_test_setup_teardown(test_trigger_replaces_previous_contents, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
