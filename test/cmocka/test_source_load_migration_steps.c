#include <float.h>
#include <limits.h>
#include <math.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-load.h"

/*
 * Characterization of the source load migration and value rules of
 * obs_load_source_type() in libobs/obs.c, moved to libobs/obs-source-load.c
 * before their Rust port:
 * - obs_source_load_prev_ver(): the saved "prev_ver" (an int64) as the
 *   uint32_t the source is created with;
 * - obs_source_load_volume() (#2128) and obs_source_load_balance() (#2058):
 *   the saved double brought into range before the float cast;
 * - obs_source_load_delay() (#2058): a negative push-to-mute or push-to-talk
 *   delay is none;
 * - obs_source_load_deinterlace_mode() (#2120): a mode past the int range is
 *   no mode;
 * - obs_source_load_monitoring_migration(): before 23.2.2 a monitor-by-default
 *   source becomes monitor-only with all six mixers; before 33.0.0
 *   "monitoring_enabled" is stored from the monitoring type.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, through their header. obs.c keeps every
 * obs_data_*() call and setter; the real-libobs tests
 * (test_source_audio_ranges, test_source_volume_mix,
 * test_deinterlace_mode_load) cover the caller.
 *
 * Every input comes from a scene collection (a JSON file the user may have
 * been given), so any int64 or double can arrive.
 */

#define V(major, minor, patch) (((uint32_t)(major) << 24) | ((uint32_t)(minor) << 16) | (uint32_t)(patch))
#define V23_2_2 V(23, 2, 2)
#define V33_0_0 V(33, 0, 0)
#define CAP ((uint32_t)OBS_SOURCE_LOAD_MONITOR_BY_DEFAULT)

/* ------------------------------------------------------------------------ */
/* helpers */

static uint32_t float_bits(float f)
{
	uint32_t u;
	memcpy(&u, &f, sizeof(u));
	return u;
}

static float bits_float(uint32_t u)
{
	float f;
	memcpy(&f, &u, sizeof(f));
	return f;
}

static double bits_double(uint64_t u)
{
	double d;
	memcpy(&d, &u, sizeof(d));
	return d;
}

/* got and expected the same float, bit for bit (so -0.0 is not 0.0); any NaN
 * matches any NaN. */
static void check_float(const char *what, double in, float got, float expected)
{
	if (isnan(expected) ? !isnan(got) : float_bits(got) != float_bits(expected)) {
		fprintf(stderr, "%s(%a): got %a (0x%08x), expected %a (0x%08x)\n", what, in, (double)got,
			float_bits(got), (double)expected, float_bits(expected));
		fail();
	}
}

static void check_plan(uint32_t prev_ver, uint32_t caps, int64_t type, struct obs_source_load_monitoring got,
		       bool set_mixers, uint32_t mixers, bool store_enabled, bool enabled)
{
	if (got.set_mixers != set_mixers || got.mixers != mixers || got.store_enabled != store_enabled ||
	    got.enabled != enabled) {
		fprintf(stderr,
			"migration(prev_ver 0x%08x, caps 0x%08x, type %lld): set_mixers %d mixers 0x%x "
			"store %d enabled %d, expected %d 0x%x %d %d\n",
			prev_ver, caps, (long long)type, got.set_mixers, got.mixers, got.store_enabled, got.enabled,
			set_mixers, mixers, store_enabled, enabled);
		fail();
	}
}

static void expect_plan(uint32_t prev_ver, uint32_t caps, int64_t type, bool set_mixers, bool store_enabled,
			bool enabled)
{
	check_plan(prev_ver, caps, type, obs_source_load_monitoring_migration(prev_ver, caps, type), set_mixers,
		   set_mixers ? 0x3Fu : 0u, store_enabled, enabled);
}

/* ------------------------------------------------------------------------ */
/* constants */

