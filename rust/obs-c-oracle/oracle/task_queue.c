/* Original extracted task queue entry points (libobs/obs-task-queue.c.h) over
 * a real-header core, with the real field adapters (obs-task-queue-fields.c.h)
 * under their own names, and one recorder for every call the entry points
 * make: the UI thread mark, the os_task_queue_*, os_event_* and os_sleep_ms
 * calls, blog, the graphics and audio task locks and pushes, and the UI task
 * handler. The route is the production one (obs_core_setup_rules_task_step /
 * _task_thread under their real names, the Rust of core_setup_rules), as in
 * hybrid libobs.
 *
 * The recorder plays the other threads: a pushed or queued task waits in its
 * queue until a wait or a sleep runs it (oldest first), with that queue's
 * thread flag set while it runs; os_task_queue_wait runs the destroy queue.
 * When asked it changes the core's destruction queue, video thread flag and
 * audio output, and swaps the UI handler, after each call, so a field read
 * moved past a call shows in the log.
 *
 * With no run in progress the adapters read a host core whose UI task handler
 * is oracle_ui_task_handler_queue (oracle/ui_task_handler.c), and every
 * thread counts as the graphics and audio thread, so obs_queue_task() runs a
 * graphics or audio task at once and hands a UI task to that recorder: the
 * stand-in the other obs-core tests had before obs_queue_task() was Rust
 * (oracle/source_profiler_host.c). Unchanged Tier 2 covers the real threads.
 *
 * No stdio: the recorder formats its own log (_NO_CRT_STDIO_INLINE, so the
 * object never defines the UCRT stdio inlines, #4877). */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <limits.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#define TQ_LOG 8192
#define TQ_TOKENS 8
#define TQ_EVENTS 4
#define TQ_QUEUE 8
#define TQ_DESTROY0 5

/* Defined here for the test binaries; libobs defines them in obs.c.h. */
THREAD_LOCAL bool is_graphics_thread = true;
THREAD_LOCAL bool is_audio_thread = true;

void oracle_ui_task_handler_queue(obs_task_t task, void *param, bool wait);

typedef void (*tq_queue_fn)(enum obs_task_type type, obs_task_t task, void *param, bool wait);

struct tq_event {
	bool live;
	bool manual;
	bool signaled;
};

struct tq_pending {
	obs_task_t task;
	void *param;
	unsigned seq;
};

struct tq_ring {
	struct tq_pending items[TQ_QUEUE];
	size_t head;
	size_t len;
};

static struct obs_core tq_host = {.ui_task_handler = oracle_ui_task_handler_queue};
static THREAD_LOCAL struct obs_core tq_core;
static THREAD_LOCAL bool tq_active;
static unsigned char tq_tokens[TQ_TOKENS];
static THREAD_LOCAL char tq_log[TQ_LOG];
static THREAD_LOCAL size_t tq_len;
static THREAD_LOCAL struct tq_event tq_events[TQ_EVENTS];
/* 0 graphics, 1 audio, 2 destroy */
static THREAD_LOCAL struct tq_ring tq_queues[3];
static THREAD_LOCAL unsigned tq_seq;
static THREAD_LOCAL bool tq_ui;
static THREAD_LOCAL bool tq_inside_answer;
static THREAD_LOCAL bool tq_on_destroy;
static THREAD_LOCAL bool tq_wait_answer;
static THREAD_LOCAL unsigned tq_fail_mask;
static THREAD_LOCAL unsigned tq_inits;
static THREAD_LOCAL bool tq_clobber_on;
static THREAD_LOCAL unsigned tq_clobbers;
static THREAD_LOCAL unsigned tq_idle_sleeps;
static THREAD_LOCAL tq_queue_fn tq_side_queue;
static THREAD_LOCAL int tq_nested_type;
static THREAD_LOCAL bool tq_nested_wait;

static struct obs_core *tq_obs(void)
{
	return tq_active ? &tq_core : &tq_host;
}

