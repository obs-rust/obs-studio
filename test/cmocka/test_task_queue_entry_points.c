/* Characterization of the task queue entry points (#5560): obs_in_task_thread(),
 * obs_queue_task() and obs_wait_for_destroy_queue(), with the
 * task_wait_callback() / task_wait_for_callback() pair they share, driven
 * through the public API on a live core (obs_startup, obs_reset_video with the
 * OpenGL module, obs_reset_audio).
 *
 * test_core_setup_rules_task_steps pins the route table on its own and
 * test_ui_task_handler the UI branch; this pins the graphics, audio and destroy
 * branches. What it pins:
 *
 *   - obs_in_task_thread() is false for GRAPHICS, AUDIO, DESTROY and UI on the
 *     test thread; inside a task queued to GRAPHICS, AUDIO or DESTROY it is
 *     true for that type and false for the other two; UI follows the thread
 *     the registered UI handler marked;
 *   - obs_queue_task(type, task, param, false) runs the task once, on that
 *     type's thread, not the caller's, with the param it was given;
 *   - obs_queue_task(type, task, param, true) returns only after the task ran
 *     on that type's thread, with param still on the caller's stack;
 *   - from a task already on the type's thread, obs_queue_task() to the same
 *     type runs the task inline on that thread before it returns, for wait
 *     true (no deadlock) and for wait false;
 *   - an unknown type ((enum obs_task_type)99, wait false) queues nothing:
 *     the task never runs on any of the three threads, and the call returns.
 *     The only error it may log is "unknown task type 99". Hybrid libobs logs
 *     it (the Rust route answers UNKNOWN, #3405); the C reference's route asks
 *     obs_in_task_thread() first (an assert, compiled out in release builds)
 *     and drops the task without a log, so the log itself is left to the Rust
 *     swap's parity. wait true is not driven: the C reference's route waits
 *     forever for a task it never queues;
 *   - obs_wait_for_destroy_queue() runs a pending graphics task and a pending
 *     audio task before it returns, returns only after an earlier DESTROY
 *     task ran, returns true when a DESTROY task ran while it waited, and
 *     false once the destroy queue is idle.
 *
 * Every wait is bounded: a wait=true call or obs_wait_for_destroy_queue() runs
 * on a helper thread the test joins with a timeout, so a deadlock fails the
 * case instead of blocking the run.
 *
 * Core video needs a graphics module and libobs needs an X display on Linux,
 * so the cases skip without one (run under xvfb-run). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/platform.h>
#include <util/threading.h>

#define TIMEOUT_MS 10000
#define UNKNOWN_TYPE ((enum obs_task_type)99)

static bool obs_up;
/* A helper thread never came back: the core cannot be shut down safely. */
static bool stuck;

static const enum obs_task_type QUEUES[] = {OBS_TASK_GRAPHICS, OBS_TASK_AUDIO, OBS_TASK_DESTROY};
#define QUEUE_COUNT (sizeof(QUEUES) / sizeof(QUEUES[0]))

static const char *type_name(enum obs_task_type type)
{
	switch (type) {
	case OBS_TASK_UI:
		return "UI";
	case OBS_TASK_GRAPHICS:
		return "GRAPHICS";
	case OBS_TASK_AUDIO:
		return "AUDIO";
	case OBS_TASK_DESTROY:
		return "DESTROY";
	}
	return "unknown";
}

/* ------------------------------------------------------------------------- */
/* log capture                                                               */

static log_handler_t old_log_handler;
static void *old_log_param;
static pthread_mutex_t log_mutex = PTHREAD_MUTEX_INITIALIZER;
static bool capturing;
static int error_count;
static char last_error[256];

#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	char message[256];
	va_list copy;
	va_copy(copy, args);
	vsnprintf(message, sizeof(message), format, copy);
	va_end(copy);

	pthread_mutex_lock(&log_mutex);
	if (capturing && level == LOG_ERROR) {
		error_count++;
		snprintf(last_error, sizeof(last_error), "%s", message);
	}
	pthread_mutex_unlock(&log_mutex);

	if (old_log_handler)
		old_log_handler(level, format, args, old_log_param);
}

static void start_capture(void)
{
	pthread_mutex_lock(&log_mutex);
	capturing = true;
	error_count = 0;
	last_error[0] = '\0';
	pthread_mutex_unlock(&log_mutex);
}

