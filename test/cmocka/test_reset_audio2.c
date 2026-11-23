/* C-only characterization before extracting obs_reset_audio2 (Refs #5484).
 *
 * test_core_setup_rules_steps already pins the refusal with no core before
 * obs_startup(), a NULL info on a live core and the buffering ticks;
 * test_get_audio pins the refusal on an active output through
 * obs_reset_audio. This test pins the rest of the public behavior:
 *
 * - obs_get_audio_info2 and obs_get_audio_info read back the rate, the
 *   speaker layout and the buffering type given, and the maximum in whole
 *   milliseconds of whole ticks; a second reset replaces the first;
 * - a NULL info closes the audio until the next reset, logging nothing;
 * - each successful reset logs one "audio settings reset" info message
 *   naming the rate, the layout, the maximum and the buffering type;
 * - on an active output it returns false, logs nothing and leaves the
 *   output and its settings as they were, also for a NULL info;
 * - with no core (before obs_startup(), after obs_shutdown()) it returns
 *   false and logs nothing, also for a NULL info.
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

/* The "audio settings reset" messages logged while capturing. Other threads
 * may log while a core runs, so the others are only passed on. */
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

	struct obs_audio_info legacy;
	memset(&legacy, 0xA5, sizeof(legacy));
	assert_true(obs_get_audio_info(&legacy));
	assert_int_equal(legacy.samples_per_sec, rate);
	assert_int_equal(legacy.speakers, speakers);
}

/* ------------------------------------------------------------------------ */
/* Cases                                                                     */

struct reset_case {
	struct obs_audio_info2 info;
	uint32_t max_ms; /* read back: whole ticks of 1024 frames, cut to ms */
};

/* The maximum is rounded up to whole ticks, then read back in whole
 * milliseconds; 0 is the default of 45 ticks. */
static const struct reset_case cases[] = {
	/* 9600 frames: 10 ticks, 10240000 / 48000 = 213.3 */
	{{48000, SPEAKERS_STEREO, 200, true}, 213},
	/* 22050 frames: 22 ticks, 22528000 / 44100 = 510.8 */
	{{44100, SPEAKERS_5POINT1, 500, false}, 510},
	/* 96000 frames: 94 ticks, 96256000 / 96000 = 1002.7 */
	{{96000, SPEAKERS_7POINT1, 1000, true}, 1002},
	/* the default: 45 ticks, 46080000 / 32000 = 1440 */
	{{32000, SPEAKERS_MONO, 0, false}, 1440},
	/* the default with fixed buffering: 46080000 / 22050 = 2089.8 */
	{{22050, SPEAKERS_4POINT1, 0, true}, 2089},
};

#define CASE_COUNT (sizeof(cases) / sizeof(cases[0]))

/* Every field given is what the getters read back, and each reset replaces
 * the one before it, in either buffering type. */
static void reads_back_each_reset(void **state)
{
	(void)state;

	for (size_t i = 0; i < CASE_COUNT; i++) {
		assert_true(obs_reset_audio2(&cases[i].info));
		assert_non_null(obs_get_audio());
		expect_info2(cases[i].info.samples_per_sec, cases[i].info.speakers, cases[i].info.fixed_buffering,
			     cases[i].max_ms);
	}

	/* And back again, the other way round. */
	for (size_t i = CASE_COUNT; i-- > 0;) {
		assert_true(obs_reset_audio2(&cases[i].info));
		expect_info2(cases[i].info.samples_per_sec, cases[i].info.speakers, cases[i].info.fixed_buffering,
			     cases[i].max_ms);
	}
}

/* A NULL info closes the audio and is a success, with no reset message:
 * nothing reads back until the next reset opens it with its own settings. */
