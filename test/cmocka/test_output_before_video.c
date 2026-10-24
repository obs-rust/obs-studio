/* Regression test for obs-rust/obs-studio#2237: creating a raw output after
 * obs_startup() but before obs_reset_video() crashed. obs_output_create()
 * stores obs_get_video() in every output without OBS_OUTPUT_ENCODED, and
 * obs_get_video() read obs->data.main_canvas->mix->video while the main
 * canvas still had no mix (it gets one from the first obs_reset_video()).
 *
 * Only a caller that creates an output before setting up video gets there: a
 * plugin, a script or a headless tool. The frontend always resets video
 * first.
 *
 * Each case runs in a forked child from obs_startup() to obs_shutdown(), so a
 * crash fails only its own case. The child records how far it got in a
 * shared page, and the parent reports the stage it died in.
 *
 * - An output created before video exists: it is created, with no video and
 *   a size of 0x0, as an output given NULL video by obs_output_set_media().
 *   The output from the issue (flags 0) and a raw video output.
 * - Starting that raw video output fails cleanly: start() runs,
 *   obs_output_can_begin_data_capture() refuses, and the output is not
 *   active.
 * - After a later obs_reset_video(), a new raw output gets the main video and
 *   its size, starts and receives frames. The one created before keeps no
 *   video and still refuses to start.
 *
 * Core video needs a graphics module and libobs needs an X display, so the
 * test runs under xvfb-run on Linux. It skips only when no display is set at
 * all; with one set, a failed obs_startup() fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <errno.h>
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
#include <util/platform.h>
#include <util/threading.h>

#define RAW_VIDEO_ID "test_obv_raw_video"
#define FLAGLESS_ID "test_obv_flagless"
#define FRAME_TIMEOUT_MS 5000

/* ------------------------------------------------------------------------- */
/* what the child reports */

enum stage {
	STAGE_NONE,
	STAGE_STARTUP_FAILED,
	STAGE_STARTED,
	STAGE_CREATE,
	STAGE_CREATED,
	STAGE_START,
	STAGE_STARTED_OUTPUT,
	STAGE_RESET,
	STAGE_NEW_OUTPUT,
	STAGE_NEW_START,
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
	case STAGE_CREATE:
		return "in obs_output_create before video";
	case STAGE_CREATED:
		return "after obs_output_create";
	case STAGE_START:
		return "in obs_output_start before video";
	case STAGE_STARTED_OUTPUT:
		return "after obs_output_start";
	case STAGE_RESET:
		return "in obs_reset_video";
	case STAGE_NEW_OUTPUT:
		return "creating an output after obs_reset_video";
	case STAGE_NEW_START:
		return "starting the output created after obs_reset_video";
	case STAGE_SHUTDOWN:
		return "in obs_shutdown";
	case STAGE_DONE:
		return "done";
	}
	return "unknown";
}

struct result {
	int stage;

	/* the output created before video */
	bool created;
	bool has_video;
	uint32_t width;
	uint32_t height;
	bool started;
	int start_calls;
	int can_begin;
	bool active;

	/* after obs_reset_video() */
	int reset_ret;
	bool old_has_video;
	bool old_started;
	bool new_created;
	bool new_video_is_main;
	uint32_t new_width;
	uint32_t new_height;
	bool new_started;
	long new_frames;
};

static struct result *shared;

/* ------------------------------------------------------------------------- */
/* the outputs */

struct out_data {
	obs_output_t *output;
};

static volatile long raw_frames;

static const char *out_name(void *type_data)
{
	(void)type_data;
	return "output before video";
}

static void *out_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	struct out_data *d = bzalloc(sizeof(*d));
	d->output = output;
	return d;
}

static void out_destroy(void *data)
{
	bfree(data);
}

/* A raw output's start(): begin only when OBS says it can. */
static bool out_start(void *data)
{
	struct out_data *d = data;
	shared->start_calls++;
	bool can_begin = obs_output_can_begin_data_capture(d->output, 0);
	shared->can_begin = can_begin ? 1 : 0;
	if (!can_begin)
		return false;
	return obs_output_begin_data_capture(d->output, 0);
}

static void out_stop(void *data, uint64_t ts)
{
	(void)ts;
	struct out_data *d = data;
	obs_output_end_data_capture(d->output);
}

static void out_raw_video(void *data, struct video_data *frame)
{
	(void)data;
	(void)frame;
	os_atomic_inc_long(&raw_frames);
}

static void register_outputs(void)
{
	struct obs_output_info raw = {
		.id = RAW_VIDEO_ID,
		.flags = OBS_OUTPUT_VIDEO,
		.get_name = out_name,
		.create = out_create,
		.destroy = out_destroy,
		.start = out_start,
		.stop = out_stop,
		.raw_video = out_raw_video,
	};
	obs_register_output(&raw);

	/* The output from the issue: no flags at all. */
	struct obs_output_info flagless = {
		.id = FLAGLESS_ID,
		.flags = 0,
		.get_name = out_name,
		.create = out_create,
		.destroy = out_destroy,
		.start = out_start,
		.stop = out_stop,
	};
	obs_register_output(&flagless);
}

/* ------------------------------------------------------------------------- */
/* the child */

enum mode { MODE_CREATE_FLAGLESS, MODE_CREATE_RAW, MODE_START_RAW, MODE_AFTER_RESET };

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

static void stop_and_wait(obs_output_t *output)
{
	obs_output_force_stop(output);
	uint64_t deadline = os_gettime_ns() + (uint64_t)FRAME_TIMEOUT_MS * 1000000ULL;
	while ((obs_output_active(output) || obs_video_active()) && os_gettime_ns() < deadline)
		os_sleep_ms(5);
}

