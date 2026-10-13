/* Regression test for obsproject/obs-studio#10746 (reported by skedgyedgy,
 * confirmed by cjmaxik): with a source set to Subtract blending, a rescaled
 * recording or stream shows solid black where the program shows the
 * subtracted colour.
 *
 * Subtract blending subtracts alpha too, so where an opaque source is
 * subtracted from an opaque background the canvas has colour but alpha 0.
 * The main output ignores alpha. An encoder with its own scaled size gets
 * its own video mix, which reuses the main mix's canvas texture
 * (draw_mix_texture in obs-video.c) by drawing it with the default
 * blending, which multiplies the colour by that alpha: black.
 *
 * The test puts an opaque grey background and an opaque darker box set to
 * Subtract on the program, runs a raw video encoder on the main mix and one
 * with a scaled size (GPU scaling), and compares the luma they get:
 *
 *   - the scaled encoder must see the same picture as the unscaled one;
 *   - controls: the unscaled encoder sees the subtracted grey, not black;
 *     with a normal (opaque) box both encoders see the same picture.
 *
 * Core video needs a graphics module and libobs needs an X display on Linux,
 * so the cases skip without one (run under xvfb-run). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>

#include <obs.h>
#include <graphics/vec4.h>
#include <util/platform.h>
#include <util/threading.h>

#define BOX_ID "test_scaled_mix_alpha_box"
#define VENC_ID "test_scaled_mix_alpha_encoder"
#define OUTPUT_ID "test_scaled_mix_alpha_output"
#define BASE 64
#define SCALED 32

static bool obs_up;

/* ------------------------------------------------------------------------- */
/* an opaque box of a given grey */

struct box {
	float level;
};

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static void *box_create(obs_data_t *settings, obs_source_t *source)
{
	(void)source;
	struct box *b = bzalloc(sizeof(*b));
	b->level = (float)obs_data_get_double(settings, "level");
	return b;
}

static void box_destroy(void *data)
{
	bfree(data);
}

static uint32_t box_size(void *data)
{
	(void)data;
	return BASE;
}

static void box_render(void *data, gs_effect_t *effect)
{
	struct box *b = data;
	(void)effect;
	gs_effect_t *solid = obs_get_base_effect(OBS_EFFECT_SOLID);
	struct vec4 color;
	vec4_set(&color, b->level, b->level, b->level, 1.0f);
	gs_effect_set_vec4(gs_effect_get_param_by_name(solid, "color"), &color);
	while (gs_effect_loop(solid, "Solid"))
		gs_draw_sprite(NULL, 0, BASE, BASE);
}

/* ------------------------------------------------------------------------- */
/* a raw video encoder that notes the mean luma of the frames it gets */

struct venc {
	pthread_mutex_t mutex;
	long frames;
	double luma;
	uint32_t width;
	uint32_t height;
};

static uint8_t packet_byte;

static const char *venc_name(void *type_data)
{
	(void)type_data;
	return "luma probe";
}

static void *venc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	struct venc *v = bzalloc(sizeof(*v));
	pthread_mutex_init(&v->mutex, NULL);
	v->width = obs_encoder_get_width(encoder);
	v->height = obs_encoder_get_height(encoder);
	return v;
}

static void venc_destroy(void *data)
{
	struct venc *v = data;
	pthread_mutex_destroy(&v->mutex);
	bfree(v);
}

static bool venc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	struct venc *v = data;
	double sum = 0.0;

	for (uint32_t y = 0; y < v->height; y++) {
		const uint8_t *row = frame->data[0] + (size_t)y * frame->linesize[0];
		for (uint32_t x = 0; x < v->width; x++)
			sum += row[x];
	}

	pthread_mutex_lock(&v->mutex);
	v->frames++;
	v->luma = sum / ((double)v->width * v->height);
	pthread_mutex_unlock(&v->mutex);

	packet->data = &packet_byte;
	packet->size = 1;
	packet->pts = frame->pts;
	packet->dts = frame->pts;
	packet->type = OBS_ENCODER_VIDEO;
	packet->keyframe = true;
	*received = true;
	return true;
}

static void venc_video_info(void *data, struct video_scale_info *info)
{
	(void)data;
	info->format = VIDEO_FORMAT_NV12;
}

/* ------------------------------------------------------------------------- */
/* a video-only output to run an encoder */

static const char *out_name(void *type_data)
{
	(void)type_data;
	return "output";
}

static void *out_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	return output;
}

static void out_destroy(void *data)
{
	(void)data;
}

static bool out_start(void *data)
{
	obs_output_t *output = data;
	if (!obs_output_can_begin_data_capture(output, 0))
		return false;
	if (!obs_output_initialize_encoders(output, 0))
		return false;
	return obs_output_begin_data_capture(output, 0);
}

static void out_stop(void *data, uint64_t ts)
{
	(void)ts;
	obs_output_end_data_capture(data);
}