static void test_constants(void **state)
{
	(void)state;
	assert_int_equal(OBS_SOURCE_LOAD_MONITOR_ONLY_BEFORE, 0x17020002);
	assert_int_equal(OBS_SOURCE_LOAD_MONITOR_ONLY_BEFORE, V23_2_2);
	assert_int_equal(OBS_SOURCE_LOAD_MONITORING_ENABLED_BEFORE, 0x21000000);
	assert_int_equal(OBS_SOURCE_LOAD_MONITORING_ENABLED_BEFORE, V33_0_0);
	assert_int_equal(OBS_SOURCE_LOAD_MONITOR_BY_DEFAULT, 0x800);
	assert_int_equal(OBS_SOURCE_LOAD_MONITORING_NONE, 0);
	assert_int_equal(OBS_SOURCE_LOAD_MONITORING_MONITOR_ONLY, 1);
	assert_int_equal(OBS_SOURCE_LOAD_ALL_MIXERS, 0x3F);
	assert_int_equal(OBS_SOURCE_LOAD_DEINTERLACE_DISABLE, 0);
}

/* ------------------------------------------------------------------------ */
/* prev_ver */

static void test_prev_ver(void **state)
{
	(void)state;
	assert_int_equal(obs_source_load_prev_ver(0), 0);
	assert_int_equal(obs_source_load_prev_ver(1), 1);
	assert_int_equal(obs_source_load_prev_ver(V23_2_2 - 1), V23_2_2 - 1);
	assert_int_equal(obs_source_load_prev_ver(V23_2_2), V23_2_2);
	assert_int_equal(obs_source_load_prev_ver(V33_0_0), V33_0_0);
	assert_int_equal(obs_source_load_prev_ver(INT32_MAX), INT32_MAX);
	assert_int_equal(obs_source_load_prev_ver((int64_t)INT32_MAX + 1), 0x80000000u);
	assert_int_equal(obs_source_load_prev_ver(UINT32_MAX), UINT32_MAX);
}

/* characterized, not endorsed: an int64 past UINT32_MAX keeps its low 32
 * bits, so a version saved as 2^32 + 23.2.1 reads as 23.2.1 (migrated), and a
 * negative one wraps. */
static void test_prev_ver_truncates(void **state)
{
	(void)state;
	assert_int_equal(obs_source_load_prev_ver((int64_t)UINT32_MAX + 1), 0);
	assert_int_equal(obs_source_load_prev_ver((int64_t)UINT32_MAX + 2), 1);
	assert_int_equal(obs_source_load_prev_ver(((int64_t)1 << 32) | V(23, 2, 1)), V(23, 2, 1));
	assert_int_equal(obs_source_load_prev_ver(((int64_t)1 << 32) | V33_0_0), V33_0_0);
	assert_int_equal(obs_source_load_prev_ver(-1), UINT32_MAX);
	assert_int_equal(obs_source_load_prev_ver(-((int64_t)1 << 32)), 0);
	assert_int_equal(obs_source_load_prev_ver(INT64_MIN), 0);
	assert_int_equal(obs_source_load_prev_ver(INT64_MAX), UINT32_MAX);
}

/* ------------------------------------------------------------------------ */
/* monitoring migration */

/* 23.2.2: below it, a monitor-by-default source becomes monitor-only with all
 * six mixers; at and above it, nothing changes the type or the mixers. */
static void test_migration_23_2_2_edges(void **state)
{
	(void)state;
	const uint32_t below[] = {0, 1, V(22, 0, 3), V(23, 0, 0), V(23, 2, 0), V(23, 2, 1), V23_2_2 - 1};
	const uint32_t at_or_above[] = {V23_2_2, V23_2_2 + 1, V(23, 2, 3), V(24, 0, 0), V(30, 2, 3)};

	for (size_t i = 0; i < sizeof(below) / sizeof(below[0]); i++) {
		expect_plan(below[i], CAP, OBS_SOURCE_LOAD_MONITORING_NONE, true, true, true);
		expect_plan(below[i], CAP, 2, true, true, true);
		expect_plan(below[i], 0, OBS_SOURCE_LOAD_MONITORING_NONE, false, true, false);
		expect_plan(below[i], 0, 1, false, true, true);
	}
	for (size_t i = 0; i < sizeof(at_or_above) / sizeof(at_or_above[0]); i++) {
		expect_plan(at_or_above[i], CAP, OBS_SOURCE_LOAD_MONITORING_NONE, false, true, false);
		expect_plan(at_or_above[i], CAP, 2, false, true, true);
		expect_plan(at_or_above[i], 0, OBS_SOURCE_LOAD_MONITORING_NONE, false, true, false);
	}
}

