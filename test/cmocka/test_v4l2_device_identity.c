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

/* The Video Capture Device source follows a /dev/videoN number
 * (obs-rust/obs-studio#2262, upstream obsproject/obs-studio#3824 and #4972).
 *
 * The source saves its camera as a device node path, and the hotplug
 * handlers compare that path. The kernel numbers the nodes in the order the
 * devices appear, so a replugged camera can come back under another number:
 * the source then never resumes. And when another camera takes the old
 * number, the source silently captures that camera instead.
 *
 * The tests run the real v4l2-input.c against a fake libv4l2 and a fake
 * sysfs and /dev (FIXTURE_DIR, a directory in the build tree). A "camera" is
 * what VIDIOC_QUERYCAP reports (card, bus_info) plus what sysfs says about
 * its node (the node index, and the USB serial number if it has one). The
 * nodes are FIFOs, so select() on them never fires and no frame arrives.
 * Hotplug goes through the plugin's udev signal handlers, which the test
 * raises after it changes the fake sysfs, as udev would. No camera, kernel
 * module, root or display is needed.
 *
 * test_v4l2_vivid_replug runs the same cases end to end with the kernel's
 * vivid driver (needs root). */

#ifndef FIXTURE_DIR
#error FIXTURE_DIR must name a scratch directory for the fake /dev and sysfs
#endif

#define WIDTH 64
#define HEIGHT 48
#define FAKE_BUFFERS 2
#define FAKE_BUFFER_SIZE (WIDTH * HEIGHT * 2)
#define MAX_FDS 1024
#define NODES 3
#define NO_CAMERA -1

/* ------------------------------------------------------------------------ */
/* The cameras */

enum {
	/* two webcams of one model, each with its own serial number */
	SERIAL_A,
	SERIAL_B,
	/* camera A with its serial number, on another USB port */
	SERIAL_A_OTHER_PORT,
	/* two webcams of one model without a serial number, on two ports */
	TWIN_1,
	TWIN_2,
	/* twin 1 on a third port */
	TWIN_1_OTHER_PORT,
	/* two instances of a capture driver without USB (like vivid) */
	CARD_0,
	CARD_1,
	CAMERA_COUNT,
};

static const struct {
	const char *card;
	const char *bus_info;
	/* the USB device's serial attribute in sysfs; NULL if it has none,
	 * and no USB device at all for a platform bus */
	const char *serial;
} cameras[CAMERA_COUNT] = {
	[SERIAL_A] = {"Fake UVC Webcam", "usb-0000:00:14.0-1", "A1B2C3"},
	[SERIAL_B] = {"Fake UVC Webcam", "usb-0000:00:14.0-2", "D4E5F6"},
	[SERIAL_A_OTHER_PORT] = {"Fake UVC Webcam", "usb-0000:00:14.0-3", "A1B2C3"},
	[TWIN_1] = {"Fake Cheap Webcam", "usb-0000:00:14.0-1", NULL},
	[TWIN_2] = {"Fake Cheap Webcam", "usb-0000:00:14.0-2", NULL},
	[TWIN_1_OTHER_PORT] = {"Fake Cheap Webcam", "usb-0000:00:14.0-3", NULL},
	[CARD_0] = {"Fake Capture", "platform:fake-000", NULL},
	[CARD_1] = {"Fake Capture", "platform:fake-001", NULL},
};

static bool is_usb(int camera)
{
	return strncmp(cameras[camera].bus_info, "usb-", 4) == 0;
}

extern struct obs_source_info v4l2_input;

/* The fake udev monitor's signals (below) */
static signal_handler_t *udev_signals;
/* The source under test; teardown destroys it if a check failed first */
static void *current_source;

/* ------------------------------------------------------------------------ */
/* Fake /dev and sysfs */

static char node_path[NODES][256];

static void run_or_fail(const char *what, int result)
{
	if (result != 0) {
		fprintf(stderr, "fixture: %s failed: %s\n", what, strerror(errno));
		fail();
	}
}

