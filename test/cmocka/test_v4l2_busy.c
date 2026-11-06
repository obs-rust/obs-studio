#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <signal.h>
#include <unistd.h>
#include <setjmp.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <cmocka.h>

#include <util/dstr.h>

#include "v4l2-busy.h"

/* A camera another process has open fails with EBUSY, and the source stayed
 * dead with only a log line (obs-rust/obs-studio#799: a stock OBS left
 * running held /dev/video0, so OBS-Studio-Rust showed no camera). The source
 * must name the holder, offer to close it, and retry until the device is
 * free. The device here is a plain file: holder lookup works on any path, so
 * neither a camera nor v4l2loopback is needed. */

struct fixture {
	char dir[64];
	char dev[128];
	/* helper still running, killed in teardown if an assertion failed */
	pid_t helper;
};

static void path_join(char *out, size_t size, const char *a, const char *b)
{
	snprintf(out, size, "%s/%s", a, b);
}

static void write_file(const char *path, const char *text)
{
	FILE *f = fopen(path, "w");
	assert_non_null(f);
	fputs(text, f);
	fclose(f);
}

static int setup(void **state)
{
	struct fixture *fx = calloc(1, sizeof(*fx));
	/* relative to the ctest working directory */
	strcpy(fx->dir, "v4l2_busy_XXXXXX");
	if (!mkdtemp(fx->dir))
		return -1;
	path_join(fx->dev, sizeof(fx->dev), fx->dir, "video0");
	write_file(fx->dev, "");
	*state = fx;
	return 0;
}

static int teardown(void **state)
{
	struct fixture *fx = *state;
	if (fx->helper > 0) {
		kill(fx->helper, SIGKILL);
		waitpid(fx->helper, NULL, 0);
	}
	char cmd[128];
	snprintf(cmd, sizeof(cmd), "rm -rf '%s'", fx->dir);
	int ret = system(cmd);
	free(fx);
	return ret == 0 ? 0 : -1;
}

/* <dir>/proc/<pid>/{comm,fd/<n>} with fd/<n> linking to target */
static void fake_process(const char *dir, int pid, const char *comm, int fd, const char *target)
{
	char path[256];
	snprintf(path, sizeof(path), "%s/proc", dir);
	mkdir(path, 0755);
	snprintf(path, sizeof(path), "%s/proc/%d", dir, pid);
	assert_int_equal(mkdir(path, 0755), 0);
	snprintf(path, sizeof(path), "%s/proc/%d/comm", dir, pid);
	char line[80];
	snprintf(line, sizeof(line), "%s\n", comm);
	write_file(path, line);
	snprintf(path, sizeof(path), "%s/proc/%d/fd", dir, pid);
	assert_int_equal(mkdir(path, 0755), 0);
	snprintf(path, sizeof(path), "%s/proc/%d/fd/%d", dir, pid, fd);
	char abs_target[PATH_MAX];
	assert_non_null(realpath(target, abs_target));
	assert_int_equal(symlink(abs_target, path), 0);
}

static void busy_errno_is_ebusy_only(void **state)
{
	(void)state;
	assert_true(v4l2_busy_errno(EBUSY));
	assert_false(v4l2_busy_errno(ENOENT));
	assert_false(v4l2_busy_errno(EACCES));
	assert_false(v4l2_busy_errno(0));
}

static void finds_the_holder_in_a_proc_tree(void **state)
{
	struct fixture *fx = *state;
	char other[128], proc[128];
	path_join(other, sizeof(other), fx->dir, "other");
	write_file(other, "");
	path_join(proc, sizeof(proc), fx->dir, "proc");

	fake_process(fx->dir, 4242, ".obs-wrapped", 98, fx->dev);
	fake_process(fx->dir, 5000, "kwin_wayland", 3, other);
	/* OBS itself while it has the device open: never its own holder */
	fake_process(fx->dir, 7, "obs-studio-rust", 12, fx->dev);

	struct v4l2_holder holders[4];
	size_t n = v4l2_find_holders(proc, fx->dev, 7, holders, 4);
	assert_int_equal(n, 1);
	assert_int_equal(holders[0].pid, 4242);
	assert_string_equal(holders[0].name, ".obs-wrapped");
	assert_true(v4l2_holder_is_obs(&holders[0]));
}

