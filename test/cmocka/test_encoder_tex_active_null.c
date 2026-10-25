/* Regression test for obs-rust/obs-studio#2267: obs_encoder_video_tex_active()
 * read the mix that get_mix_for_video() found for the encoder's video_t
 * without checking it. get_mix_for_video() returns NULL when no mix owns
 * that video_t, so the call crashed:
 *
 * - for a video encoder with no video set, before or after video is set up;
 * - for an encoder whose video belonged to a canvas mix that is gone: here a
 *   canvas created with its own video, whose last reference is released.
 *
 * Encoder plugins call it to choose between texture and raw encoding.
 *
 * Each case runs in a forked child from obs_startup() to obs_shutdown(), so a
 * crash fails only its own case. The child records how far it got in a
 * shared page, and the parent reports the stage it died in.
 *
 * - "no_video": NV12 and P010 texture encoding are not active for an
 *   encoder with no video, both before and after obs_reset_video().
 * - "canvas_gone": the same for an encoder left with the video of a canvas
 *   that was destroyed. While the canvas lives, the answer is the canvas
 *   mix's.
 * - "main_video_*": for an encoder given obs_get_video(), the answer is
 *   unchanged: NV12 (or P010) texture output is active exactly when the
 *   output format is NV12 (or P010) and gs_nv12_available() (or
 *   gs_p010_available()) says so. These pass with or without the fix.
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

#define ENCODER_ID "test_etan_video_encoder"
#define BASE_SIZE 64

/* ------------------------------------------------------------------------- */
/* what the child reports */

enum stage {
	STAGE_NONE,
	STAGE_STARTUP_FAILED,
	STAGE_STARTED,
	STAGE_BEFORE,
	STAGE_BEFORE_DONE,
	STAGE_RESET,
	STAGE_CANVAS,
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
		return "in obs_encoder_video_tex_active before video is set up";
	case STAGE_BEFORE_DONE:
		return "after the call before video is set up";
	case STAGE_RESET:
		return "in obs_reset_video";
	case STAGE_CANVAS:
		return "creating or releasing the canvas";
	case STAGE_AFTER:
		return "in obs_encoder_video_tex_active after obs_reset_video";
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
	bool encoder_created;

	/* obs_encoder_video_tex_active() for NV12, P010 and I420 */
	bool nv12_before;
	bool p010_before;
	bool i420_before;
	bool nv12_after;
	bool p010_after;
	bool i420_after;

	/* what it must match after obs_reset_video() */
	bool video_is_set;
	bool nv12_available;
	bool p010_available;

	/* the canvas case */
	bool canvas_created;
	bool canvas_has_video;
	bool canvas_nv12_alive;
	bool canvas_p010_alive;
	bool canvas_destroyed;
};

static struct result *shared;

/* ------------------------------------------------------------------------- */
/* the encoder */

static const char *enc_name(void *type_data)
{
	(void)type_data;
	return "texture active test encoder";
}

static void *enc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	return bzalloc(1);
}

static void enc_destroy(void *data)
{
	bfree(data);
}

static bool enc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received_packet)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received_packet = false;
	return true;
}

static void register_encoder(void)
{
	struct obs_encoder_info info = {
		.id = ENCODER_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = enc_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = enc_encode,
	};
	obs_register_encoder(&info);
}

/* ------------------------------------------------------------------------- */
/* the child */

enum mode { MODE_NO_VIDEO, MODE_CANVAS_GONE, MODE_MAIN_NV12, MODE_MAIN_P010 };

static struct obs_video_info make_ovi(enum video_format format)
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
	return ovi;
}

static void read_after(obs_encoder_t *encoder)
{
	shared->stage = STAGE_AFTER;
	shared->nv12_after = obs_encoder_video_tex_active(encoder, VIDEO_FORMAT_NV12);
	shared->p010_after = obs_encoder_video_tex_active(encoder, VIDEO_FORMAT_P010);
	shared->i420_after = obs_encoder_video_tex_active(encoder, VIDEO_FORMAT_I420);
	shared->stage = STAGE_AFTER_DONE;
}

static void run_canvas_gone(obs_encoder_t *encoder)
{
	shared->stage = STAGE_CANVAS;
	struct obs_video_info ovi = make_ovi(VIDEO_FORMAT_NV12);
	obs_canvas_t *canvas = obs_canvas_create_private("encoder tex active canvas", &ovi, 0);
	shared->canvas_created = canvas != NULL;
	if (!canvas)
		return;
	video_t *video = obs_canvas_get_video(canvas);
	shared->canvas_has_video = video != NULL;
	if (!video) {
		obs_canvas_release(canvas);
		return;
	}

	obs_encoder_set_video(encoder, video);
	shared->video_is_set = obs_encoder_video(encoder) == video;
	shared->canvas_nv12_alive = obs_encoder_video_tex_active(encoder, VIDEO_FORMAT_NV12);
	shared->canvas_p010_alive = obs_encoder_video_tex_active(encoder, VIDEO_FORMAT_P010);

	/* Release the last reference: the canvas and its mix are destroyed,
	 * and the encoder keeps the video_t it was given. */
	obs_weak_canvas_t *weak = obs_canvas_get_weak_canvas(canvas);
	obs_canvas_release(canvas);
	obs_canvas_t *again = obs_weak_canvas_get_canvas(weak);
	shared->canvas_destroyed = again == NULL;
	obs_canvas_release(again);
	obs_weak_canvas_release(weak);

	read_after(encoder);
}

