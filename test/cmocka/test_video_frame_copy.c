/* Characterization of the output frame copy of obs-video.c
 * (set_gpu_converted_plane(), set_gpu_converted_data() and
 * copy_rgbx_frame()) before it moves to its own file and is ported to
 * rust/obs-core. The copy plan it runs has its own tests
 * (test_video_copy_plan_steps and the video_copy_plan parity tests), and
 * test_video_output_formats checks the conversion on a uniform picture; no
 * test pinned the copy itself, the bytes a raw video callback gets for each
 * plane.
 *
 * The program shows a box of four colored quadrants over the whole canvas.
 * For each output format, with the GPU conversion on (the planar formats
 * run set_gpu_converted_data(); RGBA, BGRA and BGRX run copy_rgbx_frame(),
 * since they have no GPU conversion) and off (copy_rgbx_frame(), for the
 * 4-byte RGB formats it is written for), a raw video callback keeps the
 * first frame that shows the quadrants, and the test checks:
 * - each plane's line size as the callback sees it, and that no plane past
 *   the format's planes is set;
 * - each plane's samples: uniform in each quadrant (a row copied to the
 *   wrong place or with the wrong stride moves the quadrant edges), with
 *   the value pinned per quadrant (a plane copied from the wrong source
 *   gives another quadrant's or another plane's value).
 * Each format runs at 64x32 and at 68x34, where the half-height planes have
 * an odd number of rows. With OpenGL the staging surfaces report line sizes
 * of exactly the row's bytes, as video-io's frames have, so each plane is
 * one copy; the copy a row at a time runs where the line sizes differ
 * (Direct3D's padded pitches), which this Linux test does not reach. Nor does
 * it reach the UV plane that follows plane 0 in one buffer: OpenGL stages
 * every plane of an NV12 or P010 frame in its own surface.
 *
 * I420 is left out at 68x34. Its chroma rows are 34 bytes there, which the
 * OpenGL staging surface pads to 36 in its buffer while reporting a line
 * size of 34, so the one copy shears the chroma planes by 2 bytes a row
 * before the copy runs. That is the renderer's line size, not the copy.
 *
 * Characterized, not endorsed: an RGBA output gets the staging surface's
 * bytes as they are, B, G, R, A, the same as BGRA.
 *
 * It uses only the public API, so with the hybrid libobs it checks the Rust
 * port once there is one.
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
#include <graphics/matrix4.h>
#include <graphics/vec4.h>
#include <util/platform.h>
#include <util/threading.h>

#define BOX_ID "test_video_frame_copy_box"
/* how long to wait for a frame that shows the quadrants */
#define FRAME_TIMEOUT_MS 20000
/* samples this close (in pixels of the canvas) to a quadrant edge are not
 * checked: the chroma planes mix the two sides there */
#define EDGE_MARGIN 3

static bool obs_up;
/* the box's size: the test thread sets it, the graphics thread reads it */
static volatile long box_cx = 64;
static volatile long box_cy = 32;

/* ------------------------------------------------------------------------- */
/* a box of four quadrants: top left, top right, bottom left, bottom right */

static const float quadrant_colors[4][3] = {
	{0.875f, 0.125f, 0.125f},
	{0.125f, 0.75f, 0.25f},
	{0.25f, 0.25f, 0.875f},
	{0.625f, 0.625f, 0.5f},
};

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "quadrants";
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
	return (uint32_t)os_atomic_load_long(&box_cx);
}

static uint32_t box_height(void *data)
{
	(void)data;
	return (uint32_t)os_atomic_load_long(&box_cy);
}

