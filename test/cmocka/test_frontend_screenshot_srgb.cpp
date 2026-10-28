/* The frontend's "Screenshot (Source)" (source context menu, File menu,
 * hotkey, obs_frontend_take_source_screenshot()) and "Screenshot (Output)"
 * draw with RenderScreenshot() (frontend/utility/screenshot-render.cpp) into
 * the texture ScreenshotObj saves as a PNG. On Linux and macOS that target is
 * 8-bit sRGB (GS_CS_SRGB, GS_RGBA). libobs converts high-bit-depth async
 * frames (P010, I010, I210: HEVC Main10, VP9 profile 2, ProRes, 10-bit capture
 * cards) into a linear 16-bit float texture, so the screenshot has to encode
 * them to sRGB on the way into the 8-bit target, as scenes and the previews
 * do. Without it the image is stored linear and comes out dark: orange
 * (255, 96, 0) as (255, 30, 0), mid grey 128 as 55.
 *
 * A test async source outputs one flat colour, full range BT.709, as P010,
 * I010, I210 and, as 8-bit controls, NV12 and BGRA with alpha. The source is
 * the only item of the program scene (BGRA output). The source screenshot's
 * centre pixel must equal the expected 8-bit sRGB colour and the program
 * output's pixel. The BGRA case checks that a semi-transparent 8-bit source
 * keeps its straight (not premultiplied) colour and its alpha, and the output
 * case that "Screenshot (Output)" of a 10-bit program still matches it.
 *
 * Needs core video, so it runs under xvfb-run and skips without a display. */

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#include "screenshot-render.hpp"

#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

/* cmocka.h last: its fail() and inline macros break C++ standard headers. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

#define W 64
#define H 64
#define TOLERANCE 3

static bool obs_up;

struct rgba {
	int r, g, b, a;
};

static pthread_mutex_t program_mutex = PTHREAD_MUTEX_INITIALIZER;
static rgba program_pixel;
static uint64_t program_frames;

static void raw_video(void *, struct video_data *frame)
{
	const uint8_t *p = frame->data[0] + (H / 2) * frame->linesize[0] + (W / 2) * 4;
	pthread_mutex_lock(&program_mutex);
	program_pixel = {p[2], p[1], p[0], p[3]};
	program_frames++;
	pthread_mutex_unlock(&program_mutex);
}

static const char *flat_name(void *)
{
	return "flat colour test source";
}

static void *flat_create(obs_data_t *, obs_source_t *source)
{
	return source;
}

static void flat_destroy(void *) {}

/* Full-range BT.709 Y'CbCr codes of an 8-bit sRGB colour at the given depth. */
struct yuv {
	uint16_t y, u, v;
};

static yuv to_yuv(rgba c, int bits)
{
	const double kr = 0.2126, kb = 0.0722;
	const double r = c.r / 255.0, g = c.g / 255.0, b = c.b / 255.0;
	const double y = kr * r + (1.0 - kr - kb) * g + kb * b;
	const double cb = (b - y) / (2.0 * (1.0 - kb));
	const double cr = (r - y) / (2.0 * (1.0 - kr));
	const double max = (double)((1 << bits) - 1);
	const double mid = (double)(1 << (bits - 1));
	return {(uint16_t)lround(y * max), (uint16_t)lround(cb * max + mid), (uint16_t)lround(cr * max + mid)};
}

/* Pixel storage for one frame of a flat colour in the given format. */
struct flat_frame {
	std::vector<uint8_t> planes[3];
	struct obs_source_frame frame = {};

	flat_frame(enum video_format format, rgba c)
	{
		frame.format = format;
		frame.width = W;
		frame.height = H;
		frame.full_range = true;
		video_format_get_parameters_for_format(VIDEO_CS_709, VIDEO_RANGE_FULL, format, frame.color_matrix,
						       frame.color_range_min, frame.color_range_max);

		switch (format) {
		case VIDEO_FORMAT_BGRA: {
			plane(0, W * 4, H);
			for (size_t i = 0; i < planes[0].size(); i += 4) {
				planes[0][i + 0] = (uint8_t)c.b;
				planes[0][i + 1] = (uint8_t)c.g;
				planes[0][i + 2] = (uint8_t)c.r;
				planes[0][i + 3] = (uint8_t)c.a;
			}
			break;
		}
		case VIDEO_FORMAT_NV12: {
			const yuv q = to_yuv(c, 8);
			plane(0, W, H);
			plane(1, W, H / 2);
			memset(planes[0].data(), q.y, planes[0].size());
			for (size_t i = 0; i < planes[1].size(); i += 2) {
				planes[1][i + 0] = (uint8_t)q.u;
				planes[1][i + 1] = (uint8_t)q.v;
			}
			break;
		}
		case VIDEO_FORMAT_P010: {
			/* 16-bit little-endian samples, the 10 bits at the top */
			const yuv q = to_yuv(c, 10);
			plane(0, W * 2, H);
			plane(1, W * 2, H / 2);
			fill16(planes[0], {(uint16_t)(q.y << 6)});
			fill16(planes[1], {(uint16_t)(q.u << 6), (uint16_t)(q.v << 6)});
			break;
		}
		case VIDEO_FORMAT_I010:
		case VIDEO_FORMAT_I210: {
			/* planar 16-bit little-endian samples, the 10 bits at the bottom */
			const yuv q = to_yuv(c, 10);
			const uint32_t chroma_rows = format == VIDEO_FORMAT_I010 ? H / 2 : H;
			plane(0, W * 2, H);
			plane(1, W, chroma_rows);
			plane(2, W, chroma_rows);
			fill16(planes[0], {q.y});
			fill16(planes[1], {q.u});
			fill16(planes[2], {q.v});
			break;
		}
		default:
			break;
		}
	}

