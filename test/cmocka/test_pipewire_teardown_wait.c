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

static void teardown_waits_only_under_the_lock(void **state)
{
	(void)state;

	pw_init(NULL, NULL);

	obs_pipewire *obs_pw = bzalloc(sizeof(*obs_pw));
	/* A stop before the loop thread has entered the loop can hang: invoke then
	 * runs do_stop on this thread, with nothing to wake the loop thread if it is
	 * already on its way into epoll_wait (#3577). Teardown is what this test
	 * checks, so wait for the signal the loop thread sends once it has entered. */
	struct pw_properties *props = pw_properties_new("thread-loop.start-signal", "true", NULL);
	obs_pw->thread_loop = pw_thread_loop_new("PipeWire teardown test", &props->dict);
	pw_properties_free(props);
	assert_non_null(obs_pw->thread_loop);
	obs_pw->context = pw_context_new(pw_thread_loop_get_loop(obs_pw->thread_loop), NULL, 0);
	assert_non_null(obs_pw->context);
	pw_thread_loop_lock(obs_pw->thread_loop);
	assert_int_equal(pw_thread_loop_start(obs_pw->thread_loop), 0);
	pw_thread_loop_wait(obs_pw->thread_loop);
	pw_thread_loop_unlock(obs_pw->thread_loop);
	obs_pw->streams = g_ptr_array_new();

	obs_pipewire_destroy(obs_pw);

	assert_int_equal(unlocked_waits, 0);
	pw_deinit();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(teardown_waits_only_under_the_lock),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