/* ------------------------------------------------------------------------- */
/* log                                                                       */

static void tq_put(const char *text)
{
	if (!tq_active)
		return;
	size_t n = strlen(text);
	if (n >= TQ_LOG - tq_len)
		abort();
	memcpy(tq_log + tq_len, text, n + 1);
	tq_len += n;
}

static void tq_put_int(long long value)
{
	char digits[24];
	size_t n = 0;
	bool negative = value < 0;
	unsigned long long magnitude = negative ? 0ull - (unsigned long long)value : (unsigned long long)value;
	do {
		digits[n++] = (char)('0' + (int)(magnitude % 10));
		magnitude /= 10;
	} while (magnitude);
	char text[26];
	size_t at = 0;
	if (negative)
		text[at++] = '-';
	while (n)
		text[at++] = digits[--n];
	text[at] = 0;
	tq_put(text);
}

/* A token's index, -1 for NULL, 99 for anything else (the wait info). */
static int tq_id(const void *ptr)
{
	if (!ptr)
		return -1;
	for (int i = 0; i < TQ_TOKENS; ++i)
		if (ptr == (const void *)&tq_tokens[i])
			return i;
	for (int i = 0; i < TQ_EVENTS; ++i)
		if (ptr == (const void *)&tq_events[i])
			return 20 + i;
	return 99;
}

static void tq_task_a(void *param);
static void tq_task_nested(void *param);
static void tq_ui_handler_a(obs_task_t task, void *param, bool wait);
static void tq_ui_handler_b(obs_task_t task, void *param, bool wait);

/* 'A' and 'N' the test tasks, '0' NULL, 'W' anything else (the wait
 * callback, C's or Rust's). */
static const char *tq_fn(obs_task_t task)
{
	if (!task)
		return "0";
	if (task == tq_task_a)
		return "A";
	if (task == tq_task_nested)
		return "N";
	return "W";
}

static void tq_rec(const char *what, const char *fn, int a, int b, int c)
{
	tq_put(what);
	if (fn) {
		tq_put(" ");
		tq_put(fn);
	}
	const int values[3] = {a, b, c};
	for (int i = 0; i < 3; ++i) {
		if (values[i] == INT_MIN)
			continue;
		tq_put(" ");
		tq_put_int(values[i]);
	}
	tq_put(";");
}

#define NONE INT_MIN

/* After each call: another destruction queue, video flag, audio output and
 * UI handler. */
static void tq_clobber(void)
{
	if (!tq_clobber_on || !tq_active)
		return;
	tq_clobbers++;
	tq_core.destruction_task_thread = (os_task_queue_t *)&tq_tokens[TQ_DESTROY0 + tq_clobbers % 3];
	tq_core.video.thread_initialized = !tq_core.video.thread_initialized;
	tq_core.audio.audio = tq_core.audio.audio ? NULL : (audio_t *)&tq_tokens[4];
	if (tq_core.ui_task_handler)
		tq_core.ui_task_handler = tq_core.ui_task_handler == tq_ui_handler_a ? tq_ui_handler_b
										     : tq_ui_handler_a;
}

/* ------------------------------------------------------------------------- */
/* the other threads                                                         */

static void tq_pend(int queue, obs_task_t task, void *param)
{
	struct tq_ring *ring = &tq_queues[queue];
	if (ring->len == TQ_QUEUE)
		abort();
	ring->items[(ring->head + ring->len) % TQ_QUEUE] = (struct tq_pending){task, param, tq_seq++};
	ring->len++;
}

static size_t tq_pending_count(void)
{
	return tq_queues[0].len + tq_queues[1].len + tq_queues[2].len;
}

