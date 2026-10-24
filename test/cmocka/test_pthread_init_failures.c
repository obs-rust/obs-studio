/* Failed pthread and thread inits that libobs checked with "< 0" (#2152).
 *
 * POSIX pthread functions return 0 or a positive error number, never a
 * negative one. libobs checked some of them, and the os_event_init() result
 * that carries their error, with "< 0", so a failure was never caught:
 *
 * - os_event_init() (threading-posix.c) went on after a failed
 *   pthread_mutex_init() or pthread_cond_init(), and returned 0 with an
 *   event whose lock was not initialized.
 * - obs_context_data_init() went on after a failed init of the rename cache
 *   mutex, so a source, output, encoder, service or canvas was created
 *   around it.
 * - obs_output_create() checked its reconnect event with "< 0", so once
 *   os_event_init() reports the failure, the output was created without
 *   that event.
 * - output_reconnect() and os_inhibit_sleep_set_active() checked
 *   pthread_create() with "< 0": an output whose reconnect thread did not
 *   start stayed "reconnecting" for good, and the screen saver inhibitor
 *   reported success, then joined a thread that never existed.
 *
 * The test interposes pthread_mutex_init(), pthread_cond_init(),
 * pthread_mutex_destroy() and pthread_create() (the executable exports them,
 * see test-graph.json). Each one fails the Nth call on the test's own thread
 * on demand, and records the objects it saw, so the test fails exactly the
 * init it means and checks that nothing leaks (bnum_allocs()).
 *
 * The hybrid libobs's Rust os_event_init() and os_inhibit_sleep_*() use no
 * pthread_mutex_init() or pthread_cond_init(): the event cannot fail there,
 * and the cases check that it still works. Its inhibitor thread is a
 * std::thread, which does go through pthread_create(), and must report the
 * failure the same way.
 *
 * Callers of a failed os_event_init() (#2164). Since the fix above, a
 * failed os_event_init() leaves the event pointer as it was (NULL in these
 * callers), and these callers went on to wait on or signal it:
 *
 * - obs_queue_task() with wait, and obs_wait_for_destroy_queue(): the caller
 *   waited on the NULL event, and the task thread signalled it.
 * - buffered_file_serializer_init() ignored its mutex, event and thread
 *   results and reported success; its I/O thread then waited on the NULL
 *   event, and the free joined a thread that never started.
 * - os_inhibit_sleep_create() made an inhibitor without its stop event, and
 *   that inhibitor then reported success without doing anything.
 *
 * Those cases run in a forked child each, so a crash fails that case alone.
 * The children start OBS themselves: the parent starts no thread before it
 * forks them. A child exits with exit(), not _exit(), so a leak checker
 * loaded into the test also checks it.
 *
 * Linux only (symbol interposition). Public API plus obs-internal.h, to
 * check which mutex and event the interposers saw. Runs against libobs, the
 * hybrid libobs and libobs-c-reference. obs_startup() needs a display; the
 * cases that need it skip without one. */

#define _GNU_SOURCE

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <dlfcn.h>
#include <errno.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <dirent.h>
#include <signal.h>
#include <sys/prctl.h>
#include <sys/wait.h>
#include <unistd.h>

#include <obs-internal.h>
#include <util/bmem.h>
#include <util/buffered-file-serializer.h>
#include <util/platform.h>
#include <util/serializer.h>
#include <util/threading.h>

#define SOURCE_ID "test_pthread_init_failures_source"
#define OUTPUT_ID "test_pthread_init_failures_output"

/* ------------------------------------------------------------------------ */
/* Interposers: fail the Nth call on this thread, record what they saw       */

#define MAX_SEEN 64

struct hook {
	int fail_at; /* 1-based call to fail, 0 = none */
	int calls;
	int failed;
	void *seen[MAX_SEEN];
};

static __thread struct hook mutex_init_hook;
static __thread struct hook cond_init_hook;
static __thread struct hook mutex_destroy_hook;
static __thread struct hook create_hook;

