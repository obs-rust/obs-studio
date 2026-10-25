#include <dirent.h>
#include <errno.h>
#include <inttypes.h>
#include <fcntl.h>
#include <poll.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <setjmp.h>
#include <sys/ioctl.h>
#include <sys/wait.h>
#include <cmocka.h>

#include <libudev.h>
#include <linux/videodev2.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

/* A camera that fails without being unplugged freezes the Video Capture
 * Device source for good (obs-rust/obs-studio#2264, upstream
 * obsproject/obs-studio#9434), end to end: real libobs and the built
 * linux-v4l2 module capture from the kernel's vivid test driver onto the
 * program output.
 *
 * vivid's "Disconnect" control makes every file operation on its nodes fail
 * with ENODEV without unregistering them, so udev sends no event, as with a
 * camera whose firmware hangs. vivid reconnects once the last user closes
 * the node. Before the fix the source kept the node open and stayed on its
 * last frame; now it closes the device after the VIDIOC_DQBUF error and
 * opens it again.
 *
 * Needs root (modprobe, rmmod), the vivid module, vivid not already loaded
 * (the test leaves someone else's instance alone) and a display for core
 * video (run under xvfb-run). Otherwise it exits 77 and ctest reports it
 * skipped. test_v4l2_dqbuf_error covers the same with a fake device, in CI. */

#define BASE_W 320
#define BASE_H 240
#define VIVID_PARAMS "n_devs=1 node_types=0x1"
#define VIVID_NODES "/sys/bus/platform/devices/vivid.0/video4linux"

static bool obs_up;
/* the environment cannot run the test */
static bool skipped;
static obs_module_t *v4l2_module;
static bool vivid_ours;

/* ------------------------------------------------------------------------ */
/* UI tasks, run by the test's thread while it waits, as the frontend's */

#define MAX_TASKS 64
static pthread_mutex_t task_mutex = PTHREAD_MUTEX_INITIALIZER;
static struct {
	obs_task_t task;
	void *param;
} tasks[MAX_TASKS];
static size_t task_count;
static pthread_t main_thread;

static void ui_task_handler(obs_task_t task, void *param, bool wait)
{
	if (wait || pthread_equal(pthread_self(), main_thread)) {
		task(param);
		return;
	}
	pthread_mutex_lock(&task_mutex);
	if (task_count < MAX_TASKS) {
		tasks[task_count].task = task;
		tasks[task_count].param = param;
		task_count++;
	}
	pthread_mutex_unlock(&task_mutex);
}

static void run_ui_tasks(void)
{
	for (;;) {
		pthread_mutex_lock(&task_mutex);
		if (!task_count) {
			pthread_mutex_unlock(&task_mutex);
			return;
		}
		obs_task_t task = tasks[0].task;
		void *param = tasks[0].param;
		memmove(tasks, tasks + 1, (task_count - 1) * sizeof(tasks[0]));
		task_count--;
		pthread_mutex_unlock(&task_mutex);
		task(param);
	}
}

/* ------------------------------------------------------------------------ */
/* The program output: a hash of each frame, to see the picture move */

static pthread_mutex_t frame_mutex = PTHREAD_MUTEX_INITIALIZER;
static uint64_t frame_hash;
static uint64_t frame_changes;

static void probe_frame(void *param, struct video_data *frame)
{
	(void)param;
	uint64_t hash = 1469598103934665603ULL;
	for (uint32_t y = 0; y < BASE_H; y += 3) {
		const uint8_t *row = frame->data[0] + (size_t)y * frame->linesize[0];
		for (uint32_t x = 0; x < BASE_W * 4; x += 7)
			hash = (hash ^ row[x]) * 1099511628211ULL;
	}
	pthread_mutex_lock(&frame_mutex);
	if (hash != frame_hash)
		frame_changes++;
	frame_hash = hash;
	pthread_mutex_unlock(&frame_mutex);
}

static uint64_t changes(void)
{
	pthread_mutex_lock(&frame_mutex);
	uint64_t n = frame_changes;
	pthread_mutex_unlock(&frame_mutex);
	return n;
}

static void wait_ms(int ms)
{
	for (int t = 0; t < ms; t += 50) {
		run_ui_tasks();
		os_sleep_ms(50);
	}
	run_ui_tasks();
}

/* ------------------------------------------------------------------------ */
/* vivid */

static int run(const char *fmt, ...)
{
	char cmd[512];
	va_list args;
	va_start(args, fmt);
	vsnprintf(cmd, sizeof(cmd), fmt, args);
	va_end(args);
	int ret = system(cmd);
	return ret == -1 ? -1 : WEXITSTATUS(ret);
}

