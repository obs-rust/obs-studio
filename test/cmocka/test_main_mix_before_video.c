/* Regression test for obs-rust/obs-studio#2253: several libobs functions read
 * obs->data.main_canvas->mix without checking it, and the main canvas has no
 * mix until the first obs_reset_video(). Called after obs_startup() but
 * before video is set up, each of them crashed:
 *
 * - obs_get_main_texture();
 * - obs_add_raw_video_callback2() (and obs_add_raw_video_callback(), which
 *   calls it) and obs_remove_raw_video_callback();
 * - obs_nv12_tex_active() and obs_p010_tex_active();
 * - get_scene_dimensions() in obs-scene.c.h, which a scene item without a
 *   parent scene reads: here obs_sceneitem_get_scale() and
 *   obs_sceneitem_get_pos() on an item removed from its scene, in a scene
 *   with relative coordinates.
 *
 * Only a plugin, script or headless tool calls these before video is set up.
 * The frontend always resets video first.
 *
 * Each case runs in a forked child from obs_startup() to obs_shutdown(), so a
 * crash fails only its own case. The child records how far it got in a
 * shared page, and the parent reports the stage it died in.
 *
 * - "*_before_video": the call before video exists gives the "no video"
 *   answer (NULL, false, a 0x0 canvas; a raw callback is refused with a
 *   warning and never called, and removing one does nothing). Then, after
 *   obs_reset_video(), the same calls work normally.
 * - "*_after_reset": the calls only after obs_reset_video(), as the frontend
 *   makes them. These pass with or without the fix.
 *
 * Core video needs a graphics module and libobs needs an X display, so the
 * test runs under xvfb-run on Linux. It skips only when no display is set at
 * all; with one set, a failed obs_startup() fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <errno.h>
#include <math.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/prctl.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#include <obs.h>
#include <obs-module.h>
#include <util/base.h>
#include <util/platform.h>
#include <util/threading.h>

#define ITEM_SOURCE_ID "test_mmbv_source"
#define TIMEOUT_MS 5000
#define BASE_SIZE 64
#define SCENE_SIZE 100

/* ------------------------------------------------------------------------- */
/* what the child reports */

enum stage {
	STAGE_NONE,
	STAGE_STARTUP_FAILED,
	STAGE_STARTED,
	STAGE_BEFORE,
	STAGE_BEFORE_DONE,
	STAGE_RESET,
	STAGE_AFTER,
	STAGE_AFTER_DONE,
	STAGE_SHUTDOWN,
	STAGE_DONE,
};

static const char *stage_name(int stage)
{
	switch (stage) {
	case STAGE_NONE:
		return "before obs_startup";
	case STAGE_STARTUP_FAILED:
		return "obs_startup failed";
	case STAGE_STARTED:
		return "after obs_startup";
	case STAGE_BEFORE:
		return "in the call before video is set up";
	case STAGE_BEFORE_DONE:
		return "after the call before video is set up";
	case STAGE_RESET:
		return "in obs_reset_video";
	case STAGE_AFTER:
		return "in the call after obs_reset_video";
	case STAGE_AFTER_DONE:
		return "after the call after obs_reset_video";
	case STAGE_SHUTDOWN:
		return "in obs_shutdown";
	case STAGE_DONE:
		return "done";
	}
	return "unknown";
}

struct result {
	int stage;
	int reset_ret;

	/* obs_get_main_texture() */
	bool tex_before;
	bool tex_after;

	/* raw video callbacks */
	int add_warnings;
	long early_frames;
	long late_frames;
	long late_frames_after_remove;

	/* obs_nv12_tex_active() and obs_p010_tex_active() */
	bool nv12_before;
	bool p010_before;
	bool nv12_after;
	bool p010_after;
	bool nv12_expected;
	bool p010_expected;

