#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <stdio.h>
#include <errno.h>
#include <cmocka.h>

#include <pipewire/core.h>
#include <util/c99defs.h>

#include "reconnect-policy.h"

/* obs-rust/obs-studio#108: a PipeWire restart must be noticed, and the
 * screencast retried, without reacting to errors that would only repeat. */

static void core_epipe_is_lost_once(void **state)
{
	UNUSED_PARAMETER(state);
	bool disconnected = false;

	assert_true(obs_pipewire_core_error_is_lost(&disconnected, PW_ID_CORE, -EPIPE));
	assert_true(disconnected);
	assert_false(obs_pipewire_core_error_is_lost(&disconnected, PW_ID_CORE, -EPIPE));
}

static void other_core_errors_are_not_lost(void **state)
{
	UNUSED_PARAMETER(state);
	bool disconnected = false;

	assert_false(obs_pipewire_core_error_is_lost(&disconnected, PW_ID_CORE, -ENOENT));
	assert_false(obs_pipewire_core_error_is_lost(&disconnected, 42, -EPIPE));
	assert_false(disconnected);
}

static void stream_error_after_streaming_is_lost(void **state)
{
	UNUSED_PARAMETER(state);
	bool disconnected = false;
	bool streamed = false;

	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, false, PW_STREAM_STATE_CONNECTING));
	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, false, PW_STREAM_STATE_PAUSED));
	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, false, PW_STREAM_STATE_STREAMING));
	assert_true(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, false, PW_STREAM_STATE_ERROR));
	assert_true(disconnected);
}

static void stream_error_before_streaming_is_not_lost(void **state)
{
	UNUSED_PARAMETER(state);
	bool disconnected = false;
	bool streamed = false;

	/* No matching format or no target node: retrying would fail again. */
	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, false, PW_STREAM_STATE_CONNECTING));
	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, false, PW_STREAM_STATE_ERROR));
	assert_false(disconnected);
}

static void lost_connection_is_reported_once(void **state)
{
	UNUSED_PARAMETER(state);
	bool disconnected = false;
	bool streamed = false;

	assert_true(obs_pipewire_core_error_is_lost(&disconnected, PW_ID_CORE, -EPIPE));
	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, false, PW_STREAM_STATE_STREAMING));
	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, false, PW_STREAM_STATE_ERROR));
}

/* obsproject/obs-studio#12063: when a monitor is switched off or unplugged,
 * the compositor ends its screencast. The stream's node goes away and the
 * stream only pauses, with no error. */
static void target_node_removed_is_lost_once(void **state)
{
	UNUSED_PARAMETER(state);
	bool disconnected = false;

	assert_true(obs_pipewire_node_removed_is_lost(&disconnected, 42, 42));
	assert_true(disconnected);
	assert_false(obs_pipewire_node_removed_is_lost(&disconnected, 42, 42));
}

static void other_removed_globals_are_not_lost(void **state)
{
	UNUSED_PARAMETER(state);
	bool disconnected = false;

	/* Ports, links and other nodes come and go while streaming. */
	assert_false(obs_pipewire_node_removed_is_lost(&disconnected, 42, 43));
	/* No stream connected to a node yet. */
	assert_false(obs_pipewire_node_removed_is_lost(&disconnected, PW_ID_ANY, PW_ID_ANY));
	assert_false(obs_pipewire_node_removed_is_lost(&disconnected, PW_ID_ANY, 42));
	assert_false(disconnected);
}

static void node_removed_after_lost_connection_is_not_reported(void **state)
{
	UNUSED_PARAMETER(state);
	bool disconnected = false;

	assert_true(obs_pipewire_core_error_is_lost(&disconnected, PW_ID_CORE, -EPIPE));
	assert_false(obs_pipewire_node_removed_is_lost(&disconnected, 42, 42));
}

static void reconnect_backs_off_quickly_first(void **state)
{
	UNUSED_PARAMETER(state);
	const uint32_t expected[] = {500, 1000, 2000, 4000, 8000, 10000, 10000, 10000, 10000};
	uint32_t attempts = 0;

	for (size_t i = 0; i < sizeof(expected) / sizeof(expected[0]); i++)
		assert_int_equal(obs_pipewire_reconnect_next_delay(&attempts), expected[i]);
	assert_int_equal(attempts, OBS_PIPEWIRE_RECONNECT_FAST_ATTEMPTS - 1);
}

/* obsproject/obs-studio#12063: a monitor that is off for longer than the fast
 * backoff (about a minute) must still come back. */
static void reconnect_never_gives_up(void **state)
{
	UNUSED_PARAMETER(state);
	uint32_t attempts = 0;
	uint64_t waited_ms = 0;

	for (uint32_t i = 1; i < OBS_PIPEWIRE_RECONNECT_FAST_ATTEMPTS; i++)
		waited_ms += obs_pipewire_reconnect_next_delay(&attempts);

	/* A day of failed attempts: every one schedules another, at a slow,
	 * steady pace. */
	while (waited_ms < 24ull * 60 * 60 * 1000) {
		uint32_t delay_ms = obs_pipewire_reconnect_next_delay(&attempts);

		if (delay_ms != OBS_PIPEWIRE_RECONNECT_SLOW_DELAY_MS) {
			fprintf(stderr, "attempt %u after %llu ms: delay %u ms, expected %u\n", attempts,
				(unsigned long long)waited_ms, delay_ms, OBS_PIPEWIRE_RECONNECT_SLOW_DELAY_MS);
			fail();
		}
		waited_ms += delay_ms;
	}
}