static bool vivid_loaded(void)
{
	return access("/sys/module/vivid", F_OK) == 0;
}

static bool vivid_unload(void)
{
	for (int i = 0; i < 100 && vivid_loaded(); i++) {
		if (run("rmmod vivid >/dev/null 2>&1") == 0)
			break;
		wait_ms(100);
	}
	return !vivid_loaded();
}

/* The capture node of vivid's first instance, "" if none */
static const char *vivid_node(void)
{
	static char node[300];
	node[0] = 0;
	DIR *dir = opendir(VIVID_NODES);
	struct dirent *ent;
	while (dir && (ent = readdir(dir))) {
		if (strncmp(ent->d_name, "video", 5) == 0)
			snprintf(node, sizeof(node), "/dev/%s", ent->d_name);
	}
	if (dir)
		closedir(dir);
	return node;
}

/* Sets vivid's "Disconnect" control: the node stays, file operations on it
 * fail with ENODEV until its last user closes it. */
static bool vivid_disconnect(const char *node)
{
	int fd = open(node, O_RDWR | O_NONBLOCK | O_CLOEXEC);
	if (fd < 0)
		return false;

	bool done = false;
	struct v4l2_query_ext_ctrl query;
	memset(&query, 0, sizeof(query));
	query.id = V4L2_CTRL_FLAG_NEXT_CTRL;
	while (ioctl(fd, VIDIOC_QUERY_EXT_CTRL, &query) == 0) {
		if (strcmp(query.name, "Disconnect") == 0) {
			struct v4l2_control ctrl = {.id = query.id, .value = 1};
			done = ioctl(fd, VIDIOC_S_CTRL, &ctrl) == 0;
			break;
		}
		query.id |= V4L2_CTRL_FLAG_NEXT_CTRL;
	}
	close(fd);
	return done;
}

/* ------------------------------------------------------------------------ */
/* udev: video4linux events while the camera fails */

static struct udev *udev;
static struct udev_monitor *monitor;

static void udev_watch(void)
{
	udev = udev_new();
	monitor = udev ? udev_monitor_new_from_netlink(udev, "kernel") : NULL;
	if (!monitor) {
		fprintf(stderr, "cannot watch udev events\n");
		fail();
	}
	udev_monitor_filter_add_match_subsystem_devtype(monitor, "video4linux", NULL);
	udev_monitor_enable_receiving(monitor);
}

/* The events received since udev_watch(), "" if none */
static const char *udev_events(void)
{
	static char seen[512];
	seen[0] = 0;
	struct pollfd pfd = {.fd = udev_monitor_get_fd(monitor), .events = POLLIN};
	while (poll(&pfd, 1, 0) > 0) {
		struct udev_device *dev = udev_monitor_receive_device(monitor);
		if (!dev)
			break;
		size_t len = strlen(seen);
		snprintf(seen + len, sizeof(seen) - len, "%s %s; ", udev_device_get_action(dev),
			 udev_device_get_sysname(dev));
		udev_device_unref(dev);
	}
	return seen;
}

static void udev_unwatch(void)
{
	if (monitor)
		udev_monitor_unref(monitor);
	if (udev)
		udev_unref(udev);
	monitor = NULL;
	udev = NULL;
}

/* ------------------------------------------------------------------------ */
/* The source, on the program output (vivid captures at the output size) */

static obs_source_t *source;

/* The video node the source has open, "" if none */
static const char *source_node(void)
{
	static char node[300];
	char link[300], target[300];
	node[0] = 0;
	DIR *dir = opendir("/proc/self/fd");
	struct dirent *ent;
	while (dir && (ent = readdir(dir))) {
		snprintf(link, sizeof(link), "/proc/self/fd/%s", ent->d_name);
		ssize_t len = readlink(link, target, sizeof(target) - 1);
		if (len <= 0)
			continue;
		target[len] = 0;
		if (strncmp(target, "/dev/video", 10) == 0)
			snprintf(node, sizeof(node), "%s", target);
	}
	if (dir)
		closedir(dir);
	return node;
}

/* Waits until the source captures from the node, its picture moving */
static bool wait_capturing(const char *node, int timeout_ms)
{
	for (int t = 0; t < timeout_ms; t += 250) {
		if (strcmp(source_node(), node) == 0) {
			uint64_t before = changes();
			wait_ms(1000);
			if (changes() - before >= 5 && strcmp(source_node(), node) == 0)
				return true;
		} else {
			wait_ms(250);
		}
	}
	return false;
}