static int stop_capture(char *error, size_t size)
{
	pthread_mutex_lock(&log_mutex);
	capturing = false;
	int count = error_count;
	snprintf(error, size, "%s", last_error);
	pthread_mutex_unlock(&log_mutex);
	return count;
}

/* ------------------------------------------------------------------------- */
/* bounded waits                                                             */

struct helper {
	pthread_t thread;
	os_event_t *done;
	void (*fn)(void *);
	void *arg;
};

static void *helper_main(void *param)
{
	struct helper *h = param;
	h->fn(h->arg);
	os_event_signal(h->done);
	return NULL;
}

static void helper_start(struct helper *h, void (*fn)(void *), void *arg)
{
	h->fn = fn;
	h->arg = arg;
	assert_int_equal(os_event_init(&h->done, OS_EVENT_TYPE_MANUAL), 0);
	assert_int_equal(pthread_create(&h->thread, NULL, helper_main, h), 0);
}

static void helper_join(struct helper *h, const char *what)
{
	if (os_event_timedwait(h->done, TIMEOUT_MS) != 0) {
		/* Leave the thread and its event: it still uses both. */
		stuck = true;
		fail_msg("%s did not return within %d ms", what, TIMEOUT_MS);
	}
	pthread_join(h->thread, NULL);
	os_event_destroy(h->done);
}

/* Runs fn(arg) on a helper thread and fails if it does not return in time. */
static void bounded(void (*fn)(void *), void *arg, const char *what)
{
	struct helper h;
	helper_start(&h, fn, arg);
	helper_join(&h, what);
}

static void signal_task(void *param)
{
	os_event_signal(param);
}

/* Queues a marker behind everything queued to type so far and waits for it:
 * each queue runs its tasks in order, so the earlier ones have run. */
static void drain(enum obs_task_type type)
{
	os_event_t *event;
	assert_int_equal(os_event_init(&event, OS_EVENT_TYPE_MANUAL), 0);
	obs_queue_task(type, signal_task, event, false);
	if (os_event_timedwait(event, TIMEOUT_MS) != 0) {
		stuck = true;
		fail_msg("the %s queue did not drain within %d ms", type_name(type), TIMEOUT_MS);
	}
	os_event_destroy(event);
}

/* ------------------------------------------------------------------------- */
/* what a task saw                                                           */

struct seen {
	enum obs_task_type type;
	volatile long runs;
	pthread_t thread;
	void *param;
	/* obs_in_task_thread() for each of QUEUES, read inside the task */
	bool in[QUEUE_COUNT];
	os_event_t *ran;
};

static void record(struct seen *s, void *param)
{
	s->thread = pthread_self();
	s->param = param;
	for (size_t i = 0; i < QUEUE_COUNT; i++)
		s->in[i] = obs_in_task_thread(QUEUES[i]);
	os_atomic_inc_long(&s->runs);
	if (s->ran)
		os_event_signal(s->ran);
}

static void record_task(void *param)
{
	record(param, param);
}

static void expect_on_own_thread(const struct seen *s)
{
	assert_int_equal(os_atomic_load_long(&s->runs), 1);
	assert_false(pthread_equal(s->thread, pthread_self()));
	assert_ptr_equal(s->param, s);
	for (size_t i = 0; i < QUEUE_COUNT; i++) {
		if (QUEUES[i] == s->type) {
			if (!s->in[i])
				fail_msg("obs_in_task_thread(%s) false inside its own task", type_name(s->type));
		} else if (s->in[i]) {
			fail_msg("obs_in_task_thread(%s) true inside a %s task", type_name(QUEUES[i]),
				 type_name(s->type));
		}
	}
}

/* ------------------------------------------------------------------------- */
/* obs_in_task_thread                                                        */

static void *other_thread_ui(void *param)
{
	*(bool *)param = obs_in_task_thread(OBS_TASK_UI);
	return NULL;
}

static void inline_ui_handler(obs_task_t task, void *param, bool wait)
{
	(void)wait;
	task(param);
}

static void not_in_task_thread_on_test_thread(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	assert_false(obs_in_task_thread(OBS_TASK_GRAPHICS));
	assert_false(obs_in_task_thread(OBS_TASK_AUDIO));
	assert_false(obs_in_task_thread(OBS_TASK_DESTROY));
	/* no UI handler has marked a thread yet */
	assert_false(obs_in_task_thread(OBS_TASK_UI));
}