static bool hook_call(struct hook *h, void *object)
{
	if (h->calls < MAX_SEEN)
		h->seen[h->calls] = object;
	h->calls++;
	if (h->fail_at && h->calls == h->fail_at) {
		h->failed++;
		return true;
	}
	return false;
}

static void hook_arm(struct hook *h, int fail_at)
{
	memset(h, 0, sizeof(*h));
	h->fail_at = fail_at;
}

static bool hook_saw(const struct hook *h, const void *object)
{
	for (int i = 0; i < h->calls && i < MAX_SEEN; i++) {
		if (h->seen[i] == object)
			return true;
	}
	return false;
}

static void *real_symbol(void **slot, const char *name)
{
	void *fn = __atomic_load_n(slot, __ATOMIC_ACQUIRE);
	if (!fn) {
		fn = dlsym(RTLD_NEXT, name);
		__atomic_store_n(slot, fn, __ATOMIC_RELEASE);
	}
	return fn;
}

static void *real_mutex_init_fn;
static void *real_cond_init_fn;
static void *real_mutex_destroy_fn;
static void *real_create_fn;

__attribute__((visibility("default"))) int pthread_mutex_init(pthread_mutex_t *mutex, const pthread_mutexattr_t *attr)
{
	if (hook_call(&mutex_init_hook, mutex))
		return EAGAIN;
	int (*real)(pthread_mutex_t *, const pthread_mutexattr_t *);
	void *fn = real_symbol(&real_mutex_init_fn, "pthread_mutex_init");
	memcpy(&real, &fn, sizeof(real));
	return real(mutex, attr);
}

__attribute__((visibility("default"))) int pthread_cond_init(pthread_cond_t *cond, const pthread_condattr_t *attr)
{
	if (hook_call(&cond_init_hook, cond))
		return EAGAIN;
	int (*real)(pthread_cond_t *, const pthread_condattr_t *);
	void *fn = real_symbol(&real_cond_init_fn, "pthread_cond_init");
	memcpy(&real, &fn, sizeof(real));
	return real(cond, attr);
}

__attribute__((visibility("default"))) int pthread_mutex_destroy(pthread_mutex_t *mutex)
{
	hook_call(&mutex_destroy_hook, mutex);
	int (*real)(pthread_mutex_t *);
	void *fn = real_symbol(&real_mutex_destroy_fn, "pthread_mutex_destroy");
	memcpy(&real, &fn, sizeof(real));
	return real(mutex);
}

__attribute__((visibility("default"))) int pthread_create(pthread_t *thread, const pthread_attr_t *attr,
							  void *(*start)(void *), void *arg)
{
	if (hook_call(&create_hook, thread))
		return EAGAIN;
	int (*real)(pthread_t *, const pthread_attr_t *, void *(*)(void *), void *);
	void *fn = real_symbol(&real_create_fn, "pthread_create");
	memcpy(&real, &fn, sizeof(real));
	return real(thread, attr, start, arg);
}

static void disarm_all(void)
{
	hook_arm(&mutex_init_hook, 0);
	hook_arm(&cond_init_hook, 0);
	hook_arm(&mutex_destroy_hook, 0);
	hook_arm(&create_hook, 0);
}

/* ------------------------------------------------------------------------ */
/* os_event_init()                                                           */

static void check_event_init_failure(enum os_event_type type, struct hook *armed)
{
	os_event_t *const untouched = (os_event_t *)(uintptr_t)0x2152;
	os_event_t *event = untouched;

	long before = bnum_allocs();
	int ret = os_event_init(&event, type);
	int calls = armed->calls;
	int failed = armed->failed;
	disarm_all();

	if (!calls) {
		/* The hybrid libobs: a Rust event that uses no pthread init,
		 * and so cannot fail here. It must still work. */
		fprintf(stderr, "os_event_init() made no pthread init call: it cannot fail this way\n");
		assert_int_equal(ret, 0);
		assert_ptr_not_equal(event, untouched);
		assert_non_null(event);
		assert_int_equal(os_event_signal(event), 0);
		assert_int_equal(os_event_try(event), 0);
		os_event_destroy(event);
		assert_int_equal(bnum_allocs(), before);
		return;
	}

	assert_int_equal(failed, 1);
	if (ret == 0 && event != untouched)
		os_event_destroy(event);
	assert_int_not_equal(ret, 0);
	assert_ptr_equal(event, untouched);
	assert_int_equal(bnum_allocs(), before);
}

