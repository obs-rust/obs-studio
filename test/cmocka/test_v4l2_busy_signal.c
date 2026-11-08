#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <signal.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <setjmp.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <cmocka.h>

#include <linux/videodev2.h>
#include <libv4l2.h>

#include <obs-module.h>
#include <util/dstr.h>
#include <util/platform.h>

#include "v4l2-helpers.h"

/* A camera another process holds (EBUSY) must reach the user as a popup with
 * a Force Camera button (obs-rust/obs-studio#799): a stock OBS left running
 * held /dev/video0, and OBS-Studio-Rust showed no camera and no reason.
 *
 * The plugin cannot open a dialog itself. It announces the busy device with
 * the global signal "source_device_busy" (once per busy episode), which the
 * frontend turns into the popup, and the popup's Force Camera button calls
 * the source's "force_camera" proc. These tests run the real v4l2-input.c on
 * a real libobs source, against a fake libv4l2 whose device is busy while a
 * forked helper process holds it open. No camera or display is needed. */

#define MAX_FDS 1024

static char dir_path[64];
static char camera_path[128];

/* ------------------------------------------------------------------------ */
/* Fake libv4l2: EBUSY while the helper holds the device; otherwise a device
 * that opens but supports no ioctl, so the source gives up with EINVAL. */

static struct {
	pid_t holder;
	/* wait status of the holder, once reaped */
	int holder_status;
	bool open[MAX_FDS];
	int opens;
	int busy_opens;
} fake;

/* Whether process `pid` still has the camera open, from /proc/<pid>/fd, the
 * same view the plugin's holder scan has. */
static bool holder_has_camera_open(pid_t pid)
{
	char dir[64];
	snprintf(dir, sizeof(dir), "/proc/%d/fd", (int)pid);
	os_dir_t *fds = os_opendir(dir);
	if (!fds)
		return false;

	bool found = false;
	struct os_dirent *ent;
	while (!found && (ent = os_readdir(fds)) != NULL) {
		char link[PATH_MAX], target[PATH_MAX];
		snprintf(link, sizeof(link), "%s/%s", dir, ent->d_name);
		ssize_t n = readlink(link, target, sizeof(target) - 1);
		if (n <= 0)
			continue;
		target[n] = '\0';
		found = strcmp(target, camera_path) == 0;
	}
	os_closedir(fds);
	return found;
}

/* The device is busy while the holder has it open. A holder that has let go
 * but not finished exiting (fds closed, not yet a zombie, a window machine
 * load widens) no longer holds it: wait for it to exit and record how
 * (#953). */
static bool holder_alive(void)
{
	if (fake.holder <= 0)
		return false;
	if (waitpid(fake.holder, &fake.holder_status, WNOHANG) == 0) {
		if (holder_has_camera_open(fake.holder))
			return true;
		waitpid(fake.holder, &fake.holder_status, 0);
	}
	fake.holder = 0;
	return false;
}

int v4l2_open(const char *file, int oflag, ...)
{
	if (strcmp(file, camera_path) != 0) {
		errno = ENOENT;
		return -1;
	}
	if (holder_alive()) {
		fake.busy_opens++;
		errno = EBUSY;
		return -1;
	}

	int fd = open(file, oflag);
	if (fd < 0 || fd >= MAX_FDS) {
		if (fd >= 0)
			close(fd);
		return -1;
	}
	fake.open[fd] = true;
	fake.opens++;
	return fd;
}

int v4l2_close(int fd)
{
	if (fd >= 0 && fd < MAX_FDS)
		fake.open[fd] = false;
	return close(fd);
}

void *v4l2_mmap(void *start, size_t length, int prot, int flags, int fd, int64_t offset)
{
	(void)start;
	(void)length;
	(void)prot;
	(void)flags;
	(void)fd;
	(void)offset;
	return MAP_FAILED;
}

int v4l2_munmap(void *start, size_t length)
{
	(void)start;
	(void)length;
	return 0;
}

#ifdef LIBV4L_HAVE_POSIX_IOCTL
int v4l2_ioctl(int fd, int request, ...)
#else
int v4l2_ioctl(int fd, unsigned long int request, ...)
#endif
{
	(void)request;
	if (fd < 0 || fd >= MAX_FDS || !fake.open[fd]) {
		errno = EBADF;
		return -1;
	}
	errno = EINVAL;
	return -1;
}

const char *obs_module_text(const char *lookup)
{
	return lookup;
}

extern struct obs_source_info v4l2_input;

/* ------------------------------------------------------------------------ */
/* What the frontend sees */

static struct {
	int count;
	obs_source_t *source;
	struct dstr device;
	struct dstr holder;
	long long pid;
	bool is_obs;
	bool can_force;
} busy_signal;

