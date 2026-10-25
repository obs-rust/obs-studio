/* The original Image Slide Show (obs-slideshow.c, which sources saved before
 * the rework still use) keeps its pictures decoded up to a 400 MiB cap
 * (obs-rust/obs-studio#2279, upstream obsproject/obs-studio#14076).
 *
 * gs_image_file_ex_init() stopped filling mem_usage, so every Image source
 * reported 0 bytes, the cap never applied and every slide stayed decoded.
 * With the counter back, the slideshow must still show every slide, in
 * order, at its size, and only the slides around the current one may be
 * loaded past the cap.
 *
 * The test loads the real image-source and obs-transitions modules with core
 * video running and makes a slideshow of PICS generated 4096x4096 PNGs
 * (64 MiB each once decoded): the first RESIDENT reach the cap, the rest are
 * past it. The last picture is a few rows taller, so the slideshow's size
 * shows whether that picture was measured.
 *
 * Core video needs a graphics module and libobs needs an X display on Linux,
 * so the cases skip without one (run under xvfb-run). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <zlib.h>

#include <obs.h>
#include <obs-module.h>
#include <util/dstr.h>
#include <util/platform.h>

#define SIDE 4096
#define LAST_CY (SIDE + 8)
#define PICS 12
/* 7 * 64 MiB = 448 MiB is the first total at or over the 400 MiB cap */
#define RESIDENT 7
/* past the cap: the current slide, the next one and the previous one */
#define MAX_LIVE_PAST_CAP 3

static bool obs_up;
static obs_module_t *image_module;
static obs_module_t *transitions_module;
static char dir[512];
static char paths[PICS][600];
/* the slideshow under test, released by the teardown if a case fails */
static obs_source_t *current_ss;

/* ------------------------------------------------------------------------- */
/* PNG writer (RGB, 8 bits per channel, one colour), one row at a time */

static void put_be32(uint8_t *p, uint32_t v)
{
	p[0] = (uint8_t)(v >> 24);
	p[1] = (uint8_t)(v >> 16);
	p[2] = (uint8_t)(v >> 8);
	p[3] = (uint8_t)v;
}

static bool write_chunk(FILE *f, const char *type, const uint8_t *data, uint32_t size)
{
	uint8_t be[4];
	put_be32(be, size);
	if (fwrite(be, 1, 4, f) != 4 || fwrite(type, 1, 4, f) != 4)
		return false;
	if (size && fwrite(data, 1, size, f) != size)
		return false;
	uLong crc = crc32(0L, (const Bytef *)type, 4);
	if (size)
		crc = crc32(crc, data, size);
	put_be32(be, (uint32_t)crc);
	return fwrite(be, 1, 4, f) == 4;
}

static bool write_png(const char *path, uint32_t cx, uint32_t cy, uint8_t shade)
{
	FILE *f = fopen(path, "wb");
	if (!f)
		return false;

	static const uint8_t sig[8] = {0x89, 'P', 'N', 'G', '\r', '\n', 0x1a, '\n'};
	bool ok = fwrite(sig, 1, sizeof(sig), f) == sizeof(sig);

	uint8_t ihdr[13] = {0};
	put_be32(ihdr, cx);
	put_be32(ihdr + 4, cy);
	ihdr[8] = 8; /* bit depth */
	ihdr[9] = 2; /* colour type: RGB */
	ok = ok && write_chunk(f, "IHDR", ihdr, sizeof(ihdr));

	const size_t row_size = 1 + (size_t)cx * 3;
	uint8_t *row = malloc(row_size);
	const size_t out_size = 1 << 16;
	uint8_t *out = malloc(out_size);
	ok = ok && row && out;

	z_stream z = {0};
	ok = ok && deflateInit(&z, 9) == Z_OK;
	if (ok) {
		row[0] = 0; /* filter: none */
		memset(row + 1, shade, row_size - 1);
	}
	for (uint32_t y = 0; ok && y <= cy; y++) {
		const bool last = y == cy;
		if (!last) {
			z.next_in = row;
			z.avail_in = (uInt)row_size;
		}
		int ret;
		do {
			z.next_out = out;
			z.avail_out = (uInt)out_size;
			ret = deflate(&z, last ? Z_FINISH : Z_NO_FLUSH);
			ok = ok && (ret == Z_OK || ret == Z_STREAM_END || ret == Z_BUF_ERROR);
			const uint32_t have = (uint32_t)(out_size - z.avail_out);
			if (ok && have)
				ok = write_chunk(f, "IDAT", out, have);
		} while (ok && (z.avail_out == 0 || (last && ret != Z_STREAM_END)));
	}
	deflateEnd(&z);
	ok = ok && write_chunk(f, "IEND", NULL, 0);

	free(out);
	free(row);
	return fclose(f) == 0 && ok;
}