/* Runs the head of queue (or of the oldest queue for -1) on its thread. */
static bool tq_run_one(int queue)
{
	if (queue < 0) {
		for (int i = 0; i < 3; ++i) {
			if (!tq_queues[i].len)
				continue;
			if (queue < 0 || tq_queues[i].items[tq_queues[i].head].seq <
						 tq_queues[queue].items[tq_queues[queue].head].seq)
				queue = i;
		}
		if (queue < 0)
			return false;
	}
	struct tq_ring *ring = &tq_queues[queue];
	if (!ring->len)
		return false;
	struct tq_pending item = ring->items[ring->head];
	ring->head = (ring->head + 1) % TQ_QUEUE;
	ring->len--;

	bool graphics = is_graphics_thread, audio = is_audio_thread, destroy = tq_on_destroy;
	is_graphics_thread = queue == 0;
	is_audio_thread = queue == 1;
	tq_on_destroy = queue == 2;
	tq_rec("run", tq_fn(item.task), queue, tq_id(item.param), NONE);
	if (item.task)
		item.task(item.param);
	is_graphics_thread = graphics;
	is_audio_thread = audio;
	tq_on_destroy = destroy;
	return true;
}

/* ------------------------------------------------------------------------- */
/* the test tasks and handlers                                               */

static void tq_task_a(void *param)
{
	tq_rec("task", NULL, tq_id(param), NONE, NONE);
	tq_clobber();
}

/* Queues A with the same param to the type it was queued to. */
static void tq_task_nested(void *param)
{
	tq_rec("nested", NULL, tq_id(param), NONE, NONE);
	tq_clobber();
	tq_side_queue((enum obs_task_type)tq_nested_type, tq_task_a, param, tq_nested_wait);
	tq_rec("nested_end", NULL, NONE, NONE, NONE);
}

static void tq_ui_handler_a(obs_task_t task, void *param, bool wait)
{
	tq_rec("ui_handler_a", tq_fn(task), tq_id(param), wait, NONE);
	tq_clobber();
}

static void tq_ui_handler_b(obs_task_t task, void *param, bool wait)
{
	tq_rec("ui_handler_b", tq_fn(task), tq_id(param), wait, NONE);
	tq_clobber();
}

/* ------------------------------------------------------------------------- */
/* recording stand-ins                                                       */

bool oracle_tq_ui_thread(void);
bool oracle_tq_ui_thread(void)
{
	tq_rec("ui_thread", NULL, NONE, NONE, NONE);
	tq_clobber();
	return tq_ui;
}

bool oracle_tq_inside(os_task_queue_t *tq);
bool oracle_tq_inside(os_task_queue_t *tq)
{
	tq_rec("inside", NULL, tq_id(tq), NONE, NONE);
	tq_clobber();
	return tq_on_destroy || tq_inside_answer;
}

bool oracle_tq_queue_task(os_task_queue_t *tq, os_task_t task, void *param);
bool oracle_tq_queue_task(os_task_queue_t *tq, os_task_t task, void *param)
{
	tq_rec("destroy_push", tq_fn((obs_task_t)task), tq_id(tq), tq_id(param), NONE);
	tq_pend(2, (obs_task_t)task, param);
	tq_clobber();
	return true;
}

bool oracle_tq_wait(os_task_queue_t *tq);
bool oracle_tq_wait(os_task_queue_t *tq)
{
	tq_rec("destroy_wait", NULL, tq_id(tq), NONE, NONE);
	while (tq_run_one(2))
		;
	tq_rec("destroy_wait_end", NULL, NONE, NONE, NONE);
	tq_clobber();
	return tq_wait_answer;
}

static struct tq_event *tq_event_of(os_event_t *event)
{
	for (int i = 0; i < TQ_EVENTS; ++i)
		if (event == (os_event_t *)&tq_events[i])
			return &tq_events[i];
	abort();
}

