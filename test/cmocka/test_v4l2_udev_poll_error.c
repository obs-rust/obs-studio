#include <errno.h>
#include <pthread.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <time.h>
#include <unistd.h>
#include <setjmp.h>
#include <cmocka.h>

/* The udev hot-plug thread of linux-v4l2 (obs-rust/obs-studio#1280) polls
 * the udev netlink fd and a stop eventfd. When an fd reports POLLERR,
 * POLLHUP or POLLNVAL instead of POLLIN, poll() returns at once on every
 * call, and the loop used to `continue` forever: a busy spin on one core.
 * The test runs the plugin's real loop (v4l2-udev.c.h) on a pipe whose
 * writer is closed, so the read end reports POLLHUP, and expects the loop
 * to give up after a few iterations instead of spinning until the stop
 * event. The loop runs in a thread and the test stops it after a deadline,
 * so a regression fails the test instead of hanging it. */

#include "plugins/linux-v4l2/v4l2-udev.c.h"

struct loop_run {
	int fd;
	int stop_fd;
	os_event_t *stop;
	unsigned long iterations;
	volatile int done;
};

static void never_ready(void *ctx)
{
	UNUSED_PARAMETER(ctx);
	fail_msg("on_ready called for an fd that reported only an error");
}

static void *run_loop(void *vrun)
{
	struct loop_run *run = vrun;
	run->iterations = udev_monitor_loop(run->fd, run->stop_fd, run->stop, never_ready, NULL);
	run->done = 1;
	return NULL;
}

static void run_with_fd(int fd, int stop_fd)
{
	struct loop_run run = {.fd = fd, .stop_fd = stop_fd};
	pthread_t thread;

	assert_int_equal(os_event_init(&run.stop, OS_EVENT_TYPE_MANUAL), 0);
	assert_int_equal(pthread_create(&thread, NULL, run_loop, &run), 0);

	struct timespec nap = {0, 1000000};
	for (int i = 0; i < 500 && !run.done; i++)
		nanosleep(&nap, NULL);
	int returned = run.done;

	/* stop a spinning loop so the test ends either way */
	os_event_signal(run.stop);
	pthread_join(thread, NULL);
	os_event_destroy(run.stop);

	/* Returning within the 500 x 1 ms wait and at most two polls prove the
	 * loop stopped instead of spinning; a wall-clock bound only flakes on a
	 * loaded runner (#2198). */
	assert_true(returned);
	assert_true(run.iterations <= 2);
}

static void monitor_fd_hangup_ends_loop(void **state)
{
	UNUSED_PARAMETER(state);
	int p[2];
	int stop_fd = eventfd(0, EFD_CLOEXEC);

	assert_true(stop_fd >= 0);
	assert_int_equal(pipe(p), 0);
	close(p[1]); /* read end now reports POLLHUP on every poll */

	run_with_fd(p[0], stop_fd);

	close(p[0]);
	close(stop_fd);
}

static void monitor_fd_invalid_ends_loop(void **state)
{
	UNUSED_PARAMETER(state);
	int p[2];
	int stop_fd = eventfd(0, EFD_CLOEXEC);

	assert_true(stop_fd >= 0);
	assert_int_equal(pipe(p), 0);
	close(p[0]);
	close(p[1]); /* p[0] is now closed: POLLNVAL */

	run_with_fd(p[0], stop_fd);

	close(stop_fd);
}

static void stop_fd_hangup_ends_loop(void **state)
{
	UNUSED_PARAMETER(state);
	int mon[2], stop[2];

	assert_int_equal(pipe(mon), 0); /* idle monitor fd */
	assert_int_equal(pipe(stop), 0);
	close(stop[1]);

	run_with_fd(mon[0], stop[0]);

	close(mon[0]);
	close(mon[1]);
	close(stop[0]);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(monitor_fd_hangup_ends_loop),
		cmocka_unit_test(monitor_fd_invalid_ends_loop),
		cmocka_unit_test(stop_fd_hangup_ends_loop),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