static void test_event_mutex_init_fails(void **state)
{
	(void)state;

	hook_arm(&mutex_init_hook, 1);
	check_event_init_failure(OS_EVENT_TYPE_AUTO, &mutex_init_hook);
	hook_arm(&mutex_init_hook, 1);
	check_event_init_failure(OS_EVENT_TYPE_MANUAL, &mutex_init_hook);
}

static void test_event_cond_init_fails(void **state)
{
	(void)state;

	for (int i = 0; i < 2; i++) {
		disarm_all();
		hook_arm(&cond_init_hook, 1);
		check_event_init_failure(i ? OS_EVENT_TYPE_MANUAL : OS_EVENT_TYPE_AUTO, &cond_init_hook);
	}

	/* On the cond failure, the mutex initialized before it is destroyed. */
	disarm_all();
	hook_arm(&cond_init_hook, 1);
	os_event_t *event = NULL;
	int ret = os_event_init(&event, OS_EVENT_TYPE_AUTO);
	struct hook inits = mutex_init_hook;
	struct hook destroys = mutex_destroy_hook;
	bool cond_called = cond_init_hook.calls > 0;
	disarm_all();
	if (!cond_called) {
		assert_int_equal(ret, 0);
		os_event_destroy(event);
		return;
	}
	if (ret == 0)
		os_event_destroy(event);
	assert_int_not_equal(ret, 0);
	assert_int_equal(inits.calls, 1);
	assert_int_equal(destroys.calls, 1);
	assert_ptr_equal(destroys.seen[0], inits.seen[0]);
}

/* ------------------------------------------------------------------------ */
/* Context-backed objects                                                    */

static const char *source_name(void *type_data)
{
	(void)type_data;
	return "pthread init failures source";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return bzalloc(1);
}

static void source_destroy(void *data)
{
	bfree(data);
}

static struct obs_source_info test_source = {
	.id = SOURCE_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.get_name = source_name,
	.create = source_create,
	.destroy = source_destroy,
};

static const char *output_name(void *type_data)
{
	(void)type_data;
	return "pthread init failures output";
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	(void)output;
	return bzalloc(1);
}

static void output_destroy(void *data)
{
	bfree(data);
}

static bool output_start(void *data)
{
	(void)data;
	return false;
}

static void output_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
}

static void output_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static struct obs_output_info test_output = {
	.id = OUTPUT_ID,
	.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED,
	.get_name = output_name,
	.create = output_create,
	.destroy = output_destroy,
	.start = output_start,
	.stop = output_stop,
	.encoded_packet = output_packet,
};