static void remove_pictures(void)
{
	for (int i = 0; i < PICS; i++)
		if (*paths[i])
			unlink(paths[i]);
	if (*dir)
		rmdir(dir);
}

static bool make_pictures(void)
{
	const char *tmp = getenv("TMPDIR") ? getenv("TMPDIR") : "/tmp";
	snprintf(dir, sizeof(dir), "%s/obs-slideshow-mem-%d", tmp, (int)getpid());
	if (os_mkdirs(dir) == MKDIR_ERROR)
		return false;

	for (int i = 0; i < PICS; i++) {
		snprintf(paths[i], sizeof(paths[i]), "%s/slide%02d.png", dir, i);
		uint32_t cy = i == PICS - 1 ? LAST_CY : SIDE;
		if (!write_png(paths[i], SIDE, cy, (uint8_t)(20 * i)))
			return false;
	}
	return true;
}

/* ------------------------------------------------------------------------- */

static int path_index(const char *file)
{
	for (int i = 0; i < PICS; i++)
		if (file && strcmp(file, paths[i]) == 0)
			return i;
	return -1;
}

static int source_index(obs_source_t *source)
{
	if (!source || strcmp(obs_source_get_id(source), "image_source") != 0)
		return -1;
	obs_data_t *settings = obs_source_get_settings(source);
	int idx = path_index(obs_data_get_string(settings, "file"));
	obs_data_release(settings);
	return idx;
}

struct live_count {
	int images;
	int past_cap;
};

static bool count_live(void *param, obs_source_t *source)
{
	struct live_count *count = param;
	int idx = source_index(source);
	if (idx >= 0) {
		count->images++;
		if (idx >= RESIDENT)
			count->past_cap++;
	}
	return true;
}

static struct live_count live_images(void)
{
	struct live_count count = {0};
	obs_enum_all_sources(count_live, &count);
	return count;
}

static obs_data_t *slideshow_settings(bool randomize, const char *transition, int speed_ms)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "slide_mode", "mode_manual");
	obs_data_set_bool(settings, "loop", true);
	obs_data_set_bool(settings, "randomize", randomize);
	obs_data_set_string(settings, "transition", transition);
	obs_data_set_int(settings, "transition_speed", speed_ms);
	return settings;
}

static void set_files(obs_data_t *settings, bool folder)
{
	obs_data_array_t *files = obs_data_array_create();
	for (int i = 0; i < (folder ? 1 : PICS); i++) {
		obs_data_t *item = obs_data_create();
		obs_data_set_string(item, "value", folder ? dir : paths[i]);
		obs_data_array_push_back(files, item);
		obs_data_release(item);
	}
	obs_data_set_array(settings, "files", files);
	obs_data_array_release(files);
}

static obs_source_t *get_transition(obs_source_t *ss);

static obs_source_t *create_slideshow(obs_data_t *settings)
{
	static int count;
	char name[32];
	snprintf(name, sizeof(name), "legacy slideshow %d", ++count);

	/* "slideshow" is the original slideshow, "slideshow_v2" the rework */
	obs_source_t *ss = obs_source_create("slideshow", name, settings, NULL);
	obs_data_release(settings);
	assert_non_null(ss);
	assert_string_equal(obs_source_get_id(ss), "slideshow");

	/* on the program output, so it is active and its transition ticks */
	obs_set_output_source(0, ss);

	current_ss = ss;

	/* The update that loads the files runs on the video thread; it ends
	 * with a transition to the first slide. */
	uint64_t start = os_gettime_ns();
	for (int i = 0; i < 1500; i++) {
		obs_source_t *tr = get_transition(ss);
		obs_source_t *active = tr ? obs_transition_get_active_source(tr) : NULL;
		obs_source_release(active);
		if (active && obs_transition_get_time(tr) >= 1.0f)
			break;
		os_sleep_ms(40);
	}
	os_sleep_ms(200);
	printf("%s loaded in %.1f s\n", name, (double)(os_gettime_ns() - start) / 1e9);
	return ss;
}

static void destroy_slideshow(obs_source_t *ss)
{
	current_ss = NULL;
	obs_set_output_source(0, NULL);
	obs_source_release(ss);
	/* sources are destroyed off the caller's thread */
	for (int i = 0; i < 100 && live_images().images; i++)
		os_sleep_ms(20);
}