/* snprintf into a path buffer, failing the test if it does not fit */
#define PATHF(buf, ...)                                                       \
	do {                                                                  \
		if (snprintf(buf, sizeof(buf), __VA_ARGS__) >= (int)sizeof(buf)) { \
			fprintf(stderr, "fixture: path too long\n");         \
			fail();                                               \
		}                                                             \
	} while (0)

static void write_file(const char *path, const char *text)
{
	FILE *f = fopen(path, "w");
	if (!f) {
		fprintf(stderr, "fixture: cannot write %s: %s\n", path, strerror(errno));
		fail();
	}
	fputs(text, f);
	fclose(f);
}

static void remove_tree(const char *path)
{
	char cmd[512];
	PATHF(cmd, "rm -rf '%s'", path);
	if (system(cmd) != 0)
		fprintf(stderr, "fixture: could not remove %s\n", path);
}

/* ------------------------------------------------------------------------ */
/* Fake libv4l2 */

static struct {
	/* the camera behind each node, NO_CAMERA when nothing is plugged in */
	int at_node[NODES];
	bool open[MAX_FDS];
	int camera_of[MAX_FDS];
	/* the descriptor had its format set: the source captures from it */
	bool capturing[MAX_FDS];
	int opens;
	/* VIDIOC_S_FMT calls per camera */
	int formats[CAMERA_COUNT];
	uint8_t memory[FAKE_BUFFERS][FAKE_BUFFER_SIZE];
} fake;

/* What udev and the kernel show for a node once a camera is plugged in, laid
 * out as in sysfs: class/video4linux/videoN is a link to the node's
 * directory under its device, which has the node's index and a "device" link
 * back to the device. A USB camera's device is a USB interface, whose
 * parent is the USB device with idVendor and, if it has one, serial. */
static void write_sysfs_node(int node)
{
	char link[512], device_dir[512], node_dir[600], path[700], text[160];
	int camera = fake.at_node[node];

	PATHF(link, FIXTURE_DIR "/sys/class/video4linux/video%d", node);
	char old[512];
	ssize_t len = readlink(link, old, sizeof(old) - 1);
	if (len > 0) {
		old[len] = 0;
		remove_tree(old);
	}
	unlink(link);
	if (camera == NO_CAMERA)
		return;

	if (is_usb(camera)) {
		char usb_dir[512];
		PATHF(usb_dir, FIXTURE_DIR "/sys/devices/usb/%s", cameras[camera].bus_info);
		remove_tree(usb_dir);
		run_or_fail("mkdir usb device", mkdir(usb_dir, 0755));
		PATHF(path, "%s/idVendor", usb_dir);
		write_file(path, "1234\n");
		if (cameras[camera].serial) {
			PATHF(path, "%s/serial", usb_dir);
			PATHF(text, "%s\n", cameras[camera].serial);
			write_file(path, text);
		}
		PATHF(device_dir, "%s/1.0", usb_dir);
	} else {
		PATHF(device_dir, FIXTURE_DIR "/sys/devices/platform/%s",
		      cameras[camera].bus_info + strlen("platform:"));
		remove_tree(device_dir);
	}
	run_or_fail("mkdir device", mkdir(device_dir, 0755));
	PATHF(node_dir, "%s/video4linux", device_dir);
	mkdir(node_dir, 0755);
	PATHF(node_dir, "%s/video4linux/video%d", device_dir, node);
	run_or_fail("mkdir node", mkdir(node_dir, 0755));
	PATHF(path, "%s/index", node_dir);
	write_file(path, "0\n");
	PATHF(path, "%s/name", node_dir);
	write_file(path, cameras[camera].card);
	PATHF(path, "%s/device", node_dir);
	run_or_fail("symlink device", symlink(device_dir, path));
	run_or_fail("symlink class entry", symlink(node_dir, link));
}

