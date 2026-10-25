#include <setjmp.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <cmocka.h>

#include <graphics/graphics-internal.h>
#include <graphics/image-file.h>
#include <graphics/matrix4.h>
#include <util/base.h>
#include <util/bmem.h>
#include <util/crc32.h>
#include <util/dstr.h>
#include <util/platform.h>

/*
 * Characterization of libobs/graphics/image-file.c.h (gs_image_file_ex_*),
 * before the port to obs-graphics.
 *
 * No GPU. The fixture builds a graphics_t by hand, as test_texture_render
 * does, with a fake device that records texture creates (with the bytes
 * they are given), destroys and the bytes gs_texture_set_image writes. The
 * pictures are the image-file-* files in data/ (data/image-file.py writes
 * them; the 1024x1024 PNG is written at run time). GIFs go through
 * libnsgif, everything else through FFmpeg.
 *
 * Characterized, not endorsed:
 * - A .gif that cannot be opened, or is larger than 4096 in either
 *   direction, fails without trying FFmpeg. A one-frame .gif goes to FFmpeg.
 * - gs_image_file_ex_init does not fill alpha_mode or space.
 *
 * mem_usage is the decoded size the Image Slide Show counts against its
 * memory cap (obs-rust/obs-studio#2279, upstream #14076): width * height *
 * bytes per pixel for a still picture; for an animated GIF the frame cache,
 * every decoded frame, the current frame and the file itself.
 * - Only frame 0 gets the alpha mode given to init. Frames decoded later by
 *   tick and update_texture are always straight alpha.
 * - A GIF that loops forever never counts cur_loop up.
 * - A tick long enough to come back to the current frame returns false and
 *   decodes nothing.
 * - A GIF frame delay of 0 plays for 100 ms.
 */

/* printf-checked, so clang's -Wformat-nonliteral accepts the vsnprintf. */
#if defined(__GNUC__) || defined(__clang__)
#define RECORD_FORMAT __attribute__((format(printf, 1, 2)))
#else
#define RECORD_FORMAT
#endif

#define DATA(name) IMAGE_FILE_DATA_DIR "/" name

static char textures[8];
static uint32_t texture_cx[8];
static uint32_t texture_cy[8];
static uint8_t mapped[8][64];
static char device_token;
static size_t next_texture;

static char call_log[8192];

RECORD_FORMAT static void record(const char *format, ...)
{
	size_t len = strlen(call_log);
	va_list args;

	if (len)
		call_log[len++] = ';';

	va_start(args, format);
	vsnprintf(call_log + len, sizeof(call_log) - len, format, args);
	va_end(args);
}

static void record_bytes(const uint8_t *data, size_t size)
{
	size_t len = strlen(call_log);

	for (size_t i = 0; i < size && len + 3 < sizeof(call_log); i++) {
		if (i && i % 4 == 0)
			call_log[len++] = '.';
		snprintf(call_log + len, sizeof(call_log) - len, "%02x", data[i]);
		len += 2;
	}
}

static int texture_index(const gs_texture_t *tex)
{
	const char *p = (const char *)tex;

	if (p >= textures && p < textures + sizeof(textures))
		return (int)(p - textures);
	return -1;
}

static void fake_enter_context(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
}

static void fake_leave_context(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
}

static gs_texture_t *fake_texture_create(gs_device_t *device, uint32_t width, uint32_t height,
					 enum gs_color_format color_format, uint32_t levels, const uint8_t **data,
					 uint32_t flags)
{
	assert_ptr_equal(device, &device_token);
	assert_true(next_texture + 1 < sizeof(textures));
	next_texture++;
	texture_cx[next_texture] = width;
	texture_cy[next_texture] = height;
	record("tex_create T%d %u %u f%d l%u 0x%x ", (int)next_texture, width, height, (int)color_format, levels,
	       flags);
	if (data && data[0])
		record_bytes(data[0], (size_t)width * height * 4);
	else
		record("null");
	return (gs_texture_t *)&textures[next_texture];
}

static void fake_texture_destroy(gs_texture_t *tex)
{
	record("tex_destroy T%d", texture_index(tex));
}

static uint32_t fake_texture_get_height(const gs_texture_t *tex)
{
	return texture_cy[texture_index(tex)];
}

