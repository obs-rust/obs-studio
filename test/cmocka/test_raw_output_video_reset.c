/* Regression test: raw outputs keep a freed video_t after obs_reset_video().
 *
 * Reported in obsproject/obs-websocket#1344 by jeiea (SetVideoSettings, then
 * GetOutputList crashes in obs_output_get_width), exeldro/obs-source-record#143
 * and DistroAV/DistroAV#1096 by walker-WSH (a plugin output started after a
 * resolution change crashes on the deleted video_t), and probably
 * obsproject/obs-studio#11645 (GetOutputList crash during a profile change).
 *
 * obs_output_create() stores obs_get_video() in a raw (non-encoded) output
 * once. obs_reset_video() frees every video mix, including that video_t, and
 * creates new ones, but nothing repointed existing outputs at the new mixes.
 * Every later obs_output_get_width/height(), obs_output_video() or
 * obs_output_start() of such an output then uses freed memory.
 *
 * The cases compare the output's video_t with the live one before touching it,
 * so the stale pointer fails the test without reading freed memory, and then
 * check what a user sees: the reported size after the reset, and that the
 * output starts and receives frames.
 *
 * Without ASan the allocator often hands the freed video_t's memory straight
 * back to the new mix, which hides the bug (the stale pointer happens to point
 * at the new video_t). To keep the test deterministic, the test pins the freed
 * memory: obs_reset_video() logs "video settings reset" after it has freed the
 * old mixes and before it creates the new ones, and the test's log handler
 * then allocates blocks until each old video_t address lies inside one of
 * them. The new mixes can no longer land on the old addresses, so a stale
 * pointer always differs from the live one. The blocks are freed after the
 * reset. Under ASan freed memory is never reused, so nothing needs pinning.
 *
 * The last case covers the core_video_release signal, emitted before the
 * mixes are freed, for plugins that hold a video_t of their own.
 *
 * Core video needs a graphics module, and libobs needs an X display on Linux,
 * so the cases skip without one (run under xvfb-run).
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <malloc.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#define RAW_OUTPUT_ID "test_raw_reset_output"
#define FRAME_TIMEOUT_MS 5000

static int obs_up;
static volatile long raw_frames;

/* ------------------------------------------------------------------------- */
/* raw video output */

struct raw_output {
	obs_output_t *output;
};

static const char *raw_output_name(void *unused)
{
	(void)unused;
	return "raw output";
}

static void *raw_output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	struct raw_output *ro = bzalloc(sizeof(*ro));
	ro->output = output;
	return ro;
}

static void raw_output_destroy(void *data)
{
	bfree(data);
}

static bool raw_output_start(void *data)
{
	struct raw_output *ro = data;
	if (!obs_output_can_begin_data_capture(ro->output, 0))
		return false;
	return obs_output_begin_data_capture(ro->output, 0);
}

static void raw_output_stop(void *data, uint64_t ts)
{
	(void)ts;
	struct raw_output *ro = data;
	obs_output_end_data_capture(ro->output);
}

static void raw_output_video(void *data, struct video_data *frame)
{
	(void)data;
	(void)frame;
	os_atomic_inc_long(&raw_frames);
}

static void register_raw_output(void)
{
	struct obs_output_info info = {
		.id = RAW_OUTPUT_ID,
		.flags = OBS_OUTPUT_VIDEO,
		.get_name = raw_output_name,
		.create = raw_output_create,
		.destroy = raw_output_destroy,
		.start = raw_output_start,
		.stop = raw_output_stop,
		.raw_video = raw_output_video,
	};
	obs_register_output(&info);
}

/* ------------------------------------------------------------------------- */
/* pinning freed video_t memory during obs_reset_video() */

#define MAX_PINS 2
#define MAX_HELD 32768
/* Small sizes go up in 16-byte steps so every per-size free list is drained,
 * larger ones double. */
#define PIN_SMALL_STEP 16
#define PIN_SMALL_MAX 4096
#define PIN_SMALL_BLOCKS 16
#define PIN_LARGE_BLOCKS 256
#define PIN_MAX_SIZE (64 * 1024)

static log_handler_t default_log_handler;
static void *default_log_param;

static const void *pin_targets[MAX_PINS];
static size_t num_pin_targets;
static bool pin_armed;
static void *held[MAX_HELD];
static size_t num_held;
static size_t num_pinned;

static bool block_contains(void *block, const void *target)
{
	uintptr_t start = (uintptr_t)block;
	uintptr_t t = (uintptr_t)target;
	return t >= start && t < start + malloc_usable_size(block);
}