static int setup(void **state)
{
	(void)state;
	memset(&fake, 0, sizeof(fake));
	for (int i = 0; i < NODES; i++)
		fake.at_node[i] = NO_CAMERA;

	remove_tree(FIXTURE_DIR);
	if (mkdir(FIXTURE_DIR, 0755) != 0 || mkdir(FIXTURE_DIR "/dev", 0755) != 0 ||
	    mkdir(FIXTURE_DIR "/sys", 0755) != 0 || mkdir(FIXTURE_DIR "/sys/class", 0755) != 0 ||
	    mkdir(FIXTURE_DIR "/sys/class/video4linux", 0755) != 0 || mkdir(FIXTURE_DIR "/sys/devices", 0755) != 0 ||
	    mkdir(FIXTURE_DIR "/sys/devices/usb", 0755) != 0 || mkdir(FIXTURE_DIR "/sys/devices/platform", 0755) != 0)
		return -1;

	for (int i = 0; i < NODES; i++) {
		snprintf(node_path[i], sizeof(node_path[i]), FIXTURE_DIR "/dev/video%d", i);
		if (mkfifo(node_path[i], 0600) != 0)
			return -1;
	}
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (current_source)
		v4l2_input.destroy(current_source);
	current_source = NULL;
	if (udev_signals)
		signal_handler_destroy(udev_signals);
	udev_signals = NULL;
	remove_tree(FIXTURE_DIR);
	return 0;
}

int v4l2_open(const char *file, int oflag, ...)
{
	/* Only the test's nodes exist; real devices are never touched. */
	int camera = NO_CAMERA;
	for (int i = 0; i < NODES; i++) {
		if (strcmp(file, node_path[i]) == 0)
			camera = fake.at_node[i];
	}
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
	fake.capturing[fd] = false;
	fake.camera_of[fd] = camera;
	fake.opens++;
	return fd;
}