	/* get_scene_dimensions(), through a removed scene item */
	bool item_created;
	struct vec2 scale_before;
	struct vec2 pos_before;
	struct vec2 scale_after;
	struct vec2 pos_after;
};

static struct result *shared;

/* ------------------------------------------------------------------------- */
/* the log: count warnings about the refused raw callback */

static volatile bool counting_warnings;

static void log_handler(int level, const char *format, va_list args, void *param)
{
	(void)param;
	char msg[1024];
	vsnprintf(msg, sizeof(msg), format, args);
	if (level <= LOG_WARNING) {
		fprintf(stderr, "%s\n", msg);
		if (counting_warnings && strstr(msg, "raw video callback"))
			shared->add_warnings++;
	}
}

/* ------------------------------------------------------------------------- */
/* a plain video source for the scene item */

static const char *src_name(void *type_data)
{
	(void)type_data;
	return "main mix before video";
}

static void *src_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return bzalloc(1);
}

static void src_destroy(void *data)
{
	bfree(data);
}

static uint32_t src_size(void *data)
{
	(void)data;
	return 16;
}

static void register_source(void)
{
	struct obs_source_info info = {
		.id = ITEM_SOURCE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = src_name,
		.create = src_create,
		.destroy = src_destroy,
		.get_width = src_size,
		.get_height = src_size,
	};
	obs_register_source(&info);
}

/* ------------------------------------------------------------------------- */
/* raw video callbacks */

static volatile long early_count;
static volatile long late_count;

static void early_callback(void *param, struct video_data *frame)
{
	(void)param;
	(void)frame;
	os_atomic_inc_long(&early_count);
}

static void late_callback(void *param, struct video_data *frame)
{
	(void)param;
	(void)frame;
	os_atomic_inc_long(&late_count);
}

static bool wait_for_count(volatile long *count, long target)
{
	uint64_t deadline = os_gettime_ns() + (uint64_t)TIMEOUT_MS * 1000000ULL;
	while (os_atomic_load_long(count) < target) {
		if (os_gettime_ns() > deadline)
			return false;
		os_sleep_ms(5);
	}
	return true;
}

/* ------------------------------------------------------------------------- */
/* deprecated, but plugins still call them */

#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
static bool nv12_tex_active(void)
{
	return obs_nv12_tex_active();
}

static bool p010_tex_active(void)
{
	return obs_p010_tex_active();
}
#pragma GCC diagnostic pop

/* ------------------------------------------------------------------------- */
/* the child */

enum func { FUNC_MAIN_TEXTURE, FUNC_RAW_ADD, FUNC_RAW_REMOVE, FUNC_TEX_ACTIVE_NV12, FUNC_TEX_ACTIVE_P010, FUNC_SCENE };

static int reset_video(enum video_format format)
{
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = BASE_SIZE,
		.base_height = BASE_SIZE,
		.output_width = BASE_SIZE,
		.output_height = BASE_SIZE,
		.output_format = format,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
		.gpu_conversion = true,
	};
	return obs_reset_video(&ovi);
}

/* A scene with a custom size, so its items' scale reference does not depend
 * on video, and one item in it that is removed again: the removed item has no
 * parent, and get_scene_dimensions() reads the main canvas for it. */
static obs_sceneitem_t *make_removed_item(void)
{
	obs_scene_t *scene = obs_scene_create_private("main mix before video");
	if (!scene)
		return NULL;
	obs_source_t *scene_source = obs_scene_get_source(scene);
	obs_data_t *settings = obs_source_get_settings(scene_source);
	obs_data_set_bool(settings, "custom_size", true);
	obs_data_set_int(settings, "cx", SCENE_SIZE);
	obs_data_set_int(settings, "cy", SCENE_SIZE);
	obs_data_release(settings);
	obs_source_load(scene_source);

	obs_source_t *source = obs_source_create_private(ITEM_SOURCE_ID, "item", NULL);
	obs_sceneitem_t *item = source ? obs_scene_add(scene, source) : NULL;
	obs_source_release(source);
	if (item) {
		obs_sceneitem_addref(item);
		obs_sceneitem_remove(item);
	}
	obs_scene_release(scene);
	return item;
}