/* 33.0.0: below it, "monitoring_enabled" is stored from the type; at and above
 * it, nothing is stored and the saved value stands. */
static void test_migration_33_0_0_edges(void **state)
{
	(void)state;
	const uint32_t below[] = {V23_2_2, V(32, 255, 65535), V(32, 0, 0), V33_0_0 - 1};
	const uint32_t at_or_above[] = {V33_0_0,   V33_0_0 + 1, V(33, 0, 1),    V(33, 1, 0), V(34, 0, 0),
					INT32_MAX, 0x80000000u, UINT32_MAX - 1, UINT32_MAX};

	for (size_t i = 0; i < sizeof(below) / sizeof(below[0]); i++) {
		expect_plan(below[i], 0, 0, false, true, false);
		expect_plan(below[i], 0, 1, false, true, true);
		expect_plan(below[i], 0, 2, false, true, true);
		expect_plan(below[i], CAP, 0, false, true, false);
	}
	for (size_t i = 0; i < sizeof(at_or_above) / sizeof(at_or_above[0]); i++) {
		for (int64_t type = -1; type <= 3; type++) {
			expect_plan(at_or_above[i], 0, type, false, false, false);
			expect_plan(at_or_above[i], CAP, type, false, false, false);
			expect_plan(at_or_above[i], UINT32_MAX, type, false, false, false);
		}
	}
}

/* prev_ver 0 (never saved: obs_data_get_int() of a missing key) migrates both
 * ways; UINT32_MAX migrates neither. */
static void test_migration_prev_ver_ends(void **state)
{
	(void)state;
	expect_plan(0, CAP, 0, true, true, true);
	expect_plan(0, 0, 0, false, true, false);
	expect_plan(0, 0, 2, false, true, true);
	expect_plan(UINT32_MAX, CAP, 0, false, false, false);
	expect_plan(UINT32_MAX, 0, 2, false, false, false);
	/* the truncated int64 above UINT32_MAX (characterized, not endorsed) */
	expect_plan(obs_source_load_prev_ver(((int64_t)1 << 32) | V(23, 2, 1)), CAP, 0, true, true, true);
	expect_plan(obs_source_load_prev_ver((int64_t)UINT32_MAX + 1), 0, 2, false, true, true);
}

/* Each monitoring type, and values outside the enum, with and without the
 * cap, before 23.2.2 and between 23.2.2 and 33.0.0. Only the cap, never the
 * type, decides the mixers. */
static void test_migration_each_type(void **state)
{
	(void)state;
	/* type, enabled when not migrated to monitor-only */
	const struct {
		int64_t type;
		bool enabled;
	} types[] = {
		{0, false},      /* OBS_MONITORING_TYPE_NONE */
		{1, true},       /* OBS_MONITORING_TYPE_MONITOR_ONLY */
		{2, true},       /* OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT */
		{3, true},       /* outside the enum: not none */
		{-1, true},      /* outside the enum */
		{INT_MAX, true}, /* outside the enum */
		{INT_MIN, true}, /* outside the enum */
	};
	const uint32_t caps_list[] = {0, CAP, ~CAP, UINT32_MAX};

	for (size_t t = 0; t < sizeof(types) / sizeof(types[0]); t++) {
		for (size_t c = 0; c < sizeof(caps_list) / sizeof(caps_list[0]); c++) {
			bool cap = (caps_list[c] & CAP) != 0;
			expect_plan(V(20, 0, 0), caps_list[c], types[t].type, cap, true, cap || types[t].enabled);
			expect_plan(V(30, 0, 0), caps_list[c], types[t].type, false, true, types[t].enabled);
		}
	}
}

