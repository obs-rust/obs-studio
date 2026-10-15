/* Regression test for a crash reported in obsproject/obs-studio#11940 (Penwy)
 * and obsproject/obs-websocket#1216 (working-name): when a recording cannot
 * be written (a folder name Windows refuses, a character NTFS does not allow,
 * a read-only or missing folder), the first Start Recording fails with
 * "Unable to write to ...", and the next Start Recording crashes OBS.
 *
 * ffmpeg_mux_start() (plugins/obs-ffmpeg/obs-ffmpeg-mux.c) takes a reference
 * to the output's settings with obs_output_get_settings() and releases it
 * after ffmpeg_mux_start_internal() returns. When the writability check
 * fails, ffmpeg_mux_start_internal() calls set_file_not_readable_error(),
 * which released the same settings a second time. Each failed start so
 * drops a reference the output owns: after the first one the output's
 * settings are freed while the output still uses them, and the frontend's
 * next obs_output_update() (the next Start Recording) writes into freed
 * memory.
 *
 * obs-ffmpeg-mux.c is built into the test and runs as the real
 * "ffmpeg_muxer" output against core video, so the case needs a display
 * (xvfb-run on Linux) and skips without one. obs_data_release() is wrapped
 * (--wrap) to count how often the output code releases the output's
 * settings during a start; it must release exactly the one reference it
 * took. The path's parent is a regular file, so the start fails the same way
 * for every user, root included.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <cmocka.h>

#include <obs.h>
#include <obs-module.h>
#include <util/bmem.h>
#include <util/dstr.h>
#include <util/platform.h>

#include <unistd.h>

extern struct obs_output_info ffmpeg_muxer;

/* obs-ffmpeg-mux.c looks up its UI strings through the module locale. */
const char *obs_module_text(const char *lookup)
{
	return lookup;
}

/* ------------------------------------------------------------------------- */
/* count releases of the watched settings */

void __real_obs_data_release(obs_data_t *data);

static obs_data_t *watched;
static int watched_releases;

void __wrap_obs_data_release(obs_data_t *data)
{
	if (data && data == watched)
		watched_releases++;
	__real_obs_data_release(data);
}

/* ------------------------------------------------------------------------- */
/* encoders that accept anything */

#define VENC_ID "test_unwritable_video_encoder"
#define AENC_ID "test_unwritable_audio_encoder"

static uint8_t packet_byte;
static bool obs_up;

static const char *enc_name(void *type_data)
{
	UNUSED_PARAMETER(type_data);
	return "test encoder";
}

static void *enc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	UNUSED_PARAMETER(settings);
	return encoder;
}

static void enc_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static bool enc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	UNUSED_PARAMETER(data);
	packet->data = &packet_byte;
	packet->size = 1;
	packet->pts = frame->pts;
	packet->dts = frame->pts;
	packet->keyframe = true;
	*received = true;
	return true;
}

static size_t aenc_frame_size(void *data)
{
	UNUSED_PARAMETER(data);
	return 1024;
}

static int setup(void **state)
{
	(void)state;
	const char *display = getenv("DISPLAY");
	if (!display || !*display)
		return 0;
	if (!obs_startup("en-US", NULL, NULL))
		return 0;

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
	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS || !obs_reset_audio(&oai)) {
		fprintf(stderr, "core video/audio failed; skipping\n");
		obs_shutdown();
		return 0;
	}

	struct obs_encoder_info venc = {
		.id = VENC_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = enc_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = enc_encode,
	};
	struct obs_encoder_info aenc = {
		.id = AENC_ID,
		.type = OBS_ENCODER_AUDIO,
		.codec = "aac",
		.get_name = enc_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = enc_encode,
		.get_frame_size = aenc_frame_size,
	};
	obs_register_encoder(&venc);
	obs_register_encoder(&aenc);
	obs_register_output(&ffmpeg_muxer);
	obs_up = true;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	obs_up = false;
	return 0;
}

/* ------------------------------------------------------------------------- */