static void ui_follows_registered_ui_thread(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	/* The handler runs the bootstrap task inline, so the test thread
	 * becomes the UI thread; another thread does not. */
	obs_set_ui_task_handler(inline_ui_handler);
	assert_true(obs_in_task_thread(OBS_TASK_UI));

	bool other = true;
	pthread_t thread;
	assert_int_equal(pthread_create(&thread, NULL, other_thread_ui, &other), 0);
	pthread_join(thread, NULL);
	assert_false(other);

	/* the other types are unchanged on the UI thread */
	assert_false(obs_in_task_thread(OBS_TASK_GRAPHICS));
	assert_false(obs_in_task_thread(OBS_TASK_AUDIO));
	assert_false(obs_in_task_thread(OBS_TASK_DESTROY));

	/* Clearing the handler logs "no UI task handler" and leaves the mark
	 * (test_ui_task_handler pins both); it keeps shutdown off the inline
	 * handler. */
	obs_set_ui_task_handler(NULL);
}

/* ------------------------------------------------------------------------- */
/* obs_queue_task, wait false                                                */

static void queue_without_wait_runs_once_on_type_thread(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	for (size_t i = 0; i < QUEUE_COUNT; i++) {
		struct seen s = {.type = QUEUES[i]};
		assert_int_equal(os_event_init(&s.ran, OS_EVENT_TYPE_MANUAL), 0);

		obs_queue_task(QUEUES[i], record_task, &s, false);
		if (os_event_timedwait(s.ran, TIMEOUT_MS) != 0) {
			stuck = true;
			fail_msg("a %s task did not run within %d ms", type_name(QUEUES[i]), TIMEOUT_MS);
		}
		/* everything queued before the marker ran: still once */
		drain(QUEUES[i]);
		expect_on_own_thread(&s);

		os_event_destroy(s.ran);
	}
}

/* ------------------------------------------------------------------------- */
/* obs_queue_task, wait true                                                 */

struct slow {
	struct seen seen;
	volatile bool finished;
};

/* Sleeps before it finishes, so a caller that returned early would see
 * finished still false. */
static void slow_task(void *param)
{
	struct slow *s = param;
	os_sleep_ms(20);
	record(&s->seen, param);
	os_atomic_set_bool(&s->finished, true);
}

struct wait_call {
	enum obs_task_type type;
	pthread_t caller;
	bool finished_on_return;
	long runs_on_return;
	bool on_caller_thread;
	bool param_on_stack;
	bool in[QUEUE_COUNT];
};

static void queue_and_wait(void *param)
{
	struct wait_call *call = param;
	/* the task's param lives on this (the caller's) stack */
	struct slow s = {.seen = {.type = call->type}};

	call->caller = pthread_self();
	obs_queue_task(call->type, slow_task, &s, true);

	call->finished_on_return = os_atomic_load_bool(&s.finished);
	call->runs_on_return = os_atomic_load_long(&s.seen.runs);
	call->on_caller_thread = pthread_equal(s.seen.thread, call->caller);
	call->param_on_stack = s.seen.param == &s;
	/* checked on the test thread: cmocka cannot fail from this one */
	memcpy(call->in, s.seen.in, sizeof(call->in));
}

static void queue_with_wait_returns_after_task_ran(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	for (size_t i = 0; i < QUEUE_COUNT; i++) {
		struct wait_call call = {.type = QUEUES[i]};
		char what[64];
		snprintf(what, sizeof(what), "obs_queue_task(%s, wait true)", type_name(QUEUES[i]));
		bounded(queue_and_wait, &call, what);

		assert_true(call.finished_on_return);
		assert_int_equal(call.runs_on_return, 1);
		assert_false(call.on_caller_thread);
		assert_true(call.param_on_stack);
		for (size_t j = 0; j < QUEUE_COUNT; j++) {
			if ((QUEUES[j] == call.type) != call.in[j])
				fail_msg("obs_in_task_thread(%s) is %d inside a waited %s task", type_name(QUEUES[j]),
					 call.in[j], type_name(call.type));
		}

		drain(QUEUES[i]);
	}
}

/* ------------------------------------------------------------------------- */
/* obs_queue_task from the type's own thread                                 */