static void out_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static void register_types(void)
{
	struct obs_source_info box = {
		.id = BOX_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
		.get_name = box_name,
		.create = box_create,
		.destroy = box_destroy,
		.get_width = box_size,
		.get_height = box_size,
		.video_render = box_render,
	};
	obs_register_source(&box);

	struct obs_encoder_info venc = {
		.id = VENC_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = venc_name,
		.create = venc_create,
		.destroy = venc_destroy,
		.encode = venc_encode,
		.get_video_info = venc_video_info,
	};
	obs_register_encoder(&venc);

	struct obs_output_info out = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED,
		.get_name = out_name,
		.create = out_create,
		.destroy = out_destroy,
		.start = out_start,
		.stop = out_stop,
		.encoded_packet = out_packet,
	};
	obs_register_output(&out);
}

/* ------------------------------------------------------------------------- */

struct probe {
	obs_encoder_t *encoder;
	obs_output_t *output;
};

static void start_probe(struct probe *p, const char *name, uint32_t scaled)
{
	p->encoder = obs_video_encoder_create(VENC_ID, name, NULL, NULL);
	assert_non_null(p->encoder);
	if (scaled) {
		obs_encoder_set_scaled_size(p->encoder, scaled, scaled);
		obs_encoder_set_gpu_scale_type(p->encoder, OBS_SCALE_BILINEAR);
	}
	obs_encoder_set_video(p->encoder, obs_get_video());
	p->output = obs_output_create(OUTPUT_ID, name, NULL, NULL);
	assert_non_null(p->output);
	obs_output_set_video_encoder(p->output, p->encoder);
	assert_true(obs_output_start(p->output));
}

static double probe_luma(struct probe *p)
{
	struct venc *v = obs_obj_get_data(p->encoder);
	double luma = -1.0;
	for (int i = 0; i < 300; i++) {
		pthread_mutex_lock(&v->mutex);
		bool ready = v->frames >= 10;
		luma = v->luma;
		pthread_mutex_unlock(&v->mutex);
		if (ready)
			return luma;
		os_sleep_ms(10);
	}
	return -1.0;
}

static void stop_probe(struct probe *p)
{
	obs_output_stop(p->output);
	for (int i = 0; i < 300 && obs_output_active(p->output); i++)
		os_sleep_ms(10);
	obs_output_release(p->output);
	obs_encoder_release(p->encoder);
}

static obs_source_t *box(const char *name, double level)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_double(settings, "level", level);
	obs_source_t *source = obs_source_create(BOX_ID, name, settings, NULL);
	obs_data_release(settings);
	assert_non_null(source);
	return source;
}

/* Runs the unscaled and the scaled encoder on a grey background with a
 * darker box over it, normal or subtracted, and returns their mean luma. */
static void run_scene(bool subtract, double *main_luma, double *scaled_luma)
{
	obs_scene_t *scene = obs_scene_create("scene");
	obs_source_t *bg = box("background", 0.75);
	obs_source_t *fg = box("box", 0.25);
	obs_scene_add(scene, bg);
	obs_sceneitem_t *item = obs_scene_add(scene, fg);
	if (subtract)
		obs_sceneitem_set_blending_mode(item, OBS_BLEND_SUBTRACT);
	obs_set_output_source(0, obs_scene_get_source(scene));

	struct probe main_mix = {0}, scaled_mix = {0};
	start_probe(&main_mix, "unscaled", 0);
	start_probe(&scaled_mix, "scaled", SCALED);

	*main_luma = probe_luma(&main_mix);
	*scaled_luma = probe_luma(&scaled_mix);
	fprintf(stderr, "%s: mean luma unscaled %.1f, scaled %.1f (black is 16)\n",
		subtract ? "subtracted box" : "normal box", *main_luma, *scaled_luma);

	stop_probe(&scaled_mix);
	stop_probe(&main_mix);
	obs_set_output_source(0, NULL);
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_source_release(bg);
	obs_source_release(fg);
}

/* The report: an opaque box subtracted from an opaque background leaves
 * colour with alpha 0 on the canvas. */
static void subtracted_canvas_reaches_scaled_encoder(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	double main_luma, scaled_luma;
	run_scene(true, &main_luma, &scaled_luma);

	/* the program shows the subtracted grey */
	assert_true(main_luma > 40.0);
	/* the scaled output must show the same picture */
	assert_true(fabs(scaled_luma - main_luma) < 8.0);
}

/* Control: an opaque canvas reaches the scaled encoder today. */
static void opaque_canvas_reaches_scaled_encoder(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	double main_luma, scaled_luma;
	run_scene(false, &main_luma, &scaled_luma);

	assert_true(main_luma > 40.0);
	assert_true(fabs(scaled_luma - main_luma) < 8.0);
}

static int group_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = BASE,
		.base_height = BASE,
		.output_width = BASE,
		.output_height = BASE,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.gpu_conversion = true,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	register_types();
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
		cmocka_unit_test(subtracted_canvas_reaches_scaled_encoder),
		cmocka_unit_test(opaque_canvas_reaches_scaled_encoder),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