static void create_source(const char *node)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", node);
	source = obs_source_create("v4l2_input", "camera", settings, NULL);
	obs_data_release(settings);
	if (!source) {
		fprintf(stderr, "could not create the Video Capture Device source\n");
		fail();
	}
	obs_set_output_source(0, source);
}

static void release_source(void)
{
	obs_set_output_source(0, NULL);
	if (source) {
		obs_source_remove(source);
		obs_source_release(source);
	}
	source = NULL;
	for (int t = 0; t < 5000 && *source_node(); t += 50)
		wait_ms(50);
}

/* ------------------------------------------------------------------------ */

/* The camera stops working but stays registered: the picture freezes, then
 * moves again on the same node, with no udev event. Before the fix it
 * stayed frozen. */
static void a_failed_camera_that_stays_registered_resumes(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	if (run("modprobe vivid " VIVID_PARAMS) != 0) {
		fprintf(stderr, "modprobe vivid " VIVID_PARAMS " failed\n");
		fail();
	}
	vivid_ours = true;
	run("udevadm settle -t 10 >/dev/null 2>&1");
	char node[300];
	snprintf(node, sizeof(node), "%s", vivid_node());
	if (!*node) {
		fprintf(stderr, "vivid has no capture node\n");
		fail();
	}

	create_source(node);
	if (!wait_capturing(node, 10000)) {
		fprintf(stderr, "the source did not start on %s (has '%s' open)\n", node, source_node());
		fail();
	}

	udev_watch();
	if (!vivid_disconnect(node)) {
		fprintf(stderr, "could not set vivid's Disconnect control on %s\n", node);
		fail();
	}

	/* the camera failed: the picture stops */
	wait_ms(300);
	uint64_t before = changes();
	wait_ms(600);
	uint64_t while_failed = changes() - before;
	if (while_failed > 2) {
		fprintf(stderr, "the picture kept moving (%" PRIu64 " changes) after the camera failed\n",
			while_failed);
		fail();
	}

	bool resumed = wait_capturing(node, 15000);
	const char *events = udev_events();
	udev_unwatch();

	if (*events) {
		fprintf(stderr, "udev events while the camera failed: %s\n", events);
		fail();
	}
	if (!resumed) {
		fprintf(stderr,
			"15 s after the camera failed the picture is still frozen (source has '%s' open, "
			"vivid reconnects once its last user closes %s)\n",
			source_node(), node);
		fail();
	}
}

/* ------------------------------------------------------------------------ */

static int teardown(void **state)
{
	(void)state;
	if (!obs_up)
		return 0;
	release_source();
	udev_unwatch();
	if (vivid_ours && !vivid_unload()) {
		fprintf(stderr, "could not unload vivid\n");
		return -1;
	}
	vivid_ours = false;
	return 0;
}

static int group_setup(void **state)
{
	(void)state;
	main_thread = pthread_self();

	if (geteuid() != 0) {
		fprintf(stderr, "skipped: needs root to load the vivid driver\n");
		skipped = true;
		return 0;
	}
	const char *display = getenv("DISPLAY");
	if (!display || !*display) {
		fprintf(stderr, "skipped: no display (run under xvfb-run)\n");
		skipped = true;
		return 0;
	}
	if (vivid_loaded()) {
		fprintf(stderr, "skipped: vivid is already loaded, the test leaves it alone\n");
		skipped = true;
		return 0;
	}
	if (run("modprobe -n vivid >/dev/null 2>&1") != 0) {
		fprintf(stderr, "skipped: the vivid module is not available\n");
		skipped = true;
		return 0;
	}

	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed\n");
		skipped = true;
		return 0;
	}
	obs_set_ui_task_handler(ui_task_handler);

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = BASE_W,
		.base_height = BASE_H,
		.output_width = BASE_W,
		.output_height = BASE_H,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_POINT,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		skipped = true;
		return 0;
	}

	if (obs_open_module(&v4l2_module, V4L2_MODULE_PATH, V4L2_DATA_PATH) != MODULE_SUCCESS ||
	    !obs_init_module(v4l2_module)) {
		fprintf(stderr, "could not load the linux-v4l2 module\n");
		obs_shutdown();
		return -1;
	}
	obs_post_load_modules();

	obs_add_raw_video_callback(NULL, probe_frame, NULL);
	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_remove_raw_video_callback(probe_frame, NULL);
		run_ui_tasks();
		obs_shutdown();
		obs_up = false;
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(a_failed_camera_that_stays_registered_resumes, teardown),
	};

	int failed = cmocka_run_group_tests(tests, group_setup, group_teardown);
	if (!failed && skipped)
		return 77;
	return failed;
}
