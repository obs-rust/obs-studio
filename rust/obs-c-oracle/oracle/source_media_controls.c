/* Tier 3: the unchanged extracted obs_source_media_* control calls, and the
 * production adapters the Rust shims go through
 * (obs-source-media-controls-fields.c.h, defined once, here), over the real
 * private layout with a real media_actions queue. Both sides' locks, unlocks
 * and obs_source_dosignal() calls are recorded through the same stand-ins;
 * the real signal dispatch is covered by test_source_media_controls. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_media_play_pause oracle_obs_source_media_play_pause
#define obs_source_media_restart oracle_obs_source_media_restart
#define obs_source_media_stop oracle_obs_source_media_stop
#define obs_source_media_next oracle_obs_source_media_next
#define obs_source_media_previous oracle_obs_source_media_previous
#define obs_source_media_set_time oracle_obs_source_media_set_time
#define obs_source_media_started oracle_obs_source_media_started
#define obs_source_media_ended oracle_obs_source_media_ended
/* The original C admission steps (oracle/source_media_admission.c). */
#define obs_source_media_admit oracle_obs_source_media_admit
#define obs_source_media_admit_flags oracle_obs_source_media_admit_flags
#define obs_source_media_admit_callback oracle_obs_source_media_admit_callback
#include "libobs/build/libobs.pch.h"

#define MEDIA_CONTROLS_OPS 32
#define MEDIA_CONTROLS_EVENTS 4
/* Per operation: the event count, two words per event, the queue length,
 * the appended action's type and value, the callback calls, and whether
 * every byte but the queue and its mutex is as before. */
#define MEDIA_CONTROLS_TRACE (1 + 2 * MEDIA_CONTROLS_EVENTS + 5)

struct media_controls_fixture {
	struct obs_source source;
	struct obs_source before;
	int token;
	unsigned callbacks;
	size_t events;
	uint64_t trace[2 * MEDIA_CONTROLS_EVENTS];
};

static _Thread_local struct media_controls_fixture *media_controls_current;

static void media_controls_record(uint64_t kind, uint64_t value)
{
	struct media_controls_fixture *f = media_controls_current;
	if (!f)
		return;
	if (f->events < MEDIA_CONTROLS_EVENTS) {
		f->trace[2 * f->events] = kind;
		f->trace[2 * f->events + 1] = value;
	}
	f->events++;
}

/* Lock and unlock: kind 1 or 2, 0x20 when it is the source's own mutex;
 * the queue length at the time. */
static int media_controls_lock(pthread_mutex_t *mutex)
{
	struct media_controls_fixture *f = media_controls_current;
	if (f)
		media_controls_record(1 | (mutex == &f->source.media_actions_mutex ? 0x20u : 0u),
				      f->source.media_actions.num);
	return pthread_mutex_lock(mutex);
}

static int media_controls_unlock(pthread_mutex_t *mutex)
{
	struct media_controls_fixture *f = media_controls_current;
	if (f)
		media_controls_record(2 | (mutex == &f->source.media_actions_mutex ? 0x20u : 0u),
				      f->source.media_actions.num);
	return pthread_mutex_unlock(mutex);
}

/* obs_source_dosignal(): kind 3, 0x10 when the global name is NULL, 0x20
 * when it is the fixture's source; the local name as 1 (media_started),
 * 2 (media_ended), 0 (NULL) or 3 (another). */
static void media_controls_dosignal(struct obs_source *source, const char *signal_obs, const char *signal_source)
{
	struct media_controls_fixture *f = media_controls_current;
	uint64_t name = !signal_source                                ? 0
			: strcmp(signal_source, "media_started") == 0 ? 1
			: strcmp(signal_source, "media_ended") == 0   ? 2
								      : 3;
	media_controls_record(3 | (signal_obs ? 0u : 0x10u) | (f && source == &f->source ? 0x20u : 0u), name);
}

#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock media_controls_lock
#define pthread_mutex_unlock media_controls_unlock
#define obs_source_dosignal media_controls_dosignal
#include "source-media-controls.inc"
#undef obs_source_media_ended
#undef obs_source_media_started
#undef obs_source_media_set_time
#undef obs_source_media_previous
#undef obs_source_media_next
#undef obs_source_media_stop
#undef obs_source_media_restart
#undef obs_source_media_play_pause
#include "source_media_controls_fields.inc"
#undef obs_source_dosignal
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