static void resolves_a_symlinked_device_path(void **state)
{
	struct fixture *fx = *state;
	char link[128], proc[128];
	path_join(link, sizeof(link), fx->dir, "usb-Elgato_Cam_Link_4K-video-index0");
	char abs_dev[PATH_MAX];
	assert_non_null(realpath(fx->dev, abs_dev));
	assert_int_equal(symlink(abs_dev, link), 0);
	path_join(proc, sizeof(proc), fx->dir, "proc");

	fake_process(fx->dir, 31, "cheese", 20, fx->dev);

	struct v4l2_holder holders[2];
	assert_int_equal(v4l2_find_holders(proc, link, 1, holders, 2), 1);
	assert_int_equal(holders[0].pid, 31);
	assert_false(v4l2_holder_is_obs(&holders[0]));
}

static void no_holder_without_a_proc_tree(void **state)
{
	struct fixture *fx = *state;
	struct v4l2_holder holders[2];
	assert_int_equal(v4l2_find_holders("/nonexistent-proc", fx->dev, 1, holders, 2), 0);
}

static void recognises_obs_instances(void **state)
{
	(void)state;
	static const char *obs_names[] = {"obs", ".obs-wrapped", "obs-studio-rust", "AppRun"};
	static const char *other_names[] = {"cheese", "zoom", "obsidian", "firefox"};
	struct v4l2_holder h = {0};

	for (size_t i = 0; i < sizeof(obs_names) / sizeof(obs_names[0]); i++) {
		strcpy(h.name, obs_names[i]);
		assert_true(v4l2_holder_is_obs(&h));
	}
	for (size_t i = 0; i < sizeof(other_names) / sizeof(other_names[0]); i++) {
		strcpy(h.name, other_names[i]);
		assert_false(v4l2_holder_is_obs(&h));
	}
}

static void describes_the_holder(void **state)
{
	(void)state;
	struct v4l2_holder h = {.pid = 1973523};
	strcpy(h.name, "obs");
	struct dstr text = {0};

	v4l2_busy_describe(&text, "/dev/video0", &h);
	assert_non_null(strstr(text.array, "/dev/video0"));
	assert_non_null(strstr(text.array, "obs"));
	assert_non_null(strstr(text.array, "1973523"));

	v4l2_busy_describe(&text, "/dev/video0", NULL);
	assert_non_null(strstr(text.array, "/dev/video0"));
	assert_null(strstr(text.array, "PID"));
	dstr_free(&text);
}

/* A real helper process holding the device: started with the device open,
 * reports readiness through a pipe, then waits. */
static pid_t start_helper(struct fixture *fx, bool ignore_term)
{
	const char *dev = fx->dev;
	int ready[2];
	assert_int_equal(pipe(ready), 0);
	pid_t pid = fork();
	assert_true(pid >= 0);
	if (pid == 0) {
		close(ready[0]);
		if (ignore_term)
			signal(SIGTERM, SIG_IGN);
		int fd = open(dev, O_RDWR);
		if (fd < 0)
			_exit(2);
		if (write(ready[1], "r", 1) != 1)
			_exit(3);
		for (;;)
			pause();
	}
	close(ready[1]);
	char c;
	assert_int_equal(read(ready[0], &c, 1), 1);
	close(ready[0]);
	fx->helper = pid;
	return pid;
}

static void stop_helper(struct fixture *fx)
{
	kill(fx->helper, SIGKILL);
	waitpid(fx->helper, NULL, 0);
	fx->helper = 0;
}

static void finds_a_real_helper_process(void **state)
{
	struct fixture *fx = *state;
	pid_t helper = start_helper(fx, false);

	struct v4l2_holder holders[4];
	size_t n = v4l2_find_holders("/proc", fx->dev, getpid(), holders, 4);
	assert_int_equal(n, 1);
	assert_int_equal(holders[0].pid, helper);
	assert_int_equal(holders[0].uid, getuid());

	stop_helper(fx);
}

