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

#include <obs-internal.h>
#include <util/bmem.h>
#include <util/platform.h>
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

#define CASE(fn) cmocka_unit_test(fn)
#define OBS_CASE(fn) cmocka_unit_test_setup_teardown(fn, setup, teardown)

int main(void)
{
	const struct CMUnitTest tests[] = {
		/* First: before anything in the process connects to the
		 * session bus. */
		CASE(test_inhibit_thread_create_fails),
		CASE(test_event_mutex_init_fails),
		CASE(test_event_cond_init_fails),
		OBS_CASE(test_source_rename_cache_mutex_init_fails),
		OBS_CASE(test_output_reconnect_event_init_fails),
		OBS_CASE(test_output_reconnect_thread_create_fails),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
