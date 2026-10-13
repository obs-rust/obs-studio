/* Regression test for upstream obsproject/obs-studio#13283 (reported by
 * Dustinhoefer): two StartStream requests sent over obs-websocket in quick
 * succession crash OBS.
 *
 * A network output (rtmp_output) returns from start() right away and connects
 * on a thread of its own; the output only becomes active once the connection
 * is up. Until then obs_output_active() is false, so a second
 * obs_output_start() went through and ran the output's start() again on the
 * same data while the first connection attempt was still using it: rtmp_output
 * started a second connect thread on the same stream, and the two threads
 * corrupted each other's state (the crash in the report).
 *
 * obs_output_start() must refuse to start an output whose start is still in
 * progress, and must start it normally again once that start has failed or the
 * output has been stopped.
 *
 * The test registers an output that behaves like rtmp_output while it
 * connects: start() returns true without beginning data capture, and the
 * test decides when the "connection" fails. No video, audio or encoders are
 * needed. obs_startup() needs a display on Linux, so there the cases skip
 * without one (run under xvfb-run). */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#define OUTPUT_ID "test_connecting_output"

static bool obs_up;
static volatile long start_calls;
static volatile long stop_calls;

struct connecting_output {
	obs_output_t *output;
};

static const char *output_name(void *type_data)
{
	(void)type_data;
	return "connecting output";
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	struct connecting_output *data = bzalloc(sizeof(*data));
	data->output = output;
	return data;
}

static void output_destroy(void *data)
{
	bfree(data);
}

/* Like rtmp_output: the connection is made later, on another thread, so
 * start() returns true without beginning data capture. */
static bool output_start(void *data)
{
	(void)data;
	os_atomic_inc_long(&start_calls);
	return true;
}

/* Like rtmp_output stopped before it connected: nothing was captured, so
 * end the (never begun) data capture, which reports the stop. */
static void output_stop(void *data, uint64_t ts)
{
	(void)ts;
	struct connecting_output *out = data;
	os_atomic_inc_long(&stop_calls);
	obs_output_end_data_capture(out->output);
}

static void output_encoded_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static void register_output(void)
{
	struct obs_output_info info = {
		.id = OUTPUT_ID,
		/* Encoded, like rtmp_output, so creating it needs no core video. */
		.flags = OBS_OUTPUT_ENCODED,
		.get_name = output_name,
		.create = output_create,
		.destroy = output_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_encoded_packet,
	};
	obs_register_output(&info);
}

/* Keep warnings and errors only. */
static void log_handler(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	if (lvl > LOG_WARNING)
		return;
	vfprintf(stderr, msg, args);
	fprintf(stderr, "\n");
}

static int group_setup(void **state)
{
	(void)state;
	base_set_log_handler(log_handler, NULL);
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "obs_startup failed (no display?): cases skip\n");
		return 0;
	}
	register_output();
	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

static int setup(void **state)
{
	if (!obs_up)
		return 0;

	os_atomic_set_long(&start_calls, 0);
	os_atomic_set_long(&stop_calls, 0);

	obs_output_t *output = obs_output_create(OUTPUT_ID, "connecting", NULL, NULL);
	assert_non_null(output);
	*state = output;
	return 0;
}

static int teardown(void **state)
{
	obs_output_t *output = *state;
	if (!output)
		return 0;

	obs_output_force_stop(output);
	obs_output_release(output);
	*state = NULL;
	return 0;
}

/* The connection failing, as rtmp_output reports a refused connection. */
static void fail_connection(obs_output_t *output)
{
	obs_output_signal_stop(output, OBS_OUTPUT_CONNECT_FAILED);
}

/* The report: a second start while the first is still connecting must not run
 * the output's start() again. */
static void second_start_while_starting_is_refused(void **state)
{
	if (!obs_up)
		skip();
	obs_output_t *output = *state;

	assert_true(obs_output_start(output));
	assert_false(obs_output_active(output));

	assert_false(obs_output_start(output));
	assert_int_equal(os_atomic_load_long(&start_calls), 1);
}

/* Several quick starts (requests piling up) still start it once. */
static void many_starts_while_starting_start_once(void **state)
{
	if (!obs_up)
		skip();
	obs_output_t *output = *state;

	assert_true(obs_output_start(output));
	for (int i = 0; i < 5; i++)
		assert_false(obs_output_start(output));
	assert_int_equal(os_atomic_load_long(&start_calls), 1);
}

/* Guard: once the start has failed, the output starts again normally. */
static void start_after_failed_start_works(void **state)
{
	if (!obs_up)
		skip();
	obs_output_t *output = *state;

	assert_true(obs_output_start(output));
	fail_connection(output);

	assert_true(obs_output_start(output));
	assert_int_equal(os_atomic_load_long(&start_calls), 2);
}

/* Guard: once a starting output has been stopped, it starts again normally.
 * obs_output_stop() ignores an output that is not active yet; stopping a
 * stream that is still connecting takes obs_output_force_stop(). */
static void start_after_stop_works(void **state)
{
	if (!obs_up)
		skip();
	obs_output_t *output = *state;

	assert_true(obs_output_start(output));
	obs_output_force_stop(output);
	assert_int_equal(os_atomic_load_long(&stop_calls), 1);

	assert_true(obs_output_start(output));
	assert_int_equal(os_atomic_load_long(&start_calls), 2);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(second_start_while_starting_is_refused, setup, teardown),
		cmocka_unit_test_setup_teardown(many_starts_while_starting_start_once, setup, teardown),
		cmocka_unit_test_setup_teardown(start_after_failed_start_works, setup, teardown),
		cmocka_unit_test_setup_teardown(start_after_stop_works, setup, teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