static void read_item(obs_sceneitem_t *item, struct vec2 *scale, struct vec2 *pos)
{
	obs_sceneitem_get_scale(item, scale);
	obs_sceneitem_get_pos(item, pos);
}

static void call_before(enum func func, obs_sceneitem_t *item)
{
	switch (func) {
	case FUNC_MAIN_TEXTURE:
		shared->tex_before = obs_get_main_texture() != NULL;
		break;
	case FUNC_RAW_ADD:
		counting_warnings = true;
		obs_add_raw_video_callback(NULL, early_callback, NULL);
		obs_add_raw_video_callback2(NULL, 2, early_callback, (void *)1);
		counting_warnings = false;
		break;
	case FUNC_RAW_REMOVE:
		obs_remove_raw_video_callback(early_callback, NULL);
		break;
	case FUNC_TEX_ACTIVE_NV12:
	case FUNC_TEX_ACTIVE_P010:
		shared->nv12_before = nv12_tex_active();
		shared->p010_before = p010_tex_active();
		break;
	case FUNC_SCENE:
		read_item(item, &shared->scale_before, &shared->pos_before);
		break;
	}
}

static void call_after(enum func func, obs_sceneitem_t *item)
{
	switch (func) {
	case FUNC_MAIN_TEXTURE: {
		uint64_t deadline = os_gettime_ns() + (uint64_t)TIMEOUT_MS * 1000000ULL;
		while (!obs_get_main_texture() && os_gettime_ns() < deadline)
			os_sleep_ms(5);
		shared->tex_after = obs_get_main_texture() != NULL;
		break;
	}
	case FUNC_RAW_ADD:
	case FUNC_RAW_REMOVE:
		/* A callback added after the reset receives frames, and stops
		 * receiving them once removed. One refused before the reset
		 * was never added: it receives none. */
		obs_add_raw_video_callback(NULL, late_callback, NULL);
		wait_for_count(&late_count, 3);
		obs_remove_raw_video_callback(late_callback, NULL);
		shared->late_frames = os_atomic_load_long(&late_count);
		os_sleep_ms(300);
		shared->late_frames_after_remove = os_atomic_load_long(&late_count) - shared->late_frames;
		shared->early_frames = os_atomic_load_long(&early_count);
		obs_remove_raw_video_callback(early_callback, NULL);
		obs_remove_raw_video_callback(early_callback, (void *)1);
		break;
	case FUNC_TEX_ACTIVE_NV12:
	case FUNC_TEX_ACTIVE_P010:
		obs_enter_graphics();
		shared->nv12_expected = func == FUNC_TEX_ACTIVE_NV12 && gs_nv12_available();
		shared->p010_expected = func == FUNC_TEX_ACTIVE_P010 && gs_p010_available();
		obs_leave_graphics();
		shared->nv12_after = nv12_tex_active();
		shared->p010_after = p010_tex_active();
		break;
	case FUNC_SCENE:
		read_item(item, &shared->scale_after, &shared->pos_after);
		break;
	}
}