/* characterized, not endorsed: the saved int64 type is read as an int (its
 * low 32 bits), so 2^32 is none and 2^32 + 1 is monitor-only. */
static void test_migration_type_truncates(void **state)
{
	(void)state;
	expect_plan(V(30, 0, 0), 0, (int64_t)1 << 32, false, true, false);
	expect_plan(V(30, 0, 0), 0, ((int64_t)1 << 32) + 1, false, true, true);
	expect_plan(V(30, 0, 0), 0, (int64_t)UINT32_MAX, false, true, true);
	expect_plan(V(30, 0, 0), 0, INT64_MIN, false, true, false);
	expect_plan(V(30, 0, 0), 0, INT64_MAX, false, true, true);
	expect_plan(V(30, 0, 0), 0, -((int64_t)1 << 32), false, true, false);
	/* before 23.2.2 with the cap the type does not matter */
	expect_plan(V(20, 0, 0), CAP, (int64_t)1 << 32, true, true, true);
}

/* Every listed version, cap set and type against the rule written out. */
static void test_migration_cross(void **state)
{
	(void)state;
	const uint32_t versions[] = {0,           1,       V(23, 2, 1), V23_2_2,     V23_2_2 + 1, V(32, 0, 0),
				     V33_0_0 - 1, V33_0_0, V33_0_0 + 1, 0x7fffffffu, 0x80000000u, UINT32_MAX};
	const uint32_t caps_list[] = {0, 1, CAP, CAP | 1, CAP >> 1, CAP << 1, ~CAP, UINT32_MAX};
	const int64_t types[] = {
		0, 1, 2, 3, -1, INT_MAX, INT_MIN, INT64_MIN, INT64_MAX, (int64_t)1 << 32, ((int64_t)1 << 32) + 2};
	size_t n = 0;

	for (size_t v = 0; v < sizeof(versions) / sizeof(versions[0]); v++) {
		for (size_t c = 0; c < sizeof(caps_list) / sizeof(caps_list[0]); c++) {
			for (size_t t = 0; t < sizeof(types) / sizeof(types[0]); t++) {
				uint32_t ver = versions[v];
				bool monitor_only = ver < V23_2_2 && (caps_list[c] & CAP) != 0;
				int type = monitor_only ? 1 : (int)(uint32_t)(uint64_t)types[t];
				bool store = ver < V33_0_0;
				expect_plan(ver, caps_list[c], types[t], monitor_only, store, store && type != 0);
				n++;
			}
		}
	}
	assert_int_equal(n, 12 * 8 * 11);
}

/* ------------------------------------------------------------------------ */
/* volume */

static void expect_volume(double in, float expected)
{
	check_float("volume", in, obs_source_load_volume(in), expected);
}

static void test_volume_in_range(void **state)
{
	(void)state;
	expect_volume(0.0, 0.0f);
	expect_volume(0.5, 0.5f);
	expect_volume(1.0, 1.0f);
	expect_volume(2.0, 2.0f);
	expect_volume(20.0, 20.0f);
	expect_volume(0.1, 0.1f);
	/* rounds to nearest, ties to even */
	expect_volume(1.0 + 0x1p-24, 1.0f);
	expect_volume(1.0 + 0x1p-24 + 0x1p-40, 1.0f + 0x1p-23f);
	expect_volume(1.0 + 3 * 0x1p-24, 1.0f + 0x1p-22f);
	/* the smallest float subnormal; below half of it is 0 */
	expect_volume(0x1p-149, bits_float(1));
	expect_volume(0x1p-151, 0.0f);
	expect_volume((double)FLT_MIN, FLT_MIN);
	expect_volume(DBL_MIN, 0.0f);
	expect_volume(nextafter(0.0, 1.0), 0.0f);
}

static void test_volume_negative(void **state)
{
	(void)state;
	expect_volume(-1.0, 0.0f);
	expect_volume(-0.5, 0.0f);
	expect_volume(nextafter(0.0, -1.0), 0.0f);
	expect_volume(-DBL_MIN, 0.0f);
	expect_volume(-(double)FLT_MAX, 0.0f);
	expect_volume(-DBL_MAX, 0.0f);
	expect_volume(-INFINITY, 0.0f);
	/* characterized, not endorsed: -0.0 is not below 0, so it stays -0.0f
	 * (obs_source_set_volume() keeps it; it scales as 0) */
	expect_volume(-0.0, -0.0f);
}

