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
extern void __real_pw_thread_loop_wait(struct pw_thread_loop *loop);
extern struct pw_core *__real_pw_context_connect_fd(struct pw_context *context, int fd,
						    struct pw_properties *properties, size_t user_data_size);

EXPORT void __wrap_pw_thread_loop_lock(struct pw_thread_loop *loop);
EXPORT void __wrap_pw_thread_loop_unlock(struct pw_thread_loop *loop);
EXPORT int __wrap_pw_thread_loop_start(struct pw_thread_loop *loop);
EXPORT void __wrap_pw_thread_loop_stop(struct pw_thread_loop *loop);
EXPORT void __wrap_pw_thread_loop_wait(struct pw_thread_loop *loop);
EXPORT struct pw_core *__wrap_pw_context_connect_fd(struct pw_context *context, int fd,
						    struct pw_properties *properties, size_t user_data_size);

static _Thread_local int lock_depth;
static struct spa_source *marker;
static bool delayed_entry;
static bool entered;
static int premature_stops;
static int connection_attempts;
static bool fail_start;
static bool real_connect;

static int setup(void **state)
{
	(void)state;
	marker = NULL;
	delayed_entry = entered = false;
	premature_stops = connection_attempts = 0;
	fail_start = real_connect = false;
	return 0;
}

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

	if (fail_start)
		return -EAGAIN;
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

void __wrap_pw_thread_loop_wait(struct pw_thread_loop *thread_loop)
{
	__real_pw_thread_loop_wait(thread_loop);
	/* The production and marker events may be dispatched in either order.
	 * Ensure the independent marker has run before returning the lock. */
	while (!entered)
		assert_int_equal(pw_thread_loop_timed_wait(thread_loop, 5), 0);
}

struct pw_core *__wrap_pw_context_connect_fd(struct pw_context *context, int fd, struct pw_properties *properties,
					     size_t user_data_size)
{
	(void)context;
	(void)user_data_size;
	connection_attempts++;
	if (real_connect)
		return __real_pw_context_connect_fd(
			context, fd, pw_properties_new(PW_KEY_PROTOCOL, "obs-test-nonexistent-protocol", NULL),
			user_data_size);
	close(fd);
	pw_properties_free(properties);
	errno = EIO;
	return NULL;
}

void __wrap_pw_thread_loop_stop(struct pw_thread_loop *thread_loop)
{
	if (!marker) {
		__real_pw_thread_loop_stop(thread_loop);
		return;
	}
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

static void start_failure_closes_portal_fd(void **state)
{
	(void)state;
	int fds[2];
	assert_int_equal(socketpair(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0, fds), 0);
	fail_start = true;
	assert_null(obs_pipewire_connect_fd(fds[0]));
	assert_int_equal(fcntl(fds[0], F_GETFD), -1);
	assert_int_equal(errno, EBADF);
	close(fds[1]);
	assert_int_equal(connection_attempts, 0);
	assert_int_equal(lock_depth, 0);
}

static void real_unsupported_protocol_fails_cleanly(void **state)
{
	(void)state;
	int fds[2];
	assert_int_equal(socketpair(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0, fds), 0);
	real_connect = true;
	assert_null(obs_pipewire_connect_fd(fds[0]));
	close(fds[1]);
	assert_int_equal(connection_attempts, 1);
	assert_int_equal(premature_stops, 0);
	assert_int_equal(lock_depth, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(failed_connect_waits_for_loop_entry, setup),
		cmocka_unit_test_setup(start_failure_closes_portal_fd, setup),
		cmocka_unit_test_setup(real_unsupported_protocol_fails_cleanly, setup),
	};
	pw_init(NULL, NULL);
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	pw_deinit();
	return result;
}