	void plane(int i, uint32_t linesize, uint32_t rows)
	{
		planes[i].assign((size_t)linesize * rows, 0);
		frame.data[i] = planes[i].data();
		frame.linesize[i] = linesize;
	}

	static void fill16(std::vector<uint8_t> &p, std::vector<uint16_t> pattern)
	{
		for (size_t i = 0; i < p.size() / 2; i++) {
			const uint16_t v = pattern[i % pattern.size()];
			p[i * 2 + 0] = (uint8_t)(v & 0xff);
			p[i * 2 + 1] = (uint8_t)(v >> 8);
		}
	}
};

static int setup(void **)
{
	if (!obs_startup("en-US", nullptr, nullptr)) {
		fprintf(stderr, "obs_startup failed (no display?); skipping\n");
		return 0;
	}

	struct obs_video_info ovi = {};
	ovi.graphics_module = GRAPHICS_MODULE_PATH;
	ovi.fps_num = 30;
	ovi.fps_den = 1;
	ovi.base_width = W;
	ovi.base_height = H;
	ovi.output_width = W;
	ovi.output_height = H;
	ovi.output_format = VIDEO_FORMAT_BGRA;
	ovi.colorspace = VIDEO_CS_SRGB;
	ovi.range = VIDEO_RANGE_FULL;
	ovi.scale_type = OBS_SCALE_POINT;
	const int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: %d; skipping\n", ret);
		obs_shutdown();
		return 0;
	}

	struct obs_source_info info = {};
	info.id = "flat_colour_test_source";
	info.type = OBS_SOURCE_TYPE_INPUT;
	info.output_flags = OBS_SOURCE_ASYNC_VIDEO;
	info.get_name = flat_name;
	info.create = flat_create;
	info.destroy = flat_destroy;
	obs_register_source(&info);

	obs_add_raw_video_callback(nullptr, raw_video, nullptr);
	obs_up = true;
	return 0;
}

static int teardown(void **)
{
	if (obs_up) {
		obs_remove_raw_video_callback(raw_video, nullptr);
		obs_shutdown();
	}
	return 0;
}

static uint64_t frames_seen(void)
{
	pthread_mutex_lock(&program_mutex);
	const uint64_t n = program_frames;
	pthread_mutex_unlock(&program_mutex);
	return n;
}

/* Takes a screenshot the way ScreenshotObj does on Linux and macOS (8-bit
 * sRGB target, the source's own size) and returns its centre pixel. A null
 * source is "Screenshot (Output)". */
static rgba take_screenshot(obs_source_t *source)
{
	const enum gs_color_space space = GS_CS_SRGB;
	const enum gs_color_format format = gs_get_format_from_space(space);
	rgba pixel = {-1, -1, -1, -1};

	obs_enter_graphics();
	gs_texrender_t *texrender = gs_texrender_create(format, GS_ZS_NONE);
	gs_stagesurf_t *stagesurf = gs_stagesurface_create(W, H, format);

	RenderScreenshot(texrender, source, W, H, W, H, space);

	gs_stage_texture(stagesurf, gs_texrender_get_texture(texrender));
	uint8_t *data = nullptr;
	uint32_t linesize = 0;
	if (gs_stagesurface_map(stagesurf, &data, &linesize)) {
		const uint8_t *p = data + (H / 2) * linesize + (W / 2) * 4;
		pixel = {p[0], p[1], p[2], p[3]};
		gs_stagesurface_unmap(stagesurf);
	}

	gs_stagesurface_destroy(stagesurf);
	gs_texrender_destroy(texrender);
	obs_leave_graphics();
	return pixel;
}

/* The program reads black until the scene has drawn once: frames_seen() counts
 * raw-video deliveries, which begin as soon as obs_set_output_source() is set,
 * so on a cold runner (llvmpipe still compiling the scene's shaders) the first
 * frames are black. Wait for the program to paint instead of for the eighth raw
 * frame, and bound the wait so a program that never renders fails the case with
 * a reason other than a black-pixel mismatch. None of these cases expects
 * black, so "not black" is what says the scene has drawn. */
#define WAIT_FOR_PROGRAM_NS (10 * 1000000000ULL)