static void box_render(void *data, gs_effect_t *effect)
{
	(void)data;
	(void)effect;
	gs_effect_t *solid = obs_get_base_effect(OBS_EFFECT_SOLID);
	gs_eparam_t *color_param = gs_effect_get_param_by_name(solid, "color");
	const uint32_t box_w = (uint32_t)os_atomic_load_long(&box_cx);
	const uint32_t box_h = (uint32_t)os_atomic_load_long(&box_cy);
	const uint32_t half_cx = box_w / 2;
	const uint32_t half_cy = box_h / 2;

	for (size_t q = 0; q < 4; q++) {
		const uint32_t x = (q & 1) ? half_cx : 0;
		const uint32_t y = (q & 2) ? half_cy : 0;
		const uint32_t cx = (q & 1) ? box_w - half_cx : half_cx;
		const uint32_t cy = (q & 2) ? box_h - half_cy : half_cy;
		struct vec4 color;
		vec4_set(&color, quadrant_colors[q][0], quadrant_colors[q][1], quadrant_colors[q][2], 1.0f);

		gs_matrix_push();
		gs_matrix_translate3f((float)x, (float)y, 0.0f);
		gs_effect_set_vec4(color_param, &color);
		while (gs_effect_loop(solid, "Solid"))
			gs_draw_sprite(NULL, 0, cx, cy);
		gs_matrix_pop();
	}
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
	case VIDEO_FORMAT_RGBA:
	case VIDEO_FORMAT_BGRA:
	case VIDEO_FORMAT_BGRX:
		planes[0] = (struct plane_layout){1, 1, 4, 1};
		break;
	default:
		break;
	}
}

/* ------------------------------------------------------------------------- */
/* a raw video callback that keeps the first frame showing the quadrants */

struct probe {
	uint32_t width;
	uint32_t height;
	struct plane_layout planes[3];
	uint8_t *data[3];
	uint32_t linesize[3];
	bool extra_plane; /* a plane pointer past the format's planes was set */
	long frames;
	bool done;
	os_event_t *kept;
};

static uint32_t sample_at(const uint8_t *data, uint32_t linesize, const struct plane_layout *l, uint32_t x, uint32_t y,
			  uint32_t comp)
{
	const uint8_t *at = data + (size_t)y * linesize + ((size_t)x * l->comps + comp) * l->bytes;
	if (l->bytes == 1)
		return at[0];
	return (uint32_t)at[0] | ((uint32_t)at[1] << 8);
}

/* the center of quadrant q, in the plane's samples */
static void quadrant_center(const struct probe *p, const struct plane_layout *l, size_t q, uint32_t *x, uint32_t *y)
{
	const uint32_t cx = (q & 1) ? p->width * 3 / 4 : p->width / 4;
	const uint32_t cy = (q & 2) ? p->height * 3 / 4 : p->height / 4;
	*x = cx / l->wdiv;
	*y = cy / l->hdiv;
}

/* The scene is drawn once plane 0's quadrant centers differ (the frames
 * before it are uniform black). */
static bool shows_quadrants(const struct probe *p, const struct video_data *frame)
{
	const struct plane_layout *l = &p->planes[0];
	if (!frame->data[0])
		return false;
	uint32_t values[4];
	for (size_t q = 0; q < 4; q++) {
		uint32_t x, y;
		quadrant_center(p, l, q, &x, &y);
		values[q] = sample_at(frame->data[0], frame->linesize[0], l, x, y, 0);
	}
	for (size_t a = 0; a < 4; a++)
		for (size_t b = a + 1; b < 4; b++)
			if (values[a] == values[b])
				return false;
	return true;
}

static void probe_frame(void *param, struct video_data *frame)
{
	struct probe *p = param;
	if (p->done)
		return;
	p->frames++;
	if (!shows_quadrants(p, frame))
		return;

	for (size_t c = 0; c < 3; c++) {
		const struct plane_layout *l = &p->planes[c];
		p->linesize[c] = frame->linesize[c];
		if (!l->wdiv) {
			if (frame->data[c])
				p->extra_plane = true;
			continue;
		}
		if (!frame->data[c])
			continue;
		uint32_t rows = p->height / l->hdiv;
		p->data[c] = bmemdup(frame->data[c], (size_t)frame->linesize[c] * rows);
	}
	if (frame->data[3])
		p->extra_plane = true;
	p->done = true;
	os_event_signal(p->kept);
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
	if (waited != 0) {
		fprintf(stderr, "format %s %ux%u: no frame showed the quadrants in %d ms (%ld frames)\n",
			get_video_format_name(format), width, height, FRAME_TIMEOUT_MS, p->frames);
		fail();
	}
}

static void free_frame(struct probe *p)
{
	for (size_t c = 0; c < 3; c++)
		bfree(p->data[c]);
}

/* ------------------------------------------------------------------------- */
/* the cases */