static void on_device_busy(void *param, calldata_t *cd)
{
	(void)param;
	busy_signal.count++;
	busy_signal.source = calldata_ptr(cd, "source");
	dstr_copy(&busy_signal.device, calldata_string(cd, "device"));
	dstr_copy(&busy_signal.holder, calldata_string(cd, "holder"));
	busy_signal.pid = calldata_int(cd, "pid");
	busy_signal.is_obs = calldata_bool(cd, "is_obs");
	busy_signal.can_force = calldata_bool(cd, "can_force");
}

/* Run UI tasks right away, as if the UI thread were idle */
static void run_ui_task(obs_task_t task, void *param, bool wait)
{
	(void)wait;
	task(param);
}

/* ------------------------------------------------------------------------ */

/* A slow holder's SIGTERM handler: let go of the device, then take a while
 * to exit, holding open the window between closing its fds and becoming a
 * reapable zombie that machine load opens in the plain holder (#953).
 *
 * The slow holder restarts as a fresh copy of this program (exec with
 * --slow-holder) instead of running in the plain fork: ThreadSanitizer does
 * not run the signal handler in a child forked from a multithreaded parent,
 * so under TSan the handler never ran and Force Camera timed out (#3810). */
static int slow_holder_fd = -1;
static const char *test_program;

static void slow_holder_term(int sig)
{
	(void)sig;
	close(slow_holder_fd);
	struct timespec ts = {0, 300 * 1000000L};
	nanosleep(&ts, NULL);
	_exit(0);
}

/* A helper holding the device open, ready when this returns */
static pid_t start_holder_ex(bool ignore_term, bool slow_exit)
{
	int ready[2];
	assert_int_equal(pipe(ready), 0);
	char ready_fd[32];
	snprintf(ready_fd, sizeof(ready_fd), "%d", ready[1]);
	pid_t pid = fork();
	assert_true(pid >= 0);
	if (pid == 0) {
		close(ready[0]);
		if (slow_exit) {
			/* /proc/self/exe names the Valgrind runtime under Memcheck. */
			execl(test_program, "test_v4l2_busy_signal", "--slow-holder", camera_path, ready_fd,
			      (char *)NULL);
			_exit(2);
		}
		if (ignore_term)
			signal(SIGTERM, SIG_IGN);
		int fd = open(camera_path, O_RDWR | O_NONBLOCK);
		if (fd < 0 || write(ready[1], "r", 1) != 1)
			_exit(2);
		for (;;)
			pause();
	}
	close(ready[1]);
	char c;
	assert_int_equal(read(ready[0], &c, 1), 1);
	close(ready[0]);
	fake.holder = pid;
	return pid;
}

static pid_t start_holder(bool ignore_term)
{
	return start_holder_ex(ignore_term, false);
}

static obs_source_t *create_camera(void)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", camera_path);
	obs_data_set_int(settings, "input", 0);
	obs_data_set_int(settings, "pixelformat", V4L2_PIX_FMT_YUYV);
	obs_data_set_int(settings, "standard", -1);
	obs_data_set_int(settings, "dv_timing", -1);
	obs_data_set_int(settings, "resolution", v4l2_pack_tuple(64, 48));
	obs_data_set_int(settings, "framerate", v4l2_pack_tuple(1, 30));
	obs_data_set_int(settings, "color_range", VIDEO_RANGE_DEFAULT);
	obs_source_t *source = obs_source_create_private("v4l2_input", "camera", settings);
	obs_data_release(settings);
	assert_non_null(source);
	return source;
}

