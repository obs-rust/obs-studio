#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "audio-monitoring/osx/coreaudio-device-rules.h"

/* Characterization of the CoreAudio monitoring device rules, moved from
 * libobs/audio-monitoring/osx/coreaudio-enum-devices.c to
 * libobs/audio-monitoring/osx/coreaudio-device-rules.c: whether the device
 * list shows a device, how many ids it walks, the "default" id, the name
 * match and the virtual device check. The file includes no Apple header, so
 * this runs on every platform; the CoreAudio monitor itself only runs on
 * macOS. The functions are hidden in libobs, so this characterizes that C
 * source; the test calls only them. */

#define ID_SIZE 4 /* sizeof(AudioDeviceID) */

/* ------------------------------------------------------------------------ */
/* obs_enum_audio_monitoring_device() */

static void a_device_without_output_streams_is_listed_only_with_inputs(void **state)
{
	(void)state;

	assert_true(coreaudio_device_listed(true, 0));
	assert_false(coreaudio_device_listed(false, 0));
}

static void a_device_with_output_streams_is_always_listed(void **state)
{
	(void)state;

	const uint32_t sizes[] = {1, 4, 8, 0x80000000u, UINT32_MAX};
	for (size_t i = 0; i < sizeof(sizes) / sizeof(sizes[0]); i++) {
		assert_true(coreaudio_device_listed(false, sizes[i]));
		assert_true(coreaudio_device_listed(true, sizes[i]));
	}
}

/* ------------------------------------------------------------------------ */
/* enum_audio_devices() */

static void the_count_is_whole_ids(void **state)
{
	(void)state;

	assert_int_equal(coreaudio_device_count(0, 0, ID_SIZE), 0);
	assert_int_equal(coreaudio_device_count(4, 4, ID_SIZE), 1);
	assert_int_equal(coreaudio_device_count(12, 12, ID_SIZE), 3);
	/* a partial id is not counted */
	assert_int_equal(coreaudio_device_count(13, 13, ID_SIZE), 3);
	assert_int_equal(coreaudio_device_count(3, 3, ID_SIZE), 0);
	assert_int_equal(coreaudio_device_count(UINT32_MAX, UINT32_MAX, ID_SIZE), UINT32_MAX / 4);
	assert_int_equal(coreaudio_device_count(12, 12, 1), 12);
}

static void a_larger_second_size_does_not_grow_the_count(void **state)
{
	(void)state;

	/* The array holds the first size. */
	assert_int_equal(coreaudio_device_count(8, 12, ID_SIZE), 2);
	assert_int_equal(coreaudio_device_count(0, 4, ID_SIZE), 0);
}

/* ------------------------------------------------------------------------ */
/* devices_match() */

static void only_default_is_the_default_id(void **state)
{
	(void)state;

	assert_true(coreaudio_device_is_default("default"));

	const char *others[] = {"Default",  "DEFAULT",  "defaulT",
				"default ", " default", "defaul",
				"defaultx", "",         "BuiltInSpeakerDevice"};
	for (size_t i = 0; i < sizeof(others) / sizeof(others[0]); i++)
		assert_false(coreaudio_device_is_default(others[i]));
}

static void names_match_when_both_are_found_and_equal(void **state)
{
	(void)state;

	assert_true(coreaudio_device_names_match("MacBook Pro Speakers", "MacBook Pro Speakers"));
	assert_true(coreaudio_device_names_match("", ""));
	assert_true(coreaudio_device_names_match("\xc3\xa9", "\xc3\xa9"));
	assert_false(coreaudio_device_names_match("MacBook Pro Speakers", "BlackHole 2ch"));
	/* case counts */
	assert_false(coreaudio_device_names_match("Speakers", "speakers"));
	assert_false(coreaudio_device_names_match("Speakers", "Speakers "));
	assert_false(coreaudio_device_names_match("Speakers", ""));
}

static void a_name_not_found_never_matches(void **state)
{
	(void)state;

	assert_false(coreaudio_device_names_match(NULL, "Speakers"));
	assert_false(coreaudio_device_names_match("Speakers", NULL));
	assert_false(coreaudio_device_names_match(NULL, NULL));
	assert_false(coreaudio_device_names_match(NULL, ""));
	assert_false(coreaudio_device_names_match("", NULL));
}

static void two_devices_with_one_name_match(void **state)
{
	(void)state;

	/* devices_match() compares the names it looked up for both uids, so
	 * two devices with the same name match whatever their uids. The
	 * self-monitoring check relies on it. */
	const char *name_of_uid1 = "USB Audio";
	const char *name_of_uid2 = "USB Audio";
	assert_true(coreaudio_device_names_match(name_of_uid1, name_of_uid2));
}

/* ------------------------------------------------------------------------ */
/* device_is_input(), get_desktop_default_id() and find_loopback_cb() */