static int setup(void **state)
{
	(void)state;
	disarm_all();
	if (!obs_startup("en-US", NULL, NULL))
		return 0;
	obs_register_source(&test_source);
	obs_register_output(&test_output);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	disarm_all();
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

#define REQUIRE_OBS()                                                              \
	do {                                                                       \
		if (!obs_initialized()) {                                          \
			fprintf(stderr, "skipped: obs_startup failed (no display?)\n"); \
			skip();                                                    \
		}                                                                  \
	} while (0)

static obs_source_t *create_source(bool private)
{
	return private ? obs_source_create_private(SOURCE_ID, "rename cache", NULL)
		       : obs_source_create(SOURCE_ID, "rename cache", NULL, NULL);
}

static void test_source_rename_cache_mutex_init_fails(void **state)
{
	(void)state;
	REQUIRE_OBS();

	for (int i = 0; i < 2; i++) {
		bool private = i == 1;

		/* The first mutex a source create initializes is its context's
		 * rename cache mutex: obs_context_data_init() starts with it. */
		hook_arm(&mutex_init_hook, 0);
		obs_source_t *source = create_source(private);
		struct hook inits = mutex_init_hook;
		disarm_all();
		assert_non_null(source);
		assert_true(inits.calls > 0);
		assert_ptr_equal(inits.seen[0], &source->context.rename_cache_mutex);
		obs_source_release(source);
		obs_wait_for_destroy_queue();

		/* Fail that one init alone. */
		long before = bnum_allocs();
		hook_arm(&mutex_init_hook, 1);
		source = create_source(private);
		int failed = mutex_init_hook.failed;
		disarm_all();

		bool created = source != NULL;
		obs_source_release(source);
		obs_wait_for_destroy_queue();
		assert_int_equal(failed, 1);
		assert_false(created);
		assert_int_equal(bnum_allocs(), before);
	}
}

static void test_output_reconnect_event_init_fails(void **state)
{
	(void)state;
	REQUIRE_OBS();

	/* The last event obs_output_create() initializes is the reconnect
	 * stop event. */
	hook_arm(&cond_init_hook, 0);
	obs_output_t *output = obs_output_create(OUTPUT_ID, "reconnect event", NULL, NULL);
	struct hook conds = cond_init_hook;
	disarm_all();
	assert_non_null(output);

	if (!conds.calls) {
		/* The hybrid libobs: its events use no pthread_cond_init(). */
		fprintf(stderr, "obs_output_create() made no pthread_cond_init() call: its events cannot fail\n");
		assert_non_null(output->reconnect_stop_event);
		obs_output_release(output);
		return;
	}

	/* struct os_event_data starts with its mutex, then its cond. */
	uintptr_t event = (uintptr_t)output->reconnect_stop_event;
	uintptr_t last_cond = (uintptr_t)conds.seen[conds.calls - 1];
	assert_true(last_cond > event && last_cond - event <= sizeof(pthread_mutex_t) + 16);
	obs_output_release(output);
	obs_wait_for_destroy_queue();

	long before = bnum_allocs();
	hook_arm(&cond_init_hook, conds.calls);
	output = obs_output_create(OUTPUT_ID, "reconnect event", NULL, NULL);
	int failed = cond_init_hook.failed;
	disarm_all();

	bool created = output != NULL;
	bool has_event = created && output->reconnect_stop_event;
	obs_output_release(output);
	obs_wait_for_destroy_queue();
	assert_int_equal(failed, 1);
	if (created && !has_event)
		fprintf(stderr, "obs_output_create() returned an output without its reconnect event\n");
	assert_false(created);
	assert_int_equal(bnum_allocs(), before);
}

static void test_output_reconnect_thread_create_fails(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_output_t *output = obs_output_create(OUTPUT_ID, "reconnect thread", NULL, NULL);
	assert_non_null(output);
	assert_false(obs_output_reconnecting(output));

	/* A disconnect with reconnecting on (the default) starts the
	 * reconnect thread; make that fail. */
	hook_arm(&create_hook, 1);
	obs_output_signal_stop(output, OBS_OUTPUT_DISCONNECTED);
	int calls = create_hook.calls;
	int failed = create_hook.failed;
	disarm_all();

	bool reconnecting = obs_output_reconnecting(output);
	obs_output_release(output);
	assert_int_equal(calls, 1);
	assert_int_equal(failed, 1);
	assert_false(reconnecting);
}

/* ------------------------------------------------------------------------ */
/* os_inhibit_sleep_set_active()                                             */

static void test_inhibit_thread_create_fails(void **state)
{
	(void)state;

	/* With no session bus, the inhibitor falls back to a thread that
	 * runs xdg-screensaver. A failed bus connection is not cached, so the
	 * cases after this one connect as usual. */
	const char *bus = getenv("DBUS_SESSION_BUS_ADDRESS");
	char *saved_bus = bus ? bstrdup(bus) : NULL;
	setenv("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent/pthread-init-failures", 1);
	os_inhibit_t *info = os_inhibit_sleep_create("pthread init failures");
	if (saved_bus)
		setenv("DBUS_SESSION_BUS_ADDRESS", saved_bus, 1);
	else
		unsetenv("DBUS_SESSION_BUS_ADDRESS");
	bfree(saved_bus);
	if (!info) {
		fprintf(stderr, "skipped: no sleep inhibitor\n");
		skip();
	}

	hook_arm(&create_hook, 1);
	bool active = os_inhibit_sleep_set_active(info, true);
	int calls = create_hook.calls;
	disarm_all();

	if (!calls) {
		/* A desktop inhibitor (portal or session D-Bus) needs no
		 * thread; the test reached one anyway. */
		os_inhibit_sleep_destroy(info);
		fprintf(stderr, "skipped: the inhibitor uses no thread here\n");
		skip();
	}

	if (active) {
		/* The thread never started: deactivating would join it.
		 * Leave the inhibitor behind. */
		fprintf(stderr, "os_inhibit_sleep_set_active() reported success without its thread\n");
		fail();
	}
	os_inhibit_sleep_destroy(info);
}

/* ------------------------------------------------------------------------ */
/* os_inhibit_sleep_create(): its stop event fails                           */

static void test_inhibit_event_init_fails(void **state)
{
	(void)state;

	/* As above: no session bus, so the inhibitor takes the fallback that
	 * makes the stop event. The last cond init the create makes is that
	 * event's. */
	const char *bus = getenv("DBUS_SESSION_BUS_ADDRESS");
	char *saved_bus = bus ? bstrdup(bus) : NULL;
	setenv("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent/pthread-init-failures", 1);

	hook_arm(&cond_init_hook, 0);
	os_inhibit_t *info = os_inhibit_sleep_create("event init failures");
	int conds = cond_init_hook.calls;
	disarm_all();

	os_inhibit_t *failed_info = NULL;
	int failed = 0;
	long before = 0;
	long after = 0;
	if (info && conds) {
		os_inhibit_sleep_destroy(info);
		info = NULL;
		before = bnum_allocs();
		hook_arm(&cond_init_hook, conds);
		failed_info = os_inhibit_sleep_create("event init failures");
		failed = cond_init_hook.failed;
		disarm_all();
		after = bnum_allocs();
	}

	if (saved_bus)
		setenv("DBUS_SESSION_BUS_ADDRESS", saved_bus, 1);
	else
		unsetenv("DBUS_SESSION_BUS_ADDRESS");
	bfree(saved_bus);

	if (!conds) {
		/* The hybrid libobs: its stop event uses no pthread init. */
		fprintf(stderr, "os_inhibit_sleep_create() made no pthread_cond_init() call: its event cannot fail\n");
		os_inhibit_sleep_destroy(info);
		return;
	}
	if (failed_info) {
		fprintf(stderr, "os_inhibit_sleep_create() returned an inhibitor without its stop event\n");
		os_inhibit_sleep_destroy(failed_info);
	}
	assert_int_equal(failed, 1);
	assert_null(failed_info);
	assert_int_equal(after, before);
}

/* ------------------------------------------------------------------------ */
/* Forked cases                                                              */

#define CHILD_SKIP 77
#define CHILD_TIMEOUT_S 60

#define CHILD_CHECK(cond)                                                               \
	do {                                                                            \
		if (!(cond)) {                                                          \
			fprintf(stderr, "%s:%d: check failed: %s\n", __FILE__, __LINE__, #cond); \
			return 1;                                                       \
		}                                                                       \
	} while (0)

/* Runs fn in a forked child. Returns its exit code, or 128 plus the signal
 * that ended it (a crash, or SIGALRM for a child that did not end in
 * time). */
static int run_in_child(int (*fn)(void *), void *arg, const char *what)
{
	fflush(stdout);
	fflush(stderr);
	pid_t pid = fork();
	assert_true(pid >= 0);
	if (pid == 0) {
		/* cmocka traps these and would go on running the cases in
		 * the child; let a crash end the child instead. */
		signal(SIGSEGV, SIG_DFL);
		signal(SIGBUS, SIG_DFL);
		signal(SIGABRT, SIG_DFL);
		signal(SIGILL, SIG_DFL);
		signal(SIGFPE, SIG_DFL);
		/* an expected crash should not leave a core dump behind */
		prctl(PR_SET_DUMPABLE, 0);
		alarm(CHILD_TIMEOUT_S);
		int ret = fn(arg);
		fflush(stdout);
		fflush(stderr);
		exit(ret);
	}

	int status = 0;
	assert_int_equal(waitpid(pid, &status, 0), pid);
	if (WIFSIGNALED(status)) {
		fprintf(stderr, "%s: crashed with signal %d\n", what, WTERMSIG(status));
		return 128 + WTERMSIG(status);
	}
	assert_true(WIFEXITED(status));
	if (WEXITSTATUS(status) != 0 && WEXITSTATUS(status) != CHILD_SKIP)
		fprintf(stderr, "%s: failed (exit %d)\n", what, WEXITSTATUS(status));
	return WEXITSTATUS(status);
}

static void expect_child(int (*fn)(void *), void *arg, const char *what)
{
	int ret = run_in_child(fn, arg, what);
	if (ret == CHILD_SKIP)
		skip();
	assert_int_equal(ret, 0);
}

static int child_start_obs(void)
{
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return CHILD_SKIP;
	}
	return 0;
}

static bool child_reset_av(void)
{
	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai))
		return false;

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	return obs_reset_video(&ovi) == OBS_VIDEO_SUCCESS;
}