static void pin_freed_targets(void)
{
	bool pinned[MAX_PINS] = {false};

	for (size_t size = PIN_SMALL_STEP; size <= PIN_MAX_SIZE && num_pinned < num_pin_targets;
	     size = size < PIN_SMALL_MAX ? size + PIN_SMALL_STEP : size * 2) {
		size_t blocks = size < PIN_SMALL_MAX ? PIN_SMALL_BLOCKS : PIN_LARGE_BLOCKS;
		for (size_t n = 0; n < blocks && num_pinned < num_pin_targets; n++) {
			if (num_held == MAX_HELD)
				return;
			void *block = malloc(size);
			if (!block)
				return;
			held[num_held++] = block;
			for (size_t i = 0; i < num_pin_targets; i++) {
				if (!pinned[i] && block_contains(block, pin_targets[i])) {
					pinned[i] = true;
					num_pinned++;
				}
			}
		}
	}
}

static void pin_log_handler(int lvl, const char *msg, va_list args, void *param)
{
	va_list copy;
	va_copy(copy, args);
	char text[64];
	vsnprintf(text, sizeof(text), msg, copy);
	va_end(copy);

	/* The old mixes are freed, the new ones are not created yet. */
	if (pin_armed && strstr(text, "video settings reset")) {
		pin_armed = false;
		pin_freed_targets();
	}

	if (default_log_handler)
		default_log_handler(lvl, msg, args, param);
}

static void arm_pins(const void *a, const void *b)
{
	num_pin_targets = 0;
	num_pinned = 0;
	if (a)
		pin_targets[num_pin_targets++] = a;
	if (b)
		pin_targets[num_pin_targets++] = b;
	pin_armed = true;
}

static void release_pins(void)
{
	pin_armed = false;
	for (size_t i = 0; i < num_held; i++)
		free(held[i]);
	num_held = 0;
	if (num_pinned < num_pin_targets)
		fprintf(stderr, "note: pinned %zu of %zu freed video_t addresses\n", num_pinned, num_pin_targets);
	num_pin_targets = 0;
	num_pinned = 0;
}

/* ------------------------------------------------------------------------- */
/* helpers */

static int reset_video(uint32_t width, uint32_t height)
{
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = width,
		.base_height = height,
		.output_width = width,
		.output_height = height,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	return obs_reset_video(&ovi);
}

/* Fails without reading the video_t when the output still holds a freed one. */
static void check_video_is_live(obs_output_t *output, video_t *expected, const char *what)
{
	video_t *video = obs_output_video(output);
	if (video != expected) {
		fprintf(stderr, "%s: the output holds video_t %p, the live one is %p (stale pointer to a freed mix)\n",
			what, (void *)video, (void *)expected);
		fail();
	}
}

static void check_size(obs_output_t *output, uint32_t width, uint32_t height, const char *what)
{
	uint32_t cx = obs_output_get_width(output);
	uint32_t cy = obs_output_get_height(output);
	if (cx != width || cy != height) {
		fprintf(stderr, "%s: the output reports %ux%u, expected %ux%u\n", what, cx, cy, width, height);
		fail();
	}
}

static bool wait_for_frames(long count)
{
	uint64_t deadline = os_gettime_ns() + (uint64_t)FRAME_TIMEOUT_MS * 1000000ULL;
	while (os_atomic_load_long(&raw_frames) < count) {
		if (os_gettime_ns() > deadline)
			return false;
		os_sleep_ms(5);
	}
	return true;
}

static void start_and_receive(obs_output_t *output, const char *what)
{
	os_atomic_set_long(&raw_frames, 0);
	if (!obs_output_start(output)) {
		fprintf(stderr, "%s: obs_output_start failed: %s\n", what, obs_output_get_last_error(output));
		fail();
	}
	bool got_frames = wait_for_frames(3);
	obs_output_force_stop(output);
	uint64_t deadline = os_gettime_ns() + (uint64_t)FRAME_TIMEOUT_MS * 1000000ULL;
	while ((obs_output_active(output) || obs_video_active()) && os_gettime_ns() < deadline)
		os_sleep_ms(5);
	if (!got_frames) {
		fprintf(stderr, "%s: the started output received %ld raw frames in %d ms\n", what,
			os_atomic_load_long(&raw_frames), FRAME_TIMEOUT_MS);
		fail();
	}
}

/* ------------------------------------------------------------------------- */
/* cases */

static int fixture_setup(void **state)
{
	(void)state;
	if (!obs_up)
		return 0;
	int ret = reset_video(64, 64);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video(64x64) failed: %d\n", ret);
		return -1;
	}
	return 0;
}

static int fixture_teardown(void **state)
{
	(void)state;
	release_pins();
	return 0;
}