static void test_volume_past_float(void **state)
{
	(void)state;
	expect_volume((double)FLT_MAX, FLT_MAX);
	expect_volume(nextafter((double)FLT_MAX, 0.0), FLT_MAX);
	expect_volume(nextafter((double)FLT_MAX, INFINITY), INFINITY);
	/* characterized, not endorsed: past FLT_MAX by less than float rounding
	 * would take back is still infinity, not FLT_MAX */
	expect_volume((double)FLT_MAX + 0x1p103, INFINITY);
	expect_volume(3.5e38, INFINITY);
	expect_volume(1e300, INFINITY);
	expect_volume(DBL_MAX, INFINITY);
	expect_volume(INFINITY, INFINITY);
}

/* NaN is neither below 0 nor past FLT_MAX: it stays NaN, whatever its sign or
 * payload, and obs_source_set_volume() maps it (#2108). */
static void test_volume_nan(void **state)
{
	(void)state;
	const uint64_t nans[] = {0x7ff8000000000000ull, 0xfff8000000000000ull, 0x7ff0000000000001ull,
				 0x7ff4000000000000ull, 0xfff0000000000001ull, 0x7fffffffffffffffull,
				 0xffffffffffffffffull};
	for (size_t i = 0; i < sizeof(nans) / sizeof(nans[0]); i++) {
		float got = obs_source_load_volume(bits_double(nans[i]));
		if (!isnan(got)) {
			fprintf(stderr, "volume(NaN 0x%016llx): got %a\n", (unsigned long long)nans[i], (double)got);
			fail();
		}
	}
}

/* ------------------------------------------------------------------------ */
/* balance */

static void expect_balance(double in, float expected)
{
	check_float("balance", in, obs_source_load_balance(in), expected);
}

static void test_balance_in_range(void **state)
{
	(void)state;
	expect_balance(0.0, 0.0f);
	expect_balance(0.25, 0.25f);
	expect_balance(0.5, 0.5f);
	expect_balance(1.0, 1.0f);
	expect_balance(0.1, 0.1f);
	expect_balance(nextafter(1.0, 0.0), 1.0f); /* rounds up to 1 */
	expect_balance(1.0 - 0x1p-24, 1.0f - 0x1p-24f);
	expect_balance(0x1p-149, bits_float(1));
	expect_balance(nextafter(0.0, 1.0), 0.0f);
}

static void test_balance_out_of_range(void **state)
{
	(void)state;
	expect_balance(nextafter(1.0, 2.0), 1.0f);
	expect_balance(1.5, 1.0f);
	expect_balance(2.0, 1.0f);
	expect_balance((double)FLT_MAX, 1.0f);
	expect_balance(DBL_MAX, 1.0f);
	expect_balance(INFINITY, 1.0f);
	expect_balance(nextafter(0.0, -1.0), 0.0f);
	expect_balance(-0.5, 0.0f);
	expect_balance(-DBL_MAX, 0.0f);
	expect_balance(-INFINITY, 0.0f);
	/* characterized, not endorsed: -0.0 stays -0.0f (the setter's clamp
	 * keeps it too; it pans as 0) */
	expect_balance(-0.0, -0.0f);
}

/* NaN stays NaN; obs_source_set_balance_value() maps it to 0.5 (#2051). */
static void test_balance_nan(void **state)
{
	(void)state;
	const uint64_t nans[] = {0x7ff8000000000000ull, 0xfff8000000000000ull, 0x7ff0000000000001ull,
				 0xffffffffffffffffull};
	for (size_t i = 0; i < sizeof(nans) / sizeof(nans[0]); i++) {
		float got = obs_source_load_balance(bits_double(nans[i]));
		if (!isnan(got)) {
			fprintf(stderr, "balance(NaN 0x%016llx): got %a\n", (unsigned long long)nans[i], (double)got);
			fail();
		}
	}
}