static bool fake_texture_map(gs_texture_t *tex, uint8_t **ptr, uint32_t *linesize)
{
	int i = texture_index(tex);

	memset(mapped[i], 0xee, sizeof(mapped[i]));
	*ptr = mapped[i];
	*linesize = texture_cx[i] * 4;
	return true;
}

static void fake_texture_unmap(gs_texture_t *tex)
{
	int i = texture_index(tex);

	record("set_image T%d ", i);
	record_bytes(mapped[i], (size_t)texture_cx[i] * texture_cy[i] * 4);
}

/* ------------------------------------------------------------------------- */
/* log capture */

static struct dstr log_text;

static void capture_log(int level, const char *format, va_list args, void *param)
{
	char line[1024];

	(void)level;
	(void)param;
	vsnprintf(line, sizeof(line), format, args);
	dstr_cat(&log_text, line);
	dstr_cat(&log_text, "\n");
}

static graphics_t *graphics;

static int setup(void **state)
{
	struct matrix4 top;

	(void)state;

	graphics = bzalloc(sizeof(struct graphics_subsystem));
	matrix4_identity(&top);
	da_push_back(graphics->matrix_stack, &top);
	pthread_mutex_init(&graphics->mutex, NULL);
	pthread_mutex_init(&graphics->effect_mutex, NULL);

	graphics->device = (gs_device_t *)&device_token;
	graphics->exports.device_enter_context = fake_enter_context;
	graphics->exports.device_leave_context = fake_leave_context;
	graphics->exports.device_texture_create = fake_texture_create;
	graphics->exports.gs_texture_destroy = fake_texture_destroy;
	graphics->exports.gs_texture_get_height = fake_texture_get_height;
	graphics->exports.gs_texture_map = fake_texture_map;
	graphics->exports.gs_texture_unmap = fake_texture_unmap;

	next_texture = 0;
	call_log[0] = 0;
	dstr_free(&log_text);
	/* Allocated here, so a captured line does not count in bnum_allocs. */
	dstr_reserve(&log_text, 4096);
	log_text.array[0] = 0;
	base_set_log_handler(capture_log, NULL);

	gs_enter_context(graphics);
	return 0;
}

static int teardown(void **state)
{
	(void)state;

	gs_leave_context();
	base_set_log_handler(NULL, NULL);
	dstr_free(&log_text);
	pthread_mutex_destroy(&graphics->mutex);
	pthread_mutex_destroy(&graphics->effect_mutex);
	da_free(graphics->matrix_stack);
	bfree(graphics);
	graphics = NULL;
	return 0;
}

static bool log_has(const char *text)
{
	return log_text.array && strstr(log_text.array, text) != NULL;
}

static bool is_zeroed(const gs_image_file_ex_t *image)
{
	static const gs_image_file_ex_t zero;
	return memcmp(image, &zero, sizeof(zero)) == 0;
}

#define FRAME0 "00000000.ff0000ff.ff0000ff.ff0000ff.ff0000ff.ff0000ff.ff0000ff.c86432ff"
#define FRAME1 "00ff00ff.00ff00ff.00ff00ff.00ff00ff.00ff00ff.00ff00ff.00ff00ff.c86432ff"
#define FRAME2 "0000ffff.0000ffff.0000ffff.0000ffff.0000ffff.0000ffff.0000ffff.c86432ff"

#define MS 1000000ULL

/* ------------------------------------------------------------------------- */

static void null_file_test(void **state)
{
	gs_image_file_ex_t image;
	long allocs = bnum_allocs();

	(void)state;

	memset(&image, 0x5a, sizeof(image));
	gs_image_file_ex_init(&image, NULL, GS_IMAGE_ALPHA_STRAIGHT);
	assert_true(is_zeroed(&image));
	assert_int_equal(bnum_allocs(), allocs);

	gs_image_file_ex_init(NULL, DATA("image-file-anim.gif"), GS_IMAGE_ALPHA_STRAIGHT);
	gs_image_file_ex_free(NULL);

	gs_image_file_ex_init_texture(&image);
	assert_false(gs_image_file_ex_tick(&image, 100 * MS));
	gs_image_file_ex_update_texture(&image);
	gs_image_file_ex_free(&image);
	assert_true(is_zeroed(&image));
	assert_string_equal(call_log, "");
	assert_int_equal(bnum_allocs(), allocs);
}