/* ------------------------------------------------------------------------ */
/* obs_queue_task() with wait, obs_wait_for_destroy_queue()                  */

struct wait_task {
	enum obs_task_type type;
	long runs;
	bool on_task_thread;
};

static void wait_task_run(void *param)
{
	struct wait_task *t = param;
	/* Long enough that a caller that did not wait returns first. */
	os_sleep_ms(100);
	t->on_task_thread = obs_in_task_thread(t->type);
	os_atomic_inc_long(&t->runs);
}

static int queue_task_wait_case(enum obs_task_type type)
{
	struct wait_task t = {.type = type};

	/* Once first, so the queue has room for the task already. */
	struct wait_task warm = {.type = type};
	obs_queue_task(type, wait_task_run, &warm, true);
	CHILD_CHECK(os_atomic_load_long(&warm.runs) == 1);

	/* The first mutex init obs_queue_task() makes on the caller's thread
	 * is its wait event's. */
	long before = bnum_allocs();
	hook_arm(&mutex_init_hook, 1);
	obs_queue_task(type, wait_task_run, &t, true);
	struct hook inits = mutex_init_hook;
	disarm_all();
	long after = bnum_allocs();
	long runs_on_return = os_atomic_load_long(&t.runs);

	if (!inits.calls)
		fprintf(stderr, "obs_queue_task() made no pthread_mutex_init() call: its event cannot fail\n");
	else
		CHILD_CHECK(inits.failed == 1);

	/* It ran once, on its own thread, before the call returned. */
	CHILD_CHECK(runs_on_return == 1);
	CHILD_CHECK(t.on_task_thread);
	/* Only the destroy queue is idle here: other threads allocate. */
	if (type == OBS_TASK_DESTROY)
		CHILD_CHECK(after == before);

	/* And it does not run again later. */
	obs_wait_for_destroy_queue();
	os_sleep_ms(150);
	CHILD_CHECK(os_atomic_load_long(&t.runs) == 1);
	return 0;
}

