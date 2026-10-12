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

	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, PW_STREAM_STATE_CONNECTING));
	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, PW_STREAM_STATE_PAUSED));
	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, PW_STREAM_STATE_STREAMING));
	assert_true(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, PW_STREAM_STATE_ERROR));
	assert_true(disconnected);
}

static void stream_error_before_streaming_is_not_lost(void **state)
{
	UNUSED_PARAMETER(state);
	bool disconnected = false;
	bool streamed = false;

	/* No matching format or no target node: retrying would fail again. */
	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, PW_STREAM_STATE_CONNECTING));
	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, PW_STREAM_STATE_ERROR));
	assert_false(disconnected);
}

static void lost_connection_is_reported_once(void **state)
{
	UNUSED_PARAMETER(state);
	bool disconnected = false;
	bool streamed = false;

	assert_true(obs_pipewire_core_error_is_lost(&disconnected, PW_ID_CORE, -EPIPE));
	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, PW_STREAM_STATE_STREAMING));
	assert_false(obs_pipewire_stream_state_is_lost(&disconnected, &streamed, PW_STREAM_STATE_ERROR));
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
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
