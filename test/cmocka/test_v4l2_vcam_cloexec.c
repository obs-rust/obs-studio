/* linux-v4l2: the Virtual Camera cannot be restarted while OBS records
 * (obsproject/obs-studio#14054). try_connect() opened the v4l2loopback device
 * without O_CLOEXEC, and the recording output starts obs-ffmpeg-mux with
 * os_process_pipe_create(), which closes only its own pipe descriptors in
 * the child. The muxer got a copy of the device's descriptor and kept it for
 * the whole recording, so stopping the Virtual Camera did not release the
 * device, and v4l2loopback with exclusive_caps=1 refused the next start.
 * The same leak is #340's for the V4L2 input.
 *
 * The real v4l2-output.c is compiled in. ioctl() is wrapped (--wrap) to see
 * the descriptor the output opened: on VIDIOC_QUERYCAP, the first ioctl after
 * the open, the wrapper records its FD_CLOEXEC flag.
 *
 * - opened_device_is_close_on_exec needs no device, display or kernel module:
 *   try_connect() opens a temporary file, which then fails VIDIOC_QUERYCAP.
 * - virtualcam_restarts_while_a_child_runs uses vivid's video output node
 *   (load vivid with node_types=0x100 or any mask with bit 8, as root) and
 *   core video (an X display). It skips without either. vivid has no
 *   VIDIOC_S_PARM for its output, which v4l2loopback has, so the wrapper
 *   answers that one ioctl for the vivid node; everything else reaches the
 *   driver. virtualcam_start() is not called: without v4l2loopback loaded it
 *   runs "pkexec modprobe v4l2loopback". The test calls try_connect() on the
 *   vivid node, which is what virtualcam_start() does with the node it picks.
 */

/* v4l2-output.c needs it (versionsort) before the first system header. */
#define _GNU_SOURCE

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>

#include "plugins/linux-v4l2/v4l2-output.c.h"

#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <util/pipe.h>

static char probe_path[PATH_MAX];
static int querycap_calls;
static int querycap_cloexec;
/* The vivid node's rdev, or 0: its VIDIOC_S_PARM is answered here. */
static dev_t vivid_rdev;

int __real_ioctl(int fd, unsigned long request, ...);

int __wrap_ioctl(int fd, unsigned long request, ...)
{
	va_list args;
	va_start(args, request);
	void *arg = va_arg(args, void *);
	va_end(args);

	if (request == VIDIOC_QUERYCAP) {
		querycap_calls++;
		querycap_cloexec = (fcntl(fd, F_GETFD) & FD_CLOEXEC) != 0;
	}

	if (request == VIDIOC_S_PARM && vivid_rdev) {
		struct stat st;
		struct v4l2_streamparm *parm = arg;
		if (fstat(fd, &st) == 0 && st.st_rdev == vivid_rdev && parm->type == V4L2_BUF_TYPE_VIDEO_OUTPUT)
			return 0;
	}

	return __real_ioctl(fd, request, arg);
}

/* ------------------------------------------------------------------------ */
/* A child started the way obs-ffmpeg-mux is, and what it holds */

/* Starts a long-running child with os_process_pipe_create(), as the recording
 * starts the muxer. It reports its pid and waits for its stdin to close. */
static os_process_pipe_t *start_child(pid_t *pid)
{
	os_process_pipe_t *pp = os_process_pipe_create("echo $$ >&2; exec cat >/dev/null", "w");
	if (!pp) {
		fprintf(stderr, "cannot start the child\n");
		fail();
	}

	char line[32] = {0};
	size_t len = 0;
	while (len < sizeof(line) - 1) {
		uint8_t c;
		if (os_process_pipe_read_err(pp, &c, 1) != 1 || c == '\n')
			break;
		line[len++] = (char)c;
	}
	*pid = (pid_t)atoi(line);
	if (*pid <= 0) {
		fprintf(stderr, "the child did not report its pid ('%s')\n", line);
		fail();
	}
	return pp;
}

/* How many of the process's file descriptors point at path */
static int fds_on(pid_t pid, const char *path)
{
	char fd_dir[64];
	snprintf(fd_dir, sizeof(fd_dir), "/proc/%d/fd", (int)pid);

	DIR *dir = opendir(fd_dir);
	if (!dir) {
		fprintf(stderr, "cannot read %s: %s\n", fd_dir, strerror(errno));
		fail();
	}

	int count = 0;
	struct dirent *entry;
	while ((entry = readdir(dir)) != NULL) {
		if (entry->d_name[0] == '.')
			continue;

		char link[PATH_MAX];
		char target[PATH_MAX];
		snprintf(link, sizeof(link), "%s/%s", fd_dir, entry->d_name);
		ssize_t n = readlink(link, target, sizeof(target) - 1);
		if (n < 0)
			continue;
		target[n] = 0;
		if (strcmp(target, path) == 0)
			count++;
	}
	closedir(dir);
	return count;
}

/* ------------------------------------------------------------------------ */
/* No device needed */

