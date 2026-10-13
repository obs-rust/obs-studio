#include <errno.h>
#include <fcntl.h>
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
#include <callback/signal.h>

#include "v4l2-helpers.h"
#include "v4l2-udev.h"

/* Saved camera controls are never applied (obsproject/obs-studio#11633 by
 * vpzomtrrfrt, and the closed obsproject/obs-studio#3092, #3865 and #5779):
 * the zoom, focus, exposure... set in the properties of a Video Capture
 * Device are saved, but after a restart, a replug or a settings change from
 * a script or obs-websocket the camera runs on the driver's values until the
 * user moves the slider again.
 *
 * The plugin writes a control (VIDIOC_S_CTRL) only from the property's
 * modified callback, v4l2_control_changed() in v4l2-controls.c, which runs
 * when the user changes the control in the Properties window. Creating the
 * source, v4l2_update() and the hotplug path (device_added() -> v4l2_init())
 * never write the saved values.
 *
 * The tests run the real v4l2-input.c and v4l2-controls.c against a fake
 * libv4l2 (below): a camera whose controls behave like those of a UVC
 * webcam, and which records every write. Hotplug goes through the plugin's
 * own udev signal handlers, which the test raises. No camera, kernel module
 * or display is needed.
 *
 * The saved controls must also stay with the camera they were saved for
 * (follow-up of #363 / #364): a source switched to another camera, or two
 * cameras that swap /dev/videoN nodes after a reboot or a replug, must not
 * get the first camera's exposure, focus or zoom written to them. The fake
 * has a second camera of the same model on another USB port. */

#define WIDTH 64
#define HEIGHT 48
#define FAKE_BUFFERS 2
#define FAKE_BUFFER_SIZE (WIDTH * HEIGHT * 2)
#define MAX_FDS 1024
#define MAX_WRITES 256

static char dir_path[64];
/* /dev/video0 and /dev/video1 */
static char camera_path[128];
static char second_path[128];

/* ------------------------------------------------------------------------ */
/* Fake camera controls, in the order the driver enumerates them (by id) */

enum {
	BRIGHTNESS,
	POWER_LINE,
	PAN_RELATIVE,
	FOCUS_ABSOLUTE,
	FOCUS_AUTO,
	ZOOM,
	CONTROL_COUNT,
};

struct fake_control {
	uint32_t id;
	uint32_t type;
	const char *name;
	int32_t minimum;
	int32_t maximum;
	int32_t step;
	int32_t default_value;
	uint32_t flags;
};

/* Like uvcvideo: the manual focus comes before its automatic switch and is
 * inactive while autofocus is on; a relative control is write-only and
 * moves the camera each time it is written. */
static const struct fake_control controls[CONTROL_COUNT] = {
	[BRIGHTNESS] = {V4L2_CID_BRIGHTNESS, V4L2_CTRL_TYPE_INTEGER, "Brightness", 0, 255, 1, 128, 0},
	[POWER_LINE] = {V4L2_CID_POWER_LINE_FREQUENCY, V4L2_CTRL_TYPE_MENU, "Power Line Frequency", 0, 2, 1, 1, 0},
	[PAN_RELATIVE] = {V4L2_CID_PAN_RELATIVE, V4L2_CTRL_TYPE_INTEGER, "Pan, Relative", -128, 128, 1, 0,
			  V4L2_CTRL_FLAG_WRITE_ONLY},
	[FOCUS_ABSOLUTE] = {V4L2_CID_FOCUS_ABSOLUTE, V4L2_CTRL_TYPE_INTEGER, "Focus (absolute)", 0, 250, 5, 0, 0},
	[FOCUS_AUTO] = {V4L2_CID_FOCUS_AUTO, V4L2_CTRL_TYPE_BOOLEAN, "Focus, Automatic Continuous", 0, 1, 1, 1, 0},
	[ZOOM] = {V4L2_CID_ZOOM_ABSOLUTE, V4L2_CTRL_TYPE_INTEGER, "Zoom, Absolute", 100, 500, 1, 100, 0},
};

static const char *power_line_menu[] = {"Disabled", "50 Hz", "60 Hz"};

/* Two cameras of the same model on two USB ports: only bus_info (from
 * VIDIOC_QUERYCAP) tells them apart. Camera A is the test's camera. */
enum { CAMERA_A, CAMERA_B, CAMERA_COUNT };

static const struct {
	const char *card;
	const char *bus_info;
} cameras[CAMERA_COUNT] = {
	[CAMERA_A] = {"Fake UVC Webcam", "usb-0000:00:14.0-1"},
	[CAMERA_B] = {"Fake UVC Webcam", "usb-0000:00:14.0-2"},
};

#define NO_CAMERA -1

/* ------------------------------------------------------------------------ */
/* Fake libv4l2: a YUYV camera with one input and the controls above, which
 * never delivers a frame (select() on the FIFO times out). */

static struct {
	bool open[MAX_FDS];
	/* the camera behind each descriptor */
	int camera_of[MAX_FDS];
	/* the camera at camera_path and at second_path */
	int at_path[2];
	int opens;
	/* camera A */
	int32_t value[CONTROL_COUNT];
	int writes[CONTROL_COUNT];
	/* camera B */
	int32_t b_value[CONTROL_COUNT];
	int b_writes[CONTROL_COUNT];
	/* every write that changed or moved something, in order */
	int log[MAX_WRITES];
	int log_count;
	int failed_writes;
	int ext_writes;
	/* the driver does not flag the manual focus inactive, it only refuses it */
	bool no_inactive_flag;
	uint8_t memory[FAKE_BUFFERS][FAKE_BUFFER_SIZE];
} fake;

