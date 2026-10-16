#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <util/bmem.h>
#include <util/c99defs.h>

#include "pulse-reconnect.h"

/* obs-rust/obs-studio#108: a restarted server must be reconnected to, with
 * backoff, and sources must survive the moment it has no default device. */

static void failure_without_a_server_is_not_retried(void **state)
{
	UNUSED_PARAMETER(state);
	struct pulse_reconnect_state reconnect = {0};
	uint64_t delay = 1;

	assert_false(pulse_reconnect_on_failed(&reconnect, &delay));
	assert_false(reconnect.reconnecting);
}

static void lost_server_is_retried_at_once(void **state)
{
	UNUSED_PARAMETER(state);
	struct pulse_reconnect_state reconnect = {0};
	uint64_t delay = 1;

	/* The first connection is not a reconnection. */
	assert_false(pulse_reconnect_on_ready(&reconnect));
	assert_true(pulse_reconnect_on_failed(&reconnect, &delay));
	assert_int_equal(delay, 0);
	assert_true(reconnect.reconnecting);
}

static void refusing_server_backs_off(void **state)
{
	UNUSED_PARAMETER(state);
	const uint64_t expected[] = {0, 500000, 1000000, 2000000, 4000000, 8000000, 10000000, 10000000};
	struct pulse_reconnect_state reconnect = {0};
	uint64_t delay;

	pulse_reconnect_on_ready(&reconnect);
	for (size_t i = 0; i < sizeof(expected) / sizeof(expected[0]); i++) {
		assert_true(pulse_reconnect_on_failed(&reconnect, &delay));
		assert_int_equal(delay, expected[i]);
	}
}

static void reconnection_notifies_sources_and_resets(void **state)
{
	UNUSED_PARAMETER(state);
	struct pulse_reconnect_state reconnect = {0};
	uint64_t delay;

	pulse_reconnect_on_ready(&reconnect);
	assert_true(pulse_reconnect_on_failed(&reconnect, &delay));
	assert_true(pulse_reconnect_on_failed(&reconnect, &delay));
	assert_true(pulse_reconnect_on_ready(&reconnect));
	assert_false(reconnect.reconnecting);

	assert_true(pulse_reconnect_on_failed(&reconnect, &delay));
	assert_int_equal(delay, 0);
}

static void default_devices(void **state)
{
	UNUSED_PARAMETER(state);
	pa_server_info info = {.default_source_name = "mic", .default_sink_name = "speakers"};
	char *device;

	device = pulse_default_device(&info, true);
	assert_string_equal(device, "mic");
	bfree(device);

	device = pulse_default_device(&info, false);
	assert_string_equal(device, "speakers.monitor");
	bfree(device);
}

static void no_default_device(void **state)
{
	UNUSED_PARAMETER(state);
	/* What a restarted server reports before its devices are back. */
	pa_server_info info = {0};

	assert_null(pulse_default_device(&info, true));
	assert_null(pulse_default_device(&info, false));
}

static void restart_is_retried_then_given_up(void **state)
{
	UNUSED_PARAMETER(state);
	uint32_t attempts = 0;

	for (int i = 1; i < PULSE_RESTART_MAX_ATTEMPTS; i++)
		assert_true(pulse_restart_should_retry(&attempts));
	assert_false(pulse_restart_should_retry(&attempts));
	assert_int_equal(attempts, 0);
}

/* obsproject/obs-studio#10072: a source's own stream can die while the
 * connection stays up (stream killed, device gone and back). */

#define DEVICE 7
#define OTHER_DEVICE 9

static const struct pulse_stream_status running = {.has_stream = true, .device_index = DEVICE};
static const struct pulse_stream_status failed = {.has_stream = true, .failed = true, .device_index = PA_INVALID_INDEX};
static const struct pulse_stream_status stopped = {.device_index = PA_INVALID_INDEX};

static void failed_stream_is_restarted_once(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(pulse_stream_on_event(&failed, PULSE_STREAM_FAILED, PA_INVALID_INDEX), PULSE_STREAM_RESTART);
	/* Already replaced or stopped by the time the event is handled. */
	assert_int_equal(pulse_stream_on_event(&running, PULSE_STREAM_FAILED, PA_INVALID_INDEX), PULSE_STREAM_KEEP);
	assert_int_equal(pulse_stream_on_event(&stopped, PULSE_STREAM_FAILED, PA_INVALID_INDEX), PULSE_STREAM_KEEP);
}

static void removed_device_stops_its_stream_only(void **state)
{
	UNUSED_PARAMETER(state);
	struct pulse_stream_status follows_default = running;
	follows_default.follows_default = true;

	assert_int_equal(pulse_stream_on_event(&running, PULSE_SOURCE_REMOVED, DEVICE), PULSE_STREAM_STOP);
	assert_int_equal(pulse_stream_on_event(&running, PULSE_SOURCE_REMOVED, OTHER_DEVICE), PULSE_STREAM_KEEP);
	/* The server moves a stream on the default device elsewhere. */
	assert_int_equal(pulse_stream_on_event(&follows_default, PULSE_SOURCE_REMOVED, DEVICE), PULSE_STREAM_KEEP);
	assert_int_equal(pulse_stream_on_event(&stopped, PULSE_SOURCE_REMOVED, DEVICE), PULSE_STREAM_KEEP);
	assert_int_equal(pulse_stream_on_event(&failed, PULSE_SOURCE_REMOVED, DEVICE), PULSE_STREAM_KEEP);
}