static void null_info_closes_until_next_reset(void **state)
{
	(void)state;

	assert_true(obs_reset_audio2(&cases[0].info));
	assert_non_null(obs_get_audio());

	start_capture();
	assert_true(obs_reset_audio2(NULL));
	stop_capture();
	assert_int_equal(reset_messages, 0);
	assert_null(obs_get_audio());

	struct obs_audio_info2 info;
	memset(&info, 0, sizeof(info));
	assert_false(obs_get_audio_info2(&info));

	assert_true(obs_reset_audio2(&cases[1].info));
	expect_info2(44100, SPEAKERS_5POINT1, false, 510);
}

/* One info message per reset: the rate, the layout as a number, the maximum
 * and the buffering type. */
static void logs_one_reset_message(void **state)
{
	(void)state;

	start_capture();
	assert_true(obs_reset_audio2(&cases[0].info));
	stop_capture();

	assert_int_equal(reset_messages, 1);
	assert_int_equal(reset_levels[0], LOG_INFO);
	assert_string_equal(reset_texts[0], "audio settings reset:\n"
					    "\tsamples per sec: 48000\n"
					    "\tspeakers:        2\n"
					    "\tmax buffering:   213 milliseconds\n"
					    "\tbuffering type:  fixed");

	start_capture();
	assert_true(obs_reset_audio2(&cases[3].info));
	stop_capture();

	assert_int_equal(reset_messages, 1);
	assert_int_equal(reset_levels[0], LOG_INFO);
	assert_string_equal(reset_texts[0], "audio settings reset:\n"
					    "\tsamples per sec: 32000\n"
					    "\tspeakers:        1\n"
					    "\tmax buffering:   1440 milliseconds\n"
					    "\tbuffering type:  dynamically increasing");
}

static void sink(void *param, size_t mix, struct audio_data *data)
{
	(void)param;
	(void)mix;
	(void)data;
}

/* While a raw callback keeps the output active, a reset is refused, also
 * with a NULL info: the output and its settings stay, and nothing is
 * logged. Once the callback goes, the reset goes through. */
static void active_output_refuses_reset(void **state)
{
	(void)state;

	assert_true(obs_reset_audio2(&cases[0].info));
	audio_t *audio = obs_get_audio();
	assert_non_null(audio);
	obs_add_raw_audio_callback(0, NULL, sink, NULL);
	assert_true(audio_output_active(audio));

	start_capture();
	assert_false(obs_reset_audio2(&cases[1].info));
	assert_false(obs_reset_audio2(NULL));
	stop_capture();
	assert_int_equal(reset_messages, 0);
	assert_ptr_equal(obs_get_audio(), audio);
	assert_true(audio_output_active(audio));
	expect_info2(48000, SPEAKERS_STEREO, true, 213);

	obs_remove_raw_audio_callback(0, sink, NULL);
	assert_false(audio_output_active(audio));

	assert_true(obs_reset_audio2(&cases[1].info));
	expect_info2(44100, SPEAKERS_5POINT1, false, 510);
}

/* No core: refused before the core is read, with no reset message, before
 * the first obs_startup() and again after obs_shutdown(). */
static void no_core_returns_false(void **state)
{
	(void)state;

	assert_false(obs_initialized());
	start_capture();
	assert_false(obs_reset_audio2(&cases[0].info));
	assert_false(obs_reset_audio2(NULL));
	stop_capture();
	assert_int_equal(reset_messages, 0);

	assert_true(obs_startup("en-US", NULL, NULL));
	assert_true(obs_reset_audio2(&cases[0].info));
	obs_shutdown();

	assert_false(obs_initialized());
	start_capture();
	assert_false(obs_reset_audio2(&cases[0].info));
	assert_false(obs_reset_audio2(NULL));
	stop_capture();
	assert_int_equal(reset_messages, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(no_core_returns_false, shutdown_core),
		cmocka_unit_test_setup_teardown(reads_back_each_reset, startup, shutdown_core),
		cmocka_unit_test_setup_teardown(null_info_closes_until_next_reset, startup, shutdown_core),
		cmocka_unit_test_setup_teardown(logs_one_reset_message, startup, shutdown_core),
		cmocka_unit_test_setup_teardown(active_output_refuses_reset, startup, shutdown_core),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