static void missing_gif_test(void **state)
{
	gs_image_file_ex_t image;
	long allocs = bnum_allocs();

	(void)state;

	gs_image_file_ex_init(&image, DATA("image-file-missing.gif"), GS_IMAGE_ALPHA_STRAIGHT);
	assert_true(is_zeroed(&image));
	assert_int_equal(bnum_allocs(), allocs);
	assert_true(log_has("init_animated_gif: Failed to open file '" DATA("image-file-missing.gif") "'"));
	/* FFmpeg is not tried. */
	assert_false(log_has("Failed to load file"));
	gs_image_file_ex_free(&image);
	assert_string_equal(call_log, "");
}

static void missing_png_test(void **state)
{
	gs_image_file_ex_t image;
	long allocs = bnum_allocs();

	(void)state;

	gs_image_file_ex_init(&image, DATA("image-file-missing.png"), GS_IMAGE_ALPHA_STRAIGHT);
	assert_true(is_zeroed(&image));
	assert_int_equal(bnum_allocs(), allocs);
	assert_true(
		log_has("gs_image_file_ex_init_internal: Failed to load file '" DATA("image-file-missing.png") "'"));
	assert_false(log_has("init_animated_gif"));
}

static void wide_gif_test(void **state)
{
	gs_image_file_ex_t image;
	long allocs = bnum_allocs();

	(void)state;

	gs_image_file_ex_init(&image, DATA("image-file-wide.gif"), GS_IMAGE_ALPHA_STRAIGHT);
	assert_true(is_zeroed(&image));
	assert_int_equal(bnum_allocs(), allocs);
	assert_true(log_has("init_animated_gif: Bad texture dimensions (4097x1) in '" DATA("image-file-wide.gif") "'"));
	assert_false(log_has("Failed to load file"));
}

static void animated_gif_load_test(void **state)
{
	gs_image_file_ex_t image;
	long allocs = bnum_allocs();

	(void)state;

	gs_image_file_ex_init(&image, DATA("image-file-anim.gif"), GS_IMAGE_ALPHA_PREMULTIPLY);
	assert_true(image.loaded);
	assert_true(image.is_animated_gif);
	assert_false(image.frame_updated);
	assert_int_equal(image.cx, 4);
	assert_int_equal(image.cy, 2);
	assert_int_equal(image.format, GS_RGBA);
	assert_non_null(image.internal);
	assert_null(image.texture);
	assert_null(image.texture_data);
	assert_int_equal(image.cur_time, 0);
	assert_int_equal(image.cur_frame, 0);
	assert_int_equal(image.cur_loop, 0);
	/* frame cache (3 pointers), 3 decoded frames, the current frame, the
	 * file */
	assert_int_equal(image.mem_usage, 3 * sizeof(uint8_t *) + 3 * (4 * 2 * 4) + 4 * 2 * 4 +
						  (uint64_t)os_get_file_size(DATA("image-file-anim.gif")));
	assert_true(image.mem_usage > 0);
	assert_int_equal(image.alpha_mode, GS_IMAGE_ALPHA_STRAIGHT);
	assert_int_equal(image.space, GS_CS_SRGB);
	/* internal, file data, nsgif's bitmap, frame cache, frame data */
	assert_int_equal(bnum_allocs(), allocs + 5);
	assert_string_equal(call_log, "");

	gs_image_file_ex_init_texture(&image);
	assert_ptr_equal(image.texture, &textures[1]);
	assert_string_equal(call_log, "tex_create T1 4 2 f3 l1 0x2 " FRAME0);
	assert_int_equal(bnum_allocs(), allocs + 5);

	call_log[0] = 0;
	gs_image_file_ex_free(&image);
	assert_true(is_zeroed(&image));
	assert_string_equal(call_log, "tex_destroy T1");
	assert_int_equal(bnum_allocs(), allocs);
}

