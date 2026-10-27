#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs.h"
#include "obs-canvas-rules.h"

/*
 * Characterization of the canvas flag rules and setter checks of
 * libobs/obs-canvas.c, moved to libobs/obs-canvas-rules.c before their Rust
 * port:
 * - obs_canvas_rules_main_flags() and _public_flags(): the flags of
 *   obs_create_main_canvas(), and the mask of obs_canvas_create(),
 *   obs_canvas_create_private() and obs_load_canvas() (after its
 *   (uint32_t) cast of the saved int64);
 * - obs_canvas_rules_main_view() and _mix_audio():
 *   obs_canvas_create_internal() and obs_canvas_reset_video_internal();
 * - obs_canvas_rules_public(): a public canvas is named, unnamed, renamed
 *   through the hash table and signalled on obs->signals;
 * - obs_canvas_rules_holds_scenes(): obs_canvas_destroy(),
 *   obs_canvas_insert_source() and obs_canvas_remove_source();
 * - obs_canvas_rules_saves(): obs_save_canvas();
 * - obs_canvas_rules_size_set() and _format_set():
 *   obs_canvas_has_valid_video_info();
 * - obs_canvas_rules_resets(): obs_canvas_reset_video();
 * - obs_canvas_rules_global_signals(): obs_canvas_rename_source() and
 *   obs_canvas_set_channel();
 * - obs_canvas_rules_channel_ok(): obs_canvas_set_channel();
 * - obs_canvas_rules_name_empty() and _renames(): obs_canvas_set_name();
 * - obs_canvas_rules_removable(), _mark_removed() and _removed():
 *   obs_canvas_remove() and obs_canvas_removed().
 * Each rule is pinned over every combination of the known flags, with and
 * without an unknown bit, and over every single bit. Cases marked
 * "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_canvas_flags,
 * test_scene_private_canvas, test_canvas_destroy_scene_refs,
 * test_shutdown_canvas_weak_ref and test_scene_canvas_characterization cover
 * the callers through the real libobs.
 */

#define REMOVED OBS_CANVAS_RULES_REMOVED
#define UNKNOWN (1u << 5)

/* the five public flags, MAIN, REMOVED and an unknown bit */
static const uint32_t BITS[] = {MAIN, ACTIVATE, MIX_AUDIO, SCENE_REF, EPHEMERAL, REMOVED, UNKNOWN};
#define BIT_COUNT (sizeof(BITS) / sizeof(BITS[0]))

static uint32_t combo(unsigned n)
{
	uint32_t flags = 0;
	for (size_t i = 0; i < BIT_COUNT; i++)
		if (n & (1u << i))
			flags |= BITS[i];
	return flags;
}

static void check_flag_rule(const char *rule, uint32_t flags, bool actual, bool expected)
{
	if (actual != expected) {
		fprintf(stderr, "%s(%#x) is %d, expected %d\n", rule, flags, actual, expected);
		fail();
	}
}

static void constants(void **state)
{
	(void)state;
	assert_int_equal(OBS_CANVAS_RULES_MAIN, MAIN);
	assert_int_equal(OBS_CANVAS_RULES_ACTIVATE, ACTIVATE);
	assert_int_equal(OBS_CANVAS_RULES_MIX_AUDIO, MIX_AUDIO);
	assert_int_equal(OBS_CANVAS_RULES_SCENE_REF, SCENE_REF);
	assert_int_equal(OBS_CANVAS_RULES_EPHEMERAL, EPHEMERAL);
	assert_int_equal(OBS_CANVAS_RULES_REMOVED, 0x80000000u);
	assert_int_equal(OBS_CANVAS_RULES_PUBLIC, ACTIVATE | MIX_AUDIO | SCENE_REF | EPHEMERAL);
	assert_int_equal(OBS_CANVAS_RULES_PUBLIC, 0x1e);
	assert_int_equal(OBS_CANVAS_RULES_FORMAT_NONE, VIDEO_FORMAT_NONE);
	assert_int_equal(OBS_CANVAS_RULES_MAX_CHANNELS, MAX_CHANNELS);
	assert_int_equal(OBS_CANVAS_RULES_MAX_CHANNELS, 64);
}

