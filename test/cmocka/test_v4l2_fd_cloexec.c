#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <setjmp.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <cmocka.h>

#include <linux/videodev2.h>
#include <libv4l2.h>

#include <obs-module.h>
#include <util/pipe.h>

#include "v4l2-helpers.h"

/* A V4L2 camera stays busy while OBS records (obsproject/obs-studio#12508 by
 * stephematician): changing the resolution or frame rate of a Video Capture
 * Device during a recording freezes it ("Unable to set input 0 ...
 * Device or resource busy") until the recording stops. With the uvcvideo
 * trace, stephematician showed that the driver only releases the camera
 * when the recording stops, in os_process_pipe_destroy(), although
 * v4l2_close() had returned 0.
 *
 * The plugin opens the camera without O_CLOEXEC, and the recording output
 * starts obs-ffmpeg-mux with posix_spawn() (os_process_pipe_create2() in
 * libobs/util/pipe-posix.c). The muxer inherits a copy of the camera's file
 * descriptor and keeps it until it exits. A driver releases the device only
 * when the last copy is closed, so the camera stays open in the muxer after
 * the plugin closed it.
 *
 * The tests run the real v4l2-input.c against a fake libv4l2 (below) whose
 * v4l2_open() opens a FIFO with the flags the plugin passes, as libv4l2's
 * v4l2_open() does (openat(AT_FDCWD, file, oflag)). The "muxer" is started
 * the way obs-ffmpeg-mux is, through os_process_pipe_create2(args, "w"),
 * and the test reads which files it holds from /proc/<pid>/fd. No camera,
 * kernel module or display is needed. */

#define WIDTH 64
#define HEIGHT 48
#define FAKE_BUFFERS 2
#define FAKE_BUFFER_SIZE (WIDTH * HEIGHT * 2)
#define MAX_FDS 1024

static char dir_path[64];
static char camera_path[128];

/* ------------------------------------------------------------------------ */
/* Fake libv4l2: a YUYV camera with one input and one control, which never
 * delivers a frame (select() on the FIFO times out). */

static struct {
	bool open[MAX_FDS];
	int opens;
	int inheritable_opens;
	uint8_t memory[FAKE_BUFFERS][FAKE_BUFFER_SIZE];
} fake;