static void tick_test(void **state)
{
	gs_image_file_ex_t image;

	(void)state;

	gs_image_file_ex_init(&image, DATA("image-file-anim.gif"), GS_IMAGE_ALPHA_STRAIGHT);
	gs_image_file_ex_init_texture(&image);
	call_log[0] = 0;

	/* frame 0: 100 ms */
	assert_false(gs_image_file_ex_tick(&image, 50 * MS));
	assert_int_equal(image.cur_time, 50 * MS);
	assert_false(gs_image_file_ex_tick(&image, 50 * MS));
	assert_int_equal(image.cur_frame, 0);
	assert_int_equal(image.cur_time, 100 * MS);
	assert_true(gs_image_file_ex_tick(&image, 1));
	assert_int_equal(image.cur_frame, 1);
	assert_int_equal(image.cur_time, 1);

	gs_image_file_ex_update_texture(&image);
	assert_string_equal(call_log, "set_image T1 " FRAME1);
	call_log[0] = 0;

	/* frame 1: delay 0, so 100 ms */
	assert_false(gs_image_file_ex_tick(&image, 100 * MS - 1));
	assert_int_equal(image.cur_frame, 1);
	assert_true(gs_image_file_ex_tick(&image, 50 * MS));
	assert_int_equal(image.cur_frame, 2);
	assert_int_equal(image.cur_time, 50 * MS);

	/* frame 2: 50 ms, then frame 0 again; the loop count stays 0. */
	assert_true(gs_image_file_ex_tick(&image, 1));
	assert_int_equal(image.cur_frame, 0);
	assert_int_equal(image.cur_loop, 0);
	assert_int_equal(image.cur_time, 1);

	/* Exactly one full loop (250 ms) comes back to frame 0: false. */
	assert_false(gs_image_file_ex_tick(&image, 250 * MS));
	assert_int_equal(image.cur_frame, 0);
	assert_int_equal(image.cur_time, 1);

	/* Frame 2 comes from the cache. */
	assert_true(gs_image_file_ex_tick(&image, 200 * MS));
	assert_int_equal(image.cur_frame, 2);
	assert_int_equal(image.cur_time, 1);
	gs_image_file_ex_update_texture(&image);
	assert_string_equal(call_log, "set_image T1 " FRAME2);
	call_log[0] = 0;

	assert_true(gs_image_file_ex_tick(&image, 50 * MS));
	assert_int_equal(image.cur_frame, 0);
	gs_image_file_ex_update_texture(&image);
	assert_string_equal(call_log, "set_image T1 " FRAME0);

	gs_image_file_ex_free(&image);
}

static void skip_frames_test(void **state)
{
	gs_image_file_ex_t image;

	(void)state;

	/* Frame 1 is decoded on the way to frame 2, then frame 2 is shown. */
	gs_image_file_ex_init(&image, DATA("image-file-anim.gif"), GS_IMAGE_ALPHA_STRAIGHT);
	gs_image_file_ex_init_texture(&image);
	call_log[0] = 0;
	assert_true(gs_image_file_ex_tick(&image, 200 * MS + 1));
	assert_int_equal(image.cur_frame, 2);
	assert_int_equal(image.cur_time, 1);
	gs_image_file_ex_update_texture(&image);
	assert_string_equal(call_log, "set_image T1 " FRAME2);
	gs_image_file_ex_free(&image);
}

static void loop_count_test(void **state)
{
	gs_image_file_ex_t image;

	(void)state;

	/* NETSCAPE loop count 2: libnsgif reports loop_max 3, three plays. */
	gs_image_file_ex_init(&image, DATA("image-file-loop2.gif"), GS_IMAGE_ALPHA_STRAIGHT);
	assert_true(image.is_animated_gif);

	assert_false(gs_image_file_ex_tick(&image, 250 * MS + 1));
	assert_int_equal(image.cur_frame, 0);
	assert_int_equal(image.cur_loop, 1);
	assert_false(gs_image_file_ex_tick(&image, 250 * MS));
	assert_int_equal(image.cur_frame, 0);
	assert_int_equal(image.cur_loop, 2);
	assert_int_equal(image.cur_time, 1);

	/* The third play ends on its last frame. */
	assert_true(gs_image_file_ex_tick(&image, 1000 * MS));
	assert_int_equal(image.cur_frame, 2);
	assert_int_equal(image.cur_loop, 3);
	assert_int_equal(image.cur_time, 750 * MS + 1);

	assert_false(gs_image_file_ex_tick(&image, 1000 * MS));
	assert_int_equal(image.cur_frame, 2);
	assert_int_equal(image.cur_time, 750 * MS + 1);
	gs_image_file_ex_free(&image);

	/* No NETSCAPE block: one play. */
	gs_image_file_ex_init(&image, DATA("image-file-noloop.gif"), GS_IMAGE_ALPHA_STRAIGHT);
	assert_true(gs_image_file_ex_tick(&image, 100 * MS + 1));
	assert_int_equal(image.cur_frame, 1);
	assert_true(gs_image_file_ex_tick(&image, 150 * MS));
	assert_int_equal(image.cur_frame, 2);
	assert_int_equal(image.cur_loop, 1);
	assert_int_equal(image.cur_time, 1);
	assert_false(gs_image_file_ex_tick(&image, 1000 * MS));
	assert_int_equal(image.cur_frame, 2);
	assert_int_equal(image.cur_time, 1);
	gs_image_file_ex_free(&image);
}

