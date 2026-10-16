#include <errno.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <setjmp.h>
#include <sys/mman.h>
#include <cmocka.h>

#include <linux/videodev2.h>
#include <libv4l2.h>

#include <obs-module.h>
#include <obs-properties.h>
#include <util/dstr.h>

#include "v4l2-framerate.h"
#include "v4l2-helpers.h"

/* Picking 30 fps for a v4l2 source puts the device at its fastest rate
 * instead (60 fps in the report, obsproject/obs-studio#13388, by acscd), and
 * every change to a camera control such as zoom or focus restarts the stream
 * (#13896, by shanefagan).
 *
 * The framerate list offers each discrete frame interval as a packed 64-bit
 * (numerator, denominator) tuple, but stored it through an int, keeping only
 * the denominator. The saved setting is then 30 for 1/30 s: unpacked as a
 * legacy 16-bit tuple that is 0/30 s per frame, an infinite rate the driver
 * clamps to its maximum. And because the device reports the real packed
 * interval back, the stored setting never equals it, so every settings
 * update looks like a framerate change and restarts the capture. */

static obs_property_t *framerate_list(obs_properties_t *props)
{
	return obs_properties_add_list(props, "framerate", "Frame Rate", OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_INT);
}

/* The value the source settings get when the user picks the discrete
 * interval numerator/denominator from the list. */
static int64_t picked_framerate(uint32_t numerator, uint32_t denominator)
{
	obs_properties_t *props = obs_properties_create();
	obs_property_t *prop = framerate_list(props);
	struct dstr buffer = {0};

	v4l2_framerate_list_add_discrete(prop, &buffer, numerator, denominator);
	int64_t value = obs_property_list_item_int(prop, 0);

	dstr_free(&buffer);
	obs_properties_destroy(props);
	return value;
}

static void picked_framerate_requests_that_interval(void **state)
{
	(void)state;

	static const uint32_t intervals[][2] = {{1, 30}, {1, 60}, {1, 15}, {1001, 30000}};

	for (size_t i = 0; i < sizeof(intervals) / sizeof(intervals[0]); i++) {
		int64_t value = picked_framerate(intervals[i][0], intervals[i][1]);
		int num = 0, denom = 0;

		/* what v4l2_set_framerate() passes to VIDIOC_S_PARM */
		v4l2_unpack_tuple(&num, &denom, value);
		if ((uint32_t)num != intervals[i][0] || (uint32_t)denom != intervals[i][1]) {
			fprintf(stderr, "picked %u/%u s per frame, setting %lld requests %d/%d\n", intervals[i][0],
				intervals[i][1], (long long)value, num, denom);
			fail();
		}
	}
}

static void control_change_does_not_restart_stream(void **state)
{
	(void)state;

	int64_t setting = picked_framerate(1, 30);
	/* the device accepted the interval and v4l2_set_framerate() reported it */
	int64_t running = v4l2_pack_tuple(1, 30);
	bool unchanged = false;

	/* any settings update, e.g. moving the zoom slider */
	if (v4l2_framerate_changed(running, setting, &unchanged)) {
		fprintf(stderr, "running at %lld, setting %lld: restart on an unrelated change\n", (long long)running,
			(long long)setting);
		fail();
	}
}

static void leave_unchanged_restarts_once(void **state)
{
	(void)state;

	int64_t running = v4l2_pack_tuple(1, 30);
	bool unchanged = false;

	assert_true(v4l2_framerate_changed(running, -1, &unchanged));
	assert_false(v4l2_framerate_changed(running, -1, &unchanged));
	assert_true(v4l2_framerate_changed(running, v4l2_pack_tuple(1, 60), &unchanged));
	assert_false(v4l2_framerate_changed(running, running, &unchanged));
}

/* ------------------------------------------------------------------------ */
/* Scene collections saved with the truncated value
 * (obsproject/obs-studio#14036, by jrsallee). The fix above (#201) stores what
 * is picked from now on, but a scene collection saved by OBS up to 32.2 still
 * holds the truncated value: "framerate": 60 for 60.00 fps, which requests
 * 0/60 s per frame and the camera runs at its maximum (144 fps on the
 * reporter's Elgato 4K X). It also never equals the interval the device
 * reports back, so every settings change restarts the capture. */