static int child_queue_task_wait_destroy(void *arg)
{
	(void)arg;
	int ret = child_start_obs();
	if (ret)
		return ret;
	ret = queue_task_wait_case(OBS_TASK_DESTROY);
	obs_shutdown();
	return ret;
}

static int child_queue_task_wait_graphics(void *arg)
{
	(void)arg;
	int ret = child_start_obs();
	if (ret)
		return ret;
	if (!child_reset_av()) {
		fprintf(stderr, "skipped: no video (no display?)\n");
		obs_shutdown();
		return CHILD_SKIP;
	}
	ret = queue_task_wait_case(OBS_TASK_GRAPHICS);
	if (!ret)
		ret = queue_task_wait_case(OBS_TASK_AUDIO);
	obs_shutdown();
	return ret;
}

static void test_queue_task_wait_event_init_fails(void **state)
{
	(void)state;
	expect_child(child_queue_task_wait_destroy, NULL, "obs_queue_task(OBS_TASK_DESTROY, wait)");
}

static void test_queue_task_wait_graphics_event_init_fails(void **state)
{
	(void)state;
	expect_child(child_queue_task_wait_graphics, NULL, "obs_queue_task(OBS_TASK_GRAPHICS/AUDIO, wait)");
}

static int child_wait_for_destroy_queue(void *arg)
{
	(void)arg;
	int ret = child_start_obs();
	if (ret)
		return ret;
	if (!child_reset_av()) {
		fprintf(stderr, "skipped: no video (no display?)\n");
		obs_shutdown();
		return CHILD_SKIP;
	}

	/* Without video and audio it waits on the destroy queue alone, which
	 * needs no event. With them, it waits for one pass of the graphics
	 * and audio threads first, so their tasks queued before have run. */
	struct wait_task gfx = {.type = OBS_TASK_GRAPHICS};
	struct wait_task audio = {.type = OBS_TASK_AUDIO};
	obs_queue_task(OBS_TASK_GRAPHICS, wait_task_run, &gfx, false);
	obs_queue_task(OBS_TASK_AUDIO, wait_task_run, &audio, false);

	hook_arm(&mutex_init_hook, 1);
	obs_wait_for_destroy_queue();
	struct hook inits = mutex_init_hook;
	disarm_all();
	long gfx_runs = os_atomic_load_long(&gfx.runs);
	long audio_runs = os_atomic_load_long(&audio.runs);

	if (!inits.calls) {
		fprintf(stderr,
			"obs_wait_for_destroy_queue() made no pthread_mutex_init() call: its event cannot fail\n");
	} else if (inits.failed != 1) {
		fprintf(stderr, "obs_wait_for_destroy_queue(): the event init did not fail\n");
		ret = 1;
	}
	if (gfx_runs != 1 || audio_runs != 1) {
		fprintf(stderr,
			"obs_wait_for_destroy_queue() returned before the graphics (%ld) and audio (%ld) tasks\n",
			gfx_runs, audio_runs);
		ret = 1;
	}
	obs_shutdown();
	return ret;
}