struct nested {
	enum obs_task_type type;
	bool wait;
	struct seen inner;
	bool inner_ran_before_return;
	bool same_thread;
	os_event_t *done;
};

static void outer_task(void *param)
{
	struct nested *n = param;
	n->inner.type = n->type;
	obs_queue_task(n->type, record_task, &n->inner, n->wait);
	n->inner_ran_before_return = os_atomic_load_long(&n->inner.runs) == 1;
	n->same_thread = n->inner_ran_before_return && pthread_equal(n->inner.thread, pthread_self());
	os_event_signal(n->done);
}

static void queue_from_own_thread_runs_inline(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	for (int waits = 1; waits >= 0; waits--) {
		for (size_t i = 0; i < QUEUE_COUNT; i++) {
			struct nested n = {.type = QUEUES[i], .wait = waits != 0};
			assert_int_equal(os_event_init(&n.done, OS_EVENT_TYPE_MANUAL), 0);

			obs_queue_task(QUEUES[i], outer_task, &n, false);
			if (os_event_timedwait(n.done, TIMEOUT_MS) != 0) {
				stuck = true;
				fail_msg("a %s task queuing to its own thread (wait %d) did not finish within %d ms",
					 type_name(QUEUES[i]), waits, TIMEOUT_MS);
			}
			drain(QUEUES[i]);

			if (!n.inner_ran_before_return || !n.same_thread)
				fail_msg("%s, wait %d: inner ran before return %d, on the same thread %d",
					 type_name(QUEUES[i]), waits, n.inner_ran_before_return, n.same_thread);
			/* inline, so once */
			assert_int_equal(os_atomic_load_long(&n.inner.runs), 1);
			expect_on_own_thread(&n.inner);

			os_event_destroy(n.done);
		}
	}
}

/* ------------------------------------------------------------------------- */
/* unknown type                                                              */

static volatile long unknown_runs;

static void unknown_task(void *param)
{
	(void)param;
	os_atomic_inc_long(&unknown_runs);
}

static void unknown_type_queues_nothing(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	os_atomic_set_long(&unknown_runs, 0);
	start_capture();
	obs_queue_task(UNKNOWN_TYPE, unknown_task, NULL, false);
	char error[256];
	int errors = stop_capture(error, sizeof(error));

	for (size_t i = 0; i < QUEUE_COUNT; i++)
		drain(QUEUES[i]);
	assert_int_equal(os_atomic_load_long(&unknown_runs), 0);

	/* Hybrid libobs logs the unknown type; the C reference drops it
	 * without a log (see the header). Nothing else is logged. */
	assert_true(errors == 0 || errors == 1);
	if (errors == 1 && !strstr(error, "unknown task type 99"))
		fail_msg("unexpected error logged: %s", error);
}

/* ------------------------------------------------------------------------- */
/* obs_wait_for_destroy_queue                                                */

struct flag_task {
	volatile bool done;
};

static void slow_flag_task(void *param)
{
	struct flag_task *f = param;
	os_sleep_ms(20);
	os_atomic_set_bool(&f->done, true);
}

struct destroy_wait {
	bool result;
	/* each read right after obs_wait_for_destroy_queue() returned */
	bool graphics_done;
	bool audio_done;
	bool destroy_done;
	struct flag_task *graphics;
	struct flag_task *audio;
	struct flag_task *destroy;
};

static void call_wait_for_destroy_queue(void *param)
{
	struct destroy_wait *w = param;
	w->result = obs_wait_for_destroy_queue();
	w->graphics_done = w->graphics && os_atomic_load_bool(&w->graphics->done);
	w->audio_done = w->audio && os_atomic_load_bool(&w->audio->done);
	w->destroy_done = w->destroy && os_atomic_load_bool(&w->destroy->done);
}

static void wait_for_destroy_queue_runs_pending_tasks(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct flag_task graphics = {0}, audio = {0}, destroy = {0};
	struct destroy_wait w = {.graphics = &graphics, .audio = &audio, .destroy = &destroy};

	obs_queue_task(OBS_TASK_GRAPHICS, slow_flag_task, &graphics, false);
	obs_queue_task(OBS_TASK_AUDIO, slow_flag_task, &audio, false);
	obs_queue_task(OBS_TASK_DESTROY, slow_flag_task, &destroy, false);
	bounded(call_wait_for_destroy_queue, &w, "obs_wait_for_destroy_queue()");

	/* The return value only counts DESTROY tasks the destroy thread starts
	 * after the wait is queued, so it is not pinned here; the next cases
	 * pin it. */
	assert_true(w.graphics_done);
	assert_true(w.audio_done);
	assert_true(w.destroy_done);
}