static void opened_device_is_close_on_exec(void **state)
{
	(void)state;
	char dir[] = "/tmp/obs-vcam-cloexec-XXXXXX";
	assert_non_null(mkdtemp(dir));
	snprintf(probe_path, sizeof(probe_path), "%s/video0", dir);
	int fd = open(probe_path, O_CREAT | O_WRONLY | O_CLOEXEC, 0600);
	assert_true(fd >= 0);
	close(fd);

	struct virtualcam_data vcam = {0};
	vcam.device = -1;
	querycap_calls = 0;
	querycap_cloexec = 0;

	/* A file is no video output: the device is opened, then closed again
	 * after VIDIOC_QUERYCAP fails. */
	bool connected = try_connect(&vcam, probe_path);

	unlink(probe_path);
	rmdir(dir);

	assert_false(connected);
	assert_int_equal(vcam.device, -1);
	if (querycap_calls != 1) {
		fprintf(stderr, "try_connect() made %d VIDIOC_QUERYCAP calls, expected 1\n", querycap_calls);
		fail();
	}
	if (!querycap_cloexec) {
		fprintf(stderr, "try_connect() opened the device without O_CLOEXEC: "
				"every process OBS starts meanwhile gets a copy\n");
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* vivid */

static int obs_up;
static char vivid_path[32];

/* vivid's video output node, or false */
static bool find_vivid_output(void)
{
	for (int i = 0; i < 64; i++) {
		char path[32];
		snprintf(path, sizeof(path), "/dev/video%d", i);
		int fd = open(path, O_RDWR | O_NONBLOCK | O_CLOEXEC);
		if (fd < 0)
			continue;

		struct v4l2_capability cap = {0};
		bool output = __real_ioctl(fd, VIDIOC_QUERYCAP, &cap) == 0 &&
			      strcmp((const char *)cap.driver, "vivid") == 0 &&
			      (cap.device_caps & V4L2_CAP_VIDEO_OUTPUT);
		struct stat st;
		if (output && fstat(fd, &st) == 0) {
			snprintf(vivid_path, sizeof(vivid_path), "%s", path);
			vivid_rdev = st.st_rdev;
		}
		close(fd);
		if (vivid_rdev)
			return true;
	}
	return false;
}

static int reset_video(void)
{
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
	return obs_reset_video(&ovi);
}

/* Starts the Virtual Camera on the vivid node, as virtualcam_start() does
 * with the device it picks */
static void start_virtualcam(obs_output_t *output, const char *what)
{
	if (!try_connect(obs_obj_get_data(output), vivid_path)) {
		fprintf(stderr, "%s: the Virtual Camera did not start on %s\n", what, vivid_path);
		fail();
	}
}

static void stop_virtualcam(obs_output_t *output)
{
	virtualcam_stop(obs_obj_get_data(output), 0);
}

/* The steps of obsproject/obs-studio#14054: the Virtual Camera runs, a
 * recording starts (its muxer is the child), the Virtual Camera is stopped
 * and started again while the recording goes on. */
static void virtualcam_restarts_while_a_child_runs(void **state)
{
	(void)state;
	if (!find_vivid_output()) {
		fprintf(stderr, "skipped: no vivid video output node (modprobe vivid node_types=0x100)\n");
		skip();
	}
	if (!obs_up) {
		fprintf(stderr, "skipped: no core video (needs a display)\n");
		skip();
	}
	if (reset_video() != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video() failed\n");
		skip();
	}

	obs_output_t *output = obs_output_create("virtualcam_output", "vcam", NULL, NULL);
	assert_non_null(output);

	start_virtualcam(output, "first start");
	assert_int_equal(fds_on(getpid(), vivid_path), 1);

	pid_t pid;
	os_process_pipe_t *child = start_child(&pid);
	int held_while_running = fds_on(pid, vivid_path);

	stop_virtualcam(output);
	int held_after_stop = fds_on(pid, vivid_path) + fds_on(getpid(), vivid_path);

	querycap_calls = 0;
	start_virtualcam(output, "restart while the child runs");
	int restart_opens = querycap_calls;
	int held_after_restart = fds_on(pid, vivid_path) + fds_on(getpid(), vivid_path);

	pid_t pid2;
	os_process_pipe_t *child2 = start_child(&pid2);
	int held_by_second_child = fds_on(pid2, vivid_path);

	stop_virtualcam(output);
	os_process_pipe_destroy(child2);
	os_process_pipe_destroy(child);
	obs_output_release(output);

	fprintf(stderr,
		"copies of %s: in a child while running %d, after stop %d, after restart %d, "
		"in a child started after the restart %d\n",
		vivid_path, held_while_running, held_after_stop, held_after_restart, held_by_second_child);
	if (held_while_running) {
		fprintf(stderr, "a process started while the Virtual Camera runs holds %d copies of %s\n",
			held_while_running, vivid_path);
		fail();
	}
	if (held_after_stop) {
		fprintf(stderr, "after the Virtual Camera stopped, %s is still open %d times\n", vivid_path,
			held_after_stop);
		fail();
	}
	assert_int_equal(restart_opens, 1);
	if (held_after_restart != 1) {
		fprintf(stderr, "after the restart %s is open %d times (expected once, by the Virtual Camera)\n",
			vivid_path, held_after_restart);
		fail();
	}
	if (held_by_second_child) {
		fprintf(stderr, "a process started after the restart holds %d copies of %s\n", held_by_second_child,
			vivid_path);
		fail();
	}
}

static int group_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (obs_up)
		obs_register_output(&virtualcam_info);
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(opened_device_is_close_on_exec),
		cmocka_unit_test(virtualcam_restarts_while_a_child_runs),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