/* ------------------------------------------------------------------------ */
/* delays */

static void test_delay(void **state)
{
	(void)state;
	assert_true(obs_source_load_delay(0) == 0);
	assert_true(obs_source_load_delay(1) == 1);
	assert_true(obs_source_load_delay(200) == 200);
	assert_true(obs_source_load_delay(INT32_MAX) == INT32_MAX);
	assert_true(obs_source_load_delay(UINT32_MAX) == UINT32_MAX);
	assert_true(obs_source_load_delay((int64_t)UINT32_MAX + 1) == (uint64_t)UINT32_MAX + 1);
	assert_true(obs_source_load_delay(INT64_MAX) == (uint64_t)INT64_MAX);
	/* negative is none, not a huge unsigned delay */
	assert_true(obs_source_load_delay(-1) == 0);
	assert_true(obs_source_load_delay(-200) == 0);
	assert_true(obs_source_load_delay(INT32_MIN) == 0);
	assert_true(obs_source_load_delay(INT64_MIN + 1) == 0);
	assert_true(obs_source_load_delay(INT64_MIN) == 0);
}

/* ------------------------------------------------------------------------ */
/* deinterlace mode */

static void test_deinterlace_mode_in_range(void **state)
{
	(void)state;
	/* every enum obs_deinterlace_mode value as itself */
	for (int64_t mode = 0; mode <= 8; mode++)
		assert_int_equal(obs_source_load_deinterlace_mode(mode), (int)mode);
	/* in the int range but outside the enum: passed on, and
	 * obs_source_set_deinterlace_mode() disables it (#2120) */
	assert_int_equal(obs_source_load_deinterlace_mode(9), 9);
	assert_int_equal(obs_source_load_deinterlace_mode(-1), -1);
	assert_int_equal(obs_source_load_deinterlace_mode(INT_MAX), INT_MAX);
	assert_int_equal(obs_source_load_deinterlace_mode(INT_MIN), INT_MIN);
}

/* Past the int range is no mode, not its low 32 bits. */
static void test_deinterlace_mode_out_of_range(void **state)
{
	(void)state;
	assert_int_equal(obs_source_load_deinterlace_mode((int64_t)INT_MAX + 1), 0);
	assert_int_equal(obs_source_load_deinterlace_mode((int64_t)INT_MIN - 1), 0);
	assert_int_equal(obs_source_load_deinterlace_mode(UINT32_MAX), 0);
	assert_int_equal(obs_source_load_deinterlace_mode(((int64_t)1 << 32) + 1), 0);
	assert_int_equal(obs_source_load_deinterlace_mode(((int64_t)1 << 32) + 5), 0);
	assert_int_equal(obs_source_load_deinterlace_mode(-((int64_t)1 << 32) + 5), 0);
	assert_int_equal(obs_source_load_deinterlace_mode(INT64_MAX), 0);
	assert_int_equal(obs_source_load_deinterlace_mode(INT64_MIN), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_constants),
		cmocka_unit_test(test_prev_ver),
		cmocka_unit_test(test_prev_ver_truncates),
		cmocka_unit_test(test_migration_23_2_2_edges),
		cmocka_unit_test(test_migration_33_0_0_edges),
		cmocka_unit_test(test_migration_prev_ver_ends),
		cmocka_unit_test(test_migration_each_type),
		cmocka_unit_test(test_migration_type_truncates),
		cmocka_unit_test(test_migration_cross),
		cmocka_unit_test(test_volume_in_range),
		cmocka_unit_test(test_volume_negative),
		cmocka_unit_test(test_volume_past_float),
		cmocka_unit_test(test_volume_nan),
		cmocka_unit_test(test_balance_in_range),
		cmocka_unit_test(test_balance_out_of_range),
		cmocka_unit_test(test_balance_nan),
		cmocka_unit_test(test_delay),
		cmocka_unit_test(test_deinterlace_mode_in_range),
		cmocka_unit_test(test_deinterlace_mode_out_of_range),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
