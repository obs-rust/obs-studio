/* obs-ffmpeg: the VA-API code opened DRM render nodes without O_CLOEXEC
 * (vaapi_open_display_drm()), so a process OBS started meanwhile got a copy
 * of the node: the recording's obs-ffmpeg-mux, started with
 * os_process_pipe_create(), held the encoder's render node for as long as
 * it ran. It is the leak of obsproject/obs-studio#14054 (the Virtual
 * Camera's device) in the VA-API encoder and its probe.
 *
 * The real vaapi-utils.c is compiled in, and vaGetDisplayDRM() is wrapped
 * (--wrap) to record the FD_CLOEXEC flag of the descriptor it is given, the
 * one vaapi_open_display_drm() just opened.
 *
 * - device_is_opened_close_on_exec needs no GPU: the wrapper refuses the
 *   display for /dev/null, as libva does for a node without a driver.
 * - probe_opens_render_nodes_close_on_exec and
 *   encoder_device_is_not_inherited use the first render node and the real
 *   libva, and skip without a render node. The second starts a child with
 *   os_process_pipe_create() while the device is open, as a recording
 *   starts its muxer while the encoder runs, and reads /proc/<pid>/fd. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>

#include "plugins/obs-ffmpeg/vaapi-utils.c.h"

#include <dirent.h>
#include <errno.h>
#include <util/pipe.h>

static pthread_mutex_t record_mutex = PTHREAD_MUTEX_INITIALIZER;
static int displays;
static int inheritable_displays;
static bool refuse_displays;

VADisplay __real_vaGetDisplayDRM(int fd);

VADisplay __wrap_vaGetDisplayDRM(int fd)
{
	bool inheritable = !(fcntl(fd, F_GETFD) & FD_CLOEXEC);

	/* The probe opens every render node at once, one thread each. */
	pthread_mutex_lock(&record_mutex);
	displays++;
	if (inheritable)
		inheritable_displays++;
	pthread_mutex_unlock(&record_mutex);

	return refuse_displays ? NULL : __real_vaGetDisplayDRM(fd);
}

static void reset_records(void)
{
	displays = 0;
	inheritable_displays = 0;
}

static void check_close_on_exec(const char *what)
{
	if (displays < 1) {
		fprintf(stderr, "%s opened no device\n", what);
		fail();
	}
	if (inheritable_displays) {
		fprintf(stderr,
			"%s: %d of %d device opens lack O_CLOEXEC; every process OBS starts meanwhile gets a copy\n",
			what, inheritable_displays, displays);
		fail();
	}
}

static char render_node[32];

static bool find_render_node(void)
{
	for (int i = 128; i < 192; i++) {
		snprintf(render_node, sizeof(render_node), "/dev/dri/renderD%d", i);
		if (access(render_node, R_OK | W_OK) == 0)
			return true;
	}
	return false;
}

/* ------------------------------------------------------------------------ */

static void device_is_opened_close_on_exec(void **state)
{
	(void)state;
	int fd = -1;
	reset_records();
	refuse_displays = true;

	VADisplay dpy = vaapi_open_device(&fd, "/dev/null", "test");

	refuse_displays = false;
	assert_null(dpy);
	/* A refused display leaves the descriptor open (reported separately);
	 * close it either way. */
	if (fd >= 0)
		close(fd);

	check_close_on_exec("vaapi_open_device()");
}

/* The encoder probe (vaapi_get_h264_default_device() and friends) opens
 * every render node */
static void probe_opens_render_nodes_close_on_exec(void **state)
{
	(void)state;
	if (!find_render_node()) {
		fprintf(stderr, "skipped: no usable render node\n");
		skip();
	}

	reset_records();
	vaapi_free_device_caps();
	(void)vaapi_device_h264_supported(render_node);
	vaapi_free_device_caps();

	check_close_on_exec("the VA-API probe");
}

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

/* A recording starts while a VA-API encoder has its device open */
static void encoder_device_is_not_inherited(void **state)
{
	(void)state;
	if (!find_render_node()) {
		fprintf(stderr, "skipped: no usable render node\n");
		skip();
	}

	reset_records();
	int fd = -1;
	VADisplay dpy = vaapi_open_device(&fd, render_node, "test");
	if (!dpy) {
		if (fd >= 0)
			close(fd);
		check_close_on_exec("vaapi_open_device()");
		fprintf(stderr, "skipped: no VA-API driver for %s\n", render_node);
		skip();
	}

	pid_t pid;
	os_process_pipe_t *child = start_child(&pid);
	int held = fds_on(pid, render_node);
	os_process_pipe_destroy(child);
	vaapi_close_device(&fd, dpy);

	if (held) {
		fprintf(stderr, "a process started while the device is open holds %d copies of %s\n", held,
			render_node);
		fail();
	}
	check_close_on_exec("vaapi_open_device()");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(device_is_opened_close_on_exec),
		cmocka_unit_test(probe_opens_render_nodes_close_on_exec),
		cmocka_unit_test(encoder_device_is_not_inherited),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