static const char *const virtual_names[][3] = {
	{"soundflower", "SOUNDFLOWER", "SoundFlower"},
	{"wavtap", "WAVTAP", "WavTap"},
	{"soundsiphon", "SOUNDSIPHON", "SoundSiphon"},
	{"ishowu", "ISHOWU", "iShowU"},
	{"blackhole", "BLACKHOLE", "BlackHole"},
	{"loopback", "LOOPBACK", "LoopBack"},
	{"groundcontrol", "GROUNDCONTROL", "GroundControl"},
	{"vbcable", "VBCABLE", "VBCable"},
};

static void each_virtual_name_in_any_case_is_not_an_input(void **state)
{
	(void)state;

	for (size_t i = 0; i < sizeof(virtual_names) / sizeof(virtual_names[0]); i++) {
		for (size_t c = 0; c < 3; c++) {
			char uid[96];
			const char *name = virtual_names[i][c];

			assert_false(coreaudio_device_is_input(name));

			snprintf(uid, sizeof(uid), "com.example.%s:2ch_UID", name);
			if (coreaudio_device_is_input(uid)) {
				fprintf(stderr, "%s counted as a real device\n", uid);
				fail();
			}

			snprintf(uid, sizeof(uid), "%s_UID", name);
			assert_false(coreaudio_device_is_input(uid));

			snprintf(uid, sizeof(uid), "\xc3\xa9%s\xc3\xa9", name);
			assert_false(coreaudio_device_is_input(uid));
		}
	}
}

static void a_uid_without_a_virtual_name_is_an_input(void **state)
{
	(void)state;

	const char *uids[] = {
		"BuiltInSpeakerDevice",
		"AppleUSBAudioEngine:Generic:USB Audio:14200000:2",
		"",
		/* near misses */
		"blackhol",
		"black hole",
		"Black-Hole",
		"VB-Cable",
		"sound flower",
		"loopbac",
		"ishow",
		/* only ASCII case is folded: a fullwidth B and a dotted I */
		"\xef\xbc\xa2lackHole",
		"\xc4\xb0ShowU",
	};
	for (size_t i = 0; i < sizeof(uids) / sizeof(uids[0]); i++) {
		if (!coreaudio_device_is_input(uids[i])) {
			fprintf(stderr, "\"%s\" counted as a virtual device\n", uids[i]);
			fail();
		}
	}
}

static void a_null_uid_is_an_input(void **state)
{
	(void)state;

	/* astrstri() finds nothing in NULL. The callers never pass one. */
	assert_true(coreaudio_device_is_input(NULL));
}

/* ------------------------------------------------------------------------ */
/* A device went away between enum_audio_devices()'s two calls */

#ifdef COREAUDIO_DEVICE_RULES_TEST_RUST

/* The Rust port's intentional difference: the walk stops at the ids the
 * second call wrote. */
static void the_port_counts_only_the_ids_written(void **state)
{
	(void)state;

	assert_int_equal(coreaudio_device_count(12, 8, ID_SIZE), 2);
	assert_int_equal(coreaudio_device_count(12, 0, ID_SIZE), 0);
	assert_int_equal(coreaudio_device_count(13, 9, ID_SIZE), 2);
	assert_int_equal(coreaudio_device_count(UINT32_MAX, 4, ID_SIZE), 1);
}

#else

/* Characterized, not endorsed: the C keeps the count of the first size, so
 * the walk reads ids the second call never wrote. */
static void the_c_counts_the_first_size(void **state)
{
	(void)state;

	assert_int_equal(coreaudio_device_count(12, 8, ID_SIZE), 3);
	assert_int_equal(coreaudio_device_count(12, 0, ID_SIZE), 3);
	assert_int_equal(coreaudio_device_count(13, 9, ID_SIZE), 3);
	assert_int_equal(coreaudio_device_count(UINT32_MAX, 4, ID_SIZE), UINT32_MAX / 4);
}

#endif

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(a_device_without_output_streams_is_listed_only_with_inputs),
		cmocka_unit_test(a_device_with_output_streams_is_always_listed),
		cmocka_unit_test(the_count_is_whole_ids),
		cmocka_unit_test(a_larger_second_size_does_not_grow_the_count),
		cmocka_unit_test(only_default_is_the_default_id),
		cmocka_unit_test(names_match_when_both_are_found_and_equal),
		cmocka_unit_test(a_name_not_found_never_matches),
		cmocka_unit_test(two_devices_with_one_name_match),
		cmocka_unit_test(each_virtual_name_in_any_case_is_not_an_input),
		cmocka_unit_test(a_uid_without_a_virtual_name_is_an_input),
		cmocka_unit_test(a_null_uid_is_an_input),
#ifdef COREAUDIO_DEVICE_RULES_TEST_RUST
		cmocka_unit_test(the_port_counts_only_the_ids_written),
#else
		cmocka_unit_test(the_c_counts_the_first_size),
#endif
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