static void test_wait_for_destroy_queue_event_init_fails(void **state)
{
	(void)state;
	expect_child(child_wait_for_destroy_queue, NULL, "obs_wait_for_destroy_queue()");
}

/* ------------------------------------------------------------------------ */
/* buffered_file_serializer_init()                                           */

struct serializer_case {
	const char *name;
	struct hook *hook;
	int fail_at;
	char path[512];
};

static int count_open_fds(void)
{
	DIR *dir = opendir("/proc/self/fd");
	if (!dir)
		return -1;
	int n = 0;
	while (readdir(dir))
		n++;
	closedir(dir);
	return n;
}

static int child_serializer_init_fails(void *arg)
{
	struct serializer_case *c = arg;
	struct serializer s = {0};

	int fds = count_open_fds();
	long before = bnum_allocs();
	hook_arm(c->hook, c->fail_at);
	bool ok = buffered_file_serializer_init(&s, c->path, 0, 0);
	int failed = c->hook->failed;
	disarm_all();

	if (ok) {
		/* Free what it made: with a NULL event, this may crash. */
		fprintf(stderr, "buffered_file_serializer_init() reported success with %s %d failing\n", c->name,
			c->fail_at);
		buffered_file_serializer_free(&s);
		return 1;
	}
	CHILD_CHECK(failed == 1);
	CHILD_CHECK(s.data == NULL);
	CHILD_CHECK(bnum_allocs() == before);
	/* The file it opened is closed again. */
	CHILD_CHECK(count_open_fds() == fds);
	return 0;
}

