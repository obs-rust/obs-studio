/* Exact original C queries, with real-header fixtures. */
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#define obs_source_media_admit oracle_obs_source_media_admit
#define obs_source_media_admit_flags oracle_obs_source_media_admit_flags
#define obs_source_media_admit_callback oracle_obs_source_media_admit_callback
#define obs_source_media_time_fallback oracle_obs_source_media_time_fallback
#define obs_source_media_state_fallback oracle_obs_source_media_state_fallback
#define obs_source_media_get_duration oracle_obs_source_media_get_duration
#define obs_source_media_get_time oracle_obs_source_media_get_time
#define obs_source_media_get_state oracle_obs_source_media_get_state
#include "obs-source-media-getters.c.h"
#undef obs_source_media_get_duration
#undef obs_source_media_get_time
#undef obs_source_media_get_state
#include "source_media_getter_fields.inc"

typedef int64_t (*media_time_query)(obs_source_t *);
typedef enum obs_media_state (*media_state_query)(obs_source_t *);
struct media_fixture {
	obs_source_t *source;
	int64_t duration, time;
	enum obs_media_state state;
	unsigned calls[3];
	bool correct_context, reenter;
	media_time_query time_query;
	media_state_query state_query;
};
static int64_t media_duration(void *data)
{
	struct media_fixture *f = data;
	f->correct_context = f->correct_context && f->source->context.data == data;
	f->calls[0]++;
	if (f->reenter) {
		f->correct_context = f->correct_context && f->time_query(f->source) == f->time;
		f->correct_context = f->correct_context && f->state_query(f->source) == f->state;
	}
	return f->duration;
}
static int64_t media_time(void *data)
{
	struct media_fixture *f = data;
	f->correct_context = f->correct_context && f->source->context.data == data;
	f->calls[1]++;
	return f->time;
}
static enum obs_media_state media_state(void *data)
{
	struct media_fixture *f = data;
	f->correct_context = f->correct_context && f->source->context.data == data;
	f->calls[2]++;
	return f->state;
}
void oracle_source_media_getters_run(media_time_query duration, media_time_query time, media_state_query state,
				     bool valid, uint32_t flags, unsigned mask, int64_t d, int64_t t, unsigned s,
				     bool reenter, int64_t *out);
void oracle_source_media_getters_run(media_time_query duration, media_time_query time, media_state_query state,
				     bool valid, uint32_t flags, unsigned mask, int64_t d, int64_t t, unsigned s,
				     bool reenter, int64_t *out)
{
	struct obs_source source = {0};
	struct media_fixture contexts[2] = {{0}, {0}};
	source.info.output_flags = flags;
	source.info.media_get_duration = mask & 1 ? media_duration : NULL;
	source.info.media_get_time = mask & 2 ? media_time : NULL;
	source.info.media_get_state = mask & 4 ? media_state : NULL;
	for (unsigned i = 0; i < 2; i++) {
		struct media_fixture *f = &contexts[i];
		f->source = &source;
		f->duration = d;
		f->time = t;
		f->state = (enum obs_media_state)s;
		f->correct_context = true;
		f->reenter = reenter;
		f->time_query = time;
		f->state_query = state;
		source.context.data = valid ? f : NULL;
		out[i * 8] = duration(&source);
		out[i * 8 + 1] = time(&source);
		out[i * 8 + 2] = state(&source);
		for (unsigned j = 0; j < 3; j++)
			out[i * 8 + 3 + j] = f->calls[j];
		out[i * 8 + 6] = f->correct_context;
		out[i * 8 + 7] = (int64_t)source.media_actions.num;
	}
}
void oracle_source_media_getters_layout(size_t *out);
void oracle_source_media_getters_layout(size_t *out)
{
	out[0] = sizeof(enum obs_media_state);
	out[1] = _Alignof(enum obs_media_state);
	out[2] = OBS_SOURCE_CONTROLLABLE_MEDIA;
	const enum obs_media_state states[] = {OBS_MEDIA_STATE_NONE,    OBS_MEDIA_STATE_PLAYING,
					       OBS_MEDIA_STATE_OPENING, OBS_MEDIA_STATE_BUFFERING,
					       OBS_MEDIA_STATE_PAUSED,  OBS_MEDIA_STATE_STOPPED,
					       OBS_MEDIA_STATE_ENDED,   OBS_MEDIA_STATE_ERROR};
	for (size_t i = 0; i < 8; i++)
		out[3 + i] = states[i];
}

struct media_getter_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void media_getter_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void media_getter_capture(int level, const char *format, va_list args, void *param)
{
	struct media_getter_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}
typedef void (*media_getter_get_log)(log_handler_t *, void **);
typedef void (*media_getter_set_log)(log_handler_t, void *);
bool oracle_source_media_getters_logs(media_time_query duration, media_time_query time, media_state_query state,
				      media_getter_get_log get_log, media_getter_set_log set_log);
bool oracle_source_media_getters_logs(media_time_query duration, media_time_query time, media_state_query state,
				      media_getter_get_log get_log, media_getter_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	bool equal = true;
	const char *expected[] = {"obs_source_media_get_duration: Null 'source' parameter",
				  "obs_source_media_get_time: Null 'source' parameter",
				  "obs_source_media_get_state: Null 'source' parameter"};
	for (unsigned i = 0; i < 3; i++) {
		struct media_getter_log r = {0}, c = {0};
		set_log(media_getter_capture, &r);
		int64_t rv = i == 0 ? duration(NULL) : i == 1 ? time(NULL) : (int64_t)state(NULL);
		set_log(media_getter_capture, &c);
		int64_t cv = i == 0   ? oracle_obs_source_media_get_duration(NULL)
			     : i == 1 ? oracle_obs_source_media_get_time(NULL)
				      : (int64_t)oracle_obs_source_media_get_state(NULL);
		equal = equal && rv == 0 && cv == 0 && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, c.message) == 0 &&
			strcmp(r.message, expected[i]) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}