static void force_closes_the_holder_with_sigterm(void **state)
{
	struct fixture *fx = *state;
	pid_t helper = start_helper(fx, false);
	struct v4l2_holder holders[2];
	assert_int_equal(v4l2_find_holders("/proc", fx->dev, getpid(), holders, 2), 1);

	assert_int_equal(v4l2_force_release("/proc", fx->dev, &holders[0], getuid(), 5000), V4L2_FORCE_RELEASED);

	int status = 0;
	assert_int_equal(waitpid(helper, &status, 0), helper);
	fx->helper = 0;
	assert_true(WIFSIGNALED(status));
	assert_int_equal(WTERMSIG(status), SIGTERM);
	assert_int_equal(v4l2_find_holders("/proc", fx->dev, getpid(), holders, 2), 0);
}

static void never_signals_another_users_process(void **state)
{
	struct fixture *fx = *state;
	pid_t helper = start_helper(fx, false);
	struct v4l2_holder holders[2];
	assert_int_equal(v4l2_find_holders("/proc", fx->dev, getpid(), holders, 2), 1);

	/* as if OBS ran as another user than the holder */
	assert_int_equal(v4l2_force_release("/proc", fx->dev, &holders[0], getuid() + 1, 1000), V4L2_FORCE_NOT_OWNER);
	assert_int_equal(waitpid(helper, NULL, WNOHANG), 0);

	stop_helper(fx);
}

static void never_escalates_to_sigkill(void **state)
{
	struct fixture *fx = *state;
	pid_t helper = start_helper(fx, true);
	struct v4l2_holder holders[2];
	assert_int_equal(v4l2_find_holders("/proc", fx->dev, getpid(), holders, 2), 1);

	assert_int_equal(v4l2_force_release("/proc", fx->dev, &holders[0], getuid(), 200), V4L2_FORCE_TIMEOUT);
	/* still alive and still holding the device */
	assert_int_equal(waitpid(helper, NULL, WNOHANG), 0);
	assert_int_equal(v4l2_find_holders("/proc", fx->dev, getpid(), holders, 2), 1);

	stop_helper(fx);
}

static void retry_delay_backs_off_to_30_s(void **state)
{
	(void)state;
	assert_int_equal(v4l2_busy_retry_delay_ms(0), 2000);
	assert_int_equal(v4l2_busy_retry_delay_ms(1), 4000);
	assert_int_equal(v4l2_busy_retry_delay_ms(2), 8000);
	assert_int_equal(v4l2_busy_retry_delay_ms(3), 16000);
	assert_int_equal(v4l2_busy_retry_delay_ms(4), 30000);
	assert_int_equal(v4l2_busy_retry_delay_ms(100), 30000);
	assert_int_equal(v4l2_busy_retry_delay_ms(UINT32_MAX), 30000);
}

/* The holder of a busy camera was not found by the first scan (#3726: Zoom
 * held /dev/video0, the warning said "another program"). A retry that finds
 * it announces again so the warning can name it; nothing else re-announces. */
static void announces_again_once_the_holder_is_known(void **state)
{
	(void)state;
	/* first failure of an episode, whether the holder is known or not */
	assert_true(v4l2_busy_should_announce(false, false, false));
	assert_true(v4l2_busy_should_announce(false, false, true));
	/* a retry that still cannot see the holder */
	assert_false(v4l2_busy_should_announce(true, false, false));
	/* a retry that finds it, after an announcement without it */
	assert_true(v4l2_busy_should_announce(true, false, true));
	/* already named: no repeat on every retry */
	assert_false(v4l2_busy_should_announce(true, true, true));
	assert_false(v4l2_busy_should_announce(true, true, false));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(busy_errno_is_ebusy_only),
		cmocka_unit_test_setup_teardown(finds_the_holder_in_a_proc_tree, setup, teardown),
		cmocka_unit_test_setup_teardown(resolves_a_symlinked_device_path, setup, teardown),
		cmocka_unit_test_setup_teardown(no_holder_without_a_proc_tree, setup, teardown),
		cmocka_unit_test(recognises_obs_instances),
		cmocka_unit_test(describes_the_holder),
		cmocka_unit_test_setup_teardown(finds_a_real_helper_process, setup, teardown),
		cmocka_unit_test_setup_teardown(force_closes_the_holder_with_sigterm, setup, teardown),
		cmocka_unit_test_setup_teardown(never_signals_another_users_process, setup, teardown),
		cmocka_unit_test_setup_teardown(never_escalates_to_sigkill, setup, teardown),
		cmocka_unit_test(retry_delay_backs_off_to_30_s),
		cmocka_unit_test(announces_again_once_the_holder_is_known),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
