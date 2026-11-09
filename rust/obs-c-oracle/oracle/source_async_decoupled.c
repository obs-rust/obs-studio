/* Real-source state and lock-boundary parity; no intended differences. */
#include "obs-internal.h"
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#if defined(_MSC_VER)
#define DECOUPLED_TLS __declspec(thread)
#else
#define DECOUPLED_TLS _Thread_local
#endif

struct decoupled_trace {
	uint64_t values[32];
	size_t count;
	obs_source_t *source;
};
static DECOUPLED_TLS struct decoupled_trace *decoupled_current;

static void decoupled_record(uint64_t event)
{
	struct decoupled_trace *trace = decoupled_current;
	obs_source_t *source = trace->source;
	trace->values[trace->count++] = event;
	trace->values[trace->count++] = source->async_decoupled;
	trace->values[trace->count++] = source->timing_set;
	trace->values[trace->count++] = source->audio_ts;
	trace->values[trace->count++] = source->next_audio_sys_ts_min;
	trace->values[trace->count++] = source->last_audio_input_buf_size;
	for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++)
		trace->values[trace->count++] = source->audio_input_buf[i].size;
}

static int decoupled_lock(pthread_mutex_t *mutex)
{
	int result = pthread_mutex_lock(mutex);
	decoupled_record(1);
	return result;
}

static int decoupled_unlock(pthread_mutex_t *mutex)
{
	decoupled_record(2);
	return pthread_mutex_unlock(mutex);
}

#define pthread_mutex_lock decoupled_lock
#define pthread_mutex_unlock decoupled_unlock
#define reset_audio_data oracle_reset_audio_data
#define obs_source_set_async_decoupled oracle_obs_source_set_async_decoupled
void oracle_reset_audio_data(obs_source_t *source, uint64_t os_time);
void oracle_obs_source_set_async_decoupled(obs_source_t *source, bool value);
#include "obs-source-async-decoupled.c.h"
#undef obs_source_set_async_decoupled
#undef reset_audio_data
/* Compile the real adapter for the Rust side with identical lock observation. */
#include "source_async_decoupled_fields.inc"
#undef pthread_mutex_unlock
#undef pthread_mutex_lock

typedef void (*decoupled_setter)(obs_source_t *, bool);

bool oracle_source_async_decoupled_compare(decoupled_setter setter, bool previous, bool value, uint64_t seed);
bool oracle_source_async_decoupled_compare(decoupled_setter setter, bool previous, bool value, uint64_t seed)
{
	obs_source_t *source = bzalloc(sizeof(*source));
	if (pthread_mutex_init(&source->audio_buf_mutex, NULL) != 0) {
		bfree(source);
		return false;
	}
	unsigned char bytes[32];
	for (size_t i = 0; i < sizeof(bytes); i++)
		bytes[i] = (unsigned char)(seed + i);
	for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++) {
		if (i != 1)
			deque_push_back(&source->audio_input_buf[i], bytes, 1 + (seed + i) % sizeof(bytes));
	}
	source->async_decoupled = previous;
	source->timing_set = (seed & 1) != 0;
	source->timing_adjust = seed;
	source->audio_ts = seed ^ 987654;
	source->next_audio_sys_ts_min = seed ^ 456789;
	source->last_audio_input_buf_size = 87;
	source->async_unbuffered = true;
	source->async_width = 37;
	source->async_height = 53;
	struct obs_source initial;
	memcpy(&initial, source, sizeof(initial));
	struct decoupled_trace c_trace = {.source = source};
	decoupled_current = &c_trace;
	oracle_obs_source_set_async_decoupled(source, value);
	unsigned char expected[sizeof(*source)];
	memcpy(expected, source, sizeof(expected));
	/* Restore changed fields and deque cursors, never copy a live pthread mutex. */
	source->async_decoupled = initial.async_decoupled;
	source->timing_set = initial.timing_set;
	source->audio_ts = initial.audio_ts;
	source->next_audio_sys_ts_min = initial.next_audio_sys_ts_min;
	source->last_audio_input_buf_size = initial.last_audio_input_buf_size;
	memcpy(source->audio_input_buf, initial.audio_input_buf, sizeof(source->audio_input_buf));
	struct decoupled_trace rust_trace = {.source = source};
	decoupled_current = &rust_trace;
	setter(source, value);
	decoupled_current = NULL;
	/* Compare the complete source apart from the opaque pthread mutex bytes. */
	size_t mutex_offset = offsetof(struct obs_source, audio_buf_mutex);
	size_t after_mutex = mutex_offset + sizeof(source->audio_buf_mutex);
	bool equal = memcmp(source, expected, mutex_offset) == 0 &&
		     memcmp((unsigned char *)source + after_mutex, expected + after_mutex,
			    sizeof(*source) - after_mutex) == 0 &&
		     c_trace.count == rust_trace.count &&
		     memcmp(c_trace.values, rust_trace.values, sizeof(c_trace.values)) == 0;
	if (!value) {
		for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++) {
			if (i == 1)
				continue;
			unsigned char actual[32] = {0};
			size_t size = source->audio_input_buf[i].size;
			deque_peek_front(&source->audio_input_buf[i], actual, size);
			equal = equal && memcmp(actual, bytes, size) == 0;
		}
	}
	int lock_result = pthread_mutex_trylock(&source->audio_buf_mutex);
	equal = equal && lock_result == 0;
	if (lock_result == 0)
		pthread_mutex_unlock(&source->audio_buf_mutex);
	for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++)
		deque_free(&source->audio_input_buf[i]);
	pthread_mutex_destroy(&source->audio_buf_mutex);
	bfree(source);
	return equal;
}

struct decoupled_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void decoupled_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void decoupled_capture(int level, const char *format, va_list args, void *param)
{
	struct decoupled_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

bool oracle_source_async_decoupled_null_log(decoupled_setter setter);
bool oracle_source_async_decoupled_null_log(decoupled_setter setter)
{
	struct decoupled_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(decoupled_capture, &log);
	setter(NULL, false);
	bool first = log.count == 1 && log.level == LOG_DEBUG &&
		     strcmp(log.message, "obs_source_set_async_decoupled: Null 'source' parameter") == 0;
	setter(NULL, true);
	base_set_log_handler(old_handler, old_param);
	return first && log.count == 2 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_source_set_async_decoupled: Null 'source' parameter") == 0;
}
#undef DECOUPLED_TLS
