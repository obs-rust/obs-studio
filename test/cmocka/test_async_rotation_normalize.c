/* An async rotation that is a multiple of 90 but not 0, 90, -90, 180 or 270
 * (obs-rust/obs-studio#2121).
 *
 * obs_source_set_async_rotation() stored any long. get_async_width() and
 * get_async_height() swap the axes for a rotation that is not a multiple of
 * 180, but rotate_async_video() only moves the turned picture back into the
 * item's box for 90, 270, -90 and 180. So -180, -270, 450 or -450 turned the
 * picture around the item's corner and drew it outside the box, and a
 * multiple of 90 near the ends of long turned it by whatever angle the float
 * conversion left. The setter now stores the rotation in 0-359, so each of
 * these draws as its equivalent.
 *
 * Each case creates an async test source, sets its rotation, shows it alone
 * on channel 0 and pushes BGRA frames at the canvas rate until an output
 * frame, read back through a raw video callback, shows the rotated picture
 * exactly in the item's box at the top left (every pixel of each quadrant in
 * its quadrant's colour, everything outside black), for at most
 * PUSH_LIMIT_NS. The source's frame is W x H with four colours, so a missing
 * swap, a mirror or a wrong turn shows. obs_source_get_width() and
 * obs_source_get_height() must give the box's size.
 *
 * The source has no rotation getter and the rotation is not saved, so there
 * is no stored value to read back.
 *
 * Rotations that are not a multiple of 90 are not defined by this fix; they
 * are only checked for the axis swap they have always had (swapped unless
 * the rotation is a multiple of 180).
 *
 * Needs core video, so it runs (under xvfb-run) on Linux and the cases skip
 * without a display. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#define SOURCE_ID "async_rotation_normalize_source"

/* The source's frame: wider than tall, so the swap shows. */
#define W 32
#define H 16

/* The canvas: room for the box either way round, and for a picture drawn
 * next to it. */
#define CANVAS 64
#define CANVAS_FPS 30

#define TOLERANCE 8

/* A check pushes frames until an output frame shows the expected picture,
 * for at most this long. A source drawn in its box gets there in a few
 * frames; one drawn elsewhere never does. */
#define PUSH_LIMIT_NS (4ULL * 1000000000ULL)

/* Quadrant colours (B, G, R) of the source's frame: top left, top right,
 * bottom left, bottom right. */
static const uint8_t quadrant_bgr[4][3] = {
	{30, 30, 220},
	{30, 200, 30},
	{220, 40, 40},
	{200, 200, 200},
};

static bool obs_up;

struct watch {
	pthread_mutex_t mutex;
	bool recording;
	/* the rotation the picture should show, in 0-359 (a multiple of 90) */
	long want;
	size_t ticks;
	size_t good;
	/* the first wrong pixel of the last frame that was not good */
	int bad_x, bad_y;
	uint8_t bad_px[3];
	int bad_want; /* the quadrant expected there, or -1 for black */
};

static struct watch watch;

static bool close_to(int v, int want)
{
	return v >= want - TOLERANCE && v <= want + TOLERANCE;
}

/* The quadrant of the source's frame that output pixel (x, y) shows for
 * rotation `r` (0, 90, 180 or 270), or -1 when (x, y) is outside the box.
 * rotate_async_video() turns the frame about -z and moves it back into
 * the box: texel (u, v) lands at (u, v), (v, W - u), (W - u, H - v) or
 * (H - v, u). */
static int quadrant_at(long r, int x, int y)
{
	const int bw = (r % 180) ? H : W;
	const int bh = (r % 180) ? W : H;
	if (x >= bw || y >= bh)
		return -1;

	/* pixel centres, doubled to stay in integers */
	const int sx = 2 * x + 1, sy = 2 * y + 1;
	int u2, v2;
	switch (r) {
	case 0:
		u2 = sx;
		v2 = sy;
		break;
	case 90:
		u2 = 2 * W - sy;
		v2 = sx;
		break;
	case 180:
		u2 = 2 * W - sx;
		v2 = 2 * H - sy;
		break;
	default: /* 270 */
		u2 = sy;
		v2 = 2 * H - sx;
		break;
	}
	return (u2 < W ? 0 : 1) + (v2 < H ? 0 : 2);
}