/* obs-rust/obs-studio#257: while a window animates, the compositor can make
 * its screencast 0 pixels wide for a moment. The stream fails ("no more input
 * formats"); the stream that replaces it fails the same way during
 * negotiation, before it ever streams. That must count as a failed attempt,
 * or the reconnection stays active with nothing scheduled: the capture stays
 * frozen. */
static void stream_error_while_reconnecting_is_lost(void **state)
{
	UNUSED_PARAMETER(state);
	bool disconnected = false;
	bool streamed = false;

	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, true, PW_STREAM_STATE_CONNECTING));
	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, true, PW_STREAM_STATE_PAUSED));
	if (!obs_pipewire_stream_state_is_lost(&disconnected, &streamed, true, PW_STREAM_STATE_ERROR)) {
		fprintf(stderr, "a replacement stream that failed before streaming was not reported\n");
		fail();
	}
	assert_true(disconnected);
	/* Reported once per connection. */
	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, true, PW_STREAM_STATE_ERROR));
}

/* Each failed replacement stream schedules another attempt, backing off like
 * any failed attempt: quickly first, then every 30 s, never giving up. */
static void failing_replacement_streams_back_off(void **state)
{
	UNUSED_PARAMETER(state);
	const uint32_t expected[] = {500,   1000,  2000,  4000,  8000,  10000, 10000,
				     10000, 10000, 30000, 30000, 30000, 30000};
	uint32_t attempts = 0;

	for (size_t i = 0; i < sizeof(expected) / sizeof(expected[0]); i++) {
		/* Every attempt is a new connection with a new stream. */
		bool disconnected = false;
		bool streamed = false;
		uint32_t delay_ms;

		obs_pipewire_stream_state_is_lost(&disconnected, &streamed, true, PW_STREAM_STATE_CONNECTING);
		if (!obs_pipewire_stream_state_is_lost(&disconnected, &streamed, true, PW_STREAM_STATE_ERROR)) {
			fprintf(stderr, "failed attempt %zu was not reported: nothing retries it\n", i + 1);
			fail();
		}
		assert_int_equal(obs_pipewire_reconnect_attempt_failed(true, false), OBS_PIPEWIRE_RECONNECT_RETRY);
		delay_ms = obs_pipewire_reconnect_next_delay(&attempts);
		if (delay_ms != expected[i]) {
			fprintf(stderr, "attempt %zu: delay %u ms, expected %u\n", i + 1, delay_ms, expected[i]);
			fail();
		}
	}
}

/* A user who cancels the portal's dialog ends the retries; failures outside a
 * reconnection start none. */
static void cancel_ends_reconnect(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(obs_pipewire_reconnect_attempt_failed(true, true), OBS_PIPEWIRE_RECONNECT_STOP);
	assert_int_equal(obs_pipewire_reconnect_attempt_failed(true, false), OBS_PIPEWIRE_RECONNECT_RETRY);
	assert_int_equal(obs_pipewire_reconnect_attempt_failed(false, false), OBS_PIPEWIRE_RECONNECT_IGNORE);
	assert_int_equal(obs_pipewire_reconnect_attempt_failed(false, true), OBS_PIPEWIRE_RECONNECT_IGNORE);
}

/* A replacement stream that never starts streaming (and never reports an
 * error) is a failed attempt too, unless its source is hidden: hiding pauses
 * the stream on purpose. */
static void replacement_stream_that_never_streams_fails(void **state)
{
	UNUSED_PARAMETER(state);

	if (obs_pipewire_reconnect_check_stream(false, true) != OBS_PIPEWIRE_RECONNECT_STREAM_FAILED) {
		fprintf(stderr, "a replacement stream that never streamed was not counted as a failed attempt\n");
		fail();
	}
	assert_int_equal(obs_pipewire_reconnect_check_stream(false, false), OBS_PIPEWIRE_RECONNECT_STREAM_WAIT);
	assert_int_equal(obs_pipewire_reconnect_check_stream(true, true), OBS_PIPEWIRE_RECONNECT_STREAM_STARTED);
	assert_int_equal(obs_pipewire_reconnect_check_stream(true, false), OBS_PIPEWIRE_RECONNECT_STREAM_STARTED);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(core_epipe_is_lost_once),
		cmocka_unit_test(other_core_errors_are_not_lost),
		cmocka_unit_test(stream_error_after_streaming_is_lost),
		cmocka_unit_test(stream_error_before_streaming_is_not_lost),
		cmocka_unit_test(lost_connection_is_reported_once),
		cmocka_unit_test(target_node_removed_is_lost_once),
		cmocka_unit_test(other_removed_globals_are_not_lost),
		cmocka_unit_test(node_removed_after_lost_connection_is_not_reported),
		cmocka_unit_test(reconnect_backs_off_quickly_first),
		cmocka_unit_test(reconnect_never_gives_up),
		cmocka_unit_test(stream_error_while_reconnecting_is_lost),
		cmocka_unit_test(failing_replacement_streams_back_off),
		cmocka_unit_test(cancel_ends_reconnect),
		cmocka_unit_test(replacement_stream_that_never_streams_fails),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
