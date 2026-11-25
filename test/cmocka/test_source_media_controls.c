/* Characterization of the eight obs_source_media_*() control calls (#5616):
 * play_pause, restart, stop, next, previous and set_time queue an action for
 * the video thread (process_media_actions() runs it in the tick); started
 * and ended emit a signal.
 *
 * What it pins, on test-owned struct obs_source values with a real signal
 * handler and a real media_actions queue, calling the exported functions of
 * the real libobs:
 *
 *   - NULL: each call logs one LOG_DEBUG line,
 *     "<function>: Null 'source' parameter", and does nothing else.
 *   - The six queueing calls accept a source only when its context.data is
 *     set, its info.output_flags has OBS_SOURCE_CONTROLLABLE_MEDIA and its
 *     own info.media_* callback is set; any other callback does not count.
 *     A refused call logs nothing, leaves the queue alone and never takes
 *     media_actions_mutex.
 *   - An accepted queueing call appends exactly one struct media_action
 *     after the ones already queued: its type, pause for play_pause, ms for
 *     set_time, and pause false for the others. It takes and releases
 *     media_actions_mutex. The callback is never called here.
 *   - started and ended need only a non-NULL source with
 *     OBS_SOURCE_CONTROLLABLE_MEDIA: a NULL context.data and missing
 *     callbacks are accepted. They emit media_started / media_ended once on
 *     the source's own handler, with only "source" in a fixed 128-byte
 *     calldata, and nothing on the core's global handler (public sources
 *     included). They never touch the queue or its mutex.
 *   - No call writes any other byte of the source. */
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned log_count;
static int log_level;
static char log_message[128];
#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static log_handler_t saved_handler;
static void *saved_param;

static void capture_start(void)
{
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	log_level = -1;
	log_message[0] = '\0';
}

static void capture_stop(void)
{
	base_set_log_handler(saved_handler, saved_param);
}

enum call {
	CALL_PLAY_PAUSE,
	CALL_RESTART,
	CALL_STOP,
	CALL_NEXT,
	CALL_PREVIOUS,
	CALL_SET_TIME,
	CALL_STARTED,
	CALL_ENDED,
	CALLS,
	QUEUEING = CALL_STARTED,
};

static const char *const names[CALLS] = {
	"obs_source_media_play_pause", "obs_source_media_restart",  "obs_source_media_stop",
	"obs_source_media_next",       "obs_source_media_previous", "obs_source_media_set_time",
	"obs_source_media_started",    "obs_source_media_ended",
};

static const enum media_action_type types[QUEUEING] = {
	MEDIA_ACTION_PLAY_PAUSE, MEDIA_ACTION_RESTART,  MEDIA_ACTION_STOP,
	MEDIA_ACTION_NEXT,       MEDIA_ACTION_PREVIOUS, MEDIA_ACTION_SET_TIME,
};

static void call(enum call which, obs_source_t *source, bool pause, int64_t ms)
{
	switch (which) {
	case CALL_PLAY_PAUSE:
		obs_source_media_play_pause(source, pause);
		break;
	case CALL_RESTART:
		obs_source_media_restart(source);
		break;
	case CALL_STOP:
		obs_source_media_stop(source);
		break;
	case CALL_NEXT:
		obs_source_media_next(source);
		break;
	case CALL_PREVIOUS:
		obs_source_media_previous(source);
		break;
	case CALL_SET_TIME:
		obs_source_media_set_time(source, ms);
		break;
	case CALL_STARTED:
		obs_source_media_started(source);
		break;
	case CALL_ENDED:
		obs_source_media_ended(source);
		break;
	default:
		fail();
	}
}

/* Every media callback is a trap: the calls under test never call one. */
static unsigned callback_calls;
static void trap_pause(void *data, bool pause)
{
	(void)data;
	(void)pause;
	callback_calls++;
}
static void trap_void(void *data)
{
	(void)data;
	callback_calls++;
}
static void trap_set_time(void *data, int64_t ms)
{
	(void)data;
	(void)ms;
	callback_calls++;
}
static int64_t trap_time(void *data)
{
	(void)data;
	callback_calls++;
	return 0;
}
static enum obs_media_state trap_state(void *data)
{
	(void)data;
	callback_calls++;
	return OBS_MEDIA_STATE_NONE;
}

#define EVENTS 4

struct event {
	int signal;
	void *source;
	size_t size;
	bool fixed;
	size_t capacity;
};