/* The output created after obs_reset_video(). */
static void run_after_reset(void)
{
	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	obs_reset_audio(&oai);

	shared->stage = STAGE_RESET;
	shared->reset_ret = reset_video();
	if (shared->reset_ret != OBS_VIDEO_SUCCESS)
		return;

	shared->stage = STAGE_NEW_OUTPUT;
	obs_output_t *output = obs_output_create(RAW_VIDEO_ID, "after video", NULL, NULL);
	shared->new_created = output != NULL;
	if (!output)
		return;
	shared->new_video_is_main = obs_output_video(output) != NULL && obs_output_video(output) == obs_get_video();
	shared->new_width = obs_output_get_width(output);
	shared->new_height = obs_output_get_height(output);

	shared->stage = STAGE_NEW_START;
	os_atomic_set_long(&raw_frames, 0);
	shared->new_started = obs_output_start(output);
	if (shared->new_started) {
		wait_for_frames(3);
		shared->new_frames = os_atomic_load_long(&raw_frames);
		stop_and_wait(output);
	}
	obs_output_release(output);
}

static void child(enum mode mode)
{
	shared->stage = STAGE_NONE;
	if (!obs_startup("en-US", NULL, NULL)) {
		shared->stage = STAGE_STARTUP_FAILED;
		return;
	}
	shared->stage = STAGE_STARTED;
	register_outputs();

	shared->stage = STAGE_CREATE;
	obs_output_t *output = obs_output_create(mode == MODE_CREATE_FLAGLESS ? FLAGLESS_ID : RAW_VIDEO_ID,
						 "before video", NULL, NULL);
	shared->stage = STAGE_CREATED;
	shared->created = output != NULL;

	if (output) {
		shared->has_video = obs_output_video(output) != NULL;
		if (mode != MODE_CREATE_FLAGLESS) {
			shared->width = obs_output_get_width(output);
			shared->height = obs_output_get_height(output);
		}

		if (mode == MODE_START_RAW || mode == MODE_AFTER_RESET) {
			shared->stage = STAGE_START;
			shared->can_begin = -1;
			shared->started = obs_output_start(output);
			shared->active = obs_output_active(output);
			shared->stage = STAGE_STARTED_OUTPUT;
			if (shared->started)
				stop_and_wait(output);
		}

		if (mode == MODE_AFTER_RESET) {
			run_after_reset();
			if (shared->reset_ret == OBS_VIDEO_SUCCESS) {
				shared->old_has_video = obs_output_video(output) != NULL;
				shared->old_started = obs_output_start(output);
				if (shared->old_started)
					stop_and_wait(output);
			}
		}
		obs_output_release(output);
	}

	shared->stage = STAGE_SHUTDOWN;
	obs_shutdown();
	shared->stage = STAGE_DONE;
}

/* Runs `mode` in a forked child; false when the test must skip. */
static bool run_child(enum mode mode, struct result *out)
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
		child(mode);
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
	return true;
}

/* ------------------------------------------------------------------------- */
/* cases */

static void check_created_without_video(const struct result *r)
{
	if (!r->created)
		return; /* NULL is a clean answer too */
	if (r->has_video) {
		fprintf(stderr, "the output created before video has a video_t\n");
		fail();
	}
}

/* The issue's output (flags 0), created before obs_reset_video(). */
static void create_flagless_before_video(void **state)
{
	(void)state;
	struct result r;
	if (!run_child(MODE_CREATE_FLAGLESS, &r))
		skip();
	check_created_without_video(&r);
}

/* A raw video output created before obs_reset_video(): no video, 0x0. */
static void create_raw_before_video(void **state)
{
	(void)state;
	struct result r;
	if (!run_child(MODE_CREATE_RAW, &r))
		skip();
	check_created_without_video(&r);
	if (r.created) {
		assert_int_equal(r.width, 0);
		assert_int_equal(r.height, 0);
	}
}

/* Starting it fails cleanly: start() runs and OBS refuses to begin. */
static void start_raw_before_video_fails(void **state)
{
	(void)state;
	struct result r;
	if (!run_child(MODE_START_RAW, &r))
		skip();
	check_created_without_video(&r);
	if (!r.created)
		return;
	if (r.started || r.active) {
		fprintf(stderr, "the output without video started (start %d, active %d)\n", r.started, r.active);
		fail();
	}
	assert_int_equal(r.start_calls, 1);
	assert_int_equal(r.can_begin, 0);
}

/* After a later obs_reset_video(), a new raw output works normally. */
static void output_after_reset_works(void **state)
{
	(void)state;
	struct result r;
	if (!run_child(MODE_AFTER_RESET, &r))
		skip();
	check_created_without_video(&r);
	if (r.created) {
		assert_false(r.started);
		assert_false(r.active);
	}

	if (r.reset_ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: %d\n", r.reset_ret);
		fail();
	}
	assert_true(r.new_created);
	assert_true(r.new_video_is_main);
	assert_int_equal(r.new_width, 64);
	assert_int_equal(r.new_height, 64);
	assert_true(r.new_started);
	if (r.new_frames < 3) {
		fprintf(stderr, "the output created after the reset received %ld raw frames in %d ms\n", r.new_frames,
			FRAME_TIMEOUT_MS);
		fail();
	}

	/* The one created before keeps no video, like an output given NULL
	 * video by obs_output_set_media(), and still refuses to start. */
	if (r.created) {
		assert_false(r.old_has_video);
		assert_false(r.old_started);
	}
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
		cmocka_unit_test(create_flagless_before_video),
		cmocka_unit_test(create_raw_before_video),
		cmocka_unit_test(start_raw_before_video_fails),
		cmocka_unit_test(output_after_reset_works),
	};
	return cmocka_run_group_tests_name("test_output_before_video", tests, group_setup, group_teardown);
}