static void still_gif_test(void **state)
{
	gs_image_file_ex_t image;
	long allocs = bnum_allocs();

	(void)state;

	/* One frame: not animated, so FFmpeg decodes it. */
	gs_image_file_ex_init(&image, DATA("image-file-still.gif"), GS_IMAGE_ALPHA_STRAIGHT);
	assert_true(image.loaded);
	assert_false(image.is_animated_gif);
	assert_int_equal(image.cx, 2);
	assert_int_equal(image.cy, 2);
	assert_int_equal(image.mem_usage, 2 * 2 * 4);
	assert_non_null(image.texture_data);
	assert_non_null(image.internal);
	/* internal, texture data */
	assert_int_equal(bnum_allocs(), allocs + 2);
	assert_false(gs_image_file_ex_tick(&image, 1000 * MS));
	gs_image_file_ex_update_texture(&image);
	assert_string_equal(call_log, "");
	gs_image_file_ex_free(&image);
	assert_true(is_zeroed(&image));
	assert_int_equal(bnum_allocs(), allocs);
}

static void png_test(void **state)
{
	gs_image_file_ex_t image;
	long allocs = bnum_allocs();

	(void)state;

	gs_image_file_ex_init(&image, DATA("image-file.png"), GS_IMAGE_ALPHA_STRAIGHT);
	assert_true(image.loaded);
	assert_false(image.is_animated_gif);
	assert_int_equal(image.cx, 3);
	assert_int_equal(image.cy, 2);
	assert_int_equal(image.format, GS_RGBA);
	assert_int_equal(image.space, GS_CS_SRGB);
	assert_int_equal(image.mem_usage, 3 * 2 * 4);
	assert_non_null(image.texture_data);
	assert_int_equal(bnum_allocs(), allocs + 2);

	gs_image_file_ex_init_texture(&image);
	assert_null(image.texture_data);
	assert_int_equal(bnum_allocs(), allocs + 1);
	assert_string_equal(call_log, "tex_create T1 3 2 f3 l1 0x0 "
				      "ff0000ff.00ff0080.0000ffff.0a141eff.28323c00.46505aff");
	call_log[0] = 0;
	gs_image_file_ex_free(&image);
	assert_string_equal(call_log, "tex_destroy T1");
	assert_int_equal(bnum_allocs(), allocs);

	gs_image_file_ex_init(&image, DATA("image-file.png"), GS_IMAGE_ALPHA_PREMULTIPLY);
	call_log[0] = 0;
	gs_image_file_ex_init_texture(&image);
	assert_string_equal(call_log, "tex_create T2 3 2 f3 l1 0x0 "
				      "ff0000ff.00800080.0000ffff.0a141eff.00000000.46505aff");
	gs_image_file_ex_free(&image);
}

/* PNG writer for one RGBA colour, deflate "stored" blocks (no zlib needed) */

static void put_be32(uint8_t *p, uint32_t v)
{
	p[0] = (uint8_t)(v >> 24);
	p[1] = (uint8_t)(v >> 16);
	p[2] = (uint8_t)(v >> 8);
	p[3] = (uint8_t)v;
}

static bool write_chunk(FILE *f, const char *type, const uint8_t *data, size_t size)
{
	uint8_t be[4];
	put_be32(be, (uint32_t)size);
	if (fwrite(be, 1, 4, f) != 4 || fwrite(type, 1, 4, f) != 4)
		return false;
	if (size && fwrite(data, 1, size, f) != size)
		return false;
	put_be32(be, calc_crc32(calc_crc32(0, type, 4), data, size));
	return fwrite(be, 1, 4, f) == 4;
}