static void child(enum func func, bool before)
{
	base_set_log_handler(log_handler, NULL);
	shared->stage = STAGE_NONE;
	if (!obs_startup("en-US", NULL, NULL)) {
		shared->stage = STAGE_STARTUP_FAILED;
		return;
	}
	shared->stage = STAGE_STARTED;
	register_source();

	obs_sceneitem_t *item = NULL;
	if (func == FUNC_SCENE) {
		item = make_removed_item();
		shared->item_created = item != NULL;
	}

	if (before && (func != FUNC_SCENE || item)) {
		shared->stage = STAGE_BEFORE;
		call_before(func, item);
		shared->stage = STAGE_BEFORE_DONE;
	}

	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	obs_reset_audio(&oai);
	shared->stage = STAGE_RESET;
	shared->reset_ret = reset_video(func == FUNC_TEX_ACTIVE_P010 ? VIDEO_FORMAT_P010 : VIDEO_FORMAT_NV12);
	if (shared->reset_ret == OBS_VIDEO_SUCCESS && (func != FUNC_SCENE || item)) {
		shared->stage = STAGE_AFTER;
		call_after(func, item);
		shared->stage = STAGE_AFTER_DONE;
	}

	obs_sceneitem_release(item);
	shared->stage = STAGE_SHUTDOWN;
	obs_shutdown();
	shared->stage = STAGE_DONE;
}

/* Runs `func` in a forked child; false when the test must skip. */
static bool run_child(enum func func, bool before, struct result *out)
{
	memset(shared, 0, sizeof(*shared));
	fflush(stdout);
	fflush(stderr);
	pid_t pid = fork();
	assert_true(pid >= 0);
	if (pid == 0) {
		/* cmocka traps SIGSEGV and would resume the test runner in the
		 * child; let a crash kill the child instead. */
		signal(SIGSEGV, SIG_DFL);
		signal(SIGBUS, SIG_DFL);
		signal(SIGILL, SIG_DFL);
		signal(SIGABRT, SIG_DFL);
		signal(SIGALRM, SIG_DFL);
		/* an expected crash should not leave a core dump behind */
		prctl(PR_SET_DUMPABLE, 0);
		alarm(60);
		child(func, before);
		_exit(0);
	}

	int status = 0;
	while (waitpid(pid, &status, 0) < 0)
		assert_int_equal(errno, EINTR);
	*out = *shared;

	if (out->stage == STAGE_STARTUP_FAILED) {
		if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
			fprintf(stderr, "obs_startup failed with a display set\n");
			fail();
		}
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return false;
	}
	if (WIFSIGNALED(status)) {
		fprintf(stderr, "the child died with signal %d %s\n", WTERMSIG(status), stage_name(out->stage));
		fail();
	}
	assert_true(WIFEXITED(status));
	assert_int_equal(WEXITSTATUS(status), 0);
	if (out->stage != STAGE_DONE) {
		fprintf(stderr, "the child stopped %s\n", stage_name(out->stage));
		fail();
	}
	if (out->reset_ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: %d\n", out->reset_ret);
		fail();
	}
	return true;
}

static void expect_near(float value, float expected, const char *what)
{
	if (!(fabsf(value - expected) < 0.001f)) {
		fprintf(stderr, "%s: %f, expected %f\n", what, (double)value, (double)expected);
		fail();
	}
}

/* ------------------------------------------------------------------------- */
/* checks */

static void check_main_texture(bool before)
{
	struct result r;
	if (!run_child(FUNC_MAIN_TEXTURE, before, &r))
		skip();
	if (before)
		assert_false(r.tex_before);
	/* the main texture once a frame is rendered */
	assert_true(r.tex_after);
}

static void check_raw_callbacks(enum func func, bool before)
{
	struct result r;
	if (!run_child(func, before, &r))
		skip();
	/* both adds before video are refused, each with a warning */
	if (before && func == FUNC_RAW_ADD)
		assert_int_equal(r.add_warnings, 2);
	else
		assert_int_equal(r.add_warnings, 0);
	assert_int_equal(r.early_frames, 0);
	if (r.late_frames < 3) {
		fprintf(stderr, "a raw callback added after the reset got %ld frames in %d ms\n", r.late_frames,
			TIMEOUT_MS);
		fail();
	}
	assert_int_equal(r.late_frames_after_remove, 0);
}