static void constructor_flags(void **state)
{
	(void)state;
	/* obs_create_main_canvas() */
	assert_int_equal(obs_canvas_rules_main_flags(), MAIN | PROGRAM);
	assert_int_equal(obs_canvas_rules_main_flags(), 0x0f);

	/* obs_canvas_create() and _create_private(): only the public flags */
	assert_int_equal(obs_canvas_rules_public_flags(0), 0);
	assert_int_equal(obs_canvas_rules_public_flags(PROGRAM), PROGRAM);
	assert_int_equal(obs_canvas_rules_public_flags(PREVIEW), PREVIEW);
	assert_int_equal(obs_canvas_rules_public_flags(DEVICE), DEVICE);
	assert_int_equal(obs_canvas_rules_public_flags(MAIN), 0);
	assert_int_equal(obs_canvas_rules_public_flags(MAIN | PROGRAM), PROGRAM);
	assert_int_equal(obs_canvas_rules_public_flags(REMOVED), 0);
	assert_int_equal(obs_canvas_rules_public_flags(REMOVED | EPHEMERAL), EPHEMERAL);
	assert_int_equal(obs_canvas_rules_public_flags(UINT32_MAX), 0x1e);
	for (unsigned bit = 0; bit < 32; bit++) {
		uint32_t flags = 1u << bit;
		bool kept = bit >= 1 && bit <= 4;
		assert_int_equal(obs_canvas_rules_public_flags(flags), kept ? flags : 0);
	}
	for (unsigned n = 0; n < (1u << BIT_COUNT); n++) {
		uint32_t flags = combo(n);
		assert_int_equal(obs_canvas_rules_public_flags(flags), flags & 0x1e);
	}
}

/* obs_load_canvas(): (uint32_t)obs_data_get_int(data, "flags"), then the
 * public mask */
static uint32_t load_flags(long long saved)
{
	return obs_canvas_rules_public_flags((uint32_t)saved);
}

static void load_flags_from_int64(void **state)
{
	(void)state;
	assert_int_equal(load_flags(0), 0);
	assert_int_equal(load_flags(PROGRAM), PROGRAM);
	assert_int_equal(load_flags(EPHEMERAL), EPHEMERAL);
	/* a saved MAIN or REMOVED is masked off */
	assert_int_equal(load_flags(MAIN | PROGRAM), PROGRAM);
	assert_int_equal(load_flags(0x80000000LL), 0);
	assert_int_equal(load_flags(0x8000001fLL), 0x1e);
	assert_int_equal(load_flags(0xffffffffLL), 0x1e);
	/* characterized, not endorsed: the int64 is truncated before the mask,
	 * so bits above 31 are dropped silently and a negative value keeps the
	 * public flags of its low word */
	assert_int_equal(load_flags(0x100000000LL), 0);
	assert_int_equal(load_flags(0x10000001eLL), 0x1e);
	assert_int_equal(load_flags(0x100000001LL), 0);
	assert_int_equal(load_flags(-1), 0x1e);
	assert_int_equal(load_flags(-2), 0x1e);
	assert_int_equal(load_flags(-32), 0);
	assert_int_equal(load_flags(LLONG_MIN), 0);
	assert_int_equal(load_flags(LLONG_MAX), 0x1e);
}

static void flag_rules_every_combination(void **state)
{
	(void)state;
	for (unsigned n = 0; n < (1u << BIT_COUNT); n++) {
		uint32_t f = combo(n);
		bool main = (f & MAIN) != 0;
		bool removed = (f & REMOVED) != 0;
		check_flag_rule("main_view", f, obs_canvas_rules_main_view(f), (f & ACTIVATE) != 0);
		check_flag_rule("mix_audio", f, obs_canvas_rules_mix_audio(f), (f & MIX_AUDIO) != 0);
		check_flag_rule("holds_scenes", f, obs_canvas_rules_holds_scenes(f), (f & SCENE_REF) != 0);
		check_flag_rule("saves", f, obs_canvas_rules_saves(f), !(f & EPHEMERAL) && !removed);
		check_flag_rule("resets", f, obs_canvas_rules_resets(f), !main);
		check_flag_rule("global_signals", f, obs_canvas_rules_global_signals(f), main);
		check_flag_rule("renames", f, obs_canvas_rules_renames(f), !main);
		check_flag_rule("removable", f, obs_canvas_rules_removable(f), !main && !removed);
		check_flag_rule("removed", f, obs_canvas_rules_removed(f), removed);
		assert_int_equal(obs_canvas_rules_mark_removed(f), f | REMOVED);
	}
}