int oracle_tq_event_init(os_event_t **event, enum os_event_type type);
int oracle_tq_event_init(os_event_t **event, enum os_event_type type)
{
	unsigned n = tq_inits++;
	tq_rec("event_init", NULL, (int)type, NONE, NONE);
	tq_clobber();
	if (n < 8 && (tq_fail_mask & (1u << n)))
		return -1;
	for (int i = 0; i < TQ_EVENTS; ++i) {
		if (!tq_events[i].live) {
			tq_events[i] = (struct tq_event){true, type == OS_EVENT_TYPE_MANUAL, false};
			*event = (os_event_t *)&tq_events[i];
			return 0;
		}
	}
	abort();
}

int oracle_tq_event_signal(os_event_t *event);
int oracle_tq_event_signal(os_event_t *event)
{
	tq_rec("signal", NULL, tq_id(event), NONE, NONE);
	tq_event_of(event)->signaled = true;
	tq_clobber();
	return 0;
}

int oracle_tq_event_wait(os_event_t *event);
int oracle_tq_event_wait(os_event_t *event)
{
	tq_rec("wait", NULL, tq_id(event), NONE, NONE);
	struct tq_event *e = tq_event_of(event);
	while (!e->signaled && tq_run_one(-1))
		;
	if (!e->signaled)
		tq_rec("deadlock", NULL, NONE, NONE, NONE);
	else if (!e->manual)
		e->signaled = false;
	tq_rec("wait_end", NULL, tq_id(event), NONE, NONE);
	tq_clobber();
	return 0;
}

void oracle_tq_event_destroy(os_event_t *event);
void oracle_tq_event_destroy(os_event_t *event)
{
	tq_rec("event_destroy", NULL, tq_id(event), NONE, NONE);
	if (event)
		tq_event_of(event)->live = false;
	tq_clobber();
}

void oracle_tq_sleep_ms(uint32_t duration);
void oracle_tq_sleep_ms(uint32_t duration)
{
	tq_rec("sleep", NULL, (int)duration, NONE, NONE);
	tq_clobber();
	if (tq_run_one(-1))
		tq_idle_sleeps = 0;
	else if (++tq_idle_sleeps > 4)
		abort(); /* polling for a task nothing will run */
}

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
void oracle_tq_blog(int level, const char *format, ...);
void oracle_tq_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	tq_put("log ");
	tq_put_int(level);
	tq_put(" ");
	char one[2] = {0, 0};
	for (const char *at = format; *at; ++at) {
		if (at[0] == '%' && at[1] == 'd') {
			tq_put_int(va_arg(args, int));
			++at;
		} else {
			one[0] = *at;
			tq_put(one);
		}
	}
	va_end(args);
	tq_put(";");
	tq_clobber();
}

static int tq_mutex_id(pthread_mutex_t *mutex)
{
	struct obs_core *core = tq_obs();
	if (mutex == &core->video.task_mutex)
		return 0;
	if (mutex == &core->audio.task_mutex)
		return 1;
	return 99;
}

int oracle_tq_lock(pthread_mutex_t *mutex);
int oracle_tq_lock(pthread_mutex_t *mutex)
{
	tq_rec("lock", NULL, tq_mutex_id(mutex), NONE, NONE);
	tq_clobber();
	return 0;
}

int oracle_tq_unlock(pthread_mutex_t *mutex);
int oracle_tq_unlock(pthread_mutex_t *mutex)
{
	tq_rec("unlock", NULL, tq_mutex_id(mutex), NONE, NONE);
	tq_clobber();
	return 0;
}

void oracle_tq_deque_push(struct deque *dq, const void *data, size_t size);
void oracle_tq_deque_push(struct deque *dq, const void *data, size_t size)
{
	struct obs_core *core = tq_obs();
	int queue = dq == &core->video.tasks ? 0 : dq == &core->audio.tasks ? 1 : 99;
	if (queue == 99 || size != sizeof(struct obs_task_info))
		abort();
	struct obs_task_info info;
	memcpy(&info, data, sizeof(info));
	tq_rec("push", tq_fn(info.task), queue, tq_id(info.param), NONE);
	tq_pend(queue, info.task, info.param);
	tq_clobber();
}