static void test_serializer_init_failures(void **state)
{
	(void)state;

	struct serializer_case c = {0};
	const char *tmp = getenv("TMPDIR");
	snprintf(c.path, sizeof(c.path), "%s/obs-pthread-init-failures-%ld.bin", tmp && *tmp ? tmp : "/tmp",
		 (long)getpid());

	/* A working open, counting the inits it makes on this thread. */
	struct serializer s = {0};
	hook_arm(&mutex_init_hook, 0);
	hook_arm(&cond_init_hook, 0);
	hook_arm(&create_hook, 0);
	assert_true(buffered_file_serializer_init(&s, c.path, 0, 0));
	int mutexes = mutex_init_hook.calls;
	int conds = cond_init_hook.calls;
	int creates = create_hook.calls;
	disarm_all();
	buffered_file_serializer_free(&s);

	fprintf(stderr, "buffered_file_serializer_init() made %d mutex, %d cond and %d thread inits\n", mutexes, conds,
		creates);
	/* C: its data mutex and its two events (a mutex and a cond each),
	 * then its I/O thread. The hybrid's Rust serializer starts a thread
	 * and makes no pthread lock init. */
	assert_int_equal(creates, 1);
	assert_true(mutexes == 0 || mutexes == 3);
	assert_true(conds == 0 || conds == 2);

	int failures = 0;
	struct {
		const char *name;
		struct hook *hook;
		int calls;
	} inits[] = {
		{"pthread_mutex_init()", &mutex_init_hook, mutexes},
		{"pthread_cond_init()", &cond_init_hook, conds},
		{"pthread_create()", &create_hook, creates},
	};
	for (size_t i = 0; i < sizeof(inits) / sizeof(inits[0]); i++) {
		for (int n = 1; n <= inits[i].calls; n++) {
			char what[128];
			snprintf(what, sizeof(what), "buffered_file_serializer_init() with %s %d of %d failing",
				 inits[i].name, n, inits[i].calls);
			c.name = inits[i].name;
			c.hook = inits[i].hook;
			c.fail_at = n;
			if (run_in_child(child_serializer_init_fails, &c, what) != 0)
				failures++;
		}
	}
	unlink(c.path);
	assert_int_equal(failures, 0);
}

#define CASE(fn) cmocka_unit_test(fn)
#define OBS_CASE(fn) cmocka_unit_test_setup_teardown(fn, setup, teardown)

int main(void)
{
	const struct CMUnitTest tests[] = {
		/* First: they fork, and the parent has started no thread. */
		CASE(test_queue_task_wait_event_init_fails),
		CASE(test_queue_task_wait_graphics_event_init_fails),
		CASE(test_wait_for_destroy_queue_event_init_fails),
		CASE(test_serializer_init_failures),
		/* Then, before anything in this process connects to the
		 * session bus. */
		CASE(test_inhibit_thread_create_fails),
		CASE(test_inhibit_event_init_fails),
		CASE(test_event_mutex_init_fails),
		CASE(test_event_cond_init_fails),
		OBS_CASE(test_source_rename_cache_mutex_init_fails),
		OBS_CASE(test_output_reconnect_event_init_fails),
		OBS_CASE(test_output_reconnect_thread_create_fails),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