static bool wait_for_painted_program(obs_source_t *source, flat_frame *f, uint64_t start)
{
	const uint64_t deadline = os_gettime_ns() + WAIT_FOR_PROGRAM_NS;
	while (os_gettime_ns() < deadline) {
		f->frame.timestamp = os_gettime_ns();
		obs_source_output_video(source, &f->frame);
		os_sleep_ms(20);

		pthread_mutex_lock(&program_mutex);
		const rgba pixel = program_pixel;
		const uint64_t frames = program_frames;
		pthread_mutex_unlock(&program_mutex);

		if (frames > start && (pixel.r || pixel.g || pixel.b)) {
			return true;
		}
	}

	return false;
}

/* Shows a flat colour in the given format as the program, takes the source
 * screenshot (or the output screenshot) and returns its centre pixel and the
 * program's. */
static void capture(enum video_format format, rgba colour, bool output, rgba *shot, rgba *program)
{
	obs_source_t *source = obs_source_create("flat_colour_test_source", "flat", nullptr, nullptr);
	obs_source_set_async_unbuffered(source, true);
	/* The program is a scene, as in OBS: scenes draw their items with linear
	 * sRGB on, so the program shows the right colours. */
	obs_scene_t *scene = obs_scene_create("program");
	obs_scene_add(scene, source);
	obs_set_output_source(0, obs_scene_get_source(scene));

	flat_frame f(format, colour);
	/* Keep sending frames until the program has painted one of them. */
	if (!wait_for_painted_program(source, &f, frames_seen())) {
		fail_msg("the program did not paint a %d frame within %d s (%llu raw frames seen)", (int)format,
			 (int)(WAIT_FOR_PROGRAM_NS / 1000000000ULL), (unsigned long long)frames_seen());
	}

	*shot = take_screenshot(output ? nullptr : source);
	assert_true(shot->r >= 0);

	pthread_mutex_lock(&program_mutex);
	*program = program_pixel;
	pthread_mutex_unlock(&program_mutex);

	obs_set_output_source(0, nullptr);
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_source_release(source);
	os_sleep_ms(50);
}

static bool close_to(rgba a, rgba b, bool with_alpha)
{
	return abs(a.r - b.r) <= TOLERANCE && abs(a.g - b.g) <= TOLERANCE && abs(a.b - b.b) <= TOLERANCE &&
	       (!with_alpha || abs(a.a - b.a) <= TOLERANCE);
}

static void check(const char *what, enum video_format format, rgba colour, bool output)
{
	if (!obs_up) {
		skip();
	}

	rgba shot, program;
	capture(format, colour, output, &shot, &program);
	printf("%s: want (%d, %d, %d), screenshot (%d, %d, %d, %d), program (%d, %d, %d)\n", what, colour.r, colour.g,
	       colour.b, shot.r, shot.g, shot.b, shot.a, program.r, program.g, program.b);

	if (!close_to(shot, colour, true) || !close_to(shot, program, false)) {
		fail_msg(
			"%s: screenshot (%d, %d, %d, %d), want (%d, %d, %d, %d) and the program (%d, %d, %d), within %d",
			what, shot.r, shot.g, shot.b, shot.a, colour.r, colour.g, colour.b, colour.a, program.r,
			program.g, program.b, TOLERANCE);
	}
}

static const rgba orange = {255, 96, 0, 255};
static const rgba grey = {128, 128, 128, 255};

static void nv12_orange_is_unchanged(void **)
{
	check("NV12 orange", VIDEO_FORMAT_NV12, orange, false);
}

static void bgra_with_alpha_keeps_straight_colour(void **)
{
	if (!obs_up) {
		skip();
	}

	/* Half transparent: the screenshot keeps the colour and alpha as sent
	 * (the program shows it blended over black, so it is not compared). */
	const rgba colour = {200, 100, 50, 128};
	rgba shot, program;
	capture(VIDEO_FORMAT_BGRA, colour, false, &shot, &program);
	printf("BGRA half transparent: want (200, 100, 50, 128), screenshot (%d, %d, %d, %d)\n", shot.r, shot.g, shot.b,
	       shot.a);
	if (!close_to(shot, colour, true)) {
		fail_msg("BGRA half transparent: screenshot (%d, %d, %d, %d), want (200, 100, 50, 128) within %d",
			 shot.r, shot.g, shot.b, shot.a, TOLERANCE);
	}
}

static void p010_grey_is_srgb(void **)
{
	check("P010 grey", VIDEO_FORMAT_P010, grey, false);
}

static void p010_orange_is_srgb(void **)
{
	check("P010 orange", VIDEO_FORMAT_P010, orange, false);
}

static void i010_orange_is_srgb(void **)
{
	check("I010 orange", VIDEO_FORMAT_I010, orange, false);
}

static void i210_orange_is_srgb(void **)
{
	check("I210 orange", VIDEO_FORMAT_I210, orange, false);
}

static void output_of_p010_program_is_unchanged(void **)
{
	check("Output, P010 orange program", VIDEO_FORMAT_P010, orange, true);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(nv12_orange_is_unchanged),
		cmocka_unit_test(bgra_with_alpha_keeps_straight_colour),
		cmocka_unit_test(p010_grey_is_srgb),
		cmocka_unit_test(p010_orange_is_srgb),
		cmocka_unit_test(i010_orange_is_srgb),
		cmocka_unit_test(i210_orange_is_srgb),
		cmocka_unit_test(output_of_p010_program_is_unchanged),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