/* A path whose parent is a regular file: no one can create it. */
static char *unwritable_path(char **blocker_out)
{
	struct dstr blocker = {0};
	const char *tmp = getenv("TMPDIR");
	dstr_printf(&blocker, "%s/obs-test-unwritable-%d", tmp && *tmp ? tmp : "/tmp", (int)getpid());
	FILE *f = os_fopen(blocker.array, "wb");
	if (!f) {
		fprintf(stderr, "could not create %s\n", blocker.array);
		fail();
	}
	fclose(f);

	struct dstr path = {0};
	dstr_printf(&path, "%s/2026-10-15 00-13-36.mkv", blocker.array);
	*blocker_out = blocker.array;
	return path.array;
}

/* Starts the output (expected to fail) and returns how often the output
 * code released the output's settings during that start. */
static int failed_start_releases(obs_output_t *output)
{
	obs_data_t *settings = obs_output_get_settings(output);
	watched = settings;
	watched_releases = 0;

	bool started = obs_output_start(output);
	int releases = watched_releases;
	watched = NULL;

	/* Put back what an extra release took, so a failing run reports the
	 * count instead of crashing on the freed settings later. */
	for (int i = 1; i < releases; i++)
		obs_data_addref(settings);

	if (started) {
		fprintf(stderr, "ffmpeg_muxer started on a path that cannot be written\n");
		obs_output_stop(output);
	}
	assert_false(started);
	obs_data_release(settings);
	return releases;
}

static obs_output_t *create_recording(const char *path, obs_encoder_t **venc, obs_encoder_t **aenc)
{
	*venc = obs_video_encoder_create(VENC_ID, "venc", NULL, NULL);
	*aenc = obs_audio_encoder_create(AENC_ID, "aenc", NULL, 0, NULL);
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "path", path);
	obs_output_t *output = obs_output_create("ffmpeg_muxer", "recording", settings, NULL);
	obs_data_release(settings);
	if (!*venc || !*aenc || !output) {
		fprintf(stderr, "could not create the encoders or the ffmpeg_muxer output\n");
		fail();
	}
	obs_encoder_set_video(*venc, obs_get_video());
	obs_encoder_set_audio(*aenc, obs_get_audio());
	obs_output_set_video_encoder(output, *venc);
	obs_output_set_audio_encoder(output, *aenc, 0);
	return output;
}

/* One failed start: "Unable to write to ..." is reported, and the output
 * releases only the reference to its settings it took. */
static void test_failed_start_releases_settings_once(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	char *blocker = NULL;
	char *path = unwritable_path(&blocker);
	obs_encoder_t *venc, *aenc;
	obs_output_t *output = create_recording(path, &venc, &aenc);

	int releases = failed_start_releases(output);
	const char *error = obs_output_get_last_error(output);
	bool reported = error && strstr(error, "UnableToWritePath");

	obs_output_release(output);
	obs_encoder_release(venc);
	obs_encoder_release(aenc);
	os_unlink(blocker);
	bfree(blocker);
	bfree(path);

	if (releases != 1)
		fprintf(stderr, "a failed start released the output's settings %d times (took 1 reference)\n",
			releases);
	assert_true(reported);
	assert_int_equal(releases, 1);
}

/* The report: Start Recording fails, then Start Recording again. Every
 * attempt must release exactly one reference, and the output's settings
 * must still be its own afterwards (the frontend updates them next). */
static void test_retry_after_failed_start_keeps_settings(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	char *blocker = NULL;
	char *path = unwritable_path(&blocker);
	obs_encoder_t *venc, *aenc;
	obs_output_t *output = create_recording(path, &venc, &aenc);

	int total = 0;
	for (int i = 0; i < 3; i++) {
		/* what the frontend's ConfigureRecording() does before each start */
		obs_data_t *update = obs_data_create();
		obs_data_set_string(update, "path", path);
		obs_output_update(output, update);
		obs_data_release(update);

		total += failed_start_releases(output);
	}

	obs_output_release(output);
	obs_encoder_release(venc);
	obs_encoder_release(aenc);
	os_unlink(blocker);
	bfree(blocker);
	bfree(path);

	if (total != 3)
		fprintf(stderr, "3 failed starts released the output's settings %d times\n", total);
	assert_int_equal(total, 3);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_failed_start_releases_settings_once),
		cmocka_unit_test(test_retry_after_failed_start_keeps_settings),
	};

	return cmocka_run_group_tests_name("ffmpeg_mux_unwritable_path", tests, setup, teardown);
}