int v4l2_close(int fd)
{
	if (fd >= 0 && fd < MAX_FDS) {
		fake.open[fd] = false;
		fake.capturing[fd] = false;
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
		snprintf((char *)cap->driver, sizeof(cap->driver), is_usb(camera) ? "uvcvideo" : "fake");
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
	case VIDIOC_S_FMT:
		fake.capturing[fd] = true;
		fake.formats[camera]++;
		return 0;
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
	case VIDIOC_S_INPUT:
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

	/* Enumerations end here, the camera has no controls, and anything
	 * else is not supported */
	errno = EINVAL;
	return -1;
}

/* ------------------------------------------------------------------------ */
/* Fake udev monitor: the plugin's hotplug handlers, raised by the test */

void v4l2_init_udev(void)
{
	if (!udev_signals) {
		udev_signals = signal_handler_create();
		signal_handler_add(udev_signals, "void device_added(string device)");
		signal_handler_add(udev_signals, "void device_removed(string device)");
	}
}

void v4l2_unref_udev(void) {}

signal_handler_t *v4l2_get_udev_signalhandler(void)
{
	return udev_signals;
}

static void udev_signal(const char *signal, int node)
{
	calldata_t cd = {0};
	calldata_set_string(&cd, "device", node_path[node]);
	signal_handler_signal(udev_signals, signal, &cd);
	calldata_free(&cd);
}

/* The camera at a node is unplugged: the node goes away */
static void unplug(int node)
{
	fake.at_node[node] = NO_CAMERA;
	write_sysfs_node(node);
	udev_signal("device_removed", node);
}

/* A camera is plugged in and gets a node; udev announces it once sysfs
 * describes it. plug_quietly() leaves the announcement for later, as when
 * several cameras appear at once. */
static void plug_quietly(int camera, int node)
{
	fake.at_node[node] = camera;
	write_sysfs_node(node);
}

static void plug(int camera, int node)
{
	plug_quietly(camera, node);
	udev_signal("device_added", node);
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

/* ------------------------------------------------------------------------ */

static obs_data_t *camera_settings(int node)
{
	obs_data_t *settings = obs_data_create();
	/* The source's settings carry its defaults, as in OBS */
	v4l2_input.get_defaults(settings);
	obs_data_set_string(settings, "device_id", node_path[node]);
	obs_data_set_int(settings, "input", 0);
	obs_data_set_int(settings, "pixelformat", V4L2_PIX_FMT_YUYV);
	obs_data_set_int(settings, "resolution", v4l2_pack_tuple(WIDTH, HEIGHT));
	obs_data_set_int(settings, "framerate", v4l2_pack_tuple(1, 30));
	return settings;
}

/* The camera the source captures from, NO_CAMERA if none */
static int captured_camera(void)
{
	int camera = NO_CAMERA;
	for (int fd = 0; fd < MAX_FDS; fd++) {
		if (fake.open[fd] && fake.capturing[fd]) {
			if (camera != NO_CAMERA) {
				fprintf(stderr, "the source captures from two cameras at once\n");
				fail();
			}
			camera = fake.camera_of[fd];
		}
	}
	return camera;
}

static const char *camera_name(int camera)
{
	static char name[2][96];
	static int next;
	char *buf = name[next++ % 2];
	if (camera == NO_CAMERA)
		return "no camera";
	snprintf(buf, sizeof(name[0]), "%s (%s)", cameras[camera].card, cameras[camera].bus_info);
	return buf;
}

static void check_captures(int expected, const char *when)
{
	int camera = captured_camera();
	if (camera != expected) {
		fprintf(stderr, "%s: the source captures %s, expected %s\n", when, camera_name(camera),
			camera_name(expected));
		fail();
	}
}

static void check_never_captured(int camera, const char *when)
{
	if (fake.formats[camera]) {
		fprintf(stderr, "%s: the source set up %s for capture %d times, expected never\n", when,
			camera_name(camera), fake.formats[camera]);
		fail();
	}
}

/* OBS saves the scene collection: the source's save callback, if it has
 * one, fills its settings. */
static void save_source(void *source, obs_data_t *settings)
{
	if (v4l2_input.save)
		v4l2_input.save(source, settings);
}

static void check_saved_node(obs_data_t *settings, int node, const char *when)
{
	const char *saved = obs_data_get_string(settings, "device_id");
	if (strcmp(saved, node_path[node]) != 0) {
		fprintf(stderr, "%s: device_id is %s, expected %s\n", when, saved, node_path[node]);
		fail();
	}
}

/* The settings name the camera itself, next to device_id */
static void check_saved_camera(obs_data_t *settings, int camera, const char *when)
{
	obs_data_t *identity = obs_data_get_obj(settings, "device_identity");
	const char *card = identity ? obs_data_get_string(identity, "card") : "";
	const char *bus_info = identity ? obs_data_get_string(identity, "bus_info") : "";
	bool same = strcmp(card, cameras[camera].card) == 0 && strcmp(bus_info, cameras[camera].bus_info) == 0;
	if (!same) {
		fprintf(stderr, "%s: the settings name the camera '%s (%s)', expected %s\n", when, card, bus_info,
			camera_name(camera));
		obs_data_release(identity);
		fail();
	}
	obs_data_release(identity);
}

static void *create_source(obs_data_t *settings)
{
	current_source = v4l2_input.create(settings, NULL);
	return current_source;
}

static void destroy_source(void *source)
{
	v4l2_input.destroy(source);
	if (source == current_source)
		current_source = NULL;
}

/* ------------------------------------------------------------------------ */
/* The report */

/* The camera is unplugged and comes back under another number, here
 * because another device took its old one in the meantime: the source
 * resumes on the new node, and its settings name it. */
static void a_camera_replugged_under_another_number_resumes(void **state)
{
	(void)state;
	plug_quietly(SERIAL_A, 0);
	obs_data_t *settings = camera_settings(0);
	void *source = create_source(settings);
	check_captures(SERIAL_A, "after creating the source");

	unplug(0);
	check_captures(NO_CAMERA, "after unplugging the camera");
	plug(SERIAL_A, 1);
	int camera = captured_camera();
	save_source(source, settings);

	destroy_source(source);
	if (camera != SERIAL_A) {
		fprintf(stderr, "after the camera came back as video1: the source captures %s, expected %s\n",
			camera_name(camera), camera_name(SERIAL_A));
		obs_data_release(settings);
		fail();
	}
	check_saved_node(settings, 1, "after the camera came back as video1");
	obs_data_release(settings);
}

/* While the camera is away, another camera of the same model (another
 * serial number) is plugged in and gets the old number: the source must
 * not capture it. When the camera comes back, under a new number, the
 * source captures it. */
static void another_camera_at_the_old_number_is_not_captured(void **state)
{
	(void)state;
	plug_quietly(SERIAL_A, 0);
	obs_data_t *settings = camera_settings(0);
	void *source = create_source(settings);

	unplug(0);
	plug(SERIAL_B, 0);
	int while_away = captured_camera();
	plug(SERIAL_A, 1);
	int after_return = captured_camera();

	destroy_source(source);
	obs_data_release(settings);

	check_never_captured(SERIAL_B, "with another camera at the old number");
	if (while_away != NO_CAMERA || after_return != SERIAL_A) {
		fprintf(stderr,
			"captured %s while the camera was away and %s after it came back as video1, expected "
			"nothing, then %s\n",
			camera_name(while_away), camera_name(after_return), camera_name(SERIAL_A));
		fail();
	}
}

/* The same with a capture device that is not on USB (bus info
 * "platform:..."), as the kernel's vivid driver: the bus info is the
 * device, so another instance at the old number is another device. */
static void another_capture_device_at_the_old_number_is_not_captured(void **state)
{
	(void)state;
	plug_quietly(CARD_0, 0);
	obs_data_t *settings = camera_settings(0);
	void *source = create_source(settings);

	unplug(0);
	plug(CARD_1, 0);
	int while_away = captured_camera();
	plug(CARD_0, 1);
	int after_return = captured_camera();

	destroy_source(source);
	obs_data_release(settings);

	check_never_captured(CARD_1, "with another capture device at the old number");
	if (while_away != NO_CAMERA || after_return != CARD_0) {
		fprintf(stderr, "captured %s, then %s; expected nothing, then %s\n", camera_name(while_away),
			camera_name(after_return), camera_name(CARD_0));
		fail();
	}
}

/* Two cameras swap numbers while OBS is closed (another boot order): at
 * the next start the source opens its camera, not the one at its old
 * number. */
static void a_renumbered_camera_is_found_at_startup(void **state)
{
	(void)state;
	plug_quietly(SERIAL_A, 0);
	plug_quietly(SERIAL_B, 1);
	obs_data_t *settings = camera_settings(0);
	void *source = create_source(settings);
	save_source(source, settings);
	destroy_source(source);

	plug_quietly(SERIAL_B, 0);
	plug_quietly(SERIAL_A, 1);
	memset(fake.formats, 0, sizeof(fake.formats));
	source = create_source(settings);
	int camera = captured_camera();
	save_source(source, settings);
	destroy_source(source);

	check_never_captured(SERIAL_B, "at startup with the cameras renumbered");
	if (camera != SERIAL_A) {
		fprintf(stderr, "at startup with the cameras renumbered: captures %s, expected %s\n",
			camera_name(camera), camera_name(SERIAL_A));
		obs_data_release(settings);
		fail();
	}
	check_saved_node(settings, 1, "at startup with the cameras renumbered");
	obs_data_release(settings);
}

/* A camera with a serial number moved to another USB port: its bus info
 * changes, its serial number does not. */
static void a_camera_moved_to_another_port_is_followed(void **state)
{
	(void)state;
	plug_quietly(SERIAL_A, 0);
	obs_data_t *settings = camera_settings(0);
	void *source = create_source(settings);

	unplug(0);
	plug(SERIAL_B, 0);
	plug(SERIAL_A_OTHER_PORT, 1);
	int camera = captured_camera();

	destroy_source(source);
	obs_data_release(settings);

	check_never_captured(SERIAL_B, "after moving the camera to another port");
	if (camera != SERIAL_A_OTHER_PORT) {
		fprintf(stderr, "after moving the camera to another port: captures %s, expected %s\n",
			camera_name(camera), camera_name(SERIAL_A_OTHER_PORT));
		fail();
	}
}

/* Two cameras of one model without serial numbers are told apart by their
 * USB port: both are unplugged and plugged back into the same ports, but
 * get each other's numbers. */
static void identical_cameras_without_serial_are_told_apart_by_port(void **state)
{
	(void)state;
	plug_quietly(TWIN_1, 0);
	plug_quietly(TWIN_2, 1);
	obs_data_t *settings = camera_settings(0);
	void *source = create_source(settings);

	unplug(0);
	unplug(1);
	plug_quietly(TWIN_2, 0);
	plug_quietly(TWIN_1, 1);
	udev_signal("device_added", 0);
	udev_signal("device_added", 1);
	int camera = captured_camera();

	destroy_source(source);
	obs_data_release(settings);

	check_never_captured(TWIN_2, "after the twins swapped numbers");
	if (camera != TWIN_1) {
		fprintf(stderr, "after the twins swapped numbers: captures %s, expected %s\n", camera_name(camera),
			camera_name(TWIN_1));
		fail();
	}
}

/* A camera without a serial number cannot be told from its twin on
 * another port; when only the twin is there, the source takes it (it may
 * be the same camera, moved), but the camera itself wins when it comes
 * back on its own port. */
static void the_camera_replaces_its_twin_when_it_comes_back(void **state)
{
	(void)state;
	plug_quietly(TWIN_1, 0);
	obs_data_t *settings = camera_settings(0);
	void *source = create_source(settings);

	unplug(0);
	plug(TWIN_2, 0);
	plug(TWIN_1, 1);
	int camera = captured_camera();

	destroy_source(source);
	obs_data_release(settings);

	if (camera != TWIN_1) {
		fprintf(stderr, "after the camera came back on its own port: captures %s, expected %s\n",
			camera_name(camera), camera_name(TWIN_1));
		fail();
	}
}

/* Settings saved before the camera was recorded (only device_id) still
 * work: the camera at the path is captured and resumes after a replug at
 * the same number. The camera is recorded the first time it is opened, so
 * at the next start the source finds it even after a renumbering. */
static void old_settings_with_only_a_path_still_work(void **state)
{
	(void)state;
	plug_quietly(SERIAL_A, 0);
	plug_quietly(SERIAL_B, 1);
	obs_data_t *settings = camera_settings(0);
	void *source = create_source(settings);
	check_captures(SERIAL_A, "old settings, after creating the source");

	unplug(0);
	plug(SERIAL_A, 0);
	check_captures(SERIAL_A, "old settings, after a replug at the same number");
	save_source(source, settings);
	destroy_source(source);
	check_saved_camera(settings, SERIAL_A, "old settings, once saved");

	plug_quietly(SERIAL_B, 0);
	plug_quietly(SERIAL_A, 1);
	source = create_source(settings);
	int camera = captured_camera();
	destroy_source(source);
	obs_data_release(settings);

	if (camera != SERIAL_A) {
		fprintf(stderr, "old settings, next start with the cameras renumbered: captures %s, expected %s\n",
			camera_name(camera), camera_name(SERIAL_A));
		fail();
	}
}

/* Picking a camera in the Properties window stores both its node and the
 * camera; opening the window does not change which camera it is, even
 * when another camera now sits at the node. */
static void picking_a_camera_in_properties_stores_it(void **state)
{
	(void)state;
	plug_quietly(SERIAL_A, 0);
	plug_quietly(SERIAL_B, 1);
	obs_data_t *settings = camera_settings(0);
	void *source = create_source(settings);

	obs_properties_t *props = v4l2_input.get_properties(source);
	obs_properties_apply_settings(props, settings);
	obs_property_t *device = obs_properties_get(props, "device_id");
	size_t listed = obs_property_list_item_count(device);
	obs_data_set_string(settings, "device_id", node_path[1]);
	obs_property_modified(device, settings);
	v4l2_input.update(source, settings);
	obs_properties_destroy(props);
	int picked = captured_camera();
	save_source(source, settings);

	if (listed != 2) {
		fprintf(stderr, "the device list has %zu entries, expected the 2 cameras\n", listed);
		fail();
	}
	if (picked != SERIAL_B) {
		fprintf(stderr, "after picking video1 in the properties: captures %s, expected %s\n",
			camera_name(picked), camera_name(SERIAL_B));
		fail();
	}
	check_saved_camera(settings, SERIAL_B, "after picking video1 in the properties");

	/* camera B goes away and camera A takes its number; opening the
	 * properties and pressing OK keeps waiting for camera B */
	unplug(1);
	unplug(0);
	plug(SERIAL_A, 1);
	props = v4l2_input.get_properties(source);
	obs_properties_apply_settings(props, settings);
	v4l2_input.update(source, settings);
	obs_properties_destroy(props);
	int after_ok = captured_camera();

	destroy_source(source);
	check_saved_camera(settings, SERIAL_B, "after opening the properties with camera B away");
	obs_data_release(settings);
	if (after_ok != NO_CAMERA) {
		fprintf(stderr, "after opening the properties with camera B away: captures %s, expected nothing\n",
			camera_name(after_ok));
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* Controls: these pass before the fix too */

/* A camera without a serial number moved to another port that keeps its
 * number is still captured, as before. */
static void a_camera_without_serial_moved_to_another_port_is_still_captured(void **state)
{
	(void)state;
	plug_quietly(TWIN_1, 0);
	obs_data_t *settings = camera_settings(0);
	void *source = create_source(settings);

	unplug(0);
	plug(TWIN_1_OTHER_PORT, 0);
	int camera = captured_camera();

	destroy_source(source);
	obs_data_release(settings);

	if (camera != TWIN_1_OTHER_PORT) {
		fprintf(stderr, "after moving the camera to another port: captures %s, expected %s\n",
			camera_name(camera), camera_name(TWIN_1_OTHER_PORT));
		fail();
	}
}

/* Another camera plugged in while the source captures does not restart
 * the capture. */
static void plugging_in_another_camera_leaves_the_capture_alone(void **state)
{
	(void)state;
	plug_quietly(SERIAL_A, 0);
	obs_data_t *settings = camera_settings(0);
	void *source = create_source(settings);
	int formats = fake.formats[SERIAL_A];

	plug(SERIAL_B, 1);
	int camera = captured_camera();
	int restarts = fake.formats[SERIAL_A] - formats;

	destroy_source(source);
	obs_data_release(settings);

	check_never_captured(SERIAL_B, "after plugging in another camera");
	if (camera != SERIAL_A || restarts) {
		fprintf(stderr, "after plugging in another camera: captures %s after %d restarts, expected %s\n",
			camera_name(camera), restarts, camera_name(SERIAL_A));
		fail();
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(a_camera_replugged_under_another_number_resumes, setup, teardown),
		cmocka_unit_test_setup_teardown(another_camera_at_the_old_number_is_not_captured, setup, teardown),
		cmocka_unit_test_setup_teardown(another_capture_device_at_the_old_number_is_not_captured, setup,
						teardown),
		cmocka_unit_test_setup_teardown(a_renumbered_camera_is_found_at_startup, setup, teardown),
		cmocka_unit_test_setup_teardown(a_camera_moved_to_another_port_is_followed, setup, teardown),
		cmocka_unit_test_setup_teardown(identical_cameras_without_serial_are_told_apart_by_port, setup,
						teardown),
		cmocka_unit_test_setup_teardown(the_camera_replaces_its_twin_when_it_comes_back, setup, teardown),
		cmocka_unit_test_setup_teardown(old_settings_with_only_a_path_still_work, setup, teardown),
		cmocka_unit_test_setup_teardown(picking_a_camera_in_properties_stores_it, setup, teardown),
		cmocka_unit_test_setup_teardown(a_camera_without_serial_moved_to_another_port_is_still_captured, setup,
						teardown),
		cmocka_unit_test_setup_teardown(plugging_in_another_camera_leaves_the_capture_alone, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
