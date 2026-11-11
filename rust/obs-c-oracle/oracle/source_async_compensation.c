/* Original setter and real-header adapter. No parity exclusions. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_set_async_compensation oracle_obs_source_set_async_compensation
#include "obs-source-async-compensation-setter.c.h"
#undef obs_source_set_async_compensation
#include "source_async_compensation_fields.inc"

struct async_compensation_fixture {
	struct obs_source source;
	unsigned char snapshot[sizeof(struct obs_source)];
};

void *oracle_source_async_compensation_new(bool initial, uint64_t timestamp, int64_t offset);
void *oracle_source_async_compensation_new(bool initial, uint64_t timestamp, int64_t offset)
{
	struct async_compensation_fixture *f = calloc(1, sizeof(*f));
	if (f) {
		f->source.async_compensation = initial;
		f->source.last_async_compensation = !initial;
		f->source.timing_set = true;
		f->source.timing_adjust = timestamp;
		f->source.resample_offset = timestamp;
		f->source.next_resample_in_ts = timestamp;
		f->source.next_audio_ts_min = timestamp;
		f->source.next_audio_sys_ts_min = timestamp;
		f->source.last_frame_ts = timestamp;
		f->source.last_output_frame_ts = timestamp;
		f->source.own_clock_timing_adjust = timestamp;
		f->source.own_clock_timing_set = true;
		f->source.async_last_rendered_ts = timestamp;
		f->source.sync_offset = offset;
		f->source.last_sync_offset = offset;
		f->source.async_active = true;
		f->source.async_unbuffered = true;
		f->source.async_decoupled = true;
		memcpy(f->snapshot, &f->source, sizeof(f->source));
	}
	return f;
}

bool oracle_source_async_compensation_check(void *source, bool expected);
bool oracle_source_async_compensation_check(void *source, bool expected)
{
	struct async_compensation_fixture *f = source;
	/* Change exactly the flag byte in the expected object representation. */
	memcpy(f->snapshot + offsetof(struct obs_source, async_compensation), &expected, sizeof(expected));
	return os_atomic_load_bool(&f->source.async_compensation) == expected &&
	       memcmp(&f->source, f->snapshot, sizeof(f->source)) == 0;
}

void oracle_source_async_compensation_free(void *source);
void oracle_source_async_compensation_free(void *source)
{
	free(source);
}

void oracle_source_async_compensation_layout(size_t *layout);
void oracle_source_async_compensation_layout(size_t *layout)
{
	layout[0] = sizeof(bool);
	layout[1] = _Alignof(bool);
}

struct async_compensation_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void async_compensation_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void async_compensation_capture(int level, const char *format, va_list args, void *param)
{
	struct async_compensation_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*async_compensation_setter)(obs_source_t *, bool);
bool oracle_source_async_compensation_null_log(async_compensation_setter setter, bool requested);
bool oracle_source_async_compensation_null_log(async_compensation_setter setter, bool requested)
{
	struct async_compensation_log log = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(async_compensation_capture, &log);
	setter(NULL, requested);
	base_set_log_handler(old_handler, old_param);
	return log.count == 1 && log.level == LOG_DEBUG &&
	       strcmp(log.message, "obs_source_set_async_compensation: Null 'source' parameter") == 0;
}