/* The discrete frame intervals of the camera in the report: 30, 50, 59.94,
 * 60, 120 and 144 fps */
static const struct v4l2_fract camera_intervals[] = {
	{1, 30}, {1, 50}, {1001, 60000}, {1, 60}, {1, 120}, {1, 144},
};
#define CAMERA_INTERVALS (sizeof(camera_intervals) / sizeof(camera_intervals[0]))

/* What the framerate list stored for an interval before #201 */
static int64_t truncated(uint32_t numerator, uint32_t denominator)
{
	return (int)v4l2_pack_tuple(numerator, denominator);
}

static void expect_interval(int64_t setting, uint32_t numerator, uint32_t denominator)
{
	int64_t value = v4l2_framerate_saved(setting, camera_intervals, CAMERA_INTERVALS);
	int num = 0, denom = 0;

	v4l2_unpack_tuple(&num, &denom, value);
	if (value != v4l2_pack_tuple(numerator, denominator)) {
		fprintf(stderr, "saved %lld (from %u/%u s per frame) requests %d/%d (%lld)\n", (long long)setting,
			numerator, denominator, num, denom, (long long)value);
		fail();
	}
}

static void saved_truncated_framerate_requests_its_interval(void **state)
{
	(void)state;

	for (size_t i = 0; i < CAMERA_INTERVALS; i++) {
		const struct v4l2_fract *iv = &camera_intervals[i];
		expect_interval(truncated(iv->numerator, iv->denominator), iv->numerator, iv->denominator);
	}
}

/* Tuples saved before the 64-bit packing (numerator in the high 16 bits)
 * already request their interval. They are stored in the 64-bit form the
 * device reports back, so they stop restarting the capture on every
 * settings change. */
static void saved_legacy_framerate_is_stored_as_its_interval(void **state)
{
	(void)state;

	expect_interval((1 << 16) | 30, 1, 30);
	expect_interval((1 << 16) | 144, 1, 144);
	expect_interval((1001LL << 16) | 60000, 1001, 60000);
}

static void current_framerate_settings_are_kept(void **state)
{
	(void)state;

	static const int64_t kept[] = {-1, (int64_t)1 << 32 | 60, (int64_t)1001 << 32 | 60000, (int64_t)1 << 32 | 25};
	for (size_t i = 0; i < sizeof(kept) / sizeof(kept[0]); i++)
		assert_int_equal(v4l2_framerate_saved(kept[i], camera_intervals, CAMERA_INTERVALS), kept[i]);

	/* an old tuple for a rate this camera does not list is asked for as before */
	assert_int_equal(v4l2_framerate_saved((1 << 16) | 25, camera_intervals, CAMERA_INTERVALS), (1 << 16) | 25);
}

/* A truncated value no interval of the device matches names no rate at all
 * (0/25 s per frame): leave the device's rate unchanged instead. */
static void unmatched_truncated_framerate_leaves_rate_unchanged(void **state)
{
	(void)state;

	assert_int_equal(v4l2_framerate_saved(25, camera_intervals, CAMERA_INTERVALS), -1);
	assert_int_equal(v4l2_framerate_saved(60, NULL, 0), -1);

	/* A driver that does not reduce its intervals: 30 and 60 fps both keep
	 * 10000000, so which one was picked is unknown. */
	static const struct v4l2_fract unreduced[] = {{333333, 10000000}, {166667, 10000000}};
	assert_int_equal(v4l2_framerate_saved(10000000, unreduced, 2), -1);
	/* but a single one is found */
	assert_int_equal(v4l2_framerate_saved(10000000, unreduced, 1), v4l2_pack_tuple(333333, 10000000));
}

/* ------------------------------------------------------------------------ */
/* The same through the real v4l2-input.c: a fake libv4l2 with the camera
 * above. S_PARM picks the nearest interval like uvcvideo does
 * (uvc_v4l2_set_streamparm(): 0/x s per frame is the shortest interval). No
 * camera, kernel module or display is needed. */