static long long proc_int(obs_source_t *ss, const char *proc, const char *name)
{
	calldata_t cd = {0};
	proc_handler_call(obs_source_get_proc_handler(ss), proc, &cd);
	long long v = calldata_int(&cd, name);
	calldata_free(&cd);
	return v;
}

static long long current_index(obs_source_t *ss)
{
	return proc_int(ss, "current_index", "current_index");
}

static void get_transition_cb(obs_source_t *parent, obs_source_t *child, void *param)
{
	UNUSED_PARAMETER(parent);
	obs_source_t **out = param;
	if (obs_source_get_type(child) == OBS_SOURCE_TYPE_TRANSITION)
		*out = child;
}

static obs_source_t *get_transition(obs_source_t *ss)
{
	obs_source_t *transition = NULL;
	obs_source_enum_active_sources(ss, get_transition_cb, &transition);
	return transition;
}

static int shown_slide(obs_source_t *ss, uint32_t *cx, uint32_t *cy);

/* Waits for a Next/Previous to land: the index changes, the transition
 * shows that slide and finishes. A slide past the cap may still be
 * decoding when the index changes. */
static void press(obs_source_t *ss, bool next)
{
	long long before = current_index(ss);
	if (next)
		obs_source_media_next(ss);
	else
		obs_source_media_previous(ss);

	for (int i = 0; i < 250; i++) {
		os_sleep_ms(20);
		uint32_t cx, cy;
		long long idx = current_index(ss);
		obs_source_t *tr = get_transition(ss);
		if (idx != before && shown_slide(ss, &cx, &cy) == idx && tr && obs_transition_get_time(tr) >= 1.0f)
			break;
	}
	os_sleep_ms(60);
}

/* The slide on screen: its picture index, and its size once decoded. */
static int shown_slide(obs_source_t *ss, uint32_t *cx, uint32_t *cy)
{
	obs_source_t *tr = get_transition(ss);
	if (!tr)
		return -1;
	obs_source_t *active = obs_transition_get_active_source(tr);
	int idx = source_index(active);
	*cx = active ? obs_source_get_width(active) : 0;
	*cy = active ? obs_source_get_height(active) : 0;
	obs_source_release(active);
	return idx;
}

static uint32_t expected_cy(int idx)
{
	return idx == PICS - 1 ? LAST_CY : SIDE;
}

static void check_shown(obs_source_t *ss, int expected, const char *when)
{
	uint32_t cx, cy;
	int idx = shown_slide(ss, &cx, &cy);
	struct live_count live = live_images();
	printf("%s: index %lld, picture %d (%ux%u), live pictures %d, past the cap %d\n", when, current_index(ss), idx,
	       cx, cy, live.images, live.past_cap);
	if (expected >= 0) {
		assert_int_equal(current_index(ss), expected);
		assert_int_equal(idx, expected);
	} else {
		assert_true(idx >= 0);
	}
	assert_int_equal(cx, SIDE);
	assert_int_equal(cy, expected_cy(idx));
	assert_true(live.past_cap <= MAX_LIVE_PAST_CAP);
}

/* ------------------------------------------------------------------------- */

/* Next through every slide and around, then Previous back over the start. */
static void steps_through_every_slide(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_data_t *settings = slideshow_settings(false, "cut", 0);
	set_files(settings, false);
	obs_source_t *ss = create_slideshow(settings);

	long long total = proc_int(ss, "total_files", "total_files");
	printf("total_files %lld, slideshow %ux%u\n", total, obs_source_get_width(ss), obs_source_get_height(ss));
	assert_int_equal(total, PICS);
	/* the picture past the cap still sizes the slideshow */
	assert_int_equal(obs_source_get_width(ss), SIDE);
	assert_int_equal(obs_source_get_height(ss), LAST_CY);

	for (int i = 0; i < PICS; i++) {
		char when[32];
		snprintf(when, sizeof(when), "next %d", i);
		check_shown(ss, i, when);
		press(ss, true);
	}
	check_shown(ss, 0, "wrapped");

	press(ss, false);
	check_shown(ss, PICS - 1, "previous from the first");
	press(ss, false);
	check_shown(ss, PICS - 2, "previous");

	destroy_slideshow(ss);
}