struct fixture {
	struct obs_source source;
	struct obs_source before;
	int token;
	size_t calls;
	struct event events[EVENTS];
};

static void record(struct fixture *f, calldata_t *data, int signal)
{
	assert_true(f->calls < EVENTS);
	struct event *e = &f->events[f->calls++];
	e->signal = signal;
	e->source = calldata_ptr(data, "source");
	e->size = data->size;
	e->fixed = data->fixed;
	e->capacity = data->capacity;
}

static void on_started(void *param, calldata_t *data)
{
	record(param, data, CALL_STARTED);
}

static void on_ended(void *param, calldata_t *data)
{
	record(param, data, CALL_ENDED);
}

/* Sets the queueing call's callback when its bit of mask is set; NULL
 * otherwise. The getters' callbacks are always set. */
static void set_callbacks(struct obs_source *s, unsigned mask)
{
	s->info.media_play_pause = mask & (1u << CALL_PLAY_PAUSE) ? trap_pause : NULL;
	s->info.media_restart = mask & (1u << CALL_RESTART) ? trap_void : NULL;
	s->info.media_stop = mask & (1u << CALL_STOP) ? trap_void : NULL;
	s->info.media_next = mask & (1u << CALL_NEXT) ? trap_void : NULL;
	s->info.media_previous = mask & (1u << CALL_PREVIOUS) ? trap_void : NULL;
	s->info.media_set_time = mask & (1u << CALL_SET_TIME) ? trap_set_time : NULL;
	s->info.media_get_duration = trap_time;
	s->info.media_get_time = trap_time;
	s->info.media_get_state = trap_state;
}

#define ALL_CALLBACKS ((1u << QUEUEING) - 1)

/* An error-checking mutex: a lock by its owner returns EDEADLK instead of
 * hanging, so a call made while the test holds it shows whether it locked. */
static void fixture_init(struct fixture *f, unsigned char fill, uint32_t output_flags, bool data, unsigned callbacks)
{
	memset(f, fill, sizeof(*f));
	f->source.info.output_flags = output_flags;
	f->source.context.data = data ? &f->token : NULL;
	f->source.context.private = false;
	set_callbacks(&f->source, callbacks);
	da_init(f->source.media_actions);
	pthread_mutexattr_t attr;
	assert_int_equal(pthread_mutexattr_init(&attr), 0);
	assert_int_equal(pthread_mutexattr_settype(&attr, PTHREAD_MUTEX_ERRORCHECK), 0);
	assert_int_equal(pthread_mutex_init(&f->source.media_actions_mutex, &attr), 0);
	pthread_mutexattr_destroy(&attr);
	f->source.context.signals = signal_handler_create();
	assert_non_null(f->source.context.signals);
	assert_true(signal_handler_add(f->source.context.signals, "void media_started(ptr source)"));
	assert_true(signal_handler_add(f->source.context.signals, "void media_ended(ptr source)"));
	signal_handler_connect(f->source.context.signals, "media_started", on_started, f);
	signal_handler_connect(f->source.context.signals, "media_ended", on_ended, f);
	f->calls = 0;
	memcpy(&f->before, &f->source, sizeof(f->source));
}

/* Every byte of the source is as before, apart from the queue and its
 * mutex (whose bytes a lock and unlock may change on some hosts). */
static void assert_only_queue(struct fixture *f)
{
	memcpy(&f->before.media_actions, &f->source.media_actions, sizeof(f->source.media_actions));
	memcpy(&f->before.media_actions_mutex, &f->source.media_actions_mutex, sizeof(f->source.media_actions_mutex));
	assert_memory_equal(&f->before, &f->source, sizeof(f->source));
}

static void fixture_free(struct fixture *f)
{
	da_free(f->source.media_actions);
	pthread_mutex_destroy(&f->source.media_actions_mutex);
	signal_handler_destroy(f->source.context.signals);
}

/* Calls `which` while the test holds the mutex. True when the call did not
 * lock it (the test still owns it); false when it locked and unlocked it,
 * releasing the test's hold. Either way the mutex is unlocked afterwards. */
static bool call_without_mutex(struct fixture *f, enum call which, bool pause, int64_t ms)
{
	assert_int_equal(pthread_mutex_lock(&f->source.media_actions_mutex), 0);
	call(which, &f->source, pause, ms);
	const int unlocked = pthread_mutex_unlock(&f->source.media_actions_mutex);
	assert_true(unlocked == 0 || unlocked == EPERM);
	return unlocked == 0;
}