static void new_device_restarts_only_a_dead_stream(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(pulse_stream_on_event(&stopped, PULSE_SOURCE_NEW, OTHER_DEVICE), PULSE_STREAM_RESTART);
	assert_int_equal(pulse_stream_on_event(&failed, PULSE_SOURCE_NEW, OTHER_DEVICE), PULSE_STREAM_RESTART);
	assert_int_equal(pulse_stream_on_event(&running, PULSE_SOURCE_NEW, OTHER_DEVICE), PULSE_STREAM_KEEP);
}

/* Follow-up of #378: a restart that fails while the device is there is
 * retried, and a stream the server keeps failing is restarted with a pause
 * and given up, sharing one counter. */

static void failed_start_is_retried_while_the_device_is_there(void **state)
{
	UNUSED_PARAMETER(state);
	uint32_t attempts = 0;

	assert_true(pulse_start_should_retry(PULSE_START_FAILED, false, &attempts));
	assert_int_equal(attempts, 1);
	assert_true(pulse_start_should_retry(PULSE_START_FAILED, true, &attempts));
	assert_int_equal(attempts, 2);

	/* Started: nothing to retry, the counter is left to the audio. */
	assert_false(pulse_start_should_retry(PULSE_START_OK, false, &attempts));
	assert_false(pulse_start_should_retry(PULSE_START_OK, true, &attempts));
	assert_int_equal(attempts, 2);
}

static void missing_device_is_waited_for_unless_the_server_came_back(void **state)
{
	UNUSED_PARAMETER(state);
	uint32_t attempts = 0;

	/* A device that is gone for good is not polled ... */
	assert_false(pulse_start_should_retry(PULSE_START_NO_DEVICE, false, &attempts));
	/* ... and a lost server is left to the reconnection. */
	assert_false(pulse_start_should_retry(PULSE_START_NO_SERVER, false, &attempts));
	assert_int_equal(attempts, 0);

	/* After a reconnect the devices come back over the next moments. */
	assert_true(pulse_start_should_retry(PULSE_START_NO_DEVICE, true, &attempts));
	assert_true(pulse_start_should_retry(PULSE_START_NO_SERVER, true, &attempts));
	assert_int_equal(attempts, 2);
}

static void failing_stream_is_restarted_at_once_then_paced(void **state)
{
	UNUSED_PARAMETER(state);
	uint32_t attempts = 0;
	uint64_t delay = 1;

	assert_true(pulse_stream_failed_should_restart(&attempts, &delay));
	assert_int_equal(delay, 0);
	for (int i = 2; i < PULSE_RESTART_MAX_ATTEMPTS; i++) {
		assert_true(pulse_stream_failed_should_restart(&attempts, &delay));
		assert_int_equal(delay, PULSE_RESTART_RETRY_USEC);
	}
	assert_false(pulse_stream_failed_should_restart(&attempts, &delay));

	/* Given up: the next failure (after a device appeared) starts over. */
	assert_true(pulse_stream_failed_should_restart(&attempts, &delay));
	assert_int_equal(delay, 0);
}

static void stream_and_start_failures_share_the_limit(void **state)
{
	UNUSED_PARAMETER(state);
	uint32_t attempts = 0;
	uint64_t delay;
	int restarts = 0;

	/* The server fails the stream, then creating it fails, in turn. */
	for (;;) {
		if (!pulse_stream_failed_should_restart(&attempts, &delay))
			break;
		restarts++;
		if (!pulse_start_should_retry(PULSE_START_FAILED, false, &attempts))
			break;
		restarts++;
		assert_true(restarts < 2 * PULSE_RESTART_MAX_ATTEMPTS);
	}
	assert_int_equal(restarts, PULSE_RESTART_MAX_ATTEMPTS - 1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(failure_without_a_server_is_not_retried),
		cmocka_unit_test(lost_server_is_retried_at_once),
		cmocka_unit_test(refusing_server_backs_off),
		cmocka_unit_test(reconnection_notifies_sources_and_resets),
		cmocka_unit_test(default_devices),
		cmocka_unit_test(no_default_device),
		cmocka_unit_test(restart_is_retried_then_given_up),
		cmocka_unit_test(failed_stream_is_restarted_once),
		cmocka_unit_test(removed_device_stops_its_stream_only),
		cmocka_unit_test(new_device_restarts_only_a_dead_stream),
		cmocka_unit_test(failed_start_is_retried_while_the_device_is_there),
		cmocka_unit_test(missing_device_is_waited_for_unless_the_server_came_back),
		cmocka_unit_test(failing_stream_is_restarted_at_once_then_paced),
		cmocka_unit_test(stream_and_start_failures_share_the_limit),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