/* A fade between slides past the cap has both pictures decoded. */
static void fade_has_both_slides(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_data_t *settings = slideshow_settings(false, "fade", 700);
	set_files(settings, false);
	obs_source_t *ss = create_slideshow(settings);

	for (int i = 0; i < PICS - 1; i++) {
		long long before = current_index(ss);
		obs_source_media_next(ss);
		for (int t = 0; t < 250 && current_index(ss) == before; t++)
			os_sleep_ms(10);
		os_sleep_ms(100);

		obs_source_t *tr = get_transition(ss);
		assert_non_null(tr);
		obs_source_t *a = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_A);
		obs_source_t *b = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_B);
		float t = obs_transition_get_time(tr);
		int ia = source_index(a), ib = source_index(b);
		uint32_t wa = a ? obs_source_get_width(a) : 0;
		uint32_t wb = b ? obs_source_get_width(b) : 0;
		printf("fade %d -> %d at %.2f: widths %u -> %u\n", ia, ib, t, wa, wb);
		obs_source_release(a);
		obs_source_release(b);

		assert_true(t < 1.0f);
		assert_int_equal(ia, i);
		assert_int_equal(ib, i + 1);
		assert_int_equal(wa, SIDE);
		assert_int_equal(wb, SIDE);

		for (int w = 0; w < 300 && obs_transition_get_time(tr) < 1.0f; w++)
			os_sleep_ms(10);
		assert_true(live_images().past_cap <= MAX_LIVE_PAST_CAP);
	}

	destroy_slideshow(ss);
}

/* Random order: every slide shown is decoded, and few past the cap are
 * loaded. */
static void random_slides_are_loaded(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_data_t *settings = slideshow_settings(true, "cut", 0);
	set_files(settings, false);
	obs_source_t *ss = create_slideshow(settings);

	assert_int_equal(proc_int(ss, "total_files", "total_files"), PICS);
	check_shown(ss, -1, "random start");
	for (int i = 0; i < 24; i++) {
		char when[32];
		snprintf(when, sizeof(when), "random %d", i);
		press(ss, i % 4 != 3);
		check_shown(ss, -1, when);
	}

	destroy_slideshow(ss);
}

/* A folder: every picture in it is a slide. */
static void folder_keeps_every_picture(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_data_t *settings = slideshow_settings(false, "cut", 0);
	set_files(settings, true);
	obs_source_t *ss = create_slideshow(settings);

	long long total = proc_int(ss, "total_files", "total_files");
	printf("folder: total_files %lld\n", total);
	assert_int_equal(total, PICS);

	bool seen[PICS] = {0};
	for (int i = 0; i < PICS; i++) {
		uint32_t cx, cy;
		int idx = shown_slide(ss, &cx, &cy);
		struct live_count live = live_images();
		printf("folder %d: picture %d (%ux%u), live pictures %d\n", i, idx, cx, cy, live.images);
		assert_true(idx >= 0);
		assert_false(seen[idx]);
		seen[idx] = true;
		assert_int_equal(cx, SIDE);
		assert_int_equal(cy, expected_cy(idx));
		assert_true(live.images <= RESIDENT + MAX_LIVE_PAST_CAP);
		press(ss, true);
	}

	destroy_slideshow(ss);
}

/* ------------------------------------------------------------------------- */

static int group_setup(void **state)
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
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		return 0;
	}

	if (obs_open_module(&image_module, IMAGE_SOURCE_MODULE_PATH, IMAGE_SOURCE_DATA_PATH) != MODULE_SUCCESS ||
	    !obs_init_module(image_module) ||
	    obs_open_module(&transitions_module, TRANSITIONS_MODULE_PATH, TRANSITIONS_DATA_PATH) != MODULE_SUCCESS ||
	    !obs_init_module(transitions_module)) {
		fprintf(stderr, "could not load the image-source and obs-transitions modules\n");
		obs_shutdown();
		return -1;
	}

	if (!make_pictures()) {
		fprintf(stderr, "could not write the pictures in %s\n", dir);
		remove_pictures();
		obs_shutdown();
		return -1;
	}

	obs_up = true;
	return 0;
}

static int case_teardown(void **state)
{
	(void)state;
	if (current_ss)
		destroy_slideshow(current_ss);
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_shutdown();
		obs_up = false;
	}
	remove_pictures();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(steps_through_every_slide, case_teardown),
		cmocka_unit_test_teardown(fade_has_both_slides, case_teardown),
		cmocka_unit_test_teardown(random_slides_are_loaded, case_teardown),
		cmocka_unit_test_teardown(folder_keeps_every_picture, case_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