static void check_tex_active(enum func func, bool before)
{
	struct result r;
	if (!run_child(func, before, &r))
		skip();
	if (before) {
		assert_false(r.nv12_before);
		assert_false(r.p010_before);
	}
	/* after the reset: what the GPU conversion of that format gives */
	assert_int_equal(r.nv12_after, r.nv12_expected);
	assert_int_equal(r.p010_after, r.p010_expected);
}

static void check_scene_dimensions(bool before)
{
	struct result r;
	if (!run_child(FUNC_SCENE, before, &r))
		skip();
	assert_true(r.item_created);
	if (before) {
		/* a 0x0 canvas, as a scene without a custom size has before
		 * video is set up: scale 0, position 0 */
		expect_near(r.scale_before.x, 0.0f, "scale.x before video");
		expect_near(r.scale_before.y, 0.0f, "scale.y before video");
		expect_near(r.pos_before.x, 0.0f, "pos.x before video");
		expect_near(r.pos_before.y, 0.0f, "pos.y before video");
	}
	/* the main canvas: its height over the scale reference */
	const float scale = (float)BASE_SIZE / (float)SCENE_SIZE;
	expect_near(r.scale_after.x, scale, "scale.x after the reset");
	expect_near(r.scale_after.y, scale, "scale.y after the reset");
	expect_near(r.pos_after.x, 0.0f, "pos.x after the reset");
	expect_near(r.pos_after.y, 0.0f, "pos.y after the reset");
}

/* ------------------------------------------------------------------------- */
/* cases */

static void main_texture_before_video(void **state)
{
	(void)state;
	check_main_texture(true);
}

static void main_texture_after_reset(void **state)
{
	(void)state;
	check_main_texture(false);
}

static void raw_callback_add_before_video(void **state)
{
	(void)state;
	check_raw_callbacks(FUNC_RAW_ADD, true);
}

static void raw_callback_remove_before_video(void **state)
{
	(void)state;
	check_raw_callbacks(FUNC_RAW_REMOVE, true);
}

static void raw_callback_after_reset(void **state)
{
	(void)state;
	check_raw_callbacks(FUNC_RAW_ADD, false);
}

static void nv12_tex_active_before_video(void **state)
{
	(void)state;
	check_tex_active(FUNC_TEX_ACTIVE_NV12, true);
}

static void nv12_tex_active_after_reset(void **state)
{
	(void)state;
	check_tex_active(FUNC_TEX_ACTIVE_NV12, false);
}

static void p010_tex_active_before_video(void **state)
{
	(void)state;
	check_tex_active(FUNC_TEX_ACTIVE_P010, true);
}

static void p010_tex_active_after_reset(void **state)
{
	(void)state;
	check_tex_active(FUNC_TEX_ACTIVE_P010, false);
}

static void scene_dimensions_before_video(void **state)
{
	(void)state;
	check_scene_dimensions(true);
}

static void scene_dimensions_after_reset(void **state)
{
	(void)state;
	check_scene_dimensions(false);
}

static int group_setup(void **state)
{
	(void)state;
	shared = mmap(NULL, sizeof(*shared), PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS, -1, 0);
	if (shared == MAP_FAILED) {
		shared = NULL;
		return -1;
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (shared)
		munmap(shared, sizeof(*shared));
	shared = NULL;
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(main_texture_before_video),     cmocka_unit_test(main_texture_after_reset),
		cmocka_unit_test(raw_callback_add_before_video), cmocka_unit_test(raw_callback_remove_before_video),
		cmocka_unit_test(raw_callback_after_reset),      cmocka_unit_test(nv12_tex_active_before_video),
		cmocka_unit_test(nv12_tex_active_after_reset),   cmocka_unit_test(p010_tex_active_before_video),
		cmocka_unit_test(p010_tex_active_after_reset),   cmocka_unit_test(scene_dimensions_before_video),
		cmocka_unit_test(scene_dimensions_after_reset),
	};
	return cmocka_run_group_tests_name("test_main_mix_before_video", tests, group_setup, group_teardown);
}