struct copy_case {
	enum video_format format;
	bool gpu_conversion;
	/* the line size of each plane, per pixel of the frame's width */
	uint32_t linesize_per_px[3];
	/* each quadrant's sample, per plane and sample of a pixel */
	uint32_t want[4][3][4];
};

/* Measured on the C libobs with llvmpipe (VIDEO_FRAME_COPY_RECORD=1 prints
 * this table); the tolerance below allows for other renderers' rounding. All
 * BT.709, partial range. The line sizes of half-width planes are half the
 * width (linesize_per_px 0 there: see want_linesize()). */
static const struct copy_case cases[] = {
	{VIDEO_FORMAT_I420,
	 true,
	 {1, 0, 0},
	 {
		 {{78, 0, 0, 0}, {109, 0, 0, 0}, {212, 0, 0, 0}},
		 {{143, 0, 0, 0}, {88, 0, 0, 0}, {63, 0, 0, 0}},
		 {{81, 0, 0, 0}, {198, 0, 0, 0}, {122, 0, 0, 0}},
		 {{151, 0, 0, 0}, {114, 0, 0, 0}, {129, 0, 0, 0}},
	 }},
	{VIDEO_FORMAT_NV12,
	 true,
	 {1, 1, 0},
	 {
		 {{78, 0, 0, 0}, {109, 212, 0, 0}, {0, 0, 0, 0}},
		 {{143, 0, 0, 0}, {88, 63, 0, 0}, {0, 0, 0, 0}},
		 {{81, 0, 0, 0}, {198, 122, 0, 0}, {0, 0, 0, 0}},
		 {{151, 0, 0, 0}, {114, 129, 0, 0}, {0, 0, 0, 0}},
	 }},
	{VIDEO_FORMAT_I444,
	 true,
	 {1, 1, 1},
	 {
		 {{78, 0, 0, 0}, {109, 0, 0, 0}, {212, 0, 0, 0}},
		 {{143, 0, 0, 0}, {88, 0, 0, 0}, {63, 0, 0, 0}},
		 {{81, 0, 0, 0}, {198, 0, 0, 0}, {122, 0, 0, 0}},
		 {{151, 0, 0, 0}, {114, 0, 0, 0}, {129, 0, 0, 0}},
	 }},
	{VIDEO_FORMAT_I010,
	 true,
	 {2, 1, 1},
	 {
		 {{508, 0, 0, 0}, {455, 0, 0, 0}, {760, 0, 0, 0}},
		 {{722, 0, 0, 0}, {409, 0, 0, 0}, {306, 0, 0, 0}},
		 {{560, 0, 0, 0}, {694, 0, 0, 0}, {495, 0, 0, 0}},
		 {{771, 0, 0, 0}, {477, 0, 0, 0}, {515, 0, 0, 0}},
	 }},
	{VIDEO_FORMAT_P010,
	 true,
	 {2, 2, 0},
	 {
		 {{32512, 0, 0, 0}, {29120, 48640, 0, 0}, {0, 0, 0, 0}},
		 {{46208, 0, 0, 0}, {26176, 19584, 0, 0}, {0, 0, 0, 0}},
		 {{35840, 0, 0, 0}, {44416, 31680, 0, 0}, {0, 0, 0, 0}},
		 {{49344, 0, 0, 0}, {30528, 32960, 0, 0}, {0, 0, 0, 0}},
	 }},
	{VIDEO_FORMAT_P216,
	 true,
	 {2, 2, 0},
	 {
		 {{32488, 0, 0, 0}, {29126, 48662, 0, 0}, {0, 0, 0, 0}},
		 {{46220, 0, 0, 0}, {26147, 19558, 0, 0}, {0, 0, 0, 0}},
		 {{35851, 0, 0, 0}, {44403, 31701, 0, 0}, {0, 0, 0, 0}},
		 {{49329, 0, 0, 0}, {30560, 32970, 0, 0}, {0, 0, 0, 0}},
	 }},
	{VIDEO_FORMAT_P416,
	 true,
	 {2, 4, 0},
	 {
		 {{32488, 0, 0, 0}, {29126, 48662, 0, 0}, {0, 0, 0, 0}},
		 {{46220, 0, 0, 0}, {26147, 19558, 0, 0}, {0, 0, 0, 0}},
		 {{35851, 0, 0, 0}, {44403, 31701, 0, 0}, {0, 0, 0, 0}},
		 {{49329, 0, 0, 0}, {30560, 32970, 0, 0}, {0, 0, 0, 0}},
	 }},
	{VIDEO_FORMAT_RGBA,
	 true,
	 {4, 0, 0},
	 {
		 {{32, 32, 223, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{64, 191, 32, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{223, 64, 64, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{128, 159, 159, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
	 }},
	{VIDEO_FORMAT_BGRA,
	 true,
	 {4, 0, 0},
	 {
		 {{32, 32, 223, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{64, 191, 32, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{223, 64, 64, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{128, 159, 159, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
	 }},
	{VIDEO_FORMAT_BGRX,
	 true,
	 {4, 0, 0},
	 {
		 {{32, 32, 223, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{64, 191, 32, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{223, 64, 64, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{128, 159, 159, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
	 }},
	{VIDEO_FORMAT_RGBA,
	 false,
	 {4, 0, 0},
	 {
		 {{32, 32, 223, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{64, 191, 32, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{223, 64, 64, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{128, 159, 159, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
	 }},
	{VIDEO_FORMAT_BGRA,
	 false,
	 {4, 0, 0},
	 {
		 {{32, 32, 223, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{64, 191, 32, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{223, 64, 64, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{128, 159, 159, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
	 }},
	{VIDEO_FORMAT_BGRX,
	 false,
	 {4, 0, 0},
	 {
		 {{32, 32, 223, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{64, 191, 32, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{223, 64, 64, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
		 {{128, 159, 159, 255}, {0, 0, 0, 0}, {0, 0, 0, 0}},
	 }},
};

static uint32_t want_linesize(const struct copy_case *cc, size_t c, uint32_t width)
{
	if (cc->linesize_per_px[c])
		return cc->linesize_per_px[c] * width;
	/* the half-width chroma planes of I420 */
	return width / 2;
}

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

static const char *record;

static bool near_edge(const struct probe *p, const struct plane_layout *l, uint32_t x, uint32_t y)
{
	const long px = (long)x * l->wdiv;
	const long py = (long)y * l->hdiv;
	const long ex = (long)(p->width / 2);
	const long ey = (long)(p->height / 2);
	return labs(px - ex) < EDGE_MARGIN || labs(py - ey) < EDGE_MARGIN;
}

static size_t quadrant_of(const struct probe *p, const struct plane_layout *l, uint32_t x, uint32_t y)
{
	const size_t right = (size_t)x * l->wdiv >= p->width / 2;
	const size_t bottom = (size_t)y * l->hdiv >= p->height / 2;
	return right | (bottom << 1);
}

static void check_case(const struct copy_case *cc, uint32_t width, uint32_t height)
{
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 60,
		.fps_den = 1,
		.base_width = width,
		.base_height = height,
		.output_width = width,
		.output_height = height,
		.output_format = cc->format,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.gpu_conversion = cc->gpu_conversion,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	/* before the reset, so every frame after it draws the new size */
	os_atomic_set_long(&box_cx, (long)width);
	os_atomic_set_long(&box_cy, (long)height);
	assert_int_equal(obs_reset_video(&ovi), OBS_VIDEO_SUCCESS);

	struct obs_video_info got;
	assert_true(obs_get_video_info(&got));
	assert_int_equal(got.output_format, cc->format);
	assert_int_equal(got.output_width, width);
	assert_int_equal(got.output_height, height);

	struct probe p;
	take_frame(&p, cc->format, width, height);
	if (p.extra_plane) {
		fprintf(stderr, "format %s: a plane past the format's planes is set\n",
			get_video_format_name(cc->format));
		fail();
	}

	uint32_t found[4][3][4] = {{{0}}};
	for (size_t c = 0; c < 3; c++) {
		const struct plane_layout *l = &p.planes[c];
		if (!l->wdiv)
			continue;
		assert_non_null(p.data[c]);
		if (!record && p.linesize[c] != want_linesize(cc, c, width)) {
			fprintf(stderr, "format %s gpu %d %ux%u: plane %zu line size %u, want %u\n",
				get_video_format_name(cc->format), (int)cc->gpu_conversion, width, height, c,
				p.linesize[c], want_linesize(cc, c, width));
			fail();
		}

		for (size_t q = 0; q < 4; q++) {
			uint32_t x, y;
			quadrant_center(&p, l, q, &x, &y);
			for (uint32_t k = 0; k < l->comps; k++)
				found[q][c][k] = sample_at(p.data[c], p.linesize[c], l, x, y, k);
		}

		/* each quadrant is uniform over its whole size in the plane */
		const uint32_t pw = width / l->wdiv;
		const uint32_t ph = height / l->hdiv;
		for (uint32_t y = 0; y < ph; y++) {
			for (uint32_t x = 0; x < pw; x++) {
				if (near_edge(&p, l, x, y))
					continue;
				const size_t q = quadrant_of(&p, l, x, y);
				for (uint32_t k = 0; k < l->comps; k++) {
					const uint32_t s = sample_at(p.data[c], p.linesize[c], l, x, y, k);
					const uint32_t f = found[q][c][k];
					const uint32_t d = s > f ? s - f : f - s;
					if (d > 1) {
						fprintf(stderr,
							"format %s gpu %d %ux%u: plane %zu sample %u at %u,%u is %u, "
							"quadrant %zu's center %u\n",
							get_video_format_name(cc->format), (int)cc->gpu_conversion,
							width, height, c, k, x, y, s, q, f);
						fail();
					}
				}
			}
		}
	}
	const uint32_t linesizes[3] = {p.linesize[0], p.linesize[1], p.linesize[2]};
	free_frame(&p);

	if (record) {
		printf("\t{VIDEO_FORMAT_%s, %s, {%u, %u, %u}, {\n", get_video_format_name(cc->format),
		       cc->gpu_conversion ? "true" : "false", linesizes[0] / width, linesizes[1] / width,
		       linesizes[2] / width);
		for (size_t q = 0; q < 4; q++)
			printf("\t\t{{%u, %u, %u, %u}, {%u, %u, %u, %u}, {%u, %u, %u, %u}},\n", found[q][0][0],
			       found[q][0][1], found[q][0][2], found[q][0][3], found[q][1][0], found[q][1][1],
			       found[q][1][2], found[q][1][3], found[q][2][0], found[q][2][1], found[q][2][2],
			       found[q][2][3]);
		printf("\t}},\n");
		return;
	}

	const uint32_t tol = tolerance(cc->format);
	for (size_t q = 0; q < 4; q++) {
		for (size_t c = 0; c < 3; c++) {
			for (size_t k = 0; k < 4; k++) {
				const uint32_t want = cc->want[q][c][k];
				const uint32_t f = found[q][c][k];
				const uint32_t d = f > want ? f - want : want - f;
				if (d > tol) {
					fprintf(stderr,
						"format %s gpu %d %ux%u: quadrant %zu plane %zu sample %zu is %u, "
						"want %u\n",
						get_video_format_name(cc->format), (int)cc->gpu_conversion, width,
						height, q, c, k, f, want);
					fail();
				}
			}
		}
	}
}

static void run_cases(uint32_t width, uint32_t height)
{
	if (!obs_up)
		skip();
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		/* the staging surface's line size (see the top) */
		if (cases[i].format == VIDEO_FORMAT_I420 && (width / 2) % 4 != 0)
			continue;
		check_case(&cases[i], width, height);
	}
}

static void frames_64x32(void **state)
{
	(void)state;
	run_cases(64, 32);
}

/* half-height planes of 17 rows */
static void frames_68x34(void **state)
{
	(void)state;
	run_cases(68, 34);
}

/* ------------------------------------------------------------------------- */

static obs_scene_t *scene;
static obs_source_t *box;

static int group_setup(void **state)
{
	(void)state;
	record = getenv("VIDEO_FRAME_COPY_RECORD");
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 60,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 32,
		.output_width = 64,
		.output_height = 32,
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

	register_box();
	scene = obs_scene_create("scene");
	box = obs_source_create(BOX_ID, "quadrants", NULL, NULL);
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
		cmocka_unit_test(frames_64x32),
		cmocka_unit_test(frames_68x34),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