static obs_output_t *create_output(void)
{
	obs_output_t *output = obs_output_create(RAW_OUTPUT_ID, "raw reset output", NULL, NULL);
	assert_non_null(output);
	return output;
}

/* Resets the main video while the old video_t addresses stay pinned; the
 * caller releases the pins after its checks. */
static void reset_to(uint32_t width, uint32_t height, const void *pin_a, const void *pin_b)
{
	arm_pins(pin_a, pin_b);
	int ret = reset_video(width, height);
	if (ret != OBS_VIDEO_SUCCESS) {
		release_pins();
		fprintf(stderr, "obs_reset_video(%ux%u) failed: %d\n", width, height, ret);
		fail();
	}
}

/* The websocket case: change the resolution, then list outputs (sizes). */
static void size_follows_reset(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_output_t *output = create_output();
	check_size(output, 64, 64, "before the reset");

	reset_to(128, 72, obs_get_video(), NULL);
	check_video_is_live(output, obs_get_video(), "after resetting to 128x72");
	check_size(output, 128, 72, "after resetting to 128x72");
	release_pins();

	reset_to(96, 54, obs_get_video(), NULL);
	check_video_is_live(output, obs_get_video(), "after a second reset to 96x54");
	check_size(output, 96, 54, "after a second reset to 96x54");
	release_pins();

	obs_output_release(output);
}

/* The plugin case (DistroAV, source-record): start an output created before
 * the resolution change. */
static void output_starts_after_reset(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_output_t *output = create_output();
	start_and_receive(output, "before the reset");

	reset_to(128, 72, obs_get_video(), NULL);
	check_video_is_live(output, obs_get_video(), "after resetting to 128x72");
	release_pins();
	start_and_receive(output, "after resetting to 128x72");
	check_size(output, 128, 72, "after starting at 128x72");

	obs_output_release(output);
}

/* An output pointed at a secondary canvas follows that canvas's new mix,
 * which keeps its own size across a reset of the main video. */
static void canvas_output_follows_reset(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 48,
		.base_height = 32,
		.output_width = 48,
		.output_height = 32,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	obs_canvas_t *canvas = obs_canvas_create("raw reset canvas", &ovi, DEVICE);
	assert_non_null(canvas);
	assert_non_null(obs_canvas_get_video(canvas));

	obs_output_t *output = create_output();
	obs_output_set_media(output, obs_canvas_get_video(canvas), obs_get_audio());
	check_size(output, 48, 32, "before the reset");

	reset_to(128, 72, obs_get_video(), obs_canvas_get_video(canvas));
	check_video_is_live(output, obs_canvas_get_video(canvas), "after resetting the main video");
	release_pins();
	check_size(output, 48, 32, "after resetting the main video");
	start_and_receive(output, "after resetting the main video");

	obs_output_release(output);
	obs_canvas_remove(canvas);
	obs_canvas_release(canvas);
}

/* Plugins that hold their own video_t (obs_view_add(), a cached
 * obs_get_video()) learn about the reset before the old mixes are freed
 * (port of obsproject/obs-studio#11686 by walker-WSH). */
static long release_calls;
static video_t *video_at_release;

static void on_core_video_release(void *param, calldata_t *cd)
{
	(void)param;
	(void)cd;
	release_calls++;
	video_at_release = obs_get_video();
}

static void release_signal_before_free(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	video_t *old_video = obs_get_video();
	release_calls = 0;
	video_at_release = NULL;
	signal_handler_t *sh = obs_get_signal_handler();
	signal_handler_connect(sh, "core_video_release", on_core_video_release, NULL);
	reset_to(128, 72, NULL, NULL);
	release_pins();
	signal_handler_disconnect(sh, "core_video_release", on_core_video_release, NULL);

	if (release_calls != 1) {
		fprintf(stderr, "core_video_release was emitted %ld times by one reset, expected once\n",
			release_calls);
		fail();
	}
	if (video_at_release != old_video) {
		fprintf(stderr, "at core_video_release the main video was %p, expected the old one %p\n",
			(void *)video_at_release, (void *)old_video);
		fail();
	}
}

/* ------------------------------------------------------------------------- */

static int group_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up)
		return 0;
	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "obs_reset_audio failed\n");
		return -1;
	}
	register_raw_output();
	base_get_log_handler(&default_log_handler, &default_log_param);
	base_set_log_handler(pin_log_handler, default_log_param);
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
		cmocka_unit_test_setup_teardown(size_follows_reset, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(output_starts_after_reset, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(canvas_output_follows_reset, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(release_signal_before_free, fixture_setup, fixture_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
