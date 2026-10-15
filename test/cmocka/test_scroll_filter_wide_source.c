/* A Scroll filter on a source wider or taller than the GPU's largest texture
 * (16384 pixels in Direct3D 11, Metal and most OpenGL drivers) showed
 * nothing at all: a long ticker of names or sponsors in a Text (FreeType 2)
 * source vanished as soon as the Scroll filter was added (OBS forum,
 * "long text file scrolling. Text disappears", 2020-2025;
 * obsproject/obs-studio#10034). The filter drew the whole source into one
 * texture of the source's size, which cannot be created at that size, and
 * then drew nothing.
 *
 * The cases run the real Scroll filter (the built obs-filters module) on a
 * test source that draws three bands along its length: red at the start,
 * green in the middle, blue at the end. The filter's width (or height) is
 * limited to a small window, which is read back. A source within the
 * texture limit is checked too, to show what the filter does there. They
 * need core video, so they run under xvfb-run and skip without a display. */

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

#define BANDS_ID "scroll_wide_bands"
#define WINDOW 64
#define THICK 32
#define BAND 100
#define NARROW 1000
#define WIDE 20000

enum colour { RED, GREEN, BLUE, CLEAR, OTHER };
static const char *colour_names[] = {"red", "green", "blue", "transparent", "something else"};

static bool obs_up;
static obs_module_t *filters;

struct bands {
	uint32_t cx, cy;
	bool vertical;
};

static const char *bands_name(void *type_data)
{
	(void)type_data;
	return "bands";
}

static void *bands_create(obs_data_t *settings, obs_source_t *source)
{
	(void)source;
	struct bands *b = bzalloc(sizeof(*b));
	b->vertical = obs_data_get_bool(settings, "vertical");
	uint32_t len = (uint32_t)obs_data_get_int(settings, "length");
	b->cx = b->vertical ? THICK : len;
	b->cy = b->vertical ? len : THICK;
	return b;
}

static void bands_destroy(void *data)
{
	bfree(data);
}

static uint32_t bands_width(void *data)
{
	return ((struct bands *)data)->cx;
}

static uint32_t bands_height(void *data)
{
	return ((struct bands *)data)->cy;
}

static void fill(gs_effect_t *solid, gs_eparam_t *color, float x, float y, float cx, float cy, uint32_t rgba)
{
	struct vec4 c;
	vec4_from_rgba(&c, rgba);
	gs_effect_set_vec4(color, &c);
	gs_matrix_push();
	gs_matrix_translate3f(x, y, 0.0f);
	while (gs_effect_loop(solid, "Solid"))
		gs_draw_sprite(NULL, 0, (uint32_t)cx, (uint32_t)cy);
	gs_matrix_pop();
}

static void bands_render(void *data, gs_effect_t *effect)
{
	(void)effect;
	struct bands *b = data;
	gs_effect_t *solid = obs_get_base_effect(OBS_EFFECT_SOLID);
	gs_eparam_t *color = gs_effect_get_param_by_name(solid, "color");
	float len = (float)(b->vertical ? b->cy : b->cx);
	float thick = (float)THICK;

	/* RGBA in memory order, as vec4_from_rgba reads it */
	const uint32_t red = 0xFF0000FF, green = 0xFF00FF00, blue = 0xFFFF0000;
	if (b->vertical) {
		fill(solid, color, 0, 0, thick, BAND, red);
		fill(solid, color, 0, BAND, thick, len - 2 * BAND, green);
		fill(solid, color, 0, len - BAND, thick, BAND, blue);
	} else {
		fill(solid, color, 0, 0, BAND, thick, red);
		fill(solid, color, BAND, 0, len - 2 * BAND, thick, green);
		fill(solid, color, len - BAND, 0, BAND, thick, blue);
	}
}

static int setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL))
		return 0;

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 640,
		.base_height = 360,
		.output_width = 640,
		.output_height = 360,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: skipping\n");
		return 0;
	}

	if (obs_open_module(&filters, FILTERS_MODULE_PATH, FILTERS_DATA_PATH) != MODULE_SUCCESS ||
	    !obs_init_module(filters)) {
		fprintf(stderr, "could not load obs-filters\n");
		return -1;
	}

	struct obs_source_info info = {
		.id = BANDS_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
		.get_name = bands_name,
		.create = bands_create,
		.destroy = bands_destroy,
		.get_width = bands_width,
		.get_height = bands_height,
		.video_render = bands_render,
	};
	obs_register_source(&info);
	obs_up = true;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

struct scroller {
	obs_source_t *source;
	obs_source_t *filter;
	uint32_t cx, cy;
};

static void make_scroller(struct scroller *s, uint32_t length, bool vertical, double speed, bool loop)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "length", length);
	obs_data_set_bool(settings, "vertical", vertical);
	s->source = obs_source_create(BANDS_ID, "bands", settings, NULL);
	obs_data_release(settings);
	assert_non_null(s->source);

	settings = obs_data_create();
	obs_data_set_double(settings, vertical ? "speed_y" : "speed_x", speed);
	obs_data_set_bool(settings, vertical ? "limit_cy" : "limit_cx", true);
	obs_data_set_int(settings, vertical ? "cy" : "cx", WINDOW);
	obs_data_set_bool(settings, "loop", loop);
	s->filter = obs_source_create_private("scroll_filter", "scroll", settings);
	obs_data_release(settings);
	assert_non_null(s->filter);
	obs_source_filter_add(s->source, s->filter);
	/* the filter's settings are applied on the next video tick */
	os_sleep_ms(200);

	s->cx = obs_source_get_width(s->source);
	s->cy = obs_source_get_height(s->source);
}

