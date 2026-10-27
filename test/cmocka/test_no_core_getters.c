/* Regression test for obs-rust/obs-studio#2566: with no core (before
 * obs_startup(), or after obs_shutdown()), obs_get_video_info() returns false
 * (#1509), but these getters read through the NULL core:
 *
 * - obs_get_video_sdr_white_level() and
 *   obs_get_video_hdr_nominal_peak_level() (obs->video.graphics);
 * - obs_get_audio_info() (obs->audio.audio), and obs_get_audio_info2(),
 *   which calls it first;
 * - obs_nv12_tex_active() and obs_p010_tex_active()
 *   (obs->data.main_canvas).
 *
 * With no core each gives the answer it already gives with no video or no
 * audio: 300, 1000, false (the out-struct untouched), false and false.
 *
 * The test never starts the core, so it needs no display or graphics module.
 * It covers the never-started case only. After obs_shutdown() the core is
 * NULL again and the getters take the same no-core branch, so that case is
 * not run separately.
 *
 * Each getter has its own case: before the fix each case crashes, and cmocka
 * reports the crash as that case's failure. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include <obs.h>

/* obs_nv12_tex_active() and obs_p010_tex_active() are deprecated but still
 * exported, so a plugin can still call them with no core. */
#if defined(_MSC_VER)
#pragma warning(disable : 4996)
#else
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif

#define SENTINEL_RATE 12345u
#define SENTINEL_SPEAKERS SPEAKERS_4POINT1

/* The cases are about "no core": in a process that started one, skip them
 * rather than fail. */
static void skip_with_core(void)
{
	if (obs_initialized())
		skip();
}

static void test_video_info_no_core(void **state)
{
	(void)state;
	skip_with_core();
	struct obs_video_info ovi;
	memset(&ovi, 0x5a, sizeof(ovi));
	struct obs_video_info before = ovi;

	assert_false(obs_get_video_info(&ovi));
	assert_memory_equal(&ovi, &before, sizeof(ovi));
}

static void test_sdr_white_level_no_core(void **state)
{
	(void)state;
	skip_with_core();
	assert_true(obs_get_video_sdr_white_level() == 300.f);
}

static void test_hdr_nominal_peak_level_no_core(void **state)
{
	(void)state;
	skip_with_core();
	assert_true(obs_get_video_hdr_nominal_peak_level() == 1000.f);
}

static void test_audio_info_no_core(void **state)
{
	(void)state;
	skip_with_core();
	struct obs_audio_info oai = {
		.samples_per_sec = SENTINEL_RATE,
		.speakers = SENTINEL_SPEAKERS,
	};

	assert_false(obs_get_audio_info(&oai));
	assert_int_equal(oai.samples_per_sec, SENTINEL_RATE);
	assert_int_equal(oai.speakers, SENTINEL_SPEAKERS);

	assert_false(obs_get_audio_info(NULL));
}

static void test_audio_info2_no_core(void **state)
{
	(void)state;
	skip_with_core();
	struct obs_audio_info2 oai2 = {
		.samples_per_sec = SENTINEL_RATE,
		.speakers = SENTINEL_SPEAKERS,
		.fixed_buffering = true,
		.max_buffering_ms = 77,
	};

	assert_false(obs_get_audio_info2(&oai2));
	assert_int_equal(oai2.samples_per_sec, SENTINEL_RATE);
	assert_int_equal(oai2.speakers, SENTINEL_SPEAKERS);
	assert_true(oai2.fixed_buffering);
	assert_int_equal(oai2.max_buffering_ms, 77);
}

static void test_nv12_tex_active_no_core(void **state)
{
	(void)state;
	skip_with_core();
	assert_false(obs_nv12_tex_active());
}

static void test_p010_tex_active_no_core(void **state)
{
	(void)state;
	skip_with_core();
	assert_false(obs_p010_tex_active());
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_video_info_no_core),
		cmocka_unit_test(test_sdr_white_level_no_core),
		cmocka_unit_test(test_hdr_nominal_peak_level_no_core),
		cmocka_unit_test(test_audio_info_no_core),
		cmocka_unit_test(test_audio_info2_no_core),
		cmocka_unit_test(test_nv12_tex_active_no_core),
		cmocka_unit_test(test_p010_tex_active_no_core),
	};

	return cmocka_run_group_tests_name("test_no_core_getters", tests, NULL, NULL);
}
