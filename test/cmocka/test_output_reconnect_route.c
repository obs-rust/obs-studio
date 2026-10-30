/* The reconnect route of obs_output_signal_stop() through real libobs: an
 * output registered here, started with obs_output_start(), reports a stop
 * the way a plugin does. test_output_reconnect_steps compiles
 * obs-output-reconnect.c.h into itself, so it never runs the policy that
 * libobs (or libobs-with-rust) ships, nor the C that keeps the callback, the
 * reconnect thread and the stop event around it; this test does.
 *
 * It pins:
 * - a disconnect with reconnecting on asks the reconnect callback, and a
 *   refusal stops the output with the disconnect's code;
 * - with a retry count of 0 the callback is not asked and a disconnect
 *   stops the output;
 * - with reconnecting on, a code other than a disconnect (an error, or a
 *   clean stop) still asks the callback and stops the output;
 * - an agreed disconnect starts reconnecting (the "reconnect" signal with
 *   the first wait, no "stop"), and a stop while the reconnect thread waits
 *   ends it before any attempt: the thread is joined, reconnecting is
 *   cleared and the output's own stop runs.
 *
 * Only counts the settings dialog offers are used (a negative one is an
 * intentional difference of the port, in its parity test). Every signal
 * here runs on the caller's thread. The one wait is for the reconnect
 * thread to mark itself active (reconnect_thread_active, read through
 * obs-internal.h): a stop joins the thread only once it has, so stopping
 * earlier would leave it running against the output.
 *
 * obs_startup() needs a display on Linux, so there the cases skip without
 * one (run under xvfb-run). No video, audio, graphics or network is
 * needed. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <obs-internal.h>
#include <util/platform.h>

#define OUTPUT_ID "test_output_reconnect_route_output"
#define LONG_DELAY_SEC 60
#define WAIT_MS 10000

/* What the output and its signals saw. Every one of them runs on the test's
 * own thread (obs_output_signal_stop() and obs_output_stop() call them
 * inline, and a stop joins the reconnect thread first). */
static struct {
	long start_calls;
	long stop_calls;
	long stop_signals;
	long long stop_code;
	long reconnect_signals;
	long long reconnect_timeout_sec;
	long cb_calls;
	int cb_code;
	bool cb_answer;
} rec;

static bool obs_up;
static obs_output_t *output;

static const char *output_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "output reconnect route test";
}

static void *output_create(obs_data_t *settings, obs_output_t *out)
{
	UNUSED_PARAMETER(settings);
	return out;
}

static void output_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

/* Connects nothing: the test reports the outcome with
 * obs_output_signal_stop(), as a plugin's connect thread does. */
static bool output_start(void *data)
{
	UNUSED_PARAMETER(data);
	rec.start_calls++;
	return true;
}

/* Ends the capture as a clean stop, as a plugin's stop does. */
static void output_stop(void *data, uint64_t ts)
{
	UNUSED_PARAMETER(ts);
	rec.stop_calls++;
	obs_output_signal_stop(data, OBS_OUTPUT_SUCCESS);
}

static void output_raw_audio(void *data, struct audio_data *frames)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(frames);
}

static void on_stop(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	rec.stop_signals++;
	rec.stop_code = calldata_int(cd, "code");
}

static void on_reconnect(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	rec.reconnect_signals++;
	rec.reconnect_timeout_sec = calldata_int(cd, "timeout_sec");
}

static bool reconnect_cb(void *param, obs_output_t *out, int code)
{
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(out);
	rec.cb_calls++;
	rec.cb_code = code;
	return rec.cb_answer;
}

/* Keep warnings and errors only. */
static void log_handler(int lvl, const char *msg, va_list args, void *p)
{
	UNUSED_PARAMETER(p);
	if (lvl > LOG_WARNING)
		return;
	vfprintf(stderr, msg, args);
	fprintf(stderr, "\n");
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	base_set_log_handler(log_handler, NULL);
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "obs_startup failed (no display?): cases skip\n");
		return 0;
	}

	struct obs_output_info info = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_AUDIO,
		.get_name = output_name,
		.create = output_create,
		.destroy = output_destroy,
		.start = output_start,
		.stop = output_stop,
		.raw_audio = output_raw_audio,
	};
	obs_register_output(&info);
	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up)
		obs_shutdown();
	return 0;
}

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		return 0;

	memset(&rec, 0, sizeof(rec));
	rec.stop_code = -1000;
	rec.cb_code = -1000;
	output = obs_output_create(OUTPUT_ID, "reconnect route test output", NULL, NULL);
	assert_non_null(output);

	signal_handler_t *sh = obs_output_get_signal_handler(output);
	signal_handler_connect(sh, "stop", on_stop, NULL);
	signal_handler_connect(sh, "reconnect", on_reconnect, NULL);
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		return 0;
	if (obs_output_active(output))
		obs_output_force_stop(output);
	obs_output_release(output);
	output = NULL;
	return 0;
}

