/* linux-pipewire: tearing down a PipeWire connection never waits on its
 * thread loop without the loop lock (#2431). teardown_pipewire called
 * pw_thread_loop_wait() unlocked before stopping the loop; PipeWire 1.x
 * asserts "'loop->recurse > 0' failed" there, which the AppImage
 * saved-profile test caught at shutdown on Arch.
 *
 * pipewire.c is included so the test can build an obs_pipewire around a
 * running thread loop without a PipeWire daemon. pw_thread_loop_lock,
 * _unlock and _wait are wrapped (--wrap) to count waits made without the
 * lock. */

#include "plugins/linux-pipewire/pipewire.c.h"

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

extern void __real_pw_thread_loop_lock(struct pw_thread_loop *loop);
extern void __real_pw_thread_loop_unlock(struct pw_thread_loop *loop);
extern void __real_pw_thread_loop_wait(struct pw_thread_loop *loop);

EXPORT void __wrap_pw_thread_loop_lock(struct pw_thread_loop *loop);
EXPORT void __wrap_pw_thread_loop_unlock(struct pw_thread_loop *loop);
EXPORT void __wrap_pw_thread_loop_wait(struct pw_thread_loop *loop);

static __thread int lock_depth;
static int unlocked_waits;

void __wrap_pw_thread_loop_lock(struct pw_thread_loop *loop)
{
	__real_pw_thread_loop_lock(loop);
	lock_depth++;
}

void __wrap_pw_thread_loop_unlock(struct pw_thread_loop *loop)
{
	lock_depth--;
	__real_pw_thread_loop_unlock(loop);
}

/* An unlocked wait is counted and not made: PipeWire only logs it and
 * returns, and a locked one with nothing to signal it would hang. */
void __wrap_pw_thread_loop_wait(struct pw_thread_loop *loop)
{
	if (lock_depth <= 0) {
		unlocked_waits++;
		return;
	}
	__real_pw_thread_loop_wait(loop);
}

/* Starts the loop and waits, under the loop lock, until the loop thread has
 * entered the loop. A stop before that can hang: invoke then runs do_stop on
 * this thread, with nothing to wake the loop thread if it is already on its
 * way into epoll_wait (#3577). The wait is bounded, so a loop thread that
 * never says it has entered fails the test instead of hanging it.
 *
 * An event source's callback runs only on the loop thread, from inside the
 * loop, on every PipeWire version this test builds for, so that is what the
 * wait is for; thread-loop.start-signal is ignored before 0.3.77 (#3596).
 * The lock is held from before the start, so the callback cannot run before
 * the wait. */
struct loop_entered {
	struct pw_thread_loop *thread_loop;
	bool entered;
};

/* Runs on the loop thread, under the loop lock, from inside the loop. */
static void on_loop_entered(void *data, uint64_t count)
{
	struct loop_entered *le = data;

	(void)count;
	le->entered = true;
	pw_thread_loop_signal(le->thread_loop, false);
}

static bool start_and_wait_until_entered(struct pw_thread_loop *thread_loop)
{
	struct pw_loop *loop = pw_thread_loop_get_loop(thread_loop);
	struct loop_entered le = {.thread_loop = thread_loop};
	struct spa_source *event = pw_loop_add_event(loop, on_loop_entered, &le);

	if (!event)
		return false;
	pw_thread_loop_lock(thread_loop);
	bool ok = pw_thread_loop_start(thread_loop) == 0 && pw_loop_signal_event(loop, event) == 0;
	while (ok && !le.entered)
		ok = pw_thread_loop_timed_wait(thread_loop, 5) == 0;
	pw_loop_destroy_source(loop, event);
	pw_thread_loop_unlock(thread_loop);
	return le.entered;
}

static void check_teardown(bool start_signal)
{
	pw_init(NULL, NULL);

	obs_pipewire *obs_pw = bzalloc(sizeof(*obs_pw));
	struct pw_properties *props = start_signal ? pw_properties_new("thread-loop.start-signal", "true", NULL) : NULL;
	obs_pw->thread_loop = pw_thread_loop_new("PipeWire teardown test", props ? &props->dict : NULL);
	pw_properties_free(props);
	assert_non_null(obs_pw->thread_loop);
	obs_pw->context = pw_context_new(pw_thread_loop_get_loop(obs_pw->thread_loop), NULL, 0);
	assert_non_null(obs_pw->context);
	assert_true(start_and_wait_until_entered(obs_pw->thread_loop));
	obs_pw->streams = g_ptr_array_new();

	obs_pipewire_destroy(obs_pw);

	assert_int_equal(unlocked_waits, 0);
	pw_deinit();
}

static void teardown_waits_only_under_the_lock(void **state)
{
	(void)state;
	check_teardown(true);
}

/* PipeWire reads thread-loop.start-signal only from 0.3.77 on, and this test
 * builds for 0.3.60 and newer: on older versions the loop runs as if the
 * property were not set, which this case does on every version (#3596). */
static void teardown_waits_only_under_the_lock_without_start_signal(void **state)
{
	(void)state;
	check_teardown(false);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(teardown_waits_only_under_the_lock),
		cmocka_unit_test(teardown_waits_only_under_the_lock_without_start_signal),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