int v4l2_open(const char *file, int oflag, ...)
{
	/* Only the test's camera exists; real devices are never touched. */
	if (strcmp(file, camera_path) != 0) {
		errno = ENOENT;
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
	if (!(fcntl(fd, F_GETFD) & FD_CLOEXEC))
		fake.inheritable_opens++;
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
	size_t index = (size_t)offset / FAKE_BUFFER_SIZE;
	return index < FAKE_BUFFERS ? fake.memory[index] : MAP_FAILED;
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
	va_list args;
	va_start(args, request);
	void *arg = va_arg(args, void *);
	va_end(args);

	if (fd < 0 || fd >= MAX_FDS || !fake.open[fd]) {
		errno = EBADF;
		return -1;
	}

	switch ((unsigned long)request) {
	case VIDIOC_G_INPUT:
		*(int *)arg = 0;
		return 0;
	case VIDIOC_ENUMINPUT: {
		struct v4l2_input *in = arg;
		if (in->index != 0)
			break;
		snprintf((char *)in->name, sizeof(in->name), "Camera");
		in->capabilities = 0;
		return 0;
	}
	case VIDIOC_G_FMT: {
		struct v4l2_format *fmt = arg;
		fmt->fmt.pix.width = WIDTH;
		fmt->fmt.pix.height = HEIGHT;
		fmt->fmt.pix.pixelformat = V4L2_PIX_FMT_YUYV;
		fmt->fmt.pix.bytesperline = WIDTH * 2;
		return 0;
	}
	case VIDIOC_G_PARM: {
		struct v4l2_streamparm *par = arg;
		par->parm.capture.timeperframe.numerator = 1;
		par->parm.capture.timeperframe.denominator = 30;
		return 0;
	}
	case VIDIOC_REQBUFS:
		((struct v4l2_requestbuffers *)arg)->count = FAKE_BUFFERS;
		return 0;
	case VIDIOC_QUERYBUF: {
		struct v4l2_buffer *buf = arg;
		buf->length = FAKE_BUFFER_SIZE;
		buf->m.offset = buf->index * FAKE_BUFFER_SIZE;
		return 0;
	}
	case VIDIOC_QUERYCTRL: {
		/* One brightness control, so the controls get a property */
		struct v4l2_queryctrl *qctrl = arg;
		if ((qctrl->id & ~V4L2_CTRL_FLAG_NEXT_CTRL) != 0)
			break;
		memset(qctrl, 0, sizeof(*qctrl));
		qctrl->id = V4L2_CID_BRIGHTNESS;
		qctrl->type = V4L2_CTRL_TYPE_INTEGER;
		snprintf((char *)qctrl->name, sizeof(qctrl->name), "Brightness");
		qctrl->maximum = 255;
		qctrl->step = 1;
		qctrl->default_value = 128;
		return 0;
	}
	case VIDIOC_S_INPUT:
	case VIDIOC_S_FMT:
	case VIDIOC_S_PARM:
	case VIDIOC_S_CTRL:
	case VIDIOC_QBUF:
	case VIDIOC_STREAMON:
	case VIDIOC_STREAMOFF:
	case VIDIOC_LOG_STATUS:
		return 0;
	case VIDIOC_DQBUF:
		errno = EAGAIN;
		return -1;
	}

	/* Enumerations end here, and anything else is not supported */
	errno = EINVAL;
	return -1;
}

void obs_source_output_video(obs_source_t *source, const struct obs_source_frame *frame)
{
	(void)source;
	(void)frame;
}

const char *obs_module_text(const char *lookup)
{
	return lookup;
}

extern struct obs_source_info v4l2_input;

/* ------------------------------------------------------------------------ */
/* The recording's muxer and what it holds */

/* Starts a long-running child the way obs-ffmpeg-mux.c starts the muxer: with
 * os_process_pipe_create2(args, "w"), its stdin fed by OBS. The child reports
 * its pid and then waits for its stdin to close, as the muxer does. */
static os_process_pipe_t *start_muxer(pid_t *pid)
{
	struct os_process_args *args = os_process_args_create("/bin/sh");
	os_process_args_add_arg(args, "-c");
	os_process_args_add_arg(args, "echo $$ >&2; exec cat >/dev/null");
	os_process_pipe_t *pp = os_process_pipe_create2(args, "w");
	os_process_args_destroy(args);
	if (!pp) {
		fprintf(stderr, "cannot start the muxer stand-in\n");
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
		fprintf(stderr, "the muxer stand-in did not report its pid ('%s')\n", line);
		fail();
	}
	return pp;
}

/* How many of the process's file descriptors point at the camera */
static int camera_fds(pid_t pid)
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
		if (strcmp(target, camera_path) == 0)
			count++;
	}
	closedir(dir);
	return count;
}

static obs_data_t *camera_settings(int width, int height)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", camera_path);
	obs_data_set_int(settings, "input", 0);
	obs_data_set_int(settings, "pixelformat", V4L2_PIX_FMT_YUYV);
	obs_data_set_int(settings, "standard", -1);
	obs_data_set_int(settings, "dv_timing", -1);
	obs_data_set_int(settings, "resolution", v4l2_pack_tuple(width, height));
	obs_data_set_int(settings, "framerate", v4l2_pack_tuple(1, 30));
	obs_data_set_int(settings, "color_range", VIDEO_RANGE_DEFAULT);
	obs_data_set_bool(settings, "buffering", true);
	obs_data_set_bool(settings, "auto_reset", false);
	obs_data_set_int(settings, "timeout_frames", 5);
	return settings;
}

static void *start_camera(void)
{
	obs_data_t *settings = camera_settings(WIDTH, HEIGHT);
	void *source = v4l2_input.create(settings, NULL);
	obs_data_release(settings);

	if (camera_fds(getpid()) != 1) {
		fprintf(stderr, "the source did not open the fake camera\n");
		fail();
	}
	return source;
}

/* ------------------------------------------------------------------------ */

static int setup(void **state)
{
	(void)state;
	memset(&fake, 0, sizeof(fake));
	snprintf(dir_path, sizeof(dir_path), "/tmp/obs-v4l2-cloexec-XXXXXX");
	if (!mkdtemp(dir_path))
		return -1;
	snprintf(camera_path, sizeof(camera_path), "%s/video0", dir_path);
	return mkfifo(camera_path, 0600);
}

static int teardown(void **state)
{
	(void)state;
	unlink(camera_path);
	rmdir(dir_path);
	return 0;
}

/* Control: the camera's descriptor is counted where it is open, so the checks
 * below can see a leak. Since util/pipe gives a child only its standard
 * streams (obsproject/obs-studio#4596), even a descriptor opened without
 * O_CLOEXEC no longer reaches the muxer; the checks below still pin that the
 * plugin itself opens the camera close-on-exec. */
