/* #3594: failed connection setup must not stop a loop before it enters.
 * Exercise the real production connection path and real PipeWire loop. The
 * wrappers force connection failure and keep an unlocked start's loop locked
 * until stop, exposing the missing startup handshake without hanging CI. */
#include "plugins/linux-pipewire/pipewire.c.h"

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <sys/socket.h>
#include <cmocka.h>

extern void __real_pw_thread_loop_lock(struct pw_thread_loop *loop);
extern void __real_pw_thread_loop_unlock(struct pw_thread_loop *loop);
extern int __real_pw_thread_loop_start(struct pw_thread_loop *loop);
extern void __real_pw_thread_loop_stop(struct pw_thread_loop *loop);

EXPORT void __wrap_pw_thread_loop_lock(struct pw_thread_loop *loop);
EXPORT void __wrap_pw_thread_loop_unlock(struct pw_thread_loop *loop);
EXPORT int __wrap_pw_thread_loop_start(struct pw_thread_loop *loop);
EXPORT void __wrap_pw_thread_loop_stop(struct pw_thread_loop *loop);
EXPORT struct pw_core *__wrap_pw_context_connect_fd(struct pw_context *context, int fd,
						  struct pw_properties *properties, size_t user_data_size);

static _Thread_local int lock_depth;
static struct spa_source *marker;
static bool delayed_entry;
static bool entered;
static int premature_stops;
static int connection_attempts;

static void on_entered(void *data, uint64_t count)
{
	(void)count;
	entered = true;
	pw_thread_loop_signal(data, false);
}

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

int __wrap_pw_thread_loop_start(struct pw_thread_loop *thread_loop)
{
	struct pw_loop *loop = pw_thread_loop_get_loop(thread_loop);

	/* Old code starts unlocked. Keep its loop thread from dispatching until
	 * it tries to stop; a proper locked wait releases this lock itself. */
	delayed_entry = lock_depth == 0;
	if (delayed_entry)
		__real_pw_thread_loop_lock(thread_loop);
	marker = pw_loop_add_event(loop, on_entered, thread_loop);
	assert_non_null(marker);
	assert_int_equal(pw_loop_signal_event(loop, marker), 0);
	return __real_pw_thread_loop_start(thread_loop);
}

struct pw_core *__wrap_pw_context_connect_fd(struct pw_context *context, int fd, struct pw_properties *properties,
					    size_t user_data_size)
{
	(void)context;
	(void)user_data_size;
	connection_attempts++;
	close(fd);
	pw_properties_free(properties);
	errno = EIO;
	return NULL;
}

void __wrap_pw_thread_loop_stop(struct pw_thread_loop *thread_loop)
{
	__real_pw_thread_loop_lock(thread_loop);
	if (!entered)
		premature_stops++;
	if (delayed_entry) {
		__real_pw_thread_loop_unlock(thread_loop);
		delayed_entry = false;
	}
	/* Make cleanup safe even on RED. Assert the missing prerequisite after
	 * joining, rather than deliberately leaving a hung thread behind. */
	while (!entered)
		assert_int_equal(pw_thread_loop_timed_wait(thread_loop, 5), 0);
	pw_loop_destroy_source(pw_thread_loop_get_loop(thread_loop), marker);
	__real_pw_thread_loop_unlock(thread_loop);
	__real_pw_thread_loop_stop(thread_loop);
}

static void failed_connect_waits_for_loop_entry(void **state)
{
	(void)state;
	int fds[2];
	assert_int_equal(socketpair(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0, fds), 0);
	assert_null(obs_pipewire_connect_fd(fds[0]));
	assert_int_equal(fcntl(fds[0], F_GETFD), -1);
	assert_int_equal(errno, EBADF);
	close(fds[1]);
	assert_int_equal(connection_attempts, 1);
	assert_int_equal(lock_depth, 0);
	assert_int_equal(premature_stops, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(failed_connect_waits_for_loop_entry)};
	pw_init(NULL, NULL);
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	pw_deinit();
	return result;
}