static void raw_video(void *param, struct video_data *frame)
{
	struct watch *w = param;

	pthread_mutex_lock(&w->mutex);
	if (w->recording) {
		bool good = true;
		for (int y = 0; y < CANVAS && good; y++) {
			const uint8_t *row = frame->data[0] + y * frame->linesize[0];
			for (int x = 0; x < CANVAS; x++) {
				const uint8_t *px = row + x * 4;
				const int q = quadrant_at(w->want, x, y);
				bool ok;
				if (q < 0)
					ok = px[0] < TOLERANCE && px[1] < TOLERANCE && px[2] < TOLERANCE;
				else
					ok = close_to(px[0], quadrant_bgr[q][0]) &&
					     close_to(px[1], quadrant_bgr[q][1]) && close_to(px[2], quadrant_bgr[q][2]);
				if (!ok) {
					good = false;
					w->bad_x = x;
					w->bad_y = y;
					memcpy(w->bad_px, px, 3);
					w->bad_want = q;
					break;
				}
			}
		}
		w->ticks++;
		if (good)
			w->good++;
	}
	pthread_mutex_unlock(&w->mutex);
}

static const char *test_source_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "async rotation test source";
}

static void *test_source_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void test_source_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static struct obs_source_info test_source = {
	.id = SOURCE_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_ASYNC_VIDEO,
	.get_name = test_source_name,
	.create = test_source_create,
	.destroy = test_source_destroy,
};

static const uint8_t *frame_pixels(void)
{
	static uint8_t pixels[W * H * 4];
	for (int y = 0; y < H; y++) {
		for (int x = 0; x < W; x++) {
			const int q = (x < W / 2 ? 0 : 1) + (y < H / 2 ? 0 : 2);
			uint8_t *px = pixels + (y * W + x) * 4;
			px[0] = quadrant_bgr[q][0];
			px[1] = quadrant_bgr[q][1];
			px[2] = quadrant_bgr[q][2];
			px[3] = 255;
		}
	}
	return pixels;
}

/* The rotation in 0-359. */
static long normalized(long rotation)
{
	long r = rotation % 360;
	return r < 0 ? r + 360 : r;
}

struct shown {
	size_t ticks;
	size_t good;
	uint32_t width;
	uint32_t height;
};

/* Creates a source with `rotation`, shows it alone on channel 0 and pushes
 * frames at the canvas rate until an output frame shows the picture turned
 * by `want` in its box (when `want` >= 0) or, when `want` < 0, until the
 * source has a size, for at most PUSH_LIMIT_NS. */
static struct shown show(long rotation, long want)
{
	const uint8_t *pixels = frame_pixels();
	struct shown shown = {0};

	obs_source_t *source = obs_source_create(SOURCE_ID, "rotated", NULL, NULL);
	assert_non_null(source);
	obs_source_set_async_rotation(source, rotation);

	obs_scene_t *scene = obs_scene_create_private("async rotation scene");
	obs_scene_add(scene, source);
	obs_set_output_source(0, obs_scene_get_source(scene));

	pthread_mutex_lock(&watch.mutex);
	watch.want = want < 0 ? 0 : want;
	watch.ticks = 0;
	watch.good = 0;
	watch.bad_x = watch.bad_y = -1;
	watch.recording = want >= 0;
	pthread_mutex_unlock(&watch.mutex);

	const uint64_t period = 1000000000ULL / CANVAS_FPS;
	const uint64_t start = os_gettime_ns();
	for (uint64_t n = 0;; n++) {
		const uint64_t ts = start + n * period;
		if (ts - start > PUSH_LIMIT_NS)
			break;
		struct obs_source_frame frame = {
			.data = {(uint8_t *)pixels},
			.linesize = {W * 4},
			.width = W,
			.height = H,
			.format = VIDEO_FORMAT_BGRA,
			.full_range = true,
			.timestamp = ts,
		};
		os_sleepto_ns(ts);
		obs_source_output_video(source, &frame);

		bool done;
		if (want >= 0) {
			pthread_mutex_lock(&watch.mutex);
			done = watch.good > 0;
			pthread_mutex_unlock(&watch.mutex);
		} else {
			done = obs_source_get_width(source) != 0;
		}
		if (done)
			break;
	}