static const unsigned char fills[] = {0x00, 0xff, 0x5a};
static const uint32_t flag_words[] = {0,
				      OBS_SOURCE_CONTROLLABLE_MEDIA,
				      OBS_SOURCE_CONTROLLABLE_MEDIA | OBS_SOURCE_VIDEO | OBS_SOURCE_AUDIO,
				      ~(uint32_t)OBS_SOURCE_CONTROLLABLE_MEDIA,
				      0xffffffffu,
				      OBS_SOURCE_VIDEO | OBS_SOURCE_ASYNC};

static void null_source_logs_once(void **state)
{
	(void)state;
	for (int which = 0; which < CALLS; which++) {
		capture_start();
		call(which, NULL, true, 1);
		capture_stop();
		assert_int_equal(log_count, 1);
		assert_int_equal(log_level, LOG_DEBUG);
		char expected[128];
		snprintf(expected, sizeof(expected), "%s: Null 'source' parameter", names[which]);
		assert_string_equal(log_message, expected);
	}
}

static void queueing_calls_need_data_flags_and_their_callback(void **state)
{
	(void)state;
	static struct fixture f;

	capture_start();
	for (int which = 0; which < QUEUEING; which++) {
		for (size_t fill = 0; fill < sizeof(fills); fill++) {
			for (size_t w = 0; w < sizeof(flag_words) / sizeof(flag_words[0]); w++) {
				for (unsigned data = 0; data < 2; data++) {
					for (unsigned own = 0; own < 2; own++) {
						for (unsigned others = 0; others < 2; others++) {
							const unsigned mask =
								((others ? ALL_CALLBACKS : 0) & ~(1u << which)) |
								(own ? 1u << which : 0);
							fixture_init(&f, fills[fill], flag_words[w], data != 0, mask);
							const bool accept =
								data && own &&
								(flag_words[w] & OBS_SOURCE_CONTROLLABLE_MEDIA);
							callback_calls = 0;
							if (accept) {
								/* The call takes the mutex, so it releases
								 * the test's hold. */
								assert_false(call_without_mutex(&f, which, true, 7));
								assert_int_equal(f.source.media_actions.num, 1);
							} else {
								assert_true(call_without_mutex(&f, which, true, 7));
								assert_int_equal(f.source.media_actions.num, 0);
								assert_null(f.source.media_actions.array);
							}
							assert_int_equal(f.calls, 0);
							assert_int_equal(callback_calls, 0);
							assert_only_queue(&f);
							fixture_free(&f);
						}
					}
				}
			}
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

static void accepted_calls_append_one_action(void **state)
{
	(void)state;
	static struct fixture f;
	static const int64_t times[] = {0, 1, -1, INT64_MIN, INT64_MAX, 0x123456789ll, 4000};

	capture_start();
	for (int which = 0; which < QUEUEING; which++) {
		for (size_t fill = 0; fill < sizeof(fills); fill++) {
			for (size_t t = 0; t < sizeof(times) / sizeof(times[0]); t++) {
				const bool pause = (t & 1) != 0;
				fixture_init(&f, fills[fill], OBS_SOURCE_CONTROLLABLE_MEDIA, true, ALL_CALLBACKS);
				/* Two actions already queued stay first and unchanged. */
				struct media_action first = {.type = MEDIA_ACTION_NEXT};
				struct media_action second = {.type = MEDIA_ACTION_SET_TIME, .ms = 77};
				da_push_back(f.source.media_actions, &first);
				da_push_back(f.source.media_actions, &second);
				callback_calls = 0;

				call(which, &f.source, pause, times[t]);

				assert_int_equal(f.source.media_actions.num, 3);
				const struct media_action *a = f.source.media_actions.array;
				assert_int_equal(a[0].type, MEDIA_ACTION_NEXT);
				assert_false(a[0].pause);
				assert_int_equal(a[1].type, MEDIA_ACTION_SET_TIME);
				assert_int_equal(a[1].ms, 77);
				assert_int_equal(a[2].type, types[which]);
				if (which == CALL_SET_TIME)
					assert_int_equal(a[2].ms, times[t]);
				else if (which == CALL_PLAY_PAUSE)
					assert_int_equal(a[2].pause, pause);
				else
					assert_false(a[2].pause);
				/* The mutex is free again. */
				assert_int_equal(pthread_mutex_trylock(&f.source.media_actions_mutex), 0);
				assert_int_equal(pthread_mutex_unlock(&f.source.media_actions_mutex), 0);
				assert_int_equal(f.calls, 0);
				assert_int_equal(callback_calls, 0);
				assert_only_queue(&f);
				fixture_free(&f);
			}
		}
	}

	/* Calls append in call order. */
	fixture_init(&f, 0x5a, OBS_SOURCE_CONTROLLABLE_MEDIA, true, ALL_CALLBACKS);
	obs_source_media_play_pause(&f.source, true);
	obs_source_media_set_time(&f.source, -5);
	obs_source_media_stop(&f.source);
	obs_source_media_play_pause(&f.source, false);
	obs_source_media_restart(&f.source);
	obs_source_media_previous(&f.source);
	obs_source_media_next(&f.source);
	assert_int_equal(f.source.media_actions.num, 7);
	const struct media_action *a = f.source.media_actions.array;
	assert_int_equal(a[0].type, MEDIA_ACTION_PLAY_PAUSE);
	assert_true(a[0].pause);
	assert_int_equal(a[1].type, MEDIA_ACTION_SET_TIME);
	assert_int_equal(a[1].ms, -5);
	assert_int_equal(a[2].type, MEDIA_ACTION_STOP);
	assert_int_equal(a[3].type, MEDIA_ACTION_PLAY_PAUSE);
	assert_false(a[3].pause);
	assert_int_equal(a[4].type, MEDIA_ACTION_RESTART);
	assert_int_equal(a[5].type, MEDIA_ACTION_PREVIOUS);
	assert_int_equal(a[6].type, MEDIA_ACTION_NEXT);
	assert_int_equal(callback_calls, 0);
	fixture_free(&f);

	capture_stop();
	assert_int_equal(log_count, 0);
}

static unsigned global_media_signals;
static void on_global(void *param, const char *signal, calldata_t *data)
{
	(void)param;
	(void)data;
	if (strcmp(signal, "media_started") == 0 || strcmp(signal, "media_ended") == 0)
		global_media_signals++;
}

static void started_and_ended_signal_the_source(void **state)
{
	(void)state;
	static struct fixture f;

	/* A calldata holding only the source pointer. */
	uint8_t stack[128];
	calldata_t only_source;
	calldata_init_fixed(&only_source, stack, sizeof(stack));
	calldata_set_ptr(&only_source, "source", &f.source);
	const size_t source_only_size = only_source.size;

	signal_handler_t *global = obs_get_signal_handler();
	assert_non_null(global);
	signal_handler_connect_global(global, on_global, NULL);
	global_media_signals = 0;

	capture_start();
	for (int which = CALL_STARTED; which <= CALL_ENDED; which++) {
		for (size_t fill = 0; fill < sizeof(fills); fill++) {
			for (size_t w = 0; w < sizeof(flag_words) / sizeof(flag_words[0]); w++) {
				for (unsigned data = 0; data < 2; data++) {
					for (unsigned callbacks = 0; callbacks < 2; callbacks++) {
						fixture_init(&f, fills[fill], flag_words[w], data != 0,
							     callbacks ? ALL_CALLBACKS : 0);
						const bool accept = (flag_words[w] & OBS_SOURCE_CONTROLLABLE_MEDIA) !=
								    0;
						callback_calls = 0;
						/* Neither call takes the mutex. */
						assert_true(call_without_mutex(&f, which, true, 7));
						if (accept) {
							assert_int_equal(f.calls, 1);
							const struct event *e = &f.events[0];
							assert_int_equal(e->signal, which);
							assert_ptr_equal(e->source, &f.source);
							assert_int_equal(e->size, source_only_size);
							assert_true(e->fixed);
							assert_int_equal(e->capacity, 128);
						} else {
							assert_int_equal(f.calls, 0);
						}
						assert_int_equal(f.source.media_actions.num, 0);
						assert_null(f.source.media_actions.array);
						assert_int_equal(callback_calls, 0);
						assert_only_queue(&f);
						fixture_free(&f);
					}
				}
			}
		}
	}
	capture_stop();
	signal_handler_disconnect_global(global, on_global, NULL);
	assert_int_equal(log_count, 0);
	/* The core's handler never hears of them, for a public source too. */
	assert_int_equal(global_media_signals, 0);
}

static int setup(void **state)
{
	(void)state;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source_logs_once),
		cmocka_unit_test(queueing_calls_need_data_flags_and_their_callback),
		cmocka_unit_test(accepted_calls_append_one_action),
		cmocka_unit_test(started_and_ended_signal_the_source),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
