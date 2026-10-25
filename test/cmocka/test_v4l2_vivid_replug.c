#include <dirent.h>
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
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <cmocka.h>

#include <linux/videodev2.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

/* The Video Capture Device source follows a /dev/videoN number
 * (obs-rust/obs-studio#2262, upstream obsproject/obs-studio#3824), end to
 * end: real libobs and the built linux-v4l2 module capture from the kernel's
 * vivid test driver, rendered into a scene.
 *
 * Unbinding vivid's platform device unregisters its nodes the way a USB
 * unplug does (udev "remove", the open handle fails); loading the module
 * again is the replug. vivid's vid_cap_nr parameter picks the number each
 * instance gets, and node_types=0x0 leaves an instance without a camera, so
 * the test chooses which camera comes back under which number. vivid
 * instances report bus info "platform:vivid-000", "platform:vivid-001"...
 *
 * Which camera the source captures is read from the process's open file
 * descriptors and the node's VIDIOC_QUERYCAP; that it captures, from the
 * program output changing (vivid's picture moves; vivid runs at 5 fps).
 *
 * Needs root (modprobe, unbind/bind), the vivid module, no other video
 * device (the numbers must be free) and a display for core video (run under
 * xvfb-run). Otherwise it exits 77 and ctest reports it skipped.
 * test_v4l2_device_identity covers the same cases with fakes, in CI. */

#define BASE_W 320
#define BASE_H 240
#define VIVID_UNBIND "/sys/bus/platform/drivers/vivid/unbind"
#define VIVID_BIND "/sys/bus/platform/drivers/vivid/bind"
/* one camera, vivid-000, at /dev/video0 */
#define ONE_CAMERA_AT_0 "n_devs=1 node_types=0x1 vid_cap_nr=0"
/* the same camera at /dev/video1 */
#define ONE_CAMERA_AT_1 "n_devs=1 node_types=0x1 vid_cap_nr=1"
/* only the other camera, vivid-001, at /dev/video0 */
#define OTHER_CAMERA_AT_0 "n_devs=2 node_types=0x0,0x1 vid_cap_nr=-1,0"
/* vivid-000 at /dev/video0, vivid-001 at /dev/video1 */
#define BOTH_IN_ORDER "n_devs=2 node_types=0x1,0x1 vid_cap_nr=0,1"
/* vivid-000 at /dev/video1, vivid-001 at /dev/video0 */
#define BOTH_SWAPPED "n_devs=2 node_types=0x1,0x1 vid_cap_nr=1,0"
/* vivid's "Horizontal Movement" control: 6 is Move Right Fast */
#define VIVID_HORIZONTAL_MOVEMENT 0x00f0f002
#define CAMERA "platform:vivid-000"
#define OTHER_CAMERA "platform:vivid-001"

