/* Regression test for obsproject/obs-studio#6591 (reported by SuslikV): a
 * source with transparent areas set to Darken blending shows black there,
 * and its alpha punches a hole through the scene, instead of leaving the
 * background as it is.
 *
 * Darken draws the item with GS_BLEND_OP_MIN, and the graphics APIs ignore
 * the blend factors for MIN and MAX: the result is min(source, destination)
 * per channel, on premultiplied colour. A transparent source pixel is
 * (0, 0, 0, 0), so the minimum is 0 in colour and in alpha.
 *
 * The test puts an opaque grey background on the program and over it a box
 * that is transparent in its left half and an opaque darker grey in its
 * right half, and runs a raw video encoder on the program:
 *
 *   - with Darken, the transparent half must show the background, so the
 *     picture is the same as with Normal blending (where the opaque half is
 *     darker than the background, Darken and Normal agree);
 *   - control: with Normal blending the left half shows the background.
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

#define BOX_ID "test_darken_transparent_box"
#define VENC_ID "test_darken_transparent_encoder"
#define OUTPUT_ID "test_darken_transparent_output"
#define BASE 64

static bool obs_up;

/* ------------------------------------------------------------------------- */
/* a box of a given grey, opaque everywhere or only in its right half */

struct box {
	float level;
	bool half;
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
	b->half = obs_data_get_bool(settings, "half");
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

	gs_matrix_push();
	if (b->half)
		gs_matrix_translate3f((float)(BASE / 2), 0.0f, 0.0f);
	while (gs_effect_loop(solid, "Solid"))
		gs_draw_sprite(NULL, 0, b->half ? BASE / 2 : BASE, BASE);
	gs_matrix_pop();
}

/* ------------------------------------------------------------------------- */
/* a raw video encoder that notes the mean luma of each half of the frame */

struct venc {
	pthread_mutex_t mutex;
	long frames;
	double left;
	double right;
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
	double left = 0.0, right = 0.0;
	/* stay a few pixels away from the edge between the halves */
	const uint32_t margin = 4;
	uint32_t n = 0;

	for (uint32_t y = 0; y < v->height; y++) {
		const uint8_t *row = frame->data[0] + (size_t)y * frame->linesize[0];
		for (uint32_t x = 0; x < v->width / 2 - margin; x++) {
			left += row[x];
			right += row[v->width - 1 - x];
		}
		n += v->width / 2 - margin;
	}

	pthread_mutex_lock(&v->mutex);
	v->frames++;
	v->left = left / n;
	v->right = right / n;
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
/* a video-only output to run the encoder */

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

static obs_source_t *box(const char *name, double level, bool half)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_double(settings, "level", level);
	obs_data_set_bool(settings, "half", half);
	obs_source_t *source = obs_source_create(BOX_ID, name, settings, NULL);
	obs_data_release(settings);
	assert_non_null(source);
	return source;
}

/* Runs the encoder on a light grey background with the half-transparent
 * darker box over it, blended as given, and returns the mean luma of the
 * left (transparent) and right (opaque) halves. */
static void run_scene(enum obs_blending_type blend, double *left, double *right)
{
	obs_scene_t *scene = obs_scene_create("scene");
	obs_source_t *bg = box("background", 0.75, false);
	obs_source_t *fg = box("box", 0.25, true);
	obs_scene_add(scene, bg);
	obs_sceneitem_t *item = obs_scene_add(scene, fg);
	obs_sceneitem_set_blending_mode(item, blend);
	obs_set_output_source(0, obs_scene_get_source(scene));

	obs_encoder_t *encoder = obs_video_encoder_create(VENC_ID, "probe", NULL, NULL);
	assert_non_null(encoder);
	obs_encoder_set_video(encoder, obs_get_video());
	obs_output_t *output = obs_output_create(OUTPUT_ID, "probe", NULL, NULL);
	assert_non_null(output);
	obs_output_set_video_encoder(output, encoder);
	assert_true(obs_output_start(output));

	struct venc *v = obs_obj_get_data(encoder);
	*left = *right = -1.0;
	for (int i = 0; i < 300; i++) {
		pthread_mutex_lock(&v->mutex);
		bool ready = v->frames >= 10;
		*left = v->left;
		*right = v->right;
		pthread_mutex_unlock(&v->mutex);
		if (ready)
			break;
		os_sleep_ms(10);
	}
	fprintf(stderr, "%s: mean luma left (transparent) %.1f, right (opaque) %.1f (black is 16)\n",
		blend == OBS_BLEND_DARKEN ? "darken" : "normal", *left, *right);

	obs_output_stop(output);
	for (int i = 0; i < 300 && obs_output_active(output); i++)
		os_sleep_ms(10);
	obs_output_release(output);
	obs_encoder_release(encoder);
	obs_set_output_source(0, NULL);
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_source_release(bg);
	obs_source_release(fg);
}

/* The report: where the Darken source is transparent, the background must
 * show, not black. */
static void darken_keeps_background_where_transparent(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	double normal_left, normal_right, darken_left, darken_right;
	run_scene(OBS_BLEND_NORMAL, &normal_left, &normal_right);
	run_scene(OBS_BLEND_DARKEN, &darken_left, &darken_right);

	assert_true(darken_left > 100.0);
	assert_true(fabs(darken_left - normal_left) < 8.0);
	/* the opaque half is darker than the background: Darken picks it */
	assert_true(fabs(darken_right - normal_right) < 8.0);
}

/* Control: with Normal blending the transparent half shows the background
 * and the opaque half the box. */
static void normal_shows_background_where_transparent(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	double left, right;
	run_scene(OBS_BLEND_NORMAL, &left, &right);
	assert_true(left > 100.0);
	assert_true(right < left - 40.0);
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
		cmocka_unit_test(darken_keeps_background_where_transparent),
		cmocka_unit_test(normal_shows_background_where_transparent),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
