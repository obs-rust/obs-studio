/* C-only characterization before extracting obs_reset_audio (Refs #5479).
 *
 * obs_reset_audio is the legacy entry point: it takes only the sample rate
 * and the speaker layout, and calls obs_reset_audio2 with the rest zeroed.
 * test_get_audio already pins that the rate and the layout reach the audio
 * output, and that an active output refuses a reset. This test pins the rest
 * of the public behavior:
 *
 * - the fields it does not take fall back to the obs_reset_audio2 defaults:
 *   dynamic buffering and the default maximum, also after a reset that set
 *   fixed buffering and its own maximum; obs_get_audio_info2 reads back the
 *   same as after obs_reset_audio2 with those fields zeroed;
 * - each call logs one "audio settings reset" message naming the rate, the
 *   layout, the default maximum in milliseconds and dynamic buffering;
 * - without a core (before obs_startup(), after obs_shutdown()) it returns
 *   false.
 *
 * Not pinned: obs_reset_audio(NULL) reads through the NULL pointer.
 *
 * Audio only: no video, graphics module or display is needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/threading.h>

#define MAX_MESSAGES 4
#define MESSAGE_SIZE 512

/* ------------------------------------------------------------------------ */
/* Log capture                                                               */

/* The "audio settings reset" messages logged while capturing, with their
 * text and level. Other threads may log while a core runs, so the others are
 * only passed on. */
static log_handler_t prev_handler;
static void *prev_param;
static pthread_mutex_t capture_mutex = PTHREAD_MUTEX_INITIALIZER;
static int reset_messages;
static int reset_levels[MAX_MESSAGES];
static char reset_texts[MAX_MESSAGES][MESSAGE_SIZE];

static void capture_log(int lvl, const char *msg, va_list args, void *p)
{
	char text[MESSAGE_SIZE];
	va_list copy;

	va_copy(copy, args);
	vsnprintf(text, sizeof(text), msg, copy);
	va_end(copy);

	pthread_mutex_lock(&capture_mutex);
	if (strstr(text, "audio settings reset")) {
		if (reset_messages < MAX_MESSAGES) {
			reset_levels[reset_messages] = lvl;
			snprintf(reset_texts[reset_messages], sizeof(reset_texts[0]), "%s", text);
		}
		reset_messages++;
	}
	pthread_mutex_unlock(&capture_mutex);

	if (prev_handler)
		prev_handler(lvl, msg, args, p);
}

static void start_capture(void)
{
	pthread_mutex_lock(&capture_mutex);
	reset_messages = 0;
	memset(reset_levels, 0, sizeof(reset_levels));
	memset(reset_texts, 0, sizeof(reset_texts));
	pthread_mutex_unlock(&capture_mutex);
	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(capture_log, prev_param);
}

static void stop_capture(void)
{
	base_set_log_handler(prev_handler, prev_param);
}

/* ------------------------------------------------------------------------ */
/* Core lifecycle                                                            */

static int startup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	return 0;
}