static bool obs_up;
/* vivid was loaded by this test, which may unload it; never another's */
static bool loaded_vivid;
/* the environment cannot run the test */
static bool skipped;
static obs_module_t *v4l2_module;

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
	for (uint32_t y = 0; y < BASE_H; y++) {
		const uint8_t *row = frame->data[0] + (size_t)y * frame->linesize[0];
		for (uint32_t x = 0; x < BASE_W * 4; x++)
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

static bool any_video_node(void)
{
	DIR *dir = opendir("/dev");
	bool found = false;
	struct dirent *ent;
	while (dir && (ent = readdir(dir))) {
		if (strncmp(ent->d_name, "video", 5) == 0)
			found = true;
	}
	if (dir)
		closedir(dir);
	return found;
}

/* Defined below */
static void settle(void);

/* Nothing else uses vivid or a video node (another test may have started
 * since): the test leaves them alone and skips. */
static bool devices_free(void)
{
	if (vivid_loaded() || any_video_node()) {
		fprintf(stderr, "skipped: vivid or a video node is in use by someone else\n");
		return false;
	}
	return true;
}

static void settle(void)
{
	run("udevadm settle -t 10 >/dev/null 2>&1");
}

static void vivid_load(const char *params)
{
	loaded_vivid = true;
	if (run("modprobe vivid %s", params) != 0) {
		fprintf(stderr, "modprobe vivid %s failed\n", params);
		fail();
	}
	settle();

	/* The test pattern stands still by default: make it move, so the
	 * output shows whether frames arrive. */
	for (int i = 0; i < 4; i++) {
		char node[32];
		snprintf(node, sizeof(node), "/dev/video%d", i);
		int fd = open(node, O_RDWR | O_NONBLOCK | O_CLOEXEC);
		if (fd < 0)
			continue;
		struct v4l2_control move = {.id = VIVID_HORIZONTAL_MOVEMENT, .value = 6};
		ioctl(fd, VIDIOC_S_CTRL, &move);
		close(fd);
	}
}

static void write_sysfs(const char *path, const char *text)
{
	FILE *f = fopen(path, "w");
	if (f) {
		fputs(text, f);
		fclose(f);
	}
	settle();
}

/* The cameras are unplugged */
static void vivid_unplug(void)
{
	write_sysfs(VIVID_UNBIND, "vivid.0");
}

/* and plugged into the same place again */
static void vivid_replug(void)
{
	write_sysfs(VIVID_BIND, "vivid.0");
}

/* The driver can go once nothing has its nodes open */
static bool vivid_unload(void)
{
	if (!loaded_vivid)
		return !vivid_loaded();
	for (int i = 0; i < 100 && vivid_loaded(); i++) {
		if (run("rmmod vivid >/dev/null 2>&1") == 0)
			break;
		wait_ms(100);
	}
	if (vivid_loaded())
		return false;
	loaded_vivid = false;
	return true;
}

/* The bus info of the camera at a node, "" if there is none */
static const char *bus_info_at(const char *node)
{
	static char bus[sizeof(((struct v4l2_capability *)0)->bus_info) + 1];
	struct v4l2_capability cap;
	bus[0] = 0;
	int fd = open(node, O_RDWR | O_NONBLOCK | O_CLOEXEC);
	if (fd < 0)
		return bus;
	memset(&cap, 0, sizeof(cap));
	if (ioctl(fd, VIDIOC_QUERYCAP, &cap) == 0)
		snprintf(bus, sizeof(bus), "%.*s", (int)sizeof(cap.bus_info), (char *)cap.bus_info);
	close(fd);
	return bus;
}

/* The video node the source has open, "" if none */
static const char *source_node(void)
{
	static char node[64];
	char link[300], target[256];
	node[0] = 0;
	DIR *dir = opendir("/proc/self/fd");
	struct dirent *ent;
	while (dir && (ent = readdir(dir))) {
		snprintf(link, sizeof(link), "/proc/self/fd/%s", ent->d_name);
		ssize_t len = readlink(link, target, sizeof(target) - 1);
		if (len <= 0)
			continue;
		target[len] = 0;
		if (strncmp(target, "/dev/video", 10) == 0 && len < (ssize_t)sizeof(node))
			memcpy(node, target, (size_t)len + 1);
	}
	if (dir)
		closedir(dir);
	return node;
}

/* Waits until the source captures from a node, its picture moving */
static bool wait_capturing(const char *node, int timeout_ms)
{
	for (int t = 0; t < timeout_ms; t += 250) {
		if (strcmp(source_node(), node) == 0) {
			uint64_t before = changes();
			wait_ms(1000);
			uint64_t moved = changes() - before;
			if (moved >= 2 && strcmp(source_node(), node) == 0)
				return true;
			fprintf(stderr, "(%s open, the picture changed %llu times in 1 s)\n", node,
				(unsigned long long)moved);
		} else {
			wait_ms(250);
		}
	}
	return false;
}

static bool wait_closed(int timeout_ms)
{
	for (int t = 0; t < timeout_ms && *source_node(); t += 50)
		wait_ms(50);
	return !*source_node();
}

/* ------------------------------------------------------------------------ */
/* The source, in a scene on the program output */

struct fixture {
	obs_scene_t *scene;
	obs_source_t *source;
};

static struct fixture fx;

static void create_source(obs_data_t *settings)
{
	fx.scene = obs_scene_create("vivid replug");
	fx.source = obs_source_create("v4l2_input", "camera", settings, NULL);
	if (!fx.source) {
		fprintf(stderr, "could not create the Video Capture Device source\n");
		fail();
	}
	/* the whole picture, scaled into the output */
	obs_sceneitem_t *item = obs_scene_add(fx.scene, fx.source);
	struct vec2 bounds = {BASE_W, BASE_H};
	obs_sceneitem_set_bounds_type(item, OBS_BOUNDS_SCALE_INNER);
	obs_sceneitem_set_bounds(item, &bounds);
	obs_set_output_source(0, obs_scene_get_source(fx.scene));
}

static obs_data_t *settings_at(const char *node)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", node);
	return settings;
}