/* ------------------------------------------------------------------------- */
/* the original and the real adapters                                        */

#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock oracle_tq_lock
#define pthread_mutex_unlock oracle_tq_unlock
#define deque_push_back oracle_tq_deque_push
#define obs (tq_obs())

/* The real field adapters under their own names: the Rust side reads
 * through them, and obs-tests-core links them. */
#include "task-queue-fields.inc"

bool oracle_obs_in_task_thread(enum obs_task_type type);
void oracle_obs_queue_task(enum obs_task_type type, obs_task_t task, void *param, bool wait);
bool oracle_obs_wait_for_destroy_queue(void);

#define obs_in_task_thread oracle_obs_in_task_thread
#define obs_queue_task oracle_obs_queue_task
#define obs_wait_for_destroy_queue oracle_obs_wait_for_destroy_queue
#define obs_core_is_ui_thread oracle_tq_ui_thread
#define os_task_queue_inside oracle_tq_inside
#define os_task_queue_queue_task oracle_tq_queue_task
#define os_task_queue_wait oracle_tq_wait
#pragma push_macro("os_event_init")
#pragma push_macro("os_event_signal")
#pragma push_macro("os_event_wait")
#pragma push_macro("os_event_destroy")
#undef os_event_init
#undef os_event_signal
#undef os_event_wait
#undef os_event_destroy
#define os_event_init oracle_tq_event_init
#define os_event_signal oracle_tq_event_signal
#define os_event_wait oracle_tq_event_wait
#define os_event_destroy oracle_tq_event_destroy
#define os_sleep_ms oracle_tq_sleep_ms
#define blog oracle_tq_blog
#include "libobs/obs-task-queue.c.h"
#undef blog
#undef os_sleep_ms
#undef os_event_destroy
#undef os_event_wait
#undef os_event_signal
#undef os_event_init
#pragma pop_macro("os_event_destroy")
#pragma pop_macro("os_event_wait")
#pragma pop_macro("os_event_signal")
#pragma pop_macro("os_event_init")
#undef os_task_queue_wait
#undef os_task_queue_queue_task
#undef os_task_queue_inside
#undef obs_core_is_ui_thread
#undef obs_wait_for_destroy_queue
#undef obs_queue_task
#undef obs_in_task_thread
#undef obs
#undef deque_push_back
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

/* ------------------------------------------------------------------------- */
/* one run                                                                   */

typedef bool (*tq_in_fn)(enum obs_task_type type);
typedef bool (*tq_destroy_fn)(void);

/* One call. state (all small ints, checked):
 *   [0] op: 0 obs_in_task_thread, 1 obs_queue_task, 2 obs_wait_for_destroy_queue
 *   [1] the type (any int for op 1; 0..3 for op 0)
 *   [2] the task: 0 NULL, 1 A, 2 nested (queues A to the same type)
 *   [3] wait; [4] the param: -1 NULL, else token 0..3
 *   [5] is_graphics_thread; [6] is_audio_thread; [7] obs_core_is_ui_thread();
 *   [8] os_task_queue_inside() off the destroy queue
 *   [9] the UI handler: 0 NULL, 1 A, 2 B
 *   [10] video.thread_initialized; [11] audio.audio set
 *   [12] the destruction queue: token 5..7 as 0..2
 *   [13] the os_event_init() calls that fail, a bit each, in call order
 *   [14] os_task_queue_wait()'s result; [15] clobber after each call
 *   [16] tasks pending before the call, A with token 3, a bit each:
 *        1 graphics, 2 audio, 4 destroy
 *   [17] the nested task's wait
 * Writes the log (NUL-terminated) and report: [0] the result (op 0 and 2),
 * [1] the core left as it was (without clobbering), [2] tasks left pending,
 * [3] events left live. */
void oracle_task_queue_run(tq_in_fn in, tq_queue_fn queue, tq_destroy_fn destroy, const int *state, char *log,
			   size_t log_len, int *report);