static void wait_for_destroy_queue_idle_returns_false(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	for (size_t i = 0; i < QUEUE_COUNT; i++)
		drain(QUEUES[i]);

	struct destroy_wait w = {0};
	bounded(call_wait_for_destroy_queue, &w, "obs_wait_for_destroy_queue()");
	assert_false(w.result);
}

struct gate {
	os_event_t *entered;
	os_event_t *release;
	int wait_result;
};

static void gate_task(void *param)
{
	struct gate *g = param;
	os_event_signal(g->entered);
	g->wait_result = os_event_timedwait(g->release, TIMEOUT_MS);
}

static void wait_for_destroy_queue_true_after_destroy_task_ran(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	/* The destroy thread is held by a gate task, a flag task waits behind
	 * it, and obs_wait_for_destroy_queue() is called on a helper thread.
	 * Its destroy wait is queued only after it has waited for the graphics
	 * and audio threads, which the public API does not show, so the gate
	 * is released after a margin; a margin too short for a loaded host
	 * gives false (the flag task ran before the wait was queued), and the
	 * case tries again with a longer one. Every attempt still pins that the
	 * flag task ran before the call returned. */
	static const unsigned margins_ms[] = {50, 200, 1000, 3000};
	bool any_true = false;

	for (size_t m = 0; m < sizeof(margins_ms) / sizeof(margins_ms[0]) && !any_true; m++) {
		struct gate g = {0};
		struct flag_task behind = {0};
		struct destroy_wait w = {.destroy = &behind};
		assert_int_equal(os_event_init(&g.entered, OS_EVENT_TYPE_MANUAL), 0);
		assert_int_equal(os_event_init(&g.release, OS_EVENT_TYPE_MANUAL), 0);

		obs_queue_task(OBS_TASK_DESTROY, gate_task, &g, false);
		if (os_event_timedwait(g.entered, TIMEOUT_MS) != 0) {
			stuck = true;
			fail_msg("the destroy gate task did not start within %d ms", TIMEOUT_MS);
		}
		obs_queue_task(OBS_TASK_DESTROY, slow_flag_task, &behind, false);

		struct helper h;
		helper_start(&h, call_wait_for_destroy_queue, &w);
		os_sleep_ms(margins_ms[m]);
		os_event_signal(g.release);
		helper_join(&h, "obs_wait_for_destroy_queue()");

		assert_int_equal(g.wait_result, 0);
		assert_true(w.destroy_done);
		any_true = w.result;

		os_event_destroy(g.entered);
		os_event_destroy(g.release);
	}

	assert_true(any_true);
}

/* ------------------------------------------------------------------------- */

static int group_setup(void **state)
{
	(void)state;
	const char *display = getenv("DISPLAY");
	if (!display || !*display) {
		fprintf(stderr, "skipped: no display\n");
		return 0;
	}
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 16,
		.base_height = 16,
		.output_width = 16,
		.output_height = 16,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_POINT,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		return 0;
	}

	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "obs_reset_audio failed\n");
		obs_shutdown();
		return -1;
	}

	base_get_log_handler(&old_log_handler, &old_log_param);
	base_set_log_handler(capture_log, NULL);
	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		base_set_log_handler(old_log_handler, old_log_param);
		/* a helper thread is still inside libobs: shutting down under it
		 * would hang or crash instead of reporting the failure */
		if (!stuck)
			obs_shutdown();
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(not_in_task_thread_on_test_thread),
		cmocka_unit_test(queue_without_wait_runs_once_on_type_thread),
		cmocka_unit_test(queue_with_wait_returns_after_task_ran),
		cmocka_unit_test(queue_from_own_thread_runs_inline),
		cmocka_unit_test(unknown_type_queues_nothing),
		cmocka_unit_test(wait_for_destroy_queue_runs_pending_tasks),
		cmocka_unit_test(wait_for_destroy_queue_idle_returns_false),
		cmocka_unit_test(wait_for_destroy_queue_true_after_destroy_task_ran),
		/* last: the test thread stays marked as the UI thread */
		cmocka_unit_test(ui_follows_registered_ui_thread),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