/* Starts the output with retry_count retries LONG_DELAY_SEC apart, with
 * the reconnect callback answering answer when with_cb. */
static void start(int retry_count, bool with_cb, bool answer)
{
	obs_output_set_reconnect_settings(output, retry_count, LONG_DELAY_SEC);
	if (with_cb)
		obs_output_set_reconnect_callback(output, reconnect_cb, NULL);
	rec.cb_answer = answer;
	assert_true(obs_output_start(output));
	assert_int_equal(rec.start_calls, 1);
}

/* Waits until the reconnect thread runs, so that a stop joins it. */
static void wait_reconnect_thread(void)
{
	for (int ms = 0; ms < WAIT_MS; ms++) {
		if (output->reconnect_thread_active)
			return;
		os_sleep_ms(1);
	}
	fprintf(stderr, "the reconnect thread did not start in %d ms\n", WAIT_MS);
	fail();
}

static void assert_stopped_with(int code)
{
	assert_false(obs_output_reconnecting(output));
	assert_int_equal(rec.reconnect_signals, 0);
	assert_int_equal(rec.stop_signals, 1);
	assert_int_equal(rec.stop_code, code);
	assert_int_equal(rec.start_calls, 1);
}

static void callback_refusing_a_disconnect_stops(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	start(20, true, false);

	obs_output_signal_stop(output, OBS_OUTPUT_DISCONNECTED);

	assert_int_equal(rec.cb_calls, 1);
	assert_int_equal(rec.cb_code, OBS_OUTPUT_DISCONNECTED);
	assert_stopped_with(OBS_OUTPUT_DISCONNECTED);
}

static void no_callback_with_reconnecting_off(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	start(0, true, true);

	obs_output_signal_stop(output, OBS_OUTPUT_DISCONNECTED);

	assert_int_equal(rec.cb_calls, 0);
	assert_stopped_with(OBS_OUTPUT_DISCONNECTED);
}

static void an_error_asks_the_callback_and_stops(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	start(20, true, true);

	obs_output_signal_stop(output, OBS_OUTPUT_CONNECT_FAILED);

	assert_int_equal(rec.cb_calls, 1);
	assert_int_equal(rec.cb_code, OBS_OUTPUT_CONNECT_FAILED);
	assert_stopped_with(OBS_OUTPUT_CONNECT_FAILED);
}

static void a_clean_stop_asks_the_callback_and_stops(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	start(20, true, true);

	obs_output_signal_stop(output, OBS_OUTPUT_SUCCESS);

	/* Characterized, not endorsed: asked even for a clean stop. */
	assert_int_equal(rec.cb_calls, 1);
	assert_int_equal(rec.cb_code, OBS_OUTPUT_SUCCESS);
	assert_stopped_with(OBS_OUTPUT_SUCCESS);
}

static void a_stop_while_reconnecting_ends_it(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	start(20, true, true);

	obs_output_signal_stop(output, OBS_OUTPUT_DISCONNECTED);

	assert_int_equal(rec.cb_calls, 1);
	assert_true(obs_output_reconnecting(output));
	assert_true(obs_output_active(output));
	assert_int_equal(rec.reconnect_signals, 1);
	assert_int_equal(rec.reconnect_timeout_sec, LONG_DELAY_SEC);
	assert_int_equal(rec.stop_signals, 0);

	/* The reconnect thread waits its 60 s: once it runs, the stop wakes
	 * and joins it, then the output's stop ends the capture as a clean
	 * stop. */
	wait_reconnect_thread();
	obs_output_stop(output);
	assert_false(output->reconnect_thread_active);

	assert_false(obs_output_reconnecting(output));
	assert_false(obs_output_active(output));
	assert_int_equal(rec.start_calls, 1);
	assert_int_equal(rec.stop_calls, 1);
	assert_int_equal(rec.stop_signals, 1);
	assert_int_equal(rec.stop_code, OBS_OUTPUT_SUCCESS);
	assert_int_equal(rec.reconnect_signals, 1);
	/* The output's clean stop asks the callback again, with its code. */
	assert_int_equal(rec.cb_calls, 2);
	assert_int_equal(rec.cb_code, OBS_OUTPUT_SUCCESS);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(callback_refusing_a_disconnect_stops, setup, teardown),
		cmocka_unit_test_setup_teardown(no_callback_with_reconnecting_off, setup, teardown),
		cmocka_unit_test_setup_teardown(an_error_asks_the_callback_and_stops, setup, teardown),
		cmocka_unit_test_setup_teardown(a_clean_stop_asks_the_callback_and_stops, setup, teardown),
		cmocka_unit_test_setup_teardown(a_stop_while_reconnecting_ends_it, setup, teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