void oracle_task_queue_run(tq_in_fn in, tq_queue_fn queue, tq_destroy_fn destroy, const int *state, char *log,
			   size_t log_len, int *report)
{
	if (state[0] < 0 || state[0] > 2 || (state[0] == 0 && (state[1] < 0 || state[1] > 3)) || state[2] < 0 ||
	    state[2] > 2 || state[4] < -1 || state[4] > 3 || state[9] < 0 || state[9] > 2 || state[12] < 0 ||
	    state[12] > 2 || state[13] < 0 || state[13] > 255 || state[16] < 0 || state[16] > 7 || log_len < TQ_LOG)
		abort();

	memset(&tq_core, 0, sizeof(tq_core));
	tq_core.ui_task_handler = state[9] == 0 ? NULL : state[9] == 1 ? tq_ui_handler_a : tq_ui_handler_b;
	tq_core.video.thread_initialized = state[10] != 0;
	tq_core.audio.audio = state[11] ? (audio_t *)&tq_tokens[4] : NULL;
	tq_core.destruction_task_thread = (os_task_queue_t *)&tq_tokens[TQ_DESTROY0 + state[12]];
	memset(tq_events, 0, sizeof(tq_events));
	memset(tq_queues, 0, sizeof(tq_queues));
	tq_seq = 0;
	tq_ui = state[7] != 0;
	tq_inside_answer = state[8] != 0;
	tq_on_destroy = false;
	tq_wait_answer = state[14] != 0;
	tq_fail_mask = (unsigned)state[13];
	tq_inits = 0;
	tq_clobber_on = state[15] != 0;
	tq_clobbers = 0;
	tq_idle_sleeps = 0;
	tq_side_queue = queue;
	tq_nested_type = state[1];
	tq_nested_wait = state[17] != 0;
	tq_len = 0;
	tq_log[0] = 0;
	for (int i = 0; i < 3; ++i)
		if (state[16] & (1 << i))
			tq_pend(i, tq_task_a, &tq_tokens[3]);
	tq_active = true;
	is_graphics_thread = state[5] != 0;
	is_audio_thread = state[6] != 0;

	struct obs_core core;
	memcpy(&core, &tq_core, sizeof(core));

	void *param = state[4] < 0 ? NULL : &tq_tokens[state[4]];
	obs_task_t task = state[2] == 0 ? NULL : state[2] == 1 ? tq_task_a : tq_task_nested;
	int result = 0;
	switch (state[0]) {
	case 0:
		result = in((enum obs_task_type)state[1]);
		break;
	case 1:
		queue((enum obs_task_type)state[1], task, param, state[3] != 0);
		break;
	default:
		result = destroy();
		break;
	}

	memcpy(log, tq_log, tq_len + 1);
	report[0] = result;
	report[1] = tq_clobber_on || memcmp(&core, &tq_core, sizeof(core)) == 0;
	report[2] = (int)tq_pending_count();
	int live = 0;
	for (int i = 0; i < TQ_EVENTS; ++i)
		live += tq_events[i].live;
	report[3] = live;

	tq_active = false;
	is_graphics_thread = true;
	is_audio_thread = true;
	memset(&tq_core, 0, sizeof(tq_core));
}

/* The enum values and sizes the port relies on. */
void oracle_task_queue_values(int *out);
void oracle_task_queue_values(int *out)
{
	out[0] = OBS_TASK_UI;
	out[1] = OBS_TASK_GRAPHICS;
	out[2] = OBS_TASK_AUDIO;
	out[3] = OBS_TASK_DESTROY;
	out[4] = OS_EVENT_TYPE_AUTO;
	out[5] = OS_EVENT_TYPE_MANUAL;
	out[6] = LOG_ERROR;
	out[7] = LOG_WARNING;
	out[8] = (int)sizeof(enum obs_task_type);
	out[9] = (int)sizeof(obs_task_t);
	out[10] = (int)sizeof(struct obs_task_info);
}