/* The settings OBS would save for the source */
static obs_data_t *saved_settings(void)
{
	obs_data_t *saved = obs_save_source(fx.source);
	obs_data_t *settings = obs_data_get_obj(saved, "settings");
	obs_data_release(saved);
	return settings;
}

static bool gone(obs_weak_source_t *weak)
{
	obs_source_t *source = obs_weak_source_get_source(weak);
	obs_source_release(source);
	return source == NULL;
}

/* Releases the source and the scene, and waits until libobs destroyed them
 * (it does so on another thread): a source still alive would take the next
 * case's camera on its udev events. */
static void release_source(void)
{
	obs_weak_source_t *weak_source = obs_source_get_weak_source(fx.source);
	obs_weak_source_t *weak_scene = obs_source_get_weak_source(obs_scene_get_source(fx.scene));
	obs_set_output_source(0, NULL);
	if (fx.scene) {
		obs_source_remove(obs_scene_get_source(fx.scene));
		obs_scene_release(fx.scene);
	}
	if (fx.source) {
		obs_source_remove(fx.source);
		obs_source_release(fx.source);
	}
	fx.scene = NULL;
	fx.source = NULL;
	for (int t = 0; t < 10000 && !(gone(weak_source) && gone(weak_scene)); t += 50)
		wait_ms(50);
	if (!gone(weak_source))
		fprintf(stderr, "warning: the source was not destroyed\n");
	obs_weak_source_release(weak_source);
	obs_weak_source_release(weak_scene);
	wait_closed(5000);
}

static int setup(void **state)
{
	(void)state;
	memset(&fx, 0, sizeof(fx));
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (!obs_up)
		return 0;
	release_source();
	if (!loaded_vivid)
		return 0;
	vivid_unplug();
	if (!vivid_unload()) {
		fprintf(stderr, "could not unload vivid\n");
		return -1;
	}
	return 0;
}

/* The camera is captured from /dev/video0 */
static void start_on_video0(void)
{
	vivid_load(ONE_CAMERA_AT_0);
	obs_data_t *settings = settings_at("/dev/video0");
	create_source(settings);
	obs_data_release(settings);
	if (!wait_capturing("/dev/video0", 10000)) {
		fprintf(stderr, "the source did not start on /dev/video0 (has '%s' open)\n", source_node());
		fail();
	}
}

/* The camera is unplugged, and the driver goes */
static void unplug_and_unload(void)
{
	vivid_unplug();
	if (!wait_closed(5000)) {
		fprintf(stderr, "the source kept %s open after the unplug\n", source_node());
		fail();
	}
	if (!vivid_unload()) {
		fprintf(stderr, "could not unload vivid after the unplug\n");
		fail();
	}
}

/* ------------------------------------------------------------------------ */

/* The camera comes back as /dev/video1 (nothing at video0): the source
 * resumes on video1. Before the fix it stayed on its last frame. */
static void a_camera_replugged_under_another_number_resumes(void **state)
{
	(void)state;
	if (!obs_up || !devices_free())
		skip();

	start_on_video0();
	unplug_and_unload();
	vivid_load(ONE_CAMERA_AT_1);

	if (!wait_capturing("/dev/video1", 8000)) {
		fprintf(stderr, "the camera came back as /dev/video1; the source has '%s' open, expected video1\n",
			source_node());
		fail();
	}
	obs_data_t *settings = saved_settings();
	const char *saved = obs_data_get_string(settings, "device_id");
	bool follows = strcmp(saved, "/dev/video1") == 0;
	if (!follows)
		fprintf(stderr, "the saved device_id is %s, expected /dev/video1\n", saved);
	obs_data_release(settings);
	assert_true(follows);
}

/* Another camera takes /dev/video0 while the camera is away: the source
 * does not capture it. Then the camera comes back as video1, with the
 * other one still at video0: the source captures the camera. Before the
 * fix it captured the other camera. */
