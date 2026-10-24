/* Characterization of obs_reset_video() for every output format with a GPU
 * conversion, before the video output plan's Rust port. About 70 tests call
 * obs_reset_video(), but only with NV12, BGRA or RGBA; this one runs I420,
 * NV12, I444, I010, P010, P216 and P416 (each with its color spaces and
 * both ranges) and BGRA and RGBA through the real libobs and its OpenGL
 * renderer, and checks the frames the core video produces.
 *
 * The program shows one opaque box of a fixed color over the whole canvas.
 * For each case a raw video callback takes the third frame and keeps every
 * plane. The test checks each plane's size (a plane must be uniform over
 * exactly the size the format has, so a plane created at the wrong size
 * shows as a part of it that is not) and pins the sample values the
 * conversion produces (a wrong shader technique gives other values). It
 * also checks the size check and the alignment of obs_reset_video() through
 * obs_get_video_info().
 *
 * It uses only the public API, so with the hybrid libobs it checks the Rust
 * port once there is one. test_video_output_plan checks the plan itself.
 *
 * An output width of 2 or 3 would be aligned to 0. obs_reset_video()
 * refuses it before it frees the old video, which keeps running, and a later
 * reset works (#1713). Before, the reset freed the old video, then failed
 * with no video left, and the next reset crashed.
 *
 * Core video needs a graphics module, and libobs needs an X display on
 * Linux, so the cases skip without one (run under xvfb-run). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <graphics/vec4.h>
#include <util/platform.h>
#include <util/threading.h>

#define BOX_ID "test_video_output_formats_box"
#define WIDTH 64
#define HEIGHT 32
/* the frame the callback keeps; the first ones may be from before the
 * scene was drawn */
#define KEEP_FRAME 3
#define FRAME_TIMEOUT_MS 10000

static bool obs_up;
static uint32_t box_cx = WIDTH;
static uint32_t box_cy = HEIGHT;

/* ------------------------------------------------------------------------- */
/* an opaque box over the whole canvas */

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static void *box_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void box_destroy(void *data)
{
	(void)data;
}

static uint32_t box_width(void *data)
{
	(void)data;
	return box_cx;
}

static uint32_t box_height(void *data)
{
	(void)data;
	return box_cy;
}

static void box_render(void *data, gs_effect_t *effect)
{
	(void)data;
	(void)effect;
	gs_effect_t *solid = obs_get_base_effect(OBS_EFFECT_SOLID);
	struct vec4 color;
	vec4_set(&color, 0.75f, 0.25f, 0.5f, 1.0f);
	gs_effect_set_vec4(gs_effect_get_param_by_name(solid, "color"), &color);
	while (gs_effect_loop(solid, "Solid"))
		gs_draw_sprite(NULL, 0, box_cx, box_cy);
}

static void register_box(void)
{
	struct obs_source_info box = {
		.id = BOX_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
		.get_name = box_name,
		.create = box_create,
		.destroy = box_destroy,
		.get_width = box_width,
		.get_height = box_height,
		.video_render = box_render,
	};
	obs_register_source(&box);
}

/* ------------------------------------------------------------------------- */
/* the planes of each format, as the frames lay them out */

struct plane_layout {
	uint32_t wdiv; /* 0: no plane */
	uint32_t hdiv;
	uint32_t comps; /* samples per pixel */
	uint32_t bytes; /* bytes per sample */
};

static void layout_of(enum video_format format, struct plane_layout planes[3])
{
	memset(planes, 0, sizeof(struct plane_layout) * 3);
	switch (format) {
	case VIDEO_FORMAT_I420:
		planes[0] = (struct plane_layout){1, 1, 1, 1};
		planes[1] = (struct plane_layout){2, 2, 1, 1};
		planes[2] = (struct plane_layout){2, 2, 1, 1};
		break;
	case VIDEO_FORMAT_NV12:
		planes[0] = (struct plane_layout){1, 1, 1, 1};
		planes[1] = (struct plane_layout){2, 2, 2, 1};
		break;
	case VIDEO_FORMAT_I444:
		planes[0] = (struct plane_layout){1, 1, 1, 1};
		planes[1] = (struct plane_layout){1, 1, 1, 1};
		planes[2] = (struct plane_layout){1, 1, 1, 1};
		break;
	case VIDEO_FORMAT_I010:
		planes[0] = (struct plane_layout){1, 1, 1, 2};
		planes[1] = (struct plane_layout){2, 2, 1, 2};
		planes[2] = (struct plane_layout){2, 2, 1, 2};
		break;
	case VIDEO_FORMAT_P010:
		planes[0] = (struct plane_layout){1, 1, 1, 2};
		planes[1] = (struct plane_layout){2, 2, 2, 2};
		break;
	case VIDEO_FORMAT_P216:
		planes[0] = (struct plane_layout){1, 1, 1, 2};
		planes[1] = (struct plane_layout){2, 1, 2, 2};
		break;
	case VIDEO_FORMAT_P416:
		planes[0] = (struct plane_layout){1, 1, 1, 2};
		planes[1] = (struct plane_layout){1, 1, 2, 2};
		break;
	case VIDEO_FORMAT_BGRA:
	case VIDEO_FORMAT_RGBA:
		planes[0] = (struct plane_layout){1, 1, 4, 1};
		break;
	default:
		break;
	}
}