	pthread_mutex_lock(&watch.mutex);
	watch.recording = false;
	shown.ticks = watch.ticks;
	shown.good = watch.good;
	const int bad_x = watch.bad_x, bad_y = watch.bad_y, bad_want = watch.bad_want;
	const uint8_t bad_px[3] = {watch.bad_px[0], watch.bad_px[1], watch.bad_px[2]};
	pthread_mutex_unlock(&watch.mutex);

	shown.width = obs_source_get_width(source);
	shown.height = obs_source_get_height(source);

	obs_set_output_source(0, NULL);
	obs_scene_release(scene);
	obs_source_output_video(source, NULL);
	obs_source_remove(source);
	obs_source_release(source);

	/* one write, so libobs's log lines do not land in the middle */
	char msg[256];
	int len = snprintf(msg, sizeof(msg), "rotation %ld: %ux%u", rotation, shown.width, shown.height);
	if (want >= 0 && len > 0 && (size_t)len < sizeof(msg))
		len += snprintf(msg + len, sizeof(msg) - len,
				", %zu of %zu output frames showed it turned by %ld in its box", shown.good,
				shown.ticks, want);
	if (want >= 0 && !shown.good && bad_x >= 0 && len > 0 && (size_t)len < sizeof(msg))
		snprintf(msg + len, sizeof(msg) - len,
			 " (last wrong pixel (%d, %d) is B, G, R %d, %d, %d; expected %s)", bad_x, bad_y, bad_px[0],
			 bad_px[1], bad_px[2], bad_want < 0 ? "black" : "a quadrant");
	fprintf(stderr, "%s\n", msg);
	return shown;
}

/* Checks that `rotation` draws as `want` (0, 90, 180 or 270) and reports
 * the matching size. Returns the number of checks that failed. */
static int check_drawn(long rotation, long want)
{
	struct shown shown = show(rotation, want);
	const uint32_t bw = (want % 180) ? H : W;
	const uint32_t bh = (want % 180) ? W : H;
	int wrong = 0;

	if (shown.width != bw || shown.height != bh) {
		fprintf(stderr, "  size is %ux%u, expected %ux%u\n", shown.width, shown.height, bw, bh);
		wrong++;
	}
	if (!shown.good) {
		fprintf(stderr, "  the picture is not drawn turned by %ld in the item's box\n", want);
		wrong++;
	}
	return wrong;
}

/* ------------------------------------------------------------------------ */

/* The five rotations that worked before: 0, 90, 180 and 270, and -90 as
 * 270. */
static void rotations_that_worked_are_unchanged(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	int wrong = check_drawn(0, 0);
	wrong += check_drawn(90, 90);
	wrong += check_drawn(180, 180);
	wrong += check_drawn(270, 270);
	wrong += check_drawn(-90, 270);
	assert_int_equal(wrong, 0);
}

/* Other multiples of 90 draw as their equivalent in 0-359. */
static void equivalent_rotations_are_drawn_in_the_box(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	int wrong = check_drawn(-180, 180);
	wrong += check_drawn(-270, 90);
	wrong += check_drawn(360, 0);
	wrong += check_drawn(450, 90);
	wrong += check_drawn(-450, 270);
	wrong += check_drawn(720, 0);
	wrong += check_drawn(-360, 0);
	assert_int_equal(wrong, 0);
}

/* The multiples of 90 nearest LONG_MAX and LONG_MIN (neither end is one:
 * with a 64-bit long, LONG_MAX - 7 and LONG_MIN + 8 are, and both are
 * multiples of 360), and the next ones in. The setter's modulo must not
 * overflow at LONG_MIN. */
static void multiples_of_90_near_the_ends_of_long(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	const long top = LONG_MAX - LONG_MAX % 90;
	const long bottom = LONG_MIN - LONG_MIN % 90;
	const long cases[] = {top, top - 90, top - 180, bottom, bottom + 90, bottom + 180};

	int wrong = 0;
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++)
		wrong += check_drawn(cases[i], normalized(cases[i]));
	assert_int_equal(wrong, 0);
}

/* Rotations that are not a multiple of 90 keep the size they had: swapped
 * unless the rotation is a multiple of 180. How they are drawn is not
 * defined, so it is not checked. */
