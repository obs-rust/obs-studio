/* Real-header oracle, including the original getters and discovered adapters. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

bool oracle_obs_output_paused(const obs_output_t *output);
uint64_t oracle_obs_output_get_pause_offset(obs_output_t *output);

struct output_pause_queries_fixture {
	struct obs_output output;
	bool paused;
	uint64_t offset;
	unsigned locks;
	unsigned unlocks;
	bool observe;
};

static int output_pause_queries_lock_probe(pthread_mutex_t *mutex)
{
	struct output_pause_queries_fixture *fixture =
		(void *)((char *)mutex - offsetof(struct output_pause_queries_fixture, output.pause.mutex));
	int result = pthread_mutex_lock(mutex);
	fixture->locks++;
	if (fixture->observe)
		fixture->output.pause.ts_offset = 42;
	return result;
}

static int output_pause_queries_unlock_probe(pthread_mutex_t *mutex)
{
	struct output_pause_queries_fixture *fixture =
		(void *)((char *)mutex - offsetof(struct output_pause_queries_fixture, output.pause.mutex));
	fixture->unlocks++;
	if (fixture->observe)
		fixture->output.pause.ts_offset = 99;
	return pthread_mutex_unlock(mutex);
}

/* The mingw build renames every pthread function on the command line
 * (-Dpthread_mutex_lock=oracle_pthread_mutex_lock, w32-pthreads): keep that
 * name for the rest of the file and use the probes only here (#4168). */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock output_pause_queries_lock_probe
#define pthread_mutex_unlock output_pause_queries_unlock_probe
#define obs_output_paused oracle_obs_output_paused
#define obs_output_get_pause_offset oracle_obs_output_get_pause_offset
#include "obs-output-pause-queries.c.h"
#undef obs_output_get_pause_offset
#undef obs_output_paused
#include "output_pause_queries_fields.inc"
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

void *oracle_output_pause_queries_new(bool paused, uint64_t offset);
void *oracle_output_pause_queries_new(bool paused, uint64_t offset)
{
	struct output_pause_queries_fixture *fixture = calloc(1, sizeof(*fixture));
	if (!fixture)
		return NULL;
	if (pthread_mutex_init(&fixture->output.pause.mutex, NULL) != 0) {
		free(fixture);
		return NULL;
	}
	os_atomic_set_bool(&fixture->output.paused, paused);
	fixture->output.pause.ts_offset = offset;
	fixture->output.pause.ts_start = 41;
	fixture->output.pause.ts_end = 73;
	fixture->paused = paused;
	fixture->offset = offset;
	return fixture;
}

void oracle_output_pause_queries_free(void *handle);
void oracle_output_pause_queries_free(void *handle)
{
	struct output_pause_queries_fixture *fixture = handle;
	pthread_mutex_destroy(&fixture->output.pause.mutex);
	free(fixture);
}

bool oracle_output_pause_queries_unchanged(const void *handle);
bool oracle_output_pause_queries_unchanged(const void *handle)
{
	const struct output_pause_queries_fixture *f = handle;
	return os_atomic_load_bool(&f->output.paused) == f->paused && f->output.pause.ts_offset == f->offset &&
	       f->output.pause.ts_start == 41 && f->output.pause.ts_end == 73 && f->locks == f->unlocks;
}

bool oracle_output_pause_queries_addresses(obs_output_t *output);
bool oracle_output_pause_queries_addresses(obs_output_t *output)
{
	struct obs_output_pause_queries_fields f = obs_output_pause_queries_fields(output);
	return f.paused == &output->paused && f.offset == &output->pause.ts_offset && f.mutex == &output->pause.mutex;
}

void oracle_output_pause_queries_layout(size_t *layout);
void oracle_output_pause_queries_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_output_pause_queries_fields);
	layout[1] = _Alignof(struct obs_output_pause_queries_fields);
	layout[2] = offsetof(struct obs_output_pause_queries_fields, paused);
	layout[3] = offsetof(struct obs_output_pause_queries_fields, offset);
	layout[4] = offsetof(struct obs_output_pause_queries_fields, mutex);
	layout[5] = sizeof(((struct obs_output *)0)->paused);
	layout[6] = _Alignof(bool);
	layout[7] = sizeof(((struct obs_output *)0)->pause.ts_offset);
	layout[8] = _Alignof(uint64_t);
}

typedef uint64_t (*output_pause_queries_offset)(obs_output_t *);
bool oracle_output_pause_queries_observe(output_pause_queries_offset getter);
bool oracle_output_pause_queries_observe(output_pause_queries_offset getter)
{
	struct output_pause_queries_fixture *fixture = oracle_output_pause_queries_new(false, 13);
	if (!fixture)
		return false;
	fixture->observe = true;
	uint64_t result = getter(&fixture->output);
	bool correct = result == 42 && fixture->output.pause.ts_offset == 99 && fixture->locks == 1 &&
		       fixture->unlocks == 1;
	oracle_output_pause_queries_free(fixture);
	return correct;
}

struct output_pause_queries_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_pause_queries_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_pause_queries_capture(int level, const char *format, va_list args, void *param)
{
	struct output_pause_queries_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef bool (*output_pause_queries_paused)(const obs_output_t *);
typedef void (*output_pause_queries_get_log)(log_handler_t *, void **);
typedef void (*output_pause_queries_set_log)(log_handler_t, void *);
bool oracle_output_pause_queries_logs(output_pause_queries_paused paused, output_pause_queries_offset offset,
				      output_pause_queries_get_log get_log, output_pause_queries_set_log set_log);
bool oracle_output_pause_queries_logs(output_pause_queries_paused paused, output_pause_queries_offset offset,
				      output_pause_queries_get_log get_log, output_pause_queries_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	bool equal = true;
	const char *expected[] = {"obs_output_paused: Null 'output' parameter",
				  "obs_output_get_pause_offset: Null 'output' parameter"};
	for (size_t i = 0; i < 2; i++) {
		struct output_pause_queries_log r = {0}, c = {0};
		set_log(output_pause_queries_capture, &r);
		uint64_t rust_value = i ? offset(NULL) : paused(NULL);
		set_log(output_pause_queries_capture, &c);
		uint64_t c_value = i ? oracle_obs_output_get_pause_offset(NULL) : oracle_obs_output_paused(NULL);
		equal = equal && rust_value == 0 && c_value == 0 && r.count == 1 && c.count == 1 &&
			r.level == LOG_DEBUG && c.level == LOG_DEBUG && strcmp(r.message, expected[i]) == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}