/* ------------------------------------------------------------------------- */
/* a raw video callback that keeps one frame */

struct probe {
	uint32_t width;
	uint32_t height;
	struct plane_layout planes[3];
	uint8_t *data[3];
	uint32_t linesize[3];
	bool extra_plane; /* a fourth plane pointer was set */
	long frames;
	os_event_t *kept;
};

static void probe_frame(void *param, struct video_data *frame)
{
	struct probe *p = param;
	if (++p->frames != KEEP_FRAME)
		return;

	for (size_t c = 0; c < 3; c++) {
		const struct plane_layout *l = &p->planes[c];
		if (!l->wdiv)
			continue;
		uint32_t rows = p->height / l->hdiv;
		p->linesize[c] = frame->linesize[c];
		if (!frame->data[c])
			continue;
		p->data[c] = bmemdup(frame->data[c], (size_t)frame->linesize[c] * rows);
	}
	p->extra_plane = frame->data[3] != NULL;
	os_event_signal(p->kept);
}

static uint32_t sample(const struct probe *p, size_t c, uint32_t x, uint32_t y, uint32_t comp)
{
	const struct plane_layout *l = &p->planes[c];
	const uint8_t *at = p->data[c] + (size_t)y * p->linesize[c] + ((size_t)x * l->comps + comp) * l->bytes;
	if (l->bytes == 1)
		return at[0];
	return (uint32_t)at[0] | ((uint32_t)at[1] << 8);
}

/* ------------------------------------------------------------------------- */
/* the cases */

struct format_case {
	enum video_format format;
	enum video_colorspace colorspace;
	enum video_range_type range;
	/* the samples of the box, per plane and sample of a pixel */
	uint32_t want[3][4];
};

/* Measured on the C libobs with llvmpipe; the tolerance below allows for
 * other renderers' rounding. The test never calls obs_set_video_levels(), so
 * the Rec. 2100 (PQ and HLG) rows draw the box at the default SDR white
 * level of 300 nits. Before #2209 that level read 0 until something set it,
 * and those rows were black. */