/* The eight calls under test, original or Rust. */
struct media_controls_calls {
	void (*play_pause)(obs_source_t *, bool);
	void (*restart)(obs_source_t *);
	void (*stop)(obs_source_t *);
	void (*next)(obs_source_t *);
	void (*previous)(obs_source_t *);
	void (*set_time)(obs_source_t *, int64_t);
	void (*started)(obs_source_t *);
	void (*ended)(obs_source_t *);
};

static void media_controls_call(const struct media_controls_calls *calls, uint8_t kind, obs_source_t *source,
				int64_t value)
{
	switch (kind) {
	case 0:
		calls->play_pause(source, (value & 1) != 0);
		break;
	case 1:
		calls->restart(source);
		break;
	case 2:
		calls->stop(source);
		break;
	case 3:
		calls->next(source);
		break;
	case 4:
		calls->previous(source);
		break;
	case 5:
		calls->set_time(source, value);
		break;
	case 6:
		calls->started(source);
		break;
	default:
		calls->ended(source);
		break;
	}
}

/* Every media callback counts its calls: the calls under test make none. */
static void media_controls_trap_pause(void *data, bool pause)
{
	(void)pause;
	((struct media_controls_fixture *)((char *)data - offsetof(struct media_controls_fixture, token)))->callbacks++;
}
static void media_controls_trap(void *data)
{
	((struct media_controls_fixture *)((char *)data - offsetof(struct media_controls_fixture, token)))->callbacks++;
}
static void media_controls_trap_time(void *data, int64_t ms)
{
	(void)ms;
	((struct media_controls_fixture *)((char *)data - offsetof(struct media_controls_fixture, token)))->callbacks++;
}

/* Runs count operations on a fixture filled with noise: kinds[i] 0 to 7 is
 * play_pause (pause = values[i] & 1), restart, stop, next, previous,
 * set_time (values[i]), started, ended. callbacks has bit 1 << call set for
 * each enum obs_source_media_call whose callback is set. Writes
 * MEDIA_CONTROLS_TRACE words per operation to trace. False when the fixture
 * could not be built, an adapter does not reach its field, or a call wrote
 * another byte than the queue and its mutex. */
bool oracle_source_media_controls_run(const struct media_controls_calls *calls, uint8_t noise, uint32_t output_flags,
				      bool data, unsigned callbacks, const uint8_t *kinds, const int64_t *values,
				      size_t count, uint64_t *trace);
bool oracle_source_media_controls_run(const struct media_controls_calls *calls, uint8_t noise, uint32_t output_flags,
				      bool data, unsigned callbacks, const uint8_t *kinds, const int64_t *values,
				      size_t count, uint64_t *trace)
{
	if (count > MEDIA_CONTROLS_OPS)
		return false;
	struct media_controls_fixture *f = malloc(sizeof(*f));
	if (!f)
		return false;
	memset(f, noise, sizeof(*f));
	f->source.info.output_flags = output_flags;
	f->source.context.data = data ? &f->token : NULL;
	f->source.info.media_play_pause =
		callbacks & (1u << OBS_SOURCE_MEDIA_CALL_PLAY_PAUSE) ? media_controls_trap_pause : NULL;
	f->source.info.media_restart = callbacks & (1u << OBS_SOURCE_MEDIA_CALL_RESTART) ? media_controls_trap : NULL;
	f->source.info.media_stop = callbacks & (1u << OBS_SOURCE_MEDIA_CALL_STOP) ? media_controls_trap : NULL;
	f->source.info.media_next = callbacks & (1u << OBS_SOURCE_MEDIA_CALL_NEXT) ? media_controls_trap : NULL;
	f->source.info.media_previous = callbacks & (1u << OBS_SOURCE_MEDIA_CALL_PREVIOUS) ? media_controls_trap : NULL;
	f->source.info.media_set_time = callbacks & (1u << OBS_SOURCE_MEDIA_CALL_SET_TIME) ? media_controls_trap_time
											   : NULL;
	da_init(f->source.media_actions);
	if (pthread_mutex_init(&f->source.media_actions_mutex, NULL) != 0) {
		free(f);
		return false;
	}
	f->callbacks = 0;

	/* The adapters reach the fields they name. */
	bool valid = obs_source_media_controls_data(&f->source) == f->source.context.data &&
		     obs_source_media_controls_output_flags(&f->source) == output_flags;
	for (int call = OBS_SOURCE_MEDIA_CALL_PLAY_PAUSE; call <= OBS_SOURCE_MEDIA_CALL_ENDED; call++) {
		const bool queueing = call <= OBS_SOURCE_MEDIA_CALL_PREVIOUS || call == OBS_SOURCE_MEDIA_CALL_SET_TIME;
		const bool expected = queueing && (callbacks & (1u << call)) != 0;
		valid = valid && obs_source_media_controls_has_callback(&f->source, call) == expected;
	}

	memcpy(&f->before, &f->source, sizeof(f->source));
	media_controls_current = f;
	for (size_t i = 0; i < count; i++) {
		f->events = 0;
		memset(f->trace, 0, sizeof(f->trace));
		const size_t queued = f->source.media_actions.num;
		media_controls_call(calls, kinds[i], &f->source, values[i]);
		uint64_t *t = &trace[i * MEDIA_CONTROLS_TRACE];
		t[0] = f->events;
		memcpy(&t[1], f->trace, sizeof(f->trace));
		uint64_t *after = &t[1 + 2 * MEDIA_CONTROLS_EVENTS];
		after[0] = f->source.media_actions.num;
		after[1] = 0xffff;
		after[2] = 0;
		if (f->source.media_actions.num > queued) {
			const struct media_action *a = &f->source.media_actions.array[f->source.media_actions.num - 1];
			after[1] = (uint64_t)a->type;
			after[2] = a->type == MEDIA_ACTION_SET_TIME ? (uint64_t)a->ms : (uint64_t)a->pause;
		}
		after[3] = f->callbacks;
		memcpy(&f->before.media_actions, &f->source.media_actions, sizeof(f->source.media_actions));
		memcpy(&f->before.media_actions_mutex, &f->source.media_actions_mutex,
		       sizeof(f->source.media_actions_mutex));
		const bool others = memcmp(&f->before, &f->source, sizeof(f->source)) == 0;
		after[4] = others;
		valid = valid && others;
	}
	media_controls_current = NULL;
	da_free(f->source.media_actions);
	pthread_mutex_destroy(&f->source.media_actions_mutex);
	free(f);
	return valid;
}