static int32_t *camera_values(int camera)
{
	return camera == CAMERA_B ? fake.b_value : fake.value;
}

static int *camera_writes(int camera)
{
	return camera == CAMERA_B ? fake.b_writes : fake.writes;
}

/* A camera powers up with the driver's values, as after a replug */
static void power_up(int camera)
{
	for (int i = 0; i < CONTROL_COUNT; i++)
		camera_values(camera)[i] = controls[i].default_value;
}

static void power_up_camera(void)
{
	power_up(CAMERA_A);
}

static int find_control(uint32_t id)
{
	for (int i = 0; i < CONTROL_COUNT; i++) {
		if (controls[i].id == id)
			return i;
	}
	return -1;
}

static uint32_t control_flags(int camera, int i)
{
	uint32_t flags = controls[i].flags;
	if (i == FOCUS_ABSOLUTE && camera_values(camera)[FOCUS_AUTO] && !fake.no_inactive_flag)
		flags |= V4L2_CTRL_FLAG_INACTIVE;
	return flags;
}

static int query_control(int camera, struct v4l2_queryctrl *qctrl)
{
	uint32_t next = V4L2_CTRL_FLAG_NEXT_CTRL | V4L2_CTRL_FLAG_NEXT_COMPOUND;
	uint32_t id = qctrl->id & ~next;
	int found = -1;

	if (qctrl->id & V4L2_CTRL_FLAG_NEXT_CTRL) {
		for (int i = 0; i < CONTROL_COUNT; i++) {
			if (controls[i].id > id) {
				found = i;
				break;
			}
		}
	} else {
		found = find_control(id);
	}

	if (found < 0) {
		errno = EINVAL;
		return -1;
	}

	const struct fake_control *c = &controls[found];
	memset(qctrl, 0, sizeof(*qctrl));
	qctrl->id = c->id;
	qctrl->type = c->type;
	snprintf((char *)qctrl->name, sizeof(qctrl->name), "%s", c->name);
	qctrl->minimum = c->minimum;
	qctrl->maximum = c->maximum;
	qctrl->step = c->step;
	qctrl->default_value = c->default_value;
	qctrl->flags = control_flags(camera, found);
	return 0;
}

static int set_control(int camera, struct v4l2_control *control)
{
	int32_t *value = camera_values(camera);
	int i = find_control(control->id);
	if (i < 0) {
		errno = EINVAL;
		return -1;
	}

	camera_writes(camera)[i]++;

	/* Many UVC cameras refuse a manual focus while autofocus is on */
	if (i == FOCUS_ABSOLUTE && value[FOCUS_AUTO]) {
		fake.failed_writes++;
		errno = EBUSY;
		return -1;
	}
	if (control->value < controls[i].minimum || control->value > controls[i].maximum) {
		fake.failed_writes++;
		errno = ERANGE;
		return -1;
	}

	value[i] = control->value;
	if (fake.log_count < MAX_WRITES)
		fake.log[fake.log_count++] = i;
	return 0;
}

static int get_control(int camera, struct v4l2_control *control)
{
	int i = find_control(control->id);
	if (i < 0) {
		errno = EINVAL;
		return -1;
	}
	if (controls[i].flags & V4L2_CTRL_FLAG_WRITE_ONLY) {
		errno = EACCES;
		return -1;
	}
	control->value = camera_values(camera)[i];
	return 0;
}