static void other_rotations_keep_their_size(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	const long cases[] = {1, -1, 45, -45, 135, 315, 359, -359, 361, LONG_MAX, LONG_MIN, LONG_MIN + 1};
	int wrong = 0;
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		struct shown shown = show(cases[i], -1);
		const bool swapped = (cases[i] % 180) != 0;
		const uint32_t bw = swapped ? H : W;
		const uint32_t bh = swapped ? W : H;
		if (shown.width != bw || shown.height != bh) {
			fprintf(stderr, "  size is %ux%u, expected %ux%u\n", shown.width, shown.height, bw, bh);
			wrong++;
		}
	}
	assert_int_equal(wrong, 0);
}

/* Setting another rotation on a source that is already shown takes effect
 * at once. */
static void rotation_can_be_changed(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	/* show() creates a new source, so this sets one rotation after another
	 * on the same source itself */
	const uint8_t *pixels = frame_pixels();
	obs_source_t *source = obs_source_create(SOURCE_ID, "rotated twice", NULL, NULL);
	assert_non_null(source);
	obs_scene_t *scene = obs_scene_create_private("async rotation scene 2");
	obs_scene_add(scene, source);
	obs_set_output_source(0, obs_scene_get_source(scene));

	const long steps[][2] = {{-180, 180}, {450, 90}, {-90, 270}, {0, 0}};
	int wrong = 0;
	for (size_t i = 0; i < sizeof(steps) / sizeof(steps[0]); i++) {
		obs_source_set_async_rotation(source, steps[i][0]);

		pthread_mutex_lock(&watch.mutex);
		watch.want = steps[i][1];
		watch.ticks = 0;
		watch.good = 0;
		watch.recording = true;
		pthread_mutex_unlock(&watch.mutex);

		const uint64_t period = 1000000000ULL / CANVAS_FPS;
		const uint64_t start = os_gettime_ns();
		bool done = false;
		for (uint64_t n = 0; !done && n * period <= PUSH_LIMIT_NS; n++) {
			const uint64_t ts = start + n * period;
			struct obs_source_frame frame = {
				.data = {(uint8_t *)pixels},
				.linesize = {W * 4},
				.width = W,
				.height = H,
				.format = VIDEO_FORMAT_BGRA,
				.full_range = true,
				.timestamp = ts,
			};
			os_sleepto_ns(ts);
			obs_source_output_video(source, &frame);
			pthread_mutex_lock(&watch.mutex);
			done = watch.good > 0;
			pthread_mutex_unlock(&watch.mutex);
		}

		pthread_mutex_lock(&watch.mutex);
		watch.recording = false;
		const size_t good = watch.good, ticks = watch.ticks;
		pthread_mutex_unlock(&watch.mutex);

		const uint32_t width = obs_source_get_width(source), height = obs_source_get_height(source);
		const uint32_t bw = (steps[i][1] % 180) ? H : W, bh = (steps[i][1] % 180) ? W : H;
		fprintf(stderr, "then rotation %ld: %ux%u, %zu of %zu output frames showed it turned by %ld\n",
			steps[i][0], width, height, good, ticks, steps[i][1]);
		if (!good || width != bw || height != bh)
			wrong++;
	}

	obs_set_output_source(0, NULL);
	obs_scene_release(scene);
	obs_source_output_video(source, NULL);
	obs_source_remove(source);
	obs_source_release(source);
	assert_int_equal(wrong, 0);
}

static int setup(void **state)
{
	(void)state;
	pthread_mutex_init(&watch.mutex, NULL);

	/* obs_startup() needs a display on Linux (run under xvfb-run); the
	 * cases skip without one. */
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = CANVAS_FPS,
		.fps_den = 1,
		.base_width = CANVAS,
		.base_height = CANVAS,
		.output_width = CANVAS,
		.output_height = CANVAS,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_POINT,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	obs_register_source(&test_source);
	obs_add_raw_video_callback(NULL, raw_video, &watch);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_remove_raw_video_callback(raw_video, &watch);
		obs_shutdown();
	}
	pthread_mutex_destroy(&watch.mutex);
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(rotations_that_worked_are_unchanged),
		cmocka_unit_test(equivalent_rotations_are_drawn_in_the_box),
		cmocka_unit_test(multiples_of_90_near_the_ends_of_long),
		cmocka_unit_test(other_rotations_keep_their_size),
		cmocka_unit_test(rotation_can_be_changed),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