static void free_scroller(struct scroller *s)
{
	obs_source_filter_remove(s->source, s->filter);
	obs_source_release(s->filter);
	obs_source_release(s->source);
}

/* Renders the filtered source and returns the colour at `at` pixels along
 * its scrolling direction (across the middle of its thickness). */
static enum colour render_at(struct scroller *s, bool vertical, uint32_t at)
{
	enum colour got = OTHER;

	obs_enter_graphics();
	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_ZS_NONE);
	gs_stagesurf_t *stage = gs_stagesurface_create(s->cx, s->cy, GS_RGBA);

	if (gs_texrender_begin(tr, s->cx, s->cy)) {
		struct vec4 clear;
		vec4_zero(&clear);
		gs_clear(GS_CLEAR_COLOR, &clear, 0.0f, 0);
		gs_ortho(0.0f, (float)s->cx, 0.0f, (float)s->cy, -100.0f, 100.0f);
		gs_blend_state_push();
		gs_reset_blend_state();
		obs_source_video_render(s->source);
		gs_blend_state_pop();
		gs_texrender_end(tr);

		uint8_t *data;
		uint32_t linesize;
		gs_stage_texture(stage, gs_texrender_get_texture(tr));
		if (gs_stagesurface_map(stage, &data, &linesize)) {
			uint32_t x = vertical ? THICK / 2 : at;
			uint32_t y = vertical ? at : THICK / 2;
			const uint8_t *p = data + (size_t)y * linesize + (size_t)x * 4;
			if (p[3] < 16)
				got = CLEAR;
			else if (p[0] > 200 && p[1] < 50 && p[2] < 50)
				got = RED;
			else if (p[0] < 50 && p[1] > 200 && p[2] < 50)
				got = GREEN;
			else if (p[0] < 50 && p[1] < 50 && p[2] > 200)
				got = BLUE;
			gs_stagesurface_unmap(stage);
		}
	}

	gs_stagesurface_destroy(stage);
	gs_texrender_destroy(tr);
	obs_leave_graphics();
	return got;
}

static bool expect(struct scroller *s, bool vertical, uint32_t at, enum colour want, const char *what)
{
	enum colour got = render_at(s, vertical, at);
	if (got != want)
		fprintf(stderr, "%s: pixel %u of the %ux%u window is %s, expected %s\n", what, at, s->cx, s->cy,
			colour_names[got], colour_names[want]);
	return got == want;
}

/* The window at the start of the source shows its first band. */
static void check_start(uint32_t length, bool vertical, const char *what)
{
	if (!obs_up)
		skip();
	struct scroller s;
	make_scroller(&s, length, vertical, 0.0, true);
	bool ok = s.cx == (vertical ? THICK : WINDOW) && s.cy == (vertical ? WINDOW : THICK);
	if (!ok)
		fprintf(stderr, "%s: filter size %ux%u\n", what, s.cx, s.cy);
	ok = expect(&s, vertical, WINDOW / 2, RED, what) && ok;
	free_scroller(&s);
	assert_true(ok);
}

/* Scrolling backwards with Loop on: the window moves to the end of the
 * source at once and shows its last band, followed by its first band where
 * the window runs past the end. At 20 pixels a second the window starts
 * 20-60 pixels before the end after 1-3 s: pixel 8 is in the last band and
 * pixel 60 past the end. */
static void check_wrap(uint32_t length, bool vertical, const char *what)
{
	if (!obs_up)
		skip();
	struct scroller s;
	make_scroller(&s, length, vertical, -20.0, true);
	/* the filter learns the source's size when it first renders */
	(void)render_at(&s, vertical, 0);
	os_sleep_ms(1200);
	bool ok = expect(&s, vertical, 8, BLUE, what);
	ok = expect(&s, vertical, 60, RED, what) && ok;
	free_scroller(&s);
	assert_true(ok);
}

static void narrow_source_shows_start(void **state)
{
	(void)state;
	check_start(NARROW, false, "1000 px wide source");
}

static void narrow_source_wraps(void **state)
{
	(void)state;
	check_wrap(NARROW, false, "1000 px wide source, scrolling back");
}

static void wide_source_shows_start(void **state)
{
	(void)state;
	check_start(WIDE, false, "20000 px wide source");
}

static void wide_source_wraps(void **state)
{
	(void)state;
	check_wrap(WIDE, false, "20000 px wide source, scrolling back");
}

static void tall_source_shows_start(void **state)
{
	(void)state;
	check_start(WIDE, true, "20000 px tall source");
}

static void tall_source_wraps(void **state)
{
	(void)state;
	check_wrap(WIDE, true, "20000 px tall source, scrolling back");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(narrow_source_shows_start), cmocka_unit_test(narrow_source_wraps),
		cmocka_unit_test(wide_source_shows_start),   cmocka_unit_test(wide_source_wraps),
		cmocka_unit_test(tall_source_shows_start),   cmocka_unit_test(tall_source_wraps),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
