/* Test-only oracle: the original libobs/obs-output-delay.c.h, unmodified,
 * with its seven functions renamed oracle_* so they can link next to the
 * Rust shims, and os_gettime_ns() renamed to a clock the test sets, as
 * test_output_delay renames it.
 *
 * Everything else the file calls keeps its libobs name: the accessors
 * obs_output_delay_fields() and obs_output_delay_signal(), the
 * obs-output.c calls (can_begin, initialize_encoders, begin_data_capture,
 * actual_start, actual_stop) and obs_encoder_packet_create_instance() and
 * _release(). The cargo test that links this file defines them, recording
 * each call, and the Rust shim calls the same ones, so both sides are seen
 * through the same stubs. pthread, blog and the deque come from the real
 * headers and libraries. */
#include <stddef.h>
#include <stdint.h>

#define obs_output_set_delay oracle_obs_output_set_delay
#define obs_output_get_delay oracle_obs_output_get_delay
#define obs_output_get_active_delay oracle_obs_output_get_active_delay
#define process_delay oracle_process_delay
#define obs_output_delay_start oracle_obs_output_delay_start
#define obs_output_delay_stop oracle_obs_output_delay_stop
#define obs_output_cleanup_delay oracle_obs_output_cleanup_delay
#define os_gettime_ns oracle_output_delay_clock_ns

/* obs_output_set_delay() names itself with __FUNCTION__, a GCC, Clang and
 * MSVC extension that -Wpedantic rejects. The file is compiled unmodified. */
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wpedantic"
#endif
#include "obs-output-delay.c.h"
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic pop
#endif

/* The values and layouts the C uses, in the order of
 * rust/obs-c-oracle/src/output_delay.rs. */
enum {
	ORACLE_OUTPUT_DELAY_VALUE_COUNT = 11,
	ORACLE_OUTPUT_DELAY_LAYOUT_COUNT = 60,
};

int64_t oracle_output_delay_value(size_t which);
size_t oracle_output_delay_layout(size_t which);
void *oracle_output_delay_mutex_create(void);
void oracle_output_delay_mutex_destroy(void *mutex);
bool oracle_output_delay_mutex_held(void *mutex);

int64_t oracle_output_delay_value(size_t which)
{
	const int64_t values[ORACLE_OUTPUT_DELAY_VALUE_COUNT] = {
		DELAY_MSG_PACKET,  DELAY_MSG_START,           DELAY_MSG_STOP,        OBS_OUTPUT_ENCODED,
		OBS_OUTPUT_VIDEO,  OBS_OUTPUT_DELAY_PRESERVE, LOG_WARNING,           LOG_DEBUG,
		OBS_ENCODER_AUDIO, OBS_ENCODER_VIDEO,         (int64_t)sizeof(long),
	};
	return which < ORACLE_OUTPUT_DELAY_VALUE_COUNT ? values[which] : -1;
}

size_t oracle_output_delay_layout(size_t which)
{
	const size_t values[ORACLE_OUTPUT_DELAY_LAYOUT_COUNT] = {
		sizeof(struct encoder_packet),
		_Alignof(struct encoder_packet),
		offsetof(struct encoder_packet, data),
		offsetof(struct encoder_packet, size),
		offsetof(struct encoder_packet, pts),
		offsetof(struct encoder_packet, dts),
		offsetof(struct encoder_packet, timebase_num),
		offsetof(struct encoder_packet, timebase_den),
		offsetof(struct encoder_packet, type),
		offsetof(struct encoder_packet, keyframe),
		offsetof(struct encoder_packet, dts_usec),
		offsetof(struct encoder_packet, sys_dts_usec),
		offsetof(struct encoder_packet, priority),
		offsetof(struct encoder_packet, drop_priority),
		offsetof(struct encoder_packet, track_idx),
		offsetof(struct encoder_packet, encoder),
		sizeof(struct encoder_packet_time),
		_Alignof(struct encoder_packet_time),
		offsetof(struct encoder_packet_time, pts),
		offsetof(struct encoder_packet_time, cts),
		offsetof(struct encoder_packet_time, fer),
		offsetof(struct encoder_packet_time, ferc),
		offsetof(struct encoder_packet_time, pir),
		sizeof(struct delay_data),
		_Alignof(struct delay_data),
		offsetof(struct delay_data, msg),
		offsetof(struct delay_data, ts),
		offsetof(struct delay_data, packet),
		offsetof(struct delay_data, packet_time_valid),
		offsetof(struct delay_data, packet_time),
		sizeof(struct obs_output_delay_fields),
		_Alignof(struct obs_output_delay_fields),
		offsetof(struct obs_output_delay_fields, delay_mutex),
		offsetof(struct obs_output_delay_fields, delay_data),
		offsetof(struct obs_output_delay_fields, active_delay_ns),
		offsetof(struct obs_output_delay_fields, delay_sec),
		offsetof(struct obs_output_delay_fields, delay_flags),
		offsetof(struct obs_output_delay_fields, delay_cur_flags),
		offsetof(struct obs_output_delay_fields, delay_restart_refs),
		offsetof(struct obs_output_delay_fields, delay_active),
		offsetof(struct obs_output_delay_fields, delay_capturing),
		offsetof(struct obs_output_delay_fields, delay_callback),
		offsetof(struct obs_output_delay_fields, starting),
		offsetof(struct obs_output_delay_fields, reconnecting),
		offsetof(struct obs_output_delay_fields, flags),
		offsetof(struct obs_output_delay_fields, name),
		sizeof(enum delay_msg),
		sizeof(enum obs_encoder_type),
		sizeof(bool),
		sizeof(long),
		_Alignof(long),
		sizeof(encoded_callback_t),
		sizeof(struct deque),
		_Alignof(struct deque),
		offsetof(struct deque, data),
		offsetof(struct deque, size),
		offsetof(struct deque, start_pos),
		offsetof(struct deque, end_pos),
		offsetof(struct deque, capacity),
		sizeof(pthread_mutex_t *),
	};
	return which < ORACLE_OUTPUT_DELAY_LAYOUT_COUNT ? values[which] : 0;
}

/* A delay_mutex for a test output, as obs_output_create() makes it. */
void *oracle_output_delay_mutex_create(void)
{
	pthread_mutex_t *mutex = bzalloc(sizeof(*mutex));
	if (pthread_mutex_init(mutex, NULL) != 0) {
		bfree(mutex);
		return NULL;
	}
	return mutex;
}

void oracle_output_delay_mutex_destroy(void *mutex)
{
	pthread_mutex_destroy(mutex);
	bfree(mutex);
}

/* Whether some thread holds the mutex: the C test's mutex_held(). */
bool oracle_output_delay_mutex_held(void *mutex)
{
	if (pthread_mutex_trylock(mutex) != 0)
		return true;
	pthread_mutex_unlock(mutex);
	return false;
}