/* The original calls, for the parity test's other side. */
void oracle_source_media_controls_originals(struct media_controls_calls *calls);
void oracle_source_media_controls_originals(struct media_controls_calls *calls)
{
	calls->play_pause = oracle_obs_source_media_play_pause;
	calls->restart = oracle_obs_source_media_restart;
	calls->stop = oracle_obs_source_media_stop;
	calls->next = oracle_obs_source_media_next;
	calls->previous = oracle_obs_source_media_previous;
	calls->set_time = oracle_obs_source_media_set_time;
	calls->started = oracle_obs_source_media_started;
	calls->ended = oracle_obs_source_media_ended;
}

struct media_controls_log {
	unsigned count;
	int level;
	char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void media_controls_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void media_controls_capture(int level, const char *format, va_list args, void *param)
{
	struct media_controls_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*media_controls_get_log)(log_handler_t *, void **);
typedef void (*media_controls_set_log)(log_handler_t, void *);

/* A NULL source logs the same single LOG_DEBUG line from the given calls
 * as from the originals, naming the function. */
bool oracle_source_media_controls_null_logs(const struct media_controls_calls *calls, media_controls_get_log get,
					    media_controls_set_log set);
bool oracle_source_media_controls_null_logs(const struct media_controls_calls *calls, media_controls_get_log get,
					    media_controls_set_log set)
{
	static const char *const expected[8] = {
		"obs_source_media_play_pause: Null 'source' parameter",
		"obs_source_media_restart: Null 'source' parameter",
		"obs_source_media_stop: Null 'source' parameter",
		"obs_source_media_next: Null 'source' parameter",
		"obs_source_media_previous: Null 'source' parameter",
		"obs_source_media_set_time: Null 'source' parameter",
		"obs_source_media_started: Null 'source' parameter",
		"obs_source_media_ended: Null 'source' parameter",
	};
	struct media_controls_calls original;
	oracle_source_media_controls_originals(&original);
	log_handler_t old_handler;
	void *old_param;
	get(&old_handler, &old_param);
	bool valid = true;
	for (uint8_t kind = 0; kind < 8; kind++) {
		struct media_controls_log r = {0}, c = {0};
		set(media_controls_capture, &r);
		media_controls_call(calls, kind, NULL, 1);
		set(media_controls_capture, &c);
		media_controls_call(&original, kind, NULL, 1);
		set(old_handler, old_param);
		valid = valid && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG && c.level == LOG_DEBUG &&
			strcmp(c.message, expected[kind]) == 0 && strcmp(r.message, c.message) == 0;
	}
	return valid;
}