static bool write_solid_png(const char *path, uint32_t cx, uint32_t cy, const uint8_t rgba[4])
{
	const size_t row = 1 + (size_t)cx * 4;
	const size_t raw_size = row * cy;
	const size_t blocks = (raw_size + 65534) / 65535;
	const size_t z_size = 2 + raw_size + blocks * 5 + 4;
	uint8_t *z = malloc(z_size);
	if (!z)
		return false;

	/* zlib header, then stored blocks of filter-0 rows */
	uint8_t *out = z;
	*out++ = 0x78;
	*out++ = 0x01;
	uint32_t a = 1, b = 0;
	for (size_t done = 0; done < raw_size;) {
		const size_t n = raw_size - done < 65535 ? raw_size - done : 65535;
		*out++ = (uint8_t)(done + n == raw_size); /* BFINAL */
		*out++ = (uint8_t)n;
		*out++ = (uint8_t)(n >> 8);
		*out++ = (uint8_t)~n;
		*out++ = (uint8_t)(~n >> 8);
		for (size_t i = done; i < done + n; i++) {
			const size_t x = i % row;
			const uint8_t v = x ? rgba[(x - 1) % 4] : 0;
			*out++ = v;
			a = (a + v) % 65521;
			b = (b + a) % 65521;
		}
		done += n;
	}
	put_be32(out, (b << 16) | a);

	FILE *f = fopen(path, "wb");
	bool ok = f != NULL;
	static const uint8_t sig[8] = {0x89, 'P', 'N', 'G', '\r', '\n', 0x1a, '\n'};
	uint8_t ihdr[13] = {0};
	put_be32(ihdr, cx);
	put_be32(ihdr + 4, cy);
	ihdr[8] = 8; /* bit depth */
	ihdr[9] = 6; /* colour type: RGBA */
	ok = ok && fwrite(sig, 1, sizeof(sig), f) == sizeof(sig);
	ok = ok && write_chunk(f, "IHDR", ihdr, sizeof(ihdr));
	ok = ok && write_chunk(f, "IDAT", z, z_size);
	ok = ok && write_chunk(f, "IEND", NULL, 0);
	if (f)
		ok = fclose(f) == 0 && ok;
	free(z);
	return ok;
}

/* obs-rust/obs-studio#2279: a 1024x1024 picture counts its 4 MiB. The
 * picture is written to the working directory at run time. */
static void png_1024_mem_usage_test(void **state)
{
	static const char *path = "test_image_file_1024.png";
	static const uint8_t navy[4] = {30, 60, 90, 255};
	gs_image_file_ex_t image;

	(void)state;

	assert_true(write_solid_png(path, 1024, 1024, navy));
	gs_image_file_ex_init(&image, path, GS_IMAGE_ALPHA_PREMULTIPLY);
	remove(path);
	assert_true(image.loaded);
	assert_int_equal(image.cx, 1024);
	assert_int_equal(image.cy, 1024);
	assert_int_equal(image.format, GS_RGBA);
	assert_true(image.mem_usage >= 4 * 1024 * 1024);
	assert_int_equal(image.mem_usage, 1024 * 1024 * 4);

	gs_image_file_ex_init_texture(&image);
	/* still counted once the decoded copy is on the GPU */
	assert_int_equal(image.mem_usage, 1024 * 1024 * 4);
	gs_image_file_ex_free(&image);
	assert_true(is_zeroed(&image));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(null_file_test, setup, teardown),
		cmocka_unit_test_setup_teardown(missing_gif_test, setup, teardown),
		cmocka_unit_test_setup_teardown(missing_png_test, setup, teardown),
		cmocka_unit_test_setup_teardown(wide_gif_test, setup, teardown),
		cmocka_unit_test_setup_teardown(animated_gif_load_test, setup, teardown),
		cmocka_unit_test_setup_teardown(tick_test, setup, teardown),
		cmocka_unit_test_setup_teardown(skip_frames_test, setup, teardown),
		cmocka_unit_test_setup_teardown(loop_count_test, setup, teardown),
		cmocka_unit_test_setup_teardown(still_gif_test, setup, teardown),
		cmocka_unit_test_setup_teardown(png_test, setup, teardown),
		cmocka_unit_test_setup_teardown(png_1024_mem_usage_test, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