static void child(enum mode mode)
{
	shared->stage = STAGE_NONE;
	if (!obs_startup("en-US", NULL, NULL)) {
		shared->stage = STAGE_STARTUP_FAILED;
		return;
	}
	shared->stage = STAGE_STARTED;
	register_encoder();

	obs_encoder_t *encoder = obs_video_encoder_create(ENCODER_ID, "tex active", NULL, NULL);
	shared->encoder_created = encoder != NULL;
	if (!encoder)
		goto shutdown;

	if (mode == MODE_NO_VIDEO) {
		shared->stage = STAGE_BEFORE;
		shared->nv12_before = obs_encoder_video_tex_active(encoder, VIDEO_FORMAT_NV12);
		shared->p010_before = obs_encoder_video_tex_active(encoder, VIDEO_FORMAT_P010);
		shared->i420_before = obs_encoder_video_tex_active(encoder, VIDEO_FORMAT_I420);
		shared->stage = STAGE_BEFORE_DONE;
	}

	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	obs_reset_audio(&oai);

	shared->stage = STAGE_RESET;
	struct obs_video_info ovi = make_ovi(mode == MODE_MAIN_P010 ? VIDEO_FORMAT_P010 : VIDEO_FORMAT_NV12);
	shared->reset_ret = obs_reset_video(&ovi);
	if (shared->reset_ret != OBS_VIDEO_SUCCESS)
		goto release;

	obs_enter_graphics();
	shared->nv12_available = gs_nv12_available();
	shared->p010_available = gs_p010_available();
	obs_leave_graphics();

	switch (mode) {
	case MODE_NO_VIDEO:
		read_after(encoder);
		break;
	case MODE_CANVAS_GONE:
		run_canvas_gone(encoder);
		break;
	case MODE_MAIN_NV12:
	case MODE_MAIN_P010:
		obs_encoder_set_video(encoder, obs_get_video());
		shared->video_is_set = obs_encoder_video(encoder) == obs_get_video();
		read_after(encoder);
		break;
	}

release:
	obs_encoder_release(encoder);
shutdown:
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
	assert_true(out->encoder_created);
	if (out->reset_ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: %d\n", out->reset_ret);
		fail();
	}
	return true;
}

/* ------------------------------------------------------------------------- */
/* cases */

static void check_inactive(const char *when, bool nv12, bool p010, bool i420)
{
	if (nv12 || p010 || i420) {
		fprintf(stderr, "%s: texture encoding reported active (NV12 %d, P010 %d, I420 %d)\n", when, nv12, p010,
			i420);
		fail();
	}
}

/* A video encoder with no video set, before and after obs_reset_video(). */
static void no_video_inactive(void **state)
{
	(void)state;
	struct result r;
	if (!run_child(MODE_NO_VIDEO, &r))
		skip();
	check_inactive("no video, before obs_reset_video", r.nv12_before, r.p010_before, r.i420_before);
	check_inactive("no video, after obs_reset_video", r.nv12_after, r.p010_after, r.i420_after);
}

/* An encoder left with the video of a canvas whose mix is gone. */
static void canvas_gone_inactive(void **state)
{
	(void)state;
	struct result r;
	if (!run_child(MODE_CANVAS_GONE, &r))
		skip();
	assert_true(r.canvas_created);
	assert_true(r.canvas_has_video);
	assert_true(r.video_is_set);

	/* While the canvas lives, its NV12 mix answers like the main one. */
	if (r.canvas_nv12_alive != r.nv12_available || r.canvas_p010_alive) {
		fprintf(stderr, "live canvas: NV12 %d (gs_nv12_available %d), P010 %d\n", r.canvas_nv12_alive,
			r.nv12_available, r.canvas_p010_alive);
		fail();
	}

	assert_true(r.canvas_destroyed);
	check_inactive("canvas destroyed", r.nv12_after, r.p010_after, r.i420_after);
}

static void check_main(const struct result *r, enum video_format format)
{
	assert_true(r->video_is_set);
	bool want_nv12 = format == VIDEO_FORMAT_NV12 && r->nv12_available;
	bool want_p010 = format == VIDEO_FORMAT_P010 && r->p010_available;
	fprintf(stderr, "gs_nv12_available %d, gs_p010_available %d\n", r->nv12_available, r->p010_available);
	if (r->nv12_after != want_nv12 || r->p010_after != want_p010 || r->i420_after) {
		fprintf(stderr, "main video: NV12 %d (want %d), P010 %d (want %d), I420 %d\n", r->nv12_after, want_nv12,
			r->p010_after, want_p010, r->i420_after);
		fail();
	}
}

/* Unchanged: an encoder on the main NV12 video. */
static void main_video_nv12(void **state)
{
	(void)state;
	struct result r;
	if (!run_child(MODE_MAIN_NV12, &r))
		skip();
	check_main(&r, VIDEO_FORMAT_NV12);
}

/* Unchanged: an encoder on the main P010 video. */
static void main_video_p010(void **state)
{
	(void)state;
	struct result r;
	if (!run_child(MODE_MAIN_P010, &r))
		skip();
	check_main(&r, VIDEO_FORMAT_P010);
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
		cmocka_unit_test(no_video_inactive),
		cmocka_unit_test(canvas_gone_inactive),
		cmocka_unit_test(main_video_nv12),
		cmocka_unit_test(main_video_p010),
	};
	return cmocka_run_group_tests_name("test_encoder_tex_active_null", tests, group_setup, group_teardown);
}