static void muxer_gets_no_inheritable_fd(void **state)
{
	(void)state;
	int fd = open(camera_path, O_RDWR | O_NONBLOCK);
	assert_true(fd >= 0);

	pid_t pid;
	os_process_pipe_t *muxer = start_muxer(&pid);
	int ours = camera_fds(getpid());
	int held = camera_fds(pid);
	os_process_pipe_destroy(muxer);
	close(fd);

	assert_int_equal(ours, 1);
	assert_int_equal(held, 0);
}

/* Recording starts while the camera is capturing */
static void muxer_does_not_hold_the_capturing_camera(void **state)
{
	(void)state;
	void *source = start_camera();

	pid_t pid;
	os_process_pipe_t *muxer = start_muxer(&pid);
	int held = camera_fds(pid);

	os_process_pipe_destroy(muxer);
	v4l2_input.destroy(source);

	if (held) {
		fprintf(stderr, "the recording's muxer holds %d copies of the camera's descriptor\n", held);
		fail();
	}
}

/* The steps of obsproject/obs-studio#12508: start a recording, then change
 * the resolution, which closes the camera and opens it again. The old
 * descriptor must be gone everywhere, or the camera is still in use by the
 * time it is opened again. */
static void camera_is_released_when_restarted_while_recording(void **state)
{
	(void)state;
	void *source = start_camera();

	pid_t pid;
	os_process_pipe_t *muxer = start_muxer(&pid);

	int opens = fake.opens;
	obs_data_t *settings = camera_settings(WIDTH * 2, HEIGHT * 2);
	v4l2_input.update(source, settings);
	obs_data_release(settings);
	if (fake.opens != opens + 1) {
		fprintf(stderr, "changing the resolution did not reopen the camera\n");
		fail();
	}
	int held_after_restart = camera_fds(pid) + camera_fds(getpid());

	v4l2_input.destroy(source);
	int held_after_destroy = camera_fds(pid);

	os_process_pipe_destroy(muxer);

	if (held_after_restart != 1) {
		fprintf(stderr,
			"after the restart the camera is open %d times (expected once, by the source); "
			"the muxer still holds the descriptor the source closed\n",
			held_after_restart);
		fail();
	}
	if (held_after_destroy) {
		fprintf(stderr, "after the source is removed the muxer still holds the camera (%d)\n",
			held_after_destroy);
		fail();
	}
}

/* The properties open the camera briefly for each list and control; a spawn
 * from another thread meanwhile must not get a copy either. */
static void properties_open_the_camera_close_on_exec(void **state)
{
	(void)state;
	/* A source with no device yet, as when it is first added */
	obs_data_t *settings = camera_settings(WIDTH, HEIGHT);
	obs_data_set_string(settings, "device_id", "");
	void *source = v4l2_input.create(settings, NULL);
	obs_data_set_string(settings, "device_id", camera_path);
	assert_int_equal(fake.opens, 0);

	obs_properties_t *props = v4l2_input.get_properties(source);

	/* Selecting the device fills the inputs, formats, resolutions and
	 * frame rates in turn, each with its own open. */
	obs_property_modified(obs_properties_get(props, "device_id"), settings);

	obs_property_t *controls = obs_properties_get(props, "controls");
	assert_non_null(controls);
	obs_property_t *brightness = obs_properties_get(obs_property_group_content(controls), "Brightness");
	assert_non_null(brightness);
	obs_data_set_int(settings, "Brightness", 200);
	obs_property_modified(brightness, settings);

	obs_properties_destroy(props);
	obs_data_release(settings);
	v4l2_input.destroy(source);

	if (fake.opens < 5) {
		fprintf(stderr, "the properties opened the camera only %d times\n", fake.opens);
		fail();
	}
	if (fake.inheritable_opens) {
		fprintf(stderr, "%d of %d opens of the camera would be inherited by a spawned process\n",
			fake.inheritable_opens, fake.opens);
		fail();
	}
	assert_int_equal(camera_fds(getpid()), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(muxer_gets_no_inheritable_fd, setup, teardown),
		cmocka_unit_test_setup_teardown(muxer_does_not_hold_the_capturing_camera, setup, teardown),
		cmocka_unit_test_setup_teardown(camera_is_released_when_restarted_while_recording, setup, teardown),
		cmocka_unit_test_setup_teardown(properties_open_the_camera_close_on_exec, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