static void flag_rules_every_bit(void **state)
{
	(void)state;
	for (unsigned bit = 0; bit < 32; bit++) {
		uint32_t f = 1u << bit;
		check_flag_rule("main_view", f, obs_canvas_rules_main_view(f), bit == 1);
		check_flag_rule("mix_audio", f, obs_canvas_rules_mix_audio(f), bit == 2);
		check_flag_rule("holds_scenes", f, obs_canvas_rules_holds_scenes(f), bit == 3);
		check_flag_rule("saves", f, obs_canvas_rules_saves(f), bit != 4 && bit != 31);
		check_flag_rule("resets", f, obs_canvas_rules_resets(f), bit != 0);
		check_flag_rule("global_signals", f, obs_canvas_rules_global_signals(f), bit == 0);
		check_flag_rule("renames", f, obs_canvas_rules_renames(f), bit != 0);
		check_flag_rule("removable", f, obs_canvas_rules_removable(f), bit != 0 && bit != 31);
		check_flag_rule("removed", f, obs_canvas_rules_removed(f), bit == 31);
	}
	/* every flag set */
	check_flag_rule("saves", UINT32_MAX, obs_canvas_rules_saves(UINT32_MAX), false);
	check_flag_rule("removable", UINT32_MAX, obs_canvas_rules_removable(UINT32_MAX), false);
	assert_int_equal(obs_canvas_rules_mark_removed(UINT32_MAX), UINT32_MAX);
	assert_int_equal(obs_canvas_rules_mark_removed(0), REMOVED);
}

static void canvas_kinds(void **state)
{
	(void)state;
	/* the main canvas: a MAIN_VIEW that mixes audio, holds its scenes, is
	 * saved, signals globally and cannot be reset, renamed or removed */
	uint32_t f = obs_canvas_rules_main_flags();
	assert_true(obs_canvas_rules_main_view(f));
	assert_true(obs_canvas_rules_mix_audio(f));
	assert_true(obs_canvas_rules_holds_scenes(f));
	assert_true(obs_canvas_rules_saves(f));
	assert_false(obs_canvas_rules_resets(f));
	assert_true(obs_canvas_rules_global_signals(f));
	assert_false(obs_canvas_rules_renames(f));
	assert_false(obs_canvas_rules_removable(f));

	/* a program canvas from obs_canvas_create(PROGRAM) */
	f = obs_canvas_rules_public_flags(PROGRAM);
	assert_true(obs_canvas_rules_main_view(f));
	assert_true(obs_canvas_rules_mix_audio(f));
	assert_true(obs_canvas_rules_holds_scenes(f));
	assert_true(obs_canvas_rules_saves(f));
	assert_true(obs_canvas_rules_resets(f));
	assert_false(obs_canvas_rules_global_signals(f));
	assert_true(obs_canvas_rules_renames(f));
	assert_true(obs_canvas_rules_removable(f));

	/* a preview canvas: an AUX_VIEW that is not saved */
	f = obs_canvas_rules_public_flags(PREVIEW);
	assert_false(obs_canvas_rules_main_view(f));
	assert_false(obs_canvas_rules_mix_audio(f));
	assert_false(obs_canvas_rules_holds_scenes(f));
	assert_false(obs_canvas_rules_saves(f));

	/* a device canvas: a MAIN_VIEW that is not saved */
	f = obs_canvas_rules_public_flags(DEVICE);
	assert_true(obs_canvas_rules_main_view(f));
	assert_false(obs_canvas_rules_mix_audio(f));
	assert_false(obs_canvas_rules_saves(f));

	/* a caller asking for MAIN gets a plain canvas */
	f = obs_canvas_rules_public_flags(MAIN | PROGRAM);
	assert_false(obs_canvas_rules_global_signals(f));
	assert_true(obs_canvas_rules_renames(f));
	assert_true(obs_canvas_rules_removable(f));
}