static int shutdown_core(void **state)
{
	(void)state;
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

static void expect_info2(uint32_t rate, enum speaker_layout speakers, bool fixed, uint32_t max_ms)
{
	struct obs_audio_info2 info;
	memset(&info, 0xA5, sizeof(info));
	assert_true(obs_get_audio_info2(&info));
	assert_int_equal(info.samples_per_sec, rate);
	assert_int_equal(info.speakers, speakers);
	assert_int_equal(info.fixed_buffering, fixed);
	assert_int_equal(info.max_buffering_ms, max_ms);
}

/* ------------------------------------------------------------------------ */
/* Cases                                                                     */

/* A reset that set fixed buffering and its own maximum is fully replaced:
 * obs_reset_audio brings both back to the defaults (45 ticks of 1024 frames,
 * in whole milliseconds at the new rate). */
static void omitted_fields_fall_back_to_defaults(void **state)
{
	(void)state;

	const struct obs_audio_info2 fixed = {
		.samples_per_sec = 48000,
		.speakers = SPEAKERS_STEREO,
		.max_buffering_ms = 200,
		.fixed_buffering = true,
	};
	assert_true(obs_reset_audio2(&fixed));
	/* 200 ms at 48 kHz is 9.375 ticks, rounded up to 10: 213 ms. */
	expect_info2(48000, SPEAKERS_STEREO, true, 213);

	const struct obs_audio_info legacy = {.samples_per_sec = 44100, .speakers = SPEAKERS_5POINT1};
	assert_true(obs_reset_audio(&legacy));
	/* 45 * 1024 * 1000 / 44100 = 1044.89, cut to 1044. */
	expect_info2(44100, SPEAKERS_5POINT1, false, 1044);

	assert_true(obs_reset_audio2(&fixed));
	expect_info2(48000, SPEAKERS_STEREO, true, 213);

	const struct obs_audio_info legacy48 = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	assert_true(obs_reset_audio(&legacy48));
	expect_info2(48000, SPEAKERS_STEREO, false, 960);
}

/* Over a few rate/layout pairs, obs_reset_audio reads back exactly what
 * obs_reset_audio2 does with max_buffering_ms and fixed_buffering zeroed. */
static void same_as_reset_audio2_with_zeroed_extras(void **state)
{
	(void)state;

	static const struct obs_audio_info cases[] = {
		{.samples_per_sec = 8000, .speakers = SPEAKERS_MONO},
		{.samples_per_sec = 22050, .speakers = SPEAKERS_2POINT1},
		{.samples_per_sec = 44100, .speakers = SPEAKERS_4POINT0},
		{.samples_per_sec = 48000, .speakers = SPEAKERS_4POINT1},
		{.samples_per_sec = 96000, .speakers = SPEAKERS_7POINT1},
	};

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		const struct obs_audio_info2 zeroed = {
			.samples_per_sec = cases[i].samples_per_sec,
			.speakers = cases[i].speakers,
		};
		struct obs_audio_info2 via2;
		struct obs_audio_info2 via1;

		assert_true(obs_reset_audio2(&zeroed));
		memset(&via2, 0, sizeof(via2));
		assert_true(obs_get_audio_info2(&via2));

		assert_true(obs_reset_audio(&cases[i]));
		memset(&via1, 0, sizeof(via1));
		assert_true(obs_get_audio_info2(&via1));

		assert_int_equal(via1.samples_per_sec, cases[i].samples_per_sec);
		assert_int_equal(via1.speakers, cases[i].speakers);
		assert_false(via1.fixed_buffering);
		assert_int_equal(via1.samples_per_sec, via2.samples_per_sec);
		assert_int_equal(via1.speakers, via2.speakers);
		assert_int_equal(via1.fixed_buffering, via2.fixed_buffering);
		assert_int_equal(via1.max_buffering_ms, via2.max_buffering_ms);

		struct obs_audio_info legacy;
		memset(&legacy, 0, sizeof(legacy));
		assert_true(obs_get_audio_info(&legacy));
		assert_int_equal(legacy.samples_per_sec, cases[i].samples_per_sec);
		assert_int_equal(legacy.speakers, cases[i].speakers);
	}
}

/* Each call logs one info message: the rate, the layout as a number, the
 * default maximum and dynamic buffering. */
static void logs_one_reset_message(void **state)
{
	(void)state;

	const struct obs_audio_info info = {.samples_per_sec = 44100, .speakers = SPEAKERS_5POINT1};
	start_capture();
	assert_true(obs_reset_audio(&info));
	stop_capture();

	assert_int_equal(reset_messages, 1);
	assert_int_equal(reset_levels[0], LOG_INFO);
	assert_string_equal(reset_texts[0], "audio settings reset:\n"
					    "\tsamples per sec: 44100\n"
					    "\tspeakers:        6\n"
					    "\tmax buffering:   1044 milliseconds\n"
					    "\tbuffering type:  dynamically increasing");

	const struct obs_audio_info mono = {.samples_per_sec = 48000, .speakers = SPEAKERS_MONO};
	start_capture();
	assert_true(obs_reset_audio(&mono));
	stop_capture();

	assert_int_equal(reset_messages, 1);
	assert_int_equal(reset_levels[0], LOG_INFO);
	assert_string_equal(reset_texts[0], "audio settings reset:\n"
					    "\tsamples per sec: 48000\n"
					    "\tspeakers:        1\n"
					    "\tmax buffering:   960 milliseconds\n"
					    "\tbuffering type:  dynamically increasing");
}

/* No core: refused before the core is read, with no reset message, before
 * the first obs_startup() and again after obs_shutdown(). */
static void no_core_returns_false(void **state)
{
	(void)state;
	const struct obs_audio_info info = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};

	assert_false(obs_initialized());
	start_capture();
	assert_false(obs_reset_audio(&info));
	stop_capture();
	assert_int_equal(reset_messages, 0);

	assert_true(obs_startup("en-US", NULL, NULL));
	assert_true(obs_reset_audio(&info));
	obs_shutdown();

	assert_false(obs_initialized());
	start_capture();
	assert_false(obs_reset_audio(&info));
	stop_capture();
	assert_int_equal(reset_messages, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(omitted_fields_fall_back_to_defaults, startup, shutdown_core),
		cmocka_unit_test_setup_teardown(same_as_reset_audio2_with_zeroed_extras, startup, shutdown_core),
		cmocka_unit_test_setup_teardown(logs_one_reset_message, startup, shutdown_core),
		cmocka_unit_test_teardown(no_core_returns_false, shutdown_core),
	};
	return cmocka_run_group_tests_name("test_reset_audio", tests, NULL, NULL);
}