int v4l2_open(const char *file, int oflag, ...)
{
	/* Only the test's cameras exist; real devices are never touched. */
	int camera = NO_CAMERA;
	if (strcmp(file, camera_path) == 0)
		camera = fake.at_path[0];
	else if (strcmp(file, second_path) == 0)
		camera = fake.at_path[1];
	if (camera == NO_CAMERA) {
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
	fake.camera_of[fd] = camera;
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
	int camera = fake.camera_of[fd];

	switch ((unsigned long)request) {
	case VIDIOC_QUERYCAP: {
		struct v4l2_capability *cap = arg;
		memset(cap, 0, sizeof(*cap));
		snprintf((char *)cap->driver, sizeof(cap->driver), "uvcvideo");
		snprintf((char *)cap->card, sizeof(cap->card), "%s", cameras[camera].card);
		snprintf((char *)cap->bus_info, sizeof(cap->bus_info), "%s", cameras[camera].bus_info);
		cap->device_caps = V4L2_CAP_VIDEO_CAPTURE | V4L2_CAP_STREAMING;
		cap->capabilities = cap->device_caps | V4L2_CAP_DEVICE_CAPS;
		return 0;
	}
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
	case VIDIOC_QUERYCTRL:
		return query_control(camera, arg);
	case VIDIOC_QUERYMENU: {
		struct v4l2_querymenu *qmenu = arg;
		if (qmenu->id != V4L2_CID_POWER_LINE_FREQUENCY || qmenu->index > 2)
			break;
		snprintf((char *)qmenu->name, sizeof(qmenu->name), "%s", power_line_menu[qmenu->index]);
		return 0;
	}
	case VIDIOC_G_CTRL:
		return get_control(camera, arg);
	case VIDIOC_S_CTRL:
		return set_control(camera, arg);
	case VIDIOC_S_EXT_CTRLS: {
		/* Not used by the plugin today; counted so a switch to the
		 * extended API cannot hide writes from the checks below. */
		struct v4l2_ext_controls *ext = arg;
		for (uint32_t n = 0; n < ext->count; n++) {
			struct v4l2_control control = {.id = ext->controls[n].id, .value = ext->controls[n].value};
			fake.ext_writes++;
			if (set_control(camera, &control) != 0)
				return -1;
		}
		return 0;
	}
	case VIDIOC_S_INPUT:
	case VIDIOC_S_FMT:
	case VIDIOC_S_PARM:
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

/* ------------------------------------------------------------------------ */
/* Fake udev monitor: the plugin's hotplug handlers, raised by the test */

static signal_handler_t *udev_signals;

void v4l2_init_udev(void) {}

void v4l2_unref_udev(void) {}

signal_handler_t *v4l2_get_udev_signalhandler(void)
{
	return udev_signals;
}

static void udev_signal_at(const char *signal, const char *path)
{
	calldata_t cd = {0};
	calldata_set_string(&cd, "device", path);
	signal_handler_signal(udev_signals, signal, &cd);
	calldata_free(&cd);
}

static void udev_signal(const char *signal)
{
	udev_signal_at(signal, camera_path);
}

/* The camera is unplugged and plugged in again (or the machine resumes and
 * the camera comes back): it powers up with the driver's values. */
static void replug_camera(void)
{
	udev_signal("device_removed");
	power_up_camera();
	udev_signal("device_added");
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

static obs_data_t *camera_settings(void)
{
	obs_data_t *settings = obs_data_create();
	/* The source's settings carry its defaults, as in OBS */
	v4l2_input.get_defaults(settings);
	obs_data_set_string(settings, "device_id", camera_path);
	obs_data_set_int(settings, "input", 0);
	obs_data_set_int(settings, "pixelformat", V4L2_PIX_FMT_YUYV);
	obs_data_set_int(settings, "standard", -1);
	obs_data_set_int(settings, "dv_timing", -1);
	obs_data_set_int(settings, "resolution", v4l2_pack_tuple(WIDTH, HEIGHT));
	obs_data_set_int(settings, "framerate", v4l2_pack_tuple(1, 30));
	obs_data_set_int(settings, "color_range", VIDEO_RANGE_DEFAULT);
	obs_data_set_bool(settings, "buffering", true);
	obs_data_set_bool(settings, "auto_reset", false);
	obs_data_set_int(settings, "timeout_frames", 5);
	return settings;
}

/* Creates the source the way loading a scene collection does */
static void *create_camera(obs_data_t *settings)
{
	int opens = fake.opens;
	void *source = v4l2_input.create(settings, NULL);
	if (fake.opens == opens) {
		fprintf(stderr, "the source did not open the fake camera\n");
		fail();
	}
	return source;
}

static void check_camera_value(int i, int32_t expected, const char *when)
{
	if (fake.value[i] != expected) {
		fprintf(stderr, "%s: '%s' is %d on the camera, expected the saved %d (%d writes)\n", when,
			controls[i].name, fake.value[i], expected, fake.writes[i]);
		fail();
	}
}

static void check_no_writes(int i, const char *when)
{
	if (fake.writes[i]) {
		fprintf(stderr, "%s: '%s' was written %d times, expected none\n", when, controls[i].name,
			fake.writes[i]);
		fail();
	}
}

static void clear_writes(void)
{
	memset(fake.writes, 0, sizeof(fake.writes));
	memset(fake.b_writes, 0, sizeof(fake.b_writes));
	fake.log_count = 0;
	fake.failed_writes = 0;
	fake.ext_writes = 0;
}

/* ------------------------------------------------------------------------ */

static int setup(void **state)
{
	(void)state;
	memset(&fake, 0, sizeof(fake));
	power_up(CAMERA_A);
	power_up(CAMERA_B);
	fake.at_path[0] = CAMERA_A;
	fake.at_path[1] = CAMERA_B;

	udev_signals = signal_handler_create();
	signal_handler_add(udev_signals, "void device_added(string device)");
	signal_handler_add(udev_signals, "void device_removed(string device)");

	snprintf(dir_path, sizeof(dir_path), "/tmp/obs-v4l2-controls-XXXXXX");
	if (!mkdtemp(dir_path))
		return -1;
	snprintf(camera_path, sizeof(camera_path), "%s/video0", dir_path);
	snprintf(second_path, sizeof(second_path), "%s/video1", dir_path);
	if (mkfifo(camera_path, 0600) != 0)
		return -1;
	return mkfifo(second_path, 0600);
}

static int teardown(void **state)
{
	(void)state;
	signal_handler_destroy(udev_signals);
	udev_signals = NULL;
	unlink(camera_path);
	unlink(second_path);
	rmdir(dir_path);
	return 0;
}

/* OBS starts and loads the scene collection: the saved controls are on the
 * camera, those the user never set are left alone. */
static void saved_controls_are_applied_when_the_source_is_created(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings();
	obs_data_set_int(settings, "Zoom, Absolute", 300);
	obs_data_set_int(settings, "Brightness", 200);
	obs_data_set_int(settings, "Power Line Frequency", 2);

	void *source = create_camera(settings);
	v4l2_input.destroy(source);
	obs_data_release(settings);

	check_camera_value(ZOOM, 300, "after loading");
	check_camera_value(BRIGHTNESS, 200, "after loading");
	check_camera_value(POWER_LINE, 2, "after loading");
	check_no_writes(FOCUS_ABSOLUTE, "after loading");
	check_no_writes(FOCUS_AUTO, "after loading");
	check_no_writes(PAN_RELATIVE, "after loading");
}

/* The camera is unplugged and plugged in again while OBS runs (or comes
 * back after suspend) with the driver's values: OBS opens it again and must
 * apply the saved controls again (obsproject/obs-studio#5779). */
static void saved_controls_are_applied_again_after_a_replug(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings();
	obs_data_set_int(settings, "Zoom, Absolute", 300);
	obs_data_set_int(settings, "Brightness", 200);
	void *source = create_camera(settings);

	int opens = fake.opens;
	replug_camera();
	int reopened = fake.opens - opens;

	v4l2_input.destroy(source);
	obs_data_release(settings);

	if (reopened != 1) {
		fprintf(stderr, "the replug opened the camera %d times, expected once\n", reopened);
		fail();
	}
	check_camera_value(ZOOM, 300, "after the replug");
	check_camera_value(BRIGHTNESS, 200, "after the replug");
}

/* A script or obs-websocket SetInputSettings changes a control: the new
 * value must reach the camera without restarting the capture. */
static void a_control_set_from_a_script_is_applied(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings();
	void *source = create_camera(settings);
	clear_writes();

	int opens = fake.opens;
	obs_data_set_int(settings, "Zoom, Absolute", 400);
	obs_data_set_bool(settings, "Focus, Automatic Continuous", false);
	v4l2_input.update(source, settings);
	int reopened = fake.opens - opens;

	v4l2_input.destroy(source);
	obs_data_release(settings);

	check_camera_value(ZOOM, 400, "after obs_source_update");
	check_camera_value(FOCUS_AUTO, 0, "after obs_source_update");
	check_no_writes(BRIGHTNESS, "after obs_source_update");
	if (reopened > 1) {
		fprintf(stderr, "applying a control reopened the camera %d times\n", reopened);
		fail();
	}
}

/* Saved manual focus: the driver enumerates "Focus (absolute)" before
 * "Focus, Automatic Continuous", and the camera refuses a manual focus while
 * autofocus is on. Both must end up as saved, whatever the order. */
static void manual_focus_is_applied_after_autofocus_is_turned_off(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings();
	obs_data_set_bool(settings, "Focus, Automatic Continuous", false);
	obs_data_set_int(settings, "Focus (absolute)", 120);

	void *source = create_camera(settings);
	v4l2_input.destroy(source);
	obs_data_release(settings);

	check_camera_value(FOCUS_AUTO, 0, "after loading");
	check_camera_value(FOCUS_ABSOLUTE, 120, "after loading");
}

/* The same with a driver that does not flag the manual focus inactive and
 * only refuses it while autofocus is on */
static void manual_focus_is_applied_when_the_driver_only_refuses_it(void **state)
{
	fake.no_inactive_flag = true;
	manual_focus_is_applied_after_autofocus_is_turned_off(state);
}

/* Control: a camera that already has the saved values is not written to,
 * on creation or on a settings update, and controls without a saved value
 * are never written. */
static void unchanged_controls_are_not_written(void **state)
{
	(void)state;
	fake.value[ZOOM] = 300;
	fake.value[BRIGHTNESS] = 200;

	obs_data_t *settings = camera_settings();
	obs_data_set_int(settings, "Zoom, Absolute", 300);
	obs_data_set_int(settings, "Brightness", 200);
	void *source = create_camera(settings);
	obs_data_set_bool(settings, "buffering", false);
	v4l2_input.update(source, settings);
	v4l2_input.destroy(source);
	obs_data_release(settings);

	for (int i = 0; i < CONTROL_COUNT; i++)
		check_no_writes(i, "with the saved values already on the camera");
	assert_int_equal(fake.ext_writes, 0);
}

/* A relative control moves the camera each time it is written; its last
 * value from the Properties window is still in the settings, and must not
 * be replayed on every load, update or replug. */
static void relative_controls_are_not_replayed(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings();
	obs_data_set_int(settings, "Pan, Relative", 10);
	obs_data_set_int(settings, "Zoom, Absolute", 300);
	void *source = create_camera(settings);

	obs_data_set_bool(settings, "buffering", false);
	v4l2_input.update(source, settings);
	obs_data_set_bool(settings, "buffering", true);
	v4l2_input.update(source, settings);
	replug_camera();

	v4l2_input.destroy(source);
	obs_data_release(settings);

	check_no_writes(PAN_RELATIVE, "after loading, two updates and a replug");
}

/* The Properties window: building the properties (obs_source_properties ->
 * obs_properties_apply_settings) must not write the driver's defaults over
 * values set with another tool, and changing one control writes that
 * control only (obsproject/obs-studio#3092). */
static void opening_properties_does_not_write_defaults(void **state)
{
	(void)state;
	/* Set with v4l2-ctl or guvcview; never set in OBS */
	fake.value[ZOOM] = 250;
	fake.value[BRIGHTNESS] = 10;

	obs_data_t *settings = camera_settings();
	void *source = create_camera(settings);
	clear_writes();

	obs_properties_t *props = v4l2_input.get_properties(source);
	obs_properties_apply_settings(props, settings);

	obs_property_t *group = obs_properties_get(props, "controls");
	assert_non_null(group);
	obs_properties_t *ctrl_props = obs_property_group_content(group);
	assert_non_null(obs_properties_get(ctrl_props, "Zoom, Absolute"));
	int writes_on_open = fake.log_count + fake.failed_writes;

	/* The user moves Brightness: the widget writes the setting, runs the
	 * property's modified callback, then updates the source. */
	obs_data_set_int(settings, "Brightness", 50);
	obs_property_modified(obs_properties_get(ctrl_props, "Brightness"), settings);
	v4l2_input.update(source, settings);

	obs_properties_destroy(props);
	v4l2_input.destroy(source);
	obs_data_release(settings);

	if (writes_on_open) {
		fprintf(stderr, "opening the properties wrote %d controls\n", writes_on_open);
		fail();
	}
	check_camera_value(BRIGHTNESS, 50, "after changing Brightness");
	if (fake.value[ZOOM] != 250) {
		fprintf(stderr, "the zoom set outside OBS (250) became %d\n", fake.value[ZOOM]);
		fail();
	}
	check_no_writes(ZOOM, "after opening the properties");
	check_no_writes(FOCUS_AUTO, "after opening the properties");
}

/* ------------------------------------------------------------------------ */
/* "Restore camera controls when the camera connects" (follow-up of #363,
 * raised by alerque): with an external PTZ controller, OBS must not put the
 * saved pan, tilt or zoom back on every start, replug or settings update.
 * Unchecked, a control is written only when the user changes it in the
 * Properties window. */

#define RESTORE_CONTROLS "restore_controls"

static obs_data_t *camera_settings_without_restore(void)
{
	obs_data_t *settings = camera_settings();
	obs_data_set_bool(settings, RESTORE_CONTROLS, false);
	return settings;
}

/* The checkbox, next to the camera controls */
static obs_property_t *restore_property(obs_properties_t *props)
{
	obs_property_t *group = obs_properties_get(props, "controls");
	obs_properties_t *ctrl_props = group ? obs_property_group_content(group) : NULL;
	obs_property_t *prop = ctrl_props ? obs_properties_get(ctrl_props, RESTORE_CONTROLS) : NULL;

	if (!prop || obs_property_get_type(prop) != OBS_PROPERTY_BOOL) {
		fprintf(stderr, "no '%s' checkbox in the Camera Controls group\n", RESTORE_CONTROLS);
		fail();
	}
	return prop;
}

static void check_no_control_writes(const char *when)
{
	for (int i = 0; i < CONTROL_COUNT; i++)
		check_no_writes(i, when);
	if (fake.failed_writes || fake.ext_writes) {
		fprintf(stderr, "%s: %d refused and %d extended control writes, expected none\n", when,
			fake.failed_writes, fake.ext_writes);
		fail();
	}
}

/* The default keeps #364's behaviour, and the checkbox is in the
 * properties, also after the controls are rebuilt for the device. */
static void restoring_controls_is_on_by_default(void **state)
{
	(void)state;
	obs_data_t *defaults = obs_data_create();
	v4l2_input.get_defaults(defaults);
	bool on = obs_data_get_bool(defaults, RESTORE_CONTROLS);
	bool has_default = obs_data_has_default_value(defaults, RESTORE_CONTROLS);
	obs_data_release(defaults);

	if (!has_default || !on) {
		fprintf(stderr, "'%s' defaults to %s, expected on\n", RESTORE_CONTROLS,
			has_default ? "off" : "nothing");
		fail();
	}

	obs_data_t *settings = camera_settings();
	void *source = create_camera(settings);
	obs_properties_t *props = v4l2_input.get_properties(source);
	restore_property(props);
	/* device_selected() rebuilds the controls for the device */
	obs_properties_apply_settings(props, settings);
	const char *tooltip = obs_property_long_description(restore_property(props));
	bool has_tooltip = tooltip && *tooltip;
	obs_properties_destroy(props);
	v4l2_input.destroy(source);
	obs_data_release(settings);

	if (!has_tooltip) {
		fprintf(stderr, "the '%s' checkbox has no tooltip\n", RESTORE_CONTROLS);
		fail();
	}
}

/* Unchecked: OBS starts and loads the scene collection, the camera keeps
 * what the external controller set. */
static void unchecked_saved_controls_are_not_written_on_create(void **state)
{
	(void)state;
	fake.value[ZOOM] = 250;

	obs_data_t *settings = camera_settings_without_restore();
	obs_data_set_int(settings, "Zoom, Absolute", 300);
	obs_data_set_int(settings, "Brightness", 200);
	void *source = create_camera(settings);
	v4l2_input.destroy(source);
	obs_data_release(settings);

	check_no_control_writes("unchecked, after loading");
	check_camera_value(ZOOM, 250, "unchecked, after loading");
}

/* Unchecked: the camera is replugged while OBS runs */
static void unchecked_saved_controls_are_not_written_on_replug(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings_without_restore();
	obs_data_set_int(settings, "Zoom, Absolute", 300);
	obs_data_set_int(settings, "Brightness", 200);
	void *source = create_camera(settings);
	clear_writes();

	int opens = fake.opens;
	replug_camera();
	int reopened = fake.opens - opens;

	v4l2_input.destroy(source);
	obs_data_release(settings);

	if (reopened != 1) {
		fprintf(stderr, "the replug opened the camera %d times, expected once\n", reopened);
		fail();
	}
	check_no_control_writes("unchecked, after the replug");
}

/* Unchecked: the external controller moves the zoom, then a script or
 * obs-websocket updates another setting of the source. */
static void unchecked_saved_controls_are_not_written_on_update(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings_without_restore();
	obs_data_set_int(settings, "Zoom, Absolute", 300);
	void *source = create_camera(settings);
	clear_writes();

	fake.value[ZOOM] = 250;
	obs_data_set_bool(settings, "buffering", false);
	v4l2_input.update(source, settings);

	v4l2_input.destroy(source);
	obs_data_release(settings);

	check_no_control_writes("unchecked, after obs_source_update");
	check_camera_value(ZOOM, 250, "unchecked, after obs_source_update");
}

/* Unchecked: a control the user changes in the Properties window is still
 * written, and only that one. */
static void unchecked_a_control_changed_in_properties_is_written(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings_without_restore();
	obs_data_set_int(settings, "Zoom, Absolute", 300);
	void *source = create_camera(settings);
	fake.value[ZOOM] = 250;
	clear_writes();

	obs_properties_t *props = v4l2_input.get_properties(source);
	obs_properties_apply_settings(props, settings);
	obs_properties_t *ctrl_props = obs_property_group_content(obs_properties_get(props, "controls"));
	obs_property_t *brightness = obs_properties_get(ctrl_props, "Brightness");
	assert_non_null(brightness);

	obs_data_set_int(settings, "Brightness", 50);
	obs_property_modified(brightness, settings);
	v4l2_input.update(source, settings);

	obs_properties_destroy(props);
	v4l2_input.destroy(source);
	obs_data_release(settings);

	check_camera_value(BRIGHTNESS, 50, "unchecked, after changing Brightness");
	check_no_writes(ZOOM, "unchecked, after changing Brightness");
	check_camera_value(ZOOM, 250, "unchecked, after changing Brightness");
}

/* Ticking or unticking the checkbox in the Properties window writes no
 * control: the external controller's zoom stays. */
static void toggling_the_checkbox_writes_nothing(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings_without_restore();
	obs_data_set_int(settings, "Zoom, Absolute", 300);
	void *source = create_camera(settings);
	fake.value[ZOOM] = 250;
	clear_writes();

	obs_properties_t *props = v4l2_input.get_properties(source);
	obs_properties_apply_settings(props, settings);
	obs_property_t *restore = restore_property(props);

	obs_data_set_bool(settings, RESTORE_CONTROLS, true);
	obs_property_modified(restore, settings);
	v4l2_input.update(source, settings);
	int on_writes = fake.log_count + fake.failed_writes + fake.ext_writes;

	obs_data_set_bool(settings, RESTORE_CONTROLS, false);
	obs_property_modified(restore, settings);
	v4l2_input.update(source, settings);
	int writes = fake.log_count + fake.failed_writes + fake.ext_writes;

	obs_properties_destroy(props);
	v4l2_input.destroy(source);
	obs_data_release(settings);

	if (writes) {
		fprintf(stderr, "toggling the checkbox wrote %d controls (%d when ticking it)\n", writes, on_writes);
		fail();
	}
	check_camera_value(ZOOM, 250, "after toggling the checkbox");
}

/* ------------------------------------------------------------------------ */
/* The saved controls stay with their camera (follow-up of #363 / #364). A
 * source can be switched to another camera, and two cameras can swap their
 * /dev/videoN nodes after a reboot or a replug: the controls saved for
 * camera A must never be written to camera B. */

#define CONTROLS_DEVICE "controls_device"

/* The user changes a control in the Properties window: the widget sets the
 * value, runs the property's modified callback, then updates the source. */
static void set_in_properties(void *source, obs_data_t *settings, const char *name, long long value)
{
	obs_properties_t *props = v4l2_input.get_properties(source);
	obs_properties_apply_settings(props, settings);
	obs_properties_t *ctrl_props = obs_property_group_content(obs_properties_get(props, "controls"));
	obs_property_t *prop = obs_properties_get(ctrl_props, name);
	assert_non_null(prop);

	if (obs_property_get_type(prop) == OBS_PROPERTY_BOOL)
		obs_data_set_bool(settings, name, value != 0);
	else
		obs_data_set_int(settings, name, value);
	obs_property_modified(prop, settings);
	v4l2_input.update(source, settings);

	obs_properties_destroy(props);
}

/* The user sets zoom, brightness and the power line frequency on camera A */
static void *create_camera_with_controls_set(obs_data_t *settings)
{
	void *source = create_camera(settings);
	set_in_properties(source, settings, "Zoom, Absolute", 300);
	set_in_properties(source, settings, "Brightness", 200);
	set_in_properties(source, settings, "Power Line Frequency", 2);
	check_camera_value(ZOOM, 300, "after setting it in the properties");
	clear_writes();
	return source;
}

/* OBS saves the scene collection: the source's save callback, if it has
 * one, fills its settings. */
static void save_source(void *source, obs_data_t *settings)
{
	if (v4l2_input.save)
		v4l2_input.save(source, settings);
}

/* Camera B was opened (when expected) and is on its own values, unwritten */
static void check_camera_b_untouched(int opens_before, const char *when)
{
	if (opens_before >= 0 && fake.opens == opens_before) {
		fprintf(stderr, "%s: no camera was opened\n", when);
		fail();
	}
	for (int i = 0; i < CONTROL_COUNT; i++) {
		if (fake.b_writes[i] || fake.b_value[i] != controls[i].default_value) {
			fprintf(stderr, "%s: camera B's '%s' is %d after %d writes, expected its own %d, unwritten\n",
				when, controls[i].name, fake.b_value[i], fake.b_writes[i], controls[i].default_value);
			fail();
		}
	}
}

/* Camera A has the values the user set on it */
static void check_camera_a_restored(const char *when)
{
	check_camera_value(ZOOM, 300, when);
	check_camera_value(BRIGHTNESS, 200, when);
	check_camera_value(POWER_LINE, 2, when);
}

/* The two cameras swap their nodes: OBS sees camera_path go away and come
 * back with camera B behind it. */
static void swap_camera_nodes(void)
{
	udev_signal_at("device_removed", camera_path);
	udev_signal_at("device_removed", second_path);
	fake.at_path[0] = CAMERA_B;
	fake.at_path[1] = CAMERA_A;
	power_up(CAMERA_A);
	power_up(CAMERA_B);
	udev_signal_at("device_added", second_path);
	udev_signal_at("device_added", camera_path);
}

/* A script or obs-websocket SetInputSettings switches the source from
 * camera A to camera B: camera A's controls are not written to B. */
static void switching_the_source_to_another_camera_writes_nothing_to_it(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings();
	void *source = create_camera_with_controls_set(settings);

	int opens = fake.opens;
	obs_data_set_string(settings, "device_id", second_path);
	v4l2_input.update(source, settings);

	v4l2_input.destroy(source);
	obs_data_release(settings);

	check_camera_b_untouched(opens, "after switching the source to camera B");
}

/* A -> B -> A: camera A gets its values back when the source returns to
 * it (here after another program reset it), and B is never written. */
static void switching_back_restores_the_first_cameras_controls(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings();
	void *source = create_camera_with_controls_set(settings);

	int opens = fake.opens;
	obs_data_set_string(settings, "device_id", second_path);
	v4l2_input.update(source, settings);
	power_up(CAMERA_A);
	obs_data_set_string(settings, "device_id", camera_path);
	v4l2_input.update(source, settings);

	v4l2_input.destroy(source);
	obs_data_release(settings);

	check_camera_b_untouched(opens, "after switching to camera B and back");
	check_camera_a_restored("after switching to camera B and back");
}

/* The same switch in the Properties window, where picking the device runs
 * device_selected() before the update: nothing is written to B, the
 * sliders stop showing A's values, and switching back brings A's values
 * back, in the settings and on camera A. */
static void switching_camera_in_properties_keeps_each_cameras_controls(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings();
	void *source = create_camera_with_controls_set(settings);

	obs_properties_t *props = v4l2_input.get_properties(source);
	obs_properties_apply_settings(props, settings);
	obs_property_t *device = obs_properties_get(props, "device_id");

	int opens = fake.opens;
	obs_data_set_string(settings, "device_id", second_path);
	obs_property_modified(device, settings);
	v4l2_input.update(source, settings);
	bool shows_a_on_b = obs_data_has_user_value(settings, "Zoom, Absolute");
	long long zoom_on_b = obs_data_get_int(settings, "Zoom, Absolute");

	power_up(CAMERA_A);
	obs_data_set_string(settings, "device_id", camera_path);
	obs_property_modified(device, settings);
	v4l2_input.update(source, settings);
	long long zoom_on_a = obs_data_get_int(settings, "Zoom, Absolute");

	obs_properties_destroy(props);
	v4l2_input.destroy(source);
	obs_data_release(settings);

	check_camera_b_untouched(opens, "after picking camera B in the properties");
	if (shows_a_on_b) {
		fprintf(stderr, "with camera B picked, the zoom slider shows camera A's %lld\n", zoom_on_b);
		fail();
	}
	check_camera_a_restored("after picking camera A again");
	if (zoom_on_a != 300) {
		fprintf(stderr, "with camera A picked again, the zoom slider shows %lld, expected A's 300\n",
			zoom_on_a);
		fail();
	}
}

/* After the source is switched to camera B, the user changes Brightness on
 * B: only that control is written, and later only it is restored on B. */
static void a_control_changed_on_the_new_camera_is_the_only_one_restored(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings();
	void *source = create_camera_with_controls_set(settings);

	obs_data_set_string(settings, "device_id", second_path);
	v4l2_input.update(source, settings);
	set_in_properties(source, settings, "Brightness", 50);
	int brightness_written = fake.b_writes[BRIGHTNESS];
	udev_signal_at("device_removed", second_path);
	power_up(CAMERA_B);
	udev_signal_at("device_added", second_path);

	v4l2_input.destroy(source);
	obs_data_release(settings);

	if (!brightness_written || fake.b_value[BRIGHTNESS] != 50) {
		fprintf(stderr, "camera B's Brightness is %d after %d writes, expected the 50 set on it\n",
			fake.b_value[BRIGHTNESS], fake.b_writes[BRIGHTNESS]);
		fail();
	}
	if (fake.b_writes[ZOOM] || fake.b_writes[POWER_LINE]) {
		fprintf(stderr, "camera A's zoom and power line frequency were written to camera B (%d, %d writes)\n",
			fake.b_writes[ZOOM], fake.b_writes[POWER_LINE]);
		fail();
	}
}

/* The two cameras swap nodes while OBS runs (replug into other ports, or
 * a USB hub that resets): the source reopens its node, now camera B. */
static void a_camera_that_takes_the_node_on_replug_is_not_written(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings();
	void *source = create_camera_with_controls_set(settings);

	int opens = fake.opens;
	swap_camera_nodes();

	v4l2_input.destroy(source);
	obs_data_release(settings);

	check_camera_b_untouched(opens, "after camera B took the source's node");
}

/* The two cameras swap nodes across a reboot: OBS loads the scene
 * collection and the source opens its node, now camera B. */
static void a_camera_that_takes_the_node_at_startup_is_not_written(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings();
	void *source = create_camera_with_controls_set(settings);
	save_source(source, settings);
	v4l2_input.destroy(source);

	fake.at_path[0] = CAMERA_B;
	fake.at_path[1] = CAMERA_A;
	power_up(CAMERA_A);
	power_up(CAMERA_B);
	clear_writes();

	int opens = fake.opens;
	source = create_camera(settings);
	v4l2_input.destroy(source);
	obs_data_release(settings);

	check_camera_b_untouched(opens, "after starting with camera B on the source's node");
}

/* Control: the same camera replugged into the same port is still
 * restored, once the controls are tied to it. */
static void the_same_camera_is_still_restored_after_a_replug(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings();
	void *source = create_camera_with_controls_set(settings);
	save_source(source, settings);

	replug_camera();

	v4l2_input.destroy(source);
	obs_data_release(settings);

	check_camera_a_restored("after replugging camera A");
}

/* Settings saved before the controls were tied to a camera: they are
 * applied to the first camera opened, as before, and tied to it, so they
 * are not written to the other camera when the nodes swap. */
static void old_settings_are_tied_to_the_first_camera_opened(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings();
	obs_data_set_int(settings, "Zoom, Absolute", 300);
	obs_data_set_int(settings, "Brightness", 200);
	obs_data_set_int(settings, "Power Line Frequency", 2);

	void *source = create_camera(settings);
	check_camera_a_restored("after loading old settings");
	save_source(source, settings);
	bool tied = *obs_data_get_string(settings, CONTROLS_DEVICE) != 0;

	clear_writes();
	int opens = fake.opens;
	swap_camera_nodes();
	v4l2_input.destroy(source);
	check_camera_b_untouched(opens, "old settings, after camera B took the node on replug");

	/* and at the next start, from the saved settings */
	clear_writes();
	opens = fake.opens;
	source = create_camera(settings);
	v4l2_input.destroy(source);
	obs_data_release(settings);
	check_camera_b_untouched(opens, "old settings, at the next start with camera B on the node");

	if (!tied) {
		fprintf(stderr, "saving the source did not record the camera of its old settings\n");
		fail();
	}
}

/* Old settings, then the user picks camera B in the Properties window
 * before the scene collection is saved: the controls stay camera A's. */
static void old_settings_stay_with_the_first_camera_when_switching_in_properties(void **state)
{
	(void)state;
	obs_data_t *settings = camera_settings();
	obs_data_set_int(settings, "Zoom, Absolute", 300);
	obs_data_set_int(settings, "Brightness", 200);
	obs_data_set_int(settings, "Power Line Frequency", 2);
	void *source = create_camera(settings);
	clear_writes();

	obs_properties_t *props = v4l2_input.get_properties(source);
	obs_data_set_string(settings, "device_id", second_path);
	obs_properties_apply_settings(props, settings);
	int opens = fake.opens;
	v4l2_input.update(source, settings);

	obs_properties_destroy(props);
	v4l2_input.destroy(source);
	obs_data_release(settings);

	check_camera_b_untouched(opens, "old settings, after picking camera B in the properties");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(saved_controls_are_applied_when_the_source_is_created, setup, teardown),
		cmocka_unit_test_setup_teardown(saved_controls_are_applied_again_after_a_replug, setup, teardown),
		cmocka_unit_test_setup_teardown(a_control_set_from_a_script_is_applied, setup, teardown),
		cmocka_unit_test_setup_teardown(manual_focus_is_applied_after_autofocus_is_turned_off, setup, teardown),
		cmocka_unit_test_setup_teardown(manual_focus_is_applied_when_the_driver_only_refuses_it, setup,
						teardown),
		cmocka_unit_test_setup_teardown(unchanged_controls_are_not_written, setup, teardown),
		cmocka_unit_test_setup_teardown(relative_controls_are_not_replayed, setup, teardown),
		cmocka_unit_test_setup_teardown(opening_properties_does_not_write_defaults, setup, teardown),
		cmocka_unit_test_setup_teardown(restoring_controls_is_on_by_default, setup, teardown),
		cmocka_unit_test_setup_teardown(unchecked_saved_controls_are_not_written_on_create, setup, teardown),
		cmocka_unit_test_setup_teardown(unchecked_saved_controls_are_not_written_on_replug, setup, teardown),
		cmocka_unit_test_setup_teardown(unchecked_saved_controls_are_not_written_on_update, setup, teardown),
		cmocka_unit_test_setup_teardown(unchecked_a_control_changed_in_properties_is_written, setup, teardown),
		cmocka_unit_test_setup_teardown(toggling_the_checkbox_writes_nothing, setup, teardown),
		cmocka_unit_test_setup_teardown(switching_the_source_to_another_camera_writes_nothing_to_it, setup,
						teardown),
		cmocka_unit_test_setup_teardown(switching_back_restores_the_first_cameras_controls, setup, teardown),
		cmocka_unit_test_setup_teardown(switching_camera_in_properties_keeps_each_cameras_controls, setup,
						teardown),
		cmocka_unit_test_setup_teardown(a_control_changed_on_the_new_camera_is_the_only_one_restored, setup,
						teardown),
		cmocka_unit_test_setup_teardown(a_camera_that_takes_the_node_on_replug_is_not_written, setup, teardown),
		cmocka_unit_test_setup_teardown(a_camera_that_takes_the_node_at_startup_is_not_written, setup,
						teardown),
		cmocka_unit_test_setup_teardown(the_same_camera_is_still_restored_after_a_replug, setup, teardown),
		cmocka_unit_test_setup_teardown(old_settings_are_tied_to_the_first_camera_opened, setup, teardown),
		cmocka_unit_test_setup_teardown(old_settings_stay_with_the_first_camera_when_switching_in_properties,
						setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