static int setup(void **state)
{
	(void)state;
	memset(&fake, 0, sizeof(fake));
	dstr_free(&busy_signal.device);
	dstr_free(&busy_signal.holder);
	memset(&busy_signal, 0, sizeof(busy_signal));

	snprintf(dir_path, sizeof(dir_path), "/tmp/obs-v4l2-busy-XXXXXX");
	if (!mkdtemp(dir_path))
		return -1;
	snprintf(camera_path, sizeof(camera_path), "%s/video0", dir_path);
	if (mkfifo(camera_path, 0600) != 0)
		return -1;

	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	obs_set_ui_task_handler(run_ui_task);
	obs_register_source(&v4l2_input);
	/* the frontend's connection, made before plugins load */
	signal_handler_t *sh = obs_get_signal_handler();
	signal_handler_add(sh, "void source_device_busy(ptr source, string device, string holder, int pid, "
			       "bool is_obs, bool can_force)");
	signal_handler_connect(sh, "source_device_busy", on_device_busy, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (fake.holder > 0) {
		kill(fake.holder, SIGKILL);
		waitpid(fake.holder, NULL, 0);
		fake.holder = 0;
	}
	signal_handler_disconnect(obs_get_signal_handler(), "source_device_busy", on_device_busy, NULL);
	obs_shutdown();
	unlink(camera_path);
	rmdir(dir_path);
	return 0;
}

static void busy_camera_announces_its_holder_once(void **state)
{
	(void)state;
	pid_t holder = start_holder(false);

	obs_source_t *source = create_camera();
	assert_true(fake.busy_opens >= 1);
	assert_int_equal(busy_signal.count, 1);
	assert_ptr_equal(busy_signal.source, source);
	assert_string_equal(busy_signal.device.array, camera_path);
	assert_int_equal(busy_signal.pid, holder);
	assert_non_null(busy_signal.holder.array);
	assert_true(busy_signal.can_force);
	assert_false(busy_signal.is_obs);

	/* still busy on the next retry: no second popup */
	struct obs_source_info *info = &v4l2_input;
	for (int i = 0; i < 3; i++)
		info->video_tick(obs_obj_get_data(source), 1.0f);
	assert_int_equal(busy_signal.count, 1);

	obs_source_release(source);
}

static void force_camera_closes_the_holder_and_opens_the_camera(void **state)
{
	(void)state;
	pid_t holder = start_holder(false);
	obs_source_t *source = create_camera();
	assert_int_equal(busy_signal.count, 1);
	int opens_before = fake.opens;

	/* the popup's Force Camera button */
	calldata_t cd = {0};
	assert_true(proc_handler_call(obs_source_get_proc_handler(source), "force_camera", &cd));
	assert_true(calldata_bool(&cd, "released"));
	calldata_free(&cd);

	/* reaped by the reopen (holder_alive) */
	(void)holder;
	assert_int_equal(fake.holder, 0);
	assert_true(WIFSIGNALED(fake.holder_status));
	assert_int_equal(WTERMSIG(fake.holder_status), SIGTERM);
	/* the source opened the device again */
	assert_true(fake.opens > opens_before);

	obs_source_release(source);
}

/* The holder has closed the device but not finished exiting when the source
 * reopens it: the device is free, so the reopen must succeed (#953). */
static void force_camera_opens_while_the_holder_is_still_exiting(void **state)
{
	(void)state;
	start_holder_ex(false, true);
	obs_source_t *source = create_camera();
	assert_int_equal(busy_signal.count, 1);
	int opens_before = fake.opens;

	calldata_t cd = {0};
	assert_true(proc_handler_call(obs_source_get_proc_handler(source), "force_camera", &cd));
	assert_true(calldata_bool(&cd, "released"));
	calldata_free(&cd);

	assert_int_equal(fake.holder, 0);
	assert_true(fake.opens > opens_before);

	obs_source_release(source);
}

static void force_camera_never_kills_a_holder_that_refuses(void **state)
{
	(void)state;
	pid_t holder = start_holder(true);
	obs_source_t *source = create_camera();

	calldata_t cd = {0};
	assert_true(proc_handler_call(obs_source_get_proc_handler(source), "force_camera", &cd));
	assert_false(calldata_bool(&cd, "released"));
	calldata_free(&cd);
	assert_int_equal(waitpid(holder, NULL, WNOHANG), 0);

	obs_source_release(source);
}

static void camera_starts_when_the_holder_lets_go(void **state)
{
	(void)state;
	pid_t holder = start_holder(false);
	obs_source_t *source = create_camera();
	assert_int_equal(fake.opens, 0);

	/* the other program quits on its own */
	kill(holder, SIGKILL);
	waitpid(holder, NULL, 0);
	fake.holder = 0;

	/* the next busy check (once a second) retries the device; the tick
	 * only queues it once a second has passed since the failure */
	struct obs_source_info *info = &v4l2_input;
	for (int i = 0; i < 30 && fake.opens == 0; i++) {
		info->video_tick(obs_obj_get_data(source), 0.1f);
		os_sleep_ms(100);
	}
	assert_true(fake.opens > 0);

	obs_source_release(source);
}

int main(int argc, char **argv)
{
	test_program = argv[0];
	if (argc == 4 && strcmp(argv[1], "--slow-holder") == 0) {
		slow_holder_fd = open(argv[2], O_RDWR | O_NONBLOCK);
		signal(SIGTERM, slow_holder_term);
		if (slow_holder_fd < 0 || write(atoi(argv[3]), "r", 1) != 1)
			_exit(2);
		for (;;)
			pause();
	}
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(busy_camera_announces_its_holder_once, setup, teardown),
		cmocka_unit_test_setup_teardown(force_camera_closes_the_holder_and_opens_the_camera, setup, teardown),
		cmocka_unit_test_setup_teardown(force_camera_opens_while_the_holder_is_still_exiting, setup, teardown),
		cmocka_unit_test_setup_teardown(force_camera_never_kills_a_holder_that_refuses, setup, teardown),
		cmocka_unit_test_setup_teardown(camera_starts_when_the_holder_lets_go, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