static void public_rule(void **state)
{
	(void)state;
	assert_true(obs_canvas_rules_public(false));
	assert_false(obs_canvas_rules_public(true));
}

static void video_info_checks(void **state)
{
	(void)state;
	assert_false(obs_canvas_rules_size_set(0));
	assert_true(obs_canvas_rules_size_set(1));
	assert_true(obs_canvas_rules_size_set(1920));
	assert_true(obs_canvas_rules_size_set(UINT32_MAX));

	assert_false(obs_canvas_rules_format_set(VIDEO_FORMAT_NONE));
	assert_true(obs_canvas_rules_format_set(VIDEO_FORMAT_I420));
	assert_true(obs_canvas_rules_format_set(VIDEO_FORMAT_NV12));
	assert_true(obs_canvas_rules_format_set(VIDEO_FORMAT_P010));
	/* characterized, not endorsed: a format outside the enum counts as
	 * set */
	assert_true(obs_canvas_rules_format_set(-1));
	assert_true(obs_canvas_rules_format_set(9999));
	assert_true(obs_canvas_rules_format_set(INT_MIN));
	assert_true(obs_canvas_rules_format_set(INT_MAX));
}

static void channel_bound(void **state)
{
	(void)state;
	assert_true(obs_canvas_rules_channel_ok(0));
	assert_true(obs_canvas_rules_channel_ok(1));
	assert_true(obs_canvas_rules_channel_ok(MAX_CHANNELS - 1));
	assert_false(obs_canvas_rules_channel_ok(MAX_CHANNELS));
	assert_false(obs_canvas_rules_channel_ok(MAX_CHANNELS + 1));
	assert_false(obs_canvas_rules_channel_ok(0x80000000u));
	assert_false(obs_canvas_rules_channel_ok(UINT32_MAX));
	for (uint32_t ch = 0; ch < 1024; ch++)
		assert_int_equal(obs_canvas_rules_channel_ok(ch), ch < 64);
}

static void name_checks(void **state)
{
	(void)state;
	assert_true(obs_canvas_rules_name_empty(NULL));
	assert_true(obs_canvas_rules_name_empty(""));
	assert_false(obs_canvas_rules_name_empty(" "));
	assert_false(obs_canvas_rules_name_empty("a"));
	assert_false(obs_canvas_rules_name_empty("Main"));
	/* only the first byte is read */
	static const char embedded[] = {'\0', 'b', '\0'};
	assert_true(obs_canvas_rules_name_empty(embedded));
	static const char high[] = {(char)0xff, '\0'};
	assert_false(obs_canvas_rules_name_empty(high));
}

static void remove_twice(void **state)
{
	(void)state;
	uint32_t f = obs_canvas_rules_public_flags(PROGRAM);
	assert_true(obs_canvas_rules_removable(f));
	assert_false(obs_canvas_rules_removed(f));
	f = obs_canvas_rules_mark_removed(f);
	assert_int_equal(f, PROGRAM | REMOVED);
	assert_true(obs_canvas_rules_removed(f));
	/* the second obs_canvas_remove() returns before anything */
	assert_false(obs_canvas_rules_removable(f));
	/* a removed canvas is not saved */
	assert_false(obs_canvas_rules_saves(f));
	/* characterized, not endorsed: a removed canvas can still be reset and
	 * renamed, and keeps its other rules */
	assert_true(obs_canvas_rules_resets(f));
	assert_true(obs_canvas_rules_renames(f));
	assert_true(obs_canvas_rules_holds_scenes(f));
	assert_true(obs_canvas_rules_mix_audio(f));
	/* a reloaded removed canvas comes back as not removed */
	assert_int_equal(obs_canvas_rules_public_flags(f), PROGRAM);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(constructor_flags),
		cmocka_unit_test(load_flags_from_int64),
		cmocka_unit_test(flag_rules_every_combination),
		cmocka_unit_test(flag_rules_every_bit),
		cmocka_unit_test(canvas_kinds),
		cmocka_unit_test(public_rule),
		cmocka_unit_test(video_info_checks),
		cmocka_unit_test(channel_bound),
		cmocka_unit_test(name_checks),
		cmocka_unit_test(remove_twice),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