static void another_camera_at_the_old_number_is_not_captured(void **state)
{
	(void)state;
	if (!obs_up || !devices_free())
		skip();

	start_on_video0();
	unplug_and_unload();

	vivid_load(OTHER_CAMERA_AT_0);
	wait_ms(3000);
	char while_away[64];
	snprintf(while_away, sizeof(while_away), "%s", source_node());

	vivid_unplug();
	wait_closed(5000);
	if (!vivid_unload()) {
		fprintf(stderr, "could not unload vivid\n");
		fail();
	}
	vivid_load(BOTH_SWAPPED);
	bool back = wait_capturing("/dev/video1", 8000);
	char after[64];
	snprintf(after, sizeof(after), "%s", source_node());
	const char *bus = *after ? bus_info_at(after) : "";

	if (*while_away) {
		fprintf(stderr, "with only the other camera (%s) at /dev/video0, the source has %s open\n",
			OTHER_CAMERA, while_away);
		fail();
	}
	if (!back || strcmp(bus, CAMERA) != 0) {
		fprintf(stderr, "with the camera back at /dev/video1: the source has '%s' (%s) open, expected %s\n",
			after, bus, CAMERA);
		fail();
	}
}

/* Settings with only device_id, as saved before the fix: the camera at the
 * path is captured, resumes after a replug at the same number (this worked
 * before the fix), and is recorded in the saved settings. */
static void old_settings_with_only_a_path_still_work(void **state)
{
	(void)state;
	if (!obs_up || !devices_free())
		skip();

	start_on_video0();
	vivid_unplug();
	if (!wait_closed(5000)) {
		fprintf(stderr, "the source kept %s open after the unplug\n", source_node());
		fail();
	}
	vivid_replug();
	if (!wait_capturing("/dev/video0", 8000)) {
		fprintf(stderr, "after a replug at the same number the source has '%s' open, expected video0\n",
			source_node());
		fail();
	}

	obs_data_t *settings = saved_settings();
	obs_data_t *identity = obs_data_get_obj(settings, "device_identity");
	const char *bus = identity ? obs_data_get_string(identity, "bus_info") : "";
	bool recorded = strcmp(bus, CAMERA) == 0;
	if (!recorded)
		fprintf(stderr, "the saved settings name the camera on '%s', expected %s\n", bus, CAMERA);
	obs_data_release(identity);
	obs_data_release(settings);
	assert_true(recorded);
}

/* The two cameras swap numbers while OBS is closed: at the next start the
 * source opens its camera at its new number. */
static void a_renumbered_camera_is_found_at_startup(void **state)
{
	(void)state;
	if (!obs_up || !devices_free())
		skip();

	vivid_load(BOTH_IN_ORDER);
	obs_data_t *settings = settings_at("/dev/video0");
	create_source(settings);
	obs_data_release(settings);
	if (!wait_capturing("/dev/video0", 10000)) {
		fprintf(stderr, "the source did not start on /dev/video0 (has '%s' open)\n", source_node());
		fail();
	}
	settings = saved_settings();
	release_source();
	unplug_and_unload();

	vivid_load(BOTH_SWAPPED);
	create_source(settings);
	obs_data_release(settings);
	bool found = wait_capturing("/dev/video1", 8000);
	const char *node = source_node();
	if (!found) {
		fprintf(stderr, "after the renumbering the source has '%s' (%s) open, expected /dev/video1 (%s)\n",
			node, *node ? bus_info_at(node) : "", CAMERA);
		fail();
	}
}

/* ------------------------------------------------------------------------ */

static int group_setup(void **state)
{
	(void)state;
	main_thread = pthread_self();

	if (geteuid() != 0) {
		fprintf(stderr, "skipped: needs root to load the vivid driver and unbind/bind it\n");
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
	if (any_video_node()) {
		fprintf(stderr, "skipped: other video devices are present, the test needs the /dev/videoN numbers\n");
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
	if (vivid_loaded())
		fprintf(stderr, "warning: vivid is still loaded\n");
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(a_camera_replugged_under_another_number_resumes, setup, teardown),
		cmocka_unit_test_setup_teardown(another_camera_at_the_old_number_is_not_captured, setup, teardown),
		cmocka_unit_test_setup_teardown(old_settings_with_only_a_path_still_work, setup, teardown),
		cmocka_unit_test_setup_teardown(a_renumbered_camera_is_found_at_startup, setup, teardown),
	};

	int failed = cmocka_run_group_tests(tests, group_setup, group_teardown);
	if (!failed && skipped)
		return 77;
	return failed;
}