#define CAMERA_PATH "/dev/obs-test-v4l2-framerate"
#define WIDTH 64
#define HEIGHT 48
#define FAKE_BUFFERS 2
#define FAKE_BUFFER_SIZE (WIDTH * HEIGHT * 2)
#define MAX_FDS 1024

static struct {
	int writer[MAX_FDS]; /* write end of the pipe behind an open camera, 0 if closed */
	int opens;
	int s_parms;
	struct v4l2_fract requested;
	struct v4l2_fract running;
	uint8_t memory[FAKE_BUFFERS][FAKE_BUFFER_SIZE];
} fake;

int v4l2_open(const char *file, int oflag, ...)
{
	(void)oflag;
	int p[2];

	/* Only the test's camera exists; real devices are never touched. */
	if (strcmp(file, CAMERA_PATH) != 0) {
		errno = ENOENT;
		return -1;
	}
	/* A read end that never becomes readable: no frames, select() times out */
	if (pipe(p) != 0)
		return -1;
	if (p[0] >= MAX_FDS) {
		close(p[0]);
		close(p[1]);
		errno = EMFILE;
		return -1;
	}
	fake.writer[p[0]] = p[1];
	fake.opens++;
	return p[0];
}

int v4l2_close(int fd)
{
	if (fd >= 0 && fd < MAX_FDS && fake.writer[fd]) {
		close(fake.writer[fd]);
		fake.writer[fd] = 0;
	}
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

static double seconds(struct v4l2_fract f)
{
	return f.denominator ? (double)f.numerator / f.denominator : 0.0;
}

static void camera_set_interval(struct v4l2_fract *timeperframe)
{
	size_t best = 0;
	double want = seconds(*timeperframe);

	for (size_t i = 1; i < CAMERA_INTERVALS; i++) {
		double d = seconds(camera_intervals[i]) - want, best_d = seconds(camera_intervals[best]) - want;
		if ((d < 0 ? -d : d) < (best_d < 0 ? -best_d : best_d))
			best = i;
	}
	fake.s_parms++;
	fake.requested = *timeperframe;
	fake.running = camera_intervals[best];
	*timeperframe = fake.running;
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

	if (fd < 0 || fd >= MAX_FDS || !fake.writer[fd]) {
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
	case VIDIOC_ENUM_FRAMEINTERVALS: {
		struct v4l2_frmivalenum *frmival = arg;
		if (frmival->index >= CAMERA_INTERVALS || frmival->pixel_format != V4L2_PIX_FMT_YUYV ||
		    frmival->width != WIDTH || frmival->height != HEIGHT)
			break;
		frmival->type = V4L2_FRMIVAL_TYPE_DISCRETE;
		frmival->discrete = camera_intervals[frmival->index];
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
		par->parm.capture.timeperframe = fake.running;
		return 0;
	}
	case VIDIOC_S_PARM: {
		struct v4l2_streamparm *par = arg;
		camera_set_interval(&par->parm.capture.timeperframe);
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
	case VIDIOC_S_INPUT:
	case VIDIOC_S_FMT:
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

/* A source as a scene collection saved it */
static obs_data_t *saved_settings(int64_t framerate)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", CAMERA_PATH);
	obs_data_set_int(settings, "input", 0);
	obs_data_set_int(settings, "pixelformat", V4L2_PIX_FMT_YUYV);
	obs_data_set_int(settings, "standard", -1);
	obs_data_set_int(settings, "dv_timing", -1);
	obs_data_set_int(settings, "resolution", v4l2_pack_tuple(WIDTH, HEIGHT));
	obs_data_set_int(settings, "framerate", framerate);
	obs_data_set_int(settings, "color_range", VIDEO_RANGE_DEFAULT);
	obs_data_set_bool(settings, "buffering", true);
	obs_data_set_bool(settings, "auto_reset", false);
	obs_data_set_int(settings, "timeout_frames", 5);
	return settings;
}

static int camera_setup(void **state)
{
	(void)state;
	memset(&fake, 0, sizeof(fake));
	/* the camera starts at its default rate */
	fake.running = (struct v4l2_fract){1, 30};
	return 0;
}

/* Loads a source saved with the given framerate setting, then changes an
 * unrelated setting the way the Properties window or obs-websocket does. */
static void load_saved_source(int64_t saved, uint32_t numerator, uint32_t denominator)
{
	obs_data_t *settings = saved_settings(saved);
	void *source = v4l2_input.create(settings, NULL);
	struct v4l2_fract requested = fake.requested;
	struct v4l2_fract running = fake.running;
	int64_t stored = obs_data_get_int(settings, "framerate");

	int opens = fake.opens;
	obs_data_set_bool(settings, "buffering", false);
	v4l2_input.update(source, settings);
	int restarts = fake.opens - opens;

	v4l2_input.destroy(source);
	obs_data_release(settings);

	if (running.numerator != numerator || running.denominator != denominator) {
		fprintf(stderr,
			"saved framerate %lld: requested %u/%u s per frame, the camera runs %u/%u (%.2f fps), "
			"expected %u/%u\n",
			(long long)saved, requested.numerator, requested.denominator, running.numerator,
			running.denominator, (double)running.denominator / running.numerator, numerator, denominator);
		fail();
	}
	if (stored != v4l2_pack_tuple(numerator, denominator)) {
		fprintf(stderr, "saved framerate %lld is still stored as %lld, not as %u/%u s per frame\n",
			(long long)saved, (long long)stored, numerator, denominator);
		fail();
	}
	if (restarts) {
		fprintf(stderr, "saved framerate %lld: an unrelated settings change restarted the capture\n",
			(long long)saved);
		fail();
	}
}

/* The steps of the report: a scene saved with 60.00 fps picked */
static void source_saved_with_truncated_60_fps_runs_at_60(void **state)
{
	(void)state;
	load_saved_source(truncated(1, 60), 1, 60);
}

static void source_saved_with_truncated_59_94_fps_runs_at_59_94(void **state)
{
	(void)state;
	load_saved_source(truncated(1001, 60000), 1001, 60000);
}

/* Control: what the list stores since #201 */
static void source_saved_with_packed_framerate_runs_at_it(void **state)
{
	(void)state;
	load_saved_source(v4l2_pack_tuple(1, 50), 1, 50);
}

/* A truncated value this camera has no interval for: the camera keeps the
 * rate it runs at instead of being sent 0/25 s per frame, and the setting
 * says so ("Leave Unchanged"). */
static void source_saved_with_unmatched_truncated_framerate_keeps_rate(void **state)
{
	(void)state;
	obs_data_t *settings = saved_settings(25);
	void *source = v4l2_input.create(settings, NULL);
	int64_t stored = obs_data_get_int(settings, "framerate");
	v4l2_input.destroy(source);
	obs_data_release(settings);

	if (fake.s_parms) {
		fprintf(stderr, "saved framerate 25 requested %u/%u s per frame, the camera runs %u/%u\n",
			fake.requested.numerator, fake.requested.denominator, fake.running.numerator,
			fake.running.denominator);
		fail();
	}
	assert_int_equal(fake.running.numerator, 1);
	assert_int_equal(fake.running.denominator, 30);
	assert_int_equal(stored, -1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(picked_framerate_requests_that_interval),
		cmocka_unit_test(control_change_does_not_restart_stream),
		cmocka_unit_test(leave_unchanged_restarts_once),
		cmocka_unit_test(saved_truncated_framerate_requests_its_interval),
		cmocka_unit_test(saved_legacy_framerate_is_stored_as_its_interval),
		cmocka_unit_test(current_framerate_settings_are_kept),
		cmocka_unit_test(unmatched_truncated_framerate_leaves_rate_unchanged),
		cmocka_unit_test_setup(source_saved_with_truncated_60_fps_runs_at_60, camera_setup),
		cmocka_unit_test_setup(source_saved_with_truncated_59_94_fps_runs_at_59_94, camera_setup),
		cmocka_unit_test_setup(source_saved_with_packed_framerate_runs_at_it, camera_setup),
		cmocka_unit_test_setup(source_saved_with_unmatched_truncated_framerate_keeps_rate, camera_setup),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