static const struct format_case cases[] = {
	{VIDEO_FORMAT_I420, VIDEO_CS_601, VIDEO_RANGE_PARTIAL, {{110, 0, 0, 0}, {137, 0, 0, 0}, {179, 0, 0, 0}}},
	{VIDEO_FORMAT_I420, VIDEO_CS_601, VIDEO_RANGE_FULL, {{109, 0, 0, 0}, {139, 0, 0, 0}, {186, 0, 0, 0}}},
	{VIDEO_FORMAT_I420, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, {{98, 0, 0, 0}, {143, 0, 0, 0}, {181, 0, 0, 0}}},
	{VIDEO_FORMAT_I420, VIDEO_CS_709, VIDEO_RANGE_FULL, {{96, 0, 0, 0}, {145, 0, 0, 0}, {189, 0, 0, 0}}},
	{VIDEO_FORMAT_I420, VIDEO_CS_SRGB, VIDEO_RANGE_PARTIAL, {{98, 0, 0, 0}, {143, 0, 0, 0}, {181, 0, 0, 0}}},
	{VIDEO_FORMAT_I420, VIDEO_CS_SRGB, VIDEO_RANGE_FULL, {{96, 0, 0, 0}, {145, 0, 0, 0}, {189, 0, 0, 0}}},
	{VIDEO_FORMAT_NV12, VIDEO_CS_601, VIDEO_RANGE_PARTIAL, {{110, 0, 0, 0}, {137, 179, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_NV12, VIDEO_CS_601, VIDEO_RANGE_FULL, {{109, 0, 0, 0}, {139, 186, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_NV12, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, {{98, 0, 0, 0}, {143, 181, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_NV12, VIDEO_CS_709, VIDEO_RANGE_FULL, {{96, 0, 0, 0}, {145, 189, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_NV12, VIDEO_CS_SRGB, VIDEO_RANGE_PARTIAL, {{98, 0, 0, 0}, {143, 181, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_NV12, VIDEO_CS_SRGB, VIDEO_RANGE_FULL, {{96, 0, 0, 0}, {145, 189, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_I444, VIDEO_CS_601, VIDEO_RANGE_PARTIAL, {{110, 0, 0, 0}, {137, 0, 0, 0}, {179, 0, 0, 0}}},
	{VIDEO_FORMAT_I444, VIDEO_CS_601, VIDEO_RANGE_FULL, {{109, 0, 0, 0}, {139, 0, 0, 0}, {186, 0, 0, 0}}},
	{VIDEO_FORMAT_I444, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, {{98, 0, 0, 0}, {143, 0, 0, 0}, {181, 0, 0, 0}}},
	{VIDEO_FORMAT_I444, VIDEO_CS_709, VIDEO_RANGE_FULL, {{96, 0, 0, 0}, {145, 0, 0, 0}, {189, 0, 0, 0}}},
	{VIDEO_FORMAT_I444, VIDEO_CS_SRGB, VIDEO_RANGE_PARTIAL, {{98, 0, 0, 0}, {143, 0, 0, 0}, {181, 0, 0, 0}}},
	{VIDEO_FORMAT_I444, VIDEO_CS_SRGB, VIDEO_RANGE_FULL, {{96, 0, 0, 0}, {145, 0, 0, 0}, {189, 0, 0, 0}}},
	{VIDEO_FORMAT_I010, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, {{611, 0, 0, 0}, {566, 0, 0, 0}, {658, 0, 0, 0}}},
	{VIDEO_FORMAT_I010, VIDEO_CS_709, VIDEO_RANGE_FULL, {{639, 0, 0, 0}, {573, 0, 0, 0}, {679, 0, 0, 0}}},
	{VIDEO_FORMAT_I010, VIDEO_CS_2100_PQ, VIDEO_RANGE_PARTIAL, {{431, 0, 0, 0}, {532, 0, 0, 0}, {569, 0, 0, 0}}},
	{VIDEO_FORMAT_I010, VIDEO_CS_2100_PQ, VIDEO_RANGE_FULL, {{429, 0, 0, 0}, {535, 0, 0, 0}, {578, 0, 0, 0}}},
	{VIDEO_FORMAT_I010, VIDEO_CS_2100_HLG, VIDEO_RANGE_PARTIAL, {{458, 0, 0, 0}, {560, 0, 0, 0}, {644, 0, 0, 0}}},
	{VIDEO_FORMAT_I010, VIDEO_CS_2100_HLG, VIDEO_RANGE_FULL, {{460, 0, 0, 0}, {567, 0, 0, 0}, {662, 0, 0, 0}}},
	{VIDEO_FORMAT_P010, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, {{39104, 0, 0, 0}, {36224, 42112, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P010, VIDEO_CS_709, VIDEO_RANGE_FULL, {{40896, 0, 0, 0}, {36672, 43456, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P010,
	 VIDEO_CS_2100_PQ,
	 VIDEO_RANGE_PARTIAL,
	 {{27584, 0, 0, 0}, {34048, 36416, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P010, VIDEO_CS_2100_PQ, VIDEO_RANGE_FULL, {{27456, 0, 0, 0}, {34240, 36992, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P010,
	 VIDEO_CS_2100_HLG,
	 VIDEO_RANGE_PARTIAL,
	 {{29312, 0, 0, 0}, {35840, 41216, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P010, VIDEO_CS_2100_HLG, VIDEO_RANGE_FULL, {{29440, 0, 0, 0}, {36288, 42368, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P216, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, {{39107, 0, 0, 0}, {36194, 42102, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P216, VIDEO_CS_709, VIDEO_RANGE_FULL, {{40926, 0, 0, 0}, {36684, 43435, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P216,
	 VIDEO_CS_2100_PQ,
	 VIDEO_RANGE_PARTIAL,
	 {{27604, 0, 0, 0}, {34041, 36440, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P216, VIDEO_CS_2100_PQ, VIDEO_RANGE_FULL, {{27479, 0, 0, 0}, {34223, 36964, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P216,
	 VIDEO_CS_2100_HLG,
	 VIDEO_RANGE_PARTIAL,
	 {{29319, 0, 0, 0}, {35828, 41189, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P216, VIDEO_CS_2100_HLG, VIDEO_RANGE_FULL, {{29484, 0, 0, 0}, {36265, 42392, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P416, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, {{39107, 0, 0, 0}, {36194, 42102, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P416, VIDEO_CS_709, VIDEO_RANGE_FULL, {{40926, 0, 0, 0}, {36684, 43435, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P416,
	 VIDEO_CS_2100_PQ,
	 VIDEO_RANGE_PARTIAL,
	 {{27604, 0, 0, 0}, {34291, 36959, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P416, VIDEO_CS_2100_PQ, VIDEO_RANGE_FULL, {{27479, 0, 0, 0}, {34509, 37558, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P416,
	 VIDEO_CS_2100_HLG,
	 VIDEO_RANGE_PARTIAL,
	 {{29319, 0, 0, 0}, {35322, 39766, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_P416, VIDEO_CS_2100_HLG, VIDEO_RANGE_FULL, {{29484, 0, 0, 0}, {35687, 40766, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_BGRA, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, {{128, 64, 191, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}}},
	{VIDEO_FORMAT_RGBA, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, {{128, 64, 191, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}}},
};

static uint32_t tolerance(enum video_format format)
{
	switch (format) {
	case VIDEO_FORMAT_I010:
		return 8; /* 10 bits in the low bits */
	case VIDEO_FORMAT_P010:
	case VIDEO_FORMAT_P216:
	case VIDEO_FORMAT_P416:
		return 512; /* 16-bit samples */
	default:
		return 2;
	}
}

static struct obs_video_info make_ovi(enum video_format format, enum video_colorspace cs, enum video_range_type range,
				      uint32_t base_cx, uint32_t base_cy, uint32_t out_cx, uint32_t out_cy)
{
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 60,
		.fps_den = 1,
		.base_width = base_cx,
		.base_height = base_cy,
		.output_width = out_cx,
		.output_height = out_cy,
		.output_format = format,
		.colorspace = cs,
		.range = range,
		.gpu_conversion = true,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	return ovi;
}

static void take_frame(struct probe *p, enum video_format format, uint32_t width, uint32_t height)
{
	memset(p, 0, sizeof(*p));
	p->width = width;
	p->height = height;
	layout_of(format, p->planes);
	assert_int_equal(os_event_init(&p->kept, OS_EVENT_TYPE_MANUAL), 0);
	obs_add_raw_video_callback(NULL, probe_frame, p);
	int waited = os_event_timedwait(p->kept, FRAME_TIMEOUT_MS);
	obs_remove_raw_video_callback(probe_frame, p);
	os_event_destroy(p->kept);
	assert_int_equal(waited, 0);
}

static void free_frame(struct probe *p)
{
	for (size_t c = 0; c < 3; c++)
		bfree(p->data[c]);
}

static const char *record;

static void check_case(const struct format_case *fc)
{
	struct obs_video_info ovi = make_ovi(fc->format, fc->colorspace, fc->range, WIDTH, HEIGHT, WIDTH, HEIGHT);
	box_cx = WIDTH;
	box_cy = HEIGHT;
	assert_int_equal(obs_reset_video(&ovi), OBS_VIDEO_SUCCESS);

	struct obs_video_info got;
	assert_true(obs_get_video_info(&got));
	assert_int_equal(got.output_format, fc->format);
	assert_int_equal(got.colorspace, fc->colorspace);
	assert_int_equal(got.range, fc->range);
	assert_int_equal(got.output_width, WIDTH);
	assert_int_equal(got.output_height, HEIGHT);

	struct probe p;
	take_frame(&p, fc->format, WIDTH, HEIGHT);
	assert_false(p.extra_plane);

	uint32_t found[3][4] = {{0}};
	for (size_t c = 0; c < 3; c++) {
		const struct plane_layout *l = &p.planes[c];
		if (!l->wdiv)
			continue;
		assert_non_null(p.data[c]);
		uint32_t pw = WIDTH / l->wdiv;
		uint32_t ph = HEIGHT / l->hdiv;
		assert_true(p.linesize[c] >= pw * l->comps * l->bytes);
		for (uint32_t k = 0; k < l->comps; k++) {
			found[c][k] = sample(&p, c, 0, 0, k);
			/* the plane is uniform over its whole size */
			for (uint32_t y = 0; y < ph; y++) {
				for (uint32_t x = 0; x < pw; x++) {
					uint32_t s = sample(&p, c, x, y, k);
					uint32_t d = s > found[c][k] ? s - found[c][k] : found[c][k] - s;
					if (d > 1)
						fail_msg("format %d plane %zu sample %u at %u,%u is %u, at 0,0 %u",
							 (int)fc->format, c, k, x, y, s, found[c][k]);
				}
			}
		}
	}
	free_frame(&p);

	if (record) {
		printf("\t{VIDEO_FORMAT_%s, %d, %d, {{%u, %u, %u, %u}, {%u, %u, %u, %u}, {%u, %u, %u, %u}}},\n",
		       get_video_format_name(fc->format), (int)fc->colorspace, (int)fc->range, found[0][0], found[0][1],
		       found[0][2], found[0][3], found[1][0], found[1][1], found[1][2], found[1][3], found[2][0],
		       found[2][1], found[2][2], found[2][3]);
		return;
	}

	uint32_t tol = tolerance(fc->format);
	for (size_t c = 0; c < 3; c++) {
		for (size_t k = 0; k < 4; k++) {
			uint32_t want = fc->want[c][k];
			uint32_t d = found[c][k] > want ? found[c][k] - want : want - found[c][k];
			if (d > tol)
				fail_msg("format %s colorspace %d range %d: plane %zu sample %zu is %u, want %u",
					 get_video_format_name(fc->format), (int)fc->colorspace, (int)fc->range, c, k,
					 found[c][k], want);
		}
	}
}

static void every_format(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++)
		check_case(&cases[i]);
}

/* obs_reset_video() aligns the output width down to a multiple of 4 and
 * the height to a multiple of 2, and the frames have the aligned size. The
 * base size is not aligned. */
static void output_size_is_aligned(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	box_cx = 70;
	box_cy = 35;
	struct obs_video_info ovi = make_ovi(VIDEO_FORMAT_I420, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, 70, 35, 70, 35);
	assert_int_equal(obs_reset_video(&ovi), OBS_VIDEO_SUCCESS);
	/* the caller's struct is aligned too */
	assert_int_equal(ovi.output_width, 68);
	assert_int_equal(ovi.output_height, 34);

	struct obs_video_info got;
	assert_true(obs_get_video_info(&got));
	assert_int_equal(got.base_width, 70);
	assert_int_equal(got.base_height, 35);
	assert_int_equal(got.output_width, 68);
	assert_int_equal(got.output_height, 34);

	/* chroma planes of 34x17 */
	struct probe p;
	take_frame(&p, VIDEO_FORMAT_I420, 68, 34);
	for (size_t c = 0; c < 3; c++)
		assert_non_null(p.data[c]);
	assert_true(p.linesize[1] >= 34);
	free_frame(&p);

	/* the smallest output that stays: 4x2, with chroma planes of 2x1 */
	box_cx = WIDTH;
	box_cy = HEIGHT;
	ovi = make_ovi(VIDEO_FORMAT_I420, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, WIDTH, HEIGHT, 5, 3);
	assert_int_equal(obs_reset_video(&ovi), OBS_VIDEO_SUCCESS);
	assert_true(obs_get_video_info(&got));
	assert_int_equal(got.output_width, 4);
	assert_int_equal(got.output_height, 2);
	take_frame(&p, VIDEO_FORMAT_I420, 4, 2);
	assert_non_null(p.data[1]);
	free_frame(&p);
}

/* Sizes outside 2..32768 are refused before anything is freed, and the
 * video keeps running. */
static void size_check_refuses_before_teardown(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct obs_video_info ok =
		make_ovi(VIDEO_FORMAT_NV12, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, WIDTH, HEIGHT, WIDTH, HEIGHT);
	assert_int_equal(obs_reset_video(&ok), OBS_VIDEO_SUCCESS);

	const uint32_t bad[][4] = {
		{WIDTH, HEIGHT, 1, HEIGHT},     {WIDTH, HEIGHT, WIDTH, 1},     {WIDTH, HEIGHT, 0, 0},
		{1, HEIGHT, WIDTH, HEIGHT},     {WIDTH, 1, WIDTH, HEIGHT},     {32769, HEIGHT, WIDTH, HEIGHT},
		{WIDTH, HEIGHT, 32769, HEIGHT}, {WIDTH, HEIGHT, WIDTH, 32769},
	};
	for (size_t i = 0; i < sizeof(bad) / sizeof(bad[0]); i++) {
		struct obs_video_info ovi = make_ovi(VIDEO_FORMAT_I420, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, bad[i][0],
						     bad[i][1], bad[i][2], bad[i][3]);
		assert_int_equal(obs_reset_video(&ovi), OBS_VIDEO_INVALID_PARAM);

		/* the old video is still there */
		struct obs_video_info got;
		assert_true(obs_get_video_info(&got));
		assert_int_equal(got.output_format, VIDEO_FORMAT_NV12);
		assert_int_equal(got.output_width, WIDTH);
	}

	struct probe p;
	take_frame(&p, VIDEO_FORMAT_NV12, WIDTH, HEIGHT);
	free_frame(&p);
}

/* An output width of 2 or 3 passes the 2..32768 size check, but the
 * alignment to a multiple of 4 makes it 0. It is refused before anything is
 * freed: the old video keeps running, and a later reset works. */
static void small_output_width_refused_before_teardown(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct obs_video_info ok =
		make_ovi(VIDEO_FORMAT_NV12, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, WIDTH, HEIGHT, WIDTH, HEIGHT);
	assert_int_equal(obs_reset_video(&ok), OBS_VIDEO_SUCCESS);

	const uint32_t bad[][2] = {{2, HEIGHT}, {3, HEIGHT}, {2, 2}, {3, 3}};
	for (size_t i = 0; i < sizeof(bad) / sizeof(bad[0]); i++) {
		struct obs_video_info ovi = make_ovi(VIDEO_FORMAT_I420, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, WIDTH,
						     HEIGHT, bad[i][0], bad[i][1]);
		assert_int_equal(obs_reset_video(&ovi), OBS_VIDEO_INVALID_PARAM);
		/* refused as given, not aligned */
		assert_int_equal(ovi.output_width, bad[i][0]);

		/* the old video is still there and running */
		assert_non_null(obs_get_video());
		struct obs_video_info got;
		assert_true(obs_get_video_info(&got));
		assert_int_equal(got.output_format, VIDEO_FORMAT_NV12);
		assert_int_equal(got.output_width, WIDTH);
		assert_int_equal(got.output_height, HEIGHT);
		struct probe p;
		take_frame(&p, VIDEO_FORMAT_NV12, WIDTH, HEIGHT);
		free_frame(&p);
	}

	/* a later reset to a valid size works */
	struct obs_video_info later =
		make_ovi(VIDEO_FORMAT_I420, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, WIDTH, HEIGHT, WIDTH, HEIGHT);
	assert_int_equal(obs_reset_video(&later), OBS_VIDEO_SUCCESS);
	struct probe p;
	take_frame(&p, VIDEO_FORMAT_I420, WIDTH, HEIGHT);
	for (size_t c = 0; c < 3; c++)
		assert_non_null(p.data[c]);
	free_frame(&p);

	/* back to the video the other cases expect */
	assert_int_equal(obs_reset_video(&ok), OBS_VIDEO_SUCCESS);
}

/* ------------------------------------------------------------------------- */

static obs_scene_t *scene;
static obs_source_t *box;

static int group_setup(void **state)
{
	(void)state;
	record = getenv("VIDEO_OUTPUT_FORMATS_RECORD");
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi =
		make_ovi(VIDEO_FORMAT_NV12, VIDEO_CS_709, VIDEO_RANGE_PARTIAL, WIDTH, HEIGHT, WIDTH, HEIGHT);
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	register_box();
	scene = obs_scene_create("scene");
	box = obs_source_create(BOX_ID, "box", NULL, NULL);
	obs_scene_add(scene, box);
	obs_set_output_source(0, obs_scene_get_source(scene));
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (!obs_up)
		return 0;
	obs_set_output_source(0, NULL);
	obs_source_release(box);
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(every_format),
		cmocka_unit_test(output_size_is_aligned),
		cmocka_unit_test(size_check_refuses_before_teardown),
		cmocka_unit_test(small_output_width_refused_before_teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
