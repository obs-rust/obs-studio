#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <cmocka.h>
#include <fontconfig/fontconfig.h>
#include <ft2build.h>
#include FT_FREETYPE_H
#include <graphics/vec3.h>

#include "text-freetype2.h"

/* Exercise the registered source callbacks, real file reader and real
 * FreeType rasterizer. Only external host/graphics library calls are
 * interposed; the plugin is compiled unchanged. The tiny outline font has
 * A and eacute, so the test does not depend on installed fonts or a display. */
static struct obs_source_info source_info;
static unsigned vertex_buffers_created;

void obs_register_source_s(const struct obs_source_info *info, size_t size)
{
	assert_int_equal(size, sizeof(*info));
	if (info->version == 2)
		source_info = *info;
}

char *obs_module_get_config_path(obs_module_t *module, const char *file)
{
	(void)module;
	(void)file;
	return NULL;
}

/* A video source's create callback requests a deferred host update. The
 * test delivers that update explicitly through the registered callback. */
void obs_source_update(obs_source_t *source, obs_data_t *settings)
{
	(void)source;
	(void)settings;
}

void obs_enter_graphics(void) {}
void obs_leave_graphics(void) {}

gs_texture_t *gs_texture_create(uint32_t width, uint32_t height, enum gs_color_format format, uint32_t levels,
				const uint8_t **data, uint32_t flags)
{
	assert_int_equal(format, GS_A8);
	(void)levels;
	(void)flags;
	return (gs_texture_t *)bmemdup(*data, (size_t)width * height);
}

void gs_texture_destroy(gs_texture_t *texture)
{
	bfree(texture);
}

gs_vertbuffer_t *gs_vertexbuffer_create(struct gs_vb_data *data, uint32_t flags)
{
	(void)flags;
	vertex_buffers_created++;
	return (gs_vertbuffer_t *)data;
}

void gs_vertexbuffer_destroy(gs_vertbuffer_t *buffer)
{
	gs_vbdata_destroy((struct gs_vb_data *)buffer);
}

struct gs_vb_data *gs_vertexbuffer_get_data(const gs_vertbuffer_t *buffer)
{
	return (struct gs_vb_data *)buffer;
}

void gs_vertexbuffer_flush(gs_vertbuffer_t *buffer)
{
	assert_non_null(buffer);
}

struct fixture {
	char directory[64];
	char path[128];
	obs_data_t *settings;
	struct ft2_source *source;
};

static void expect_glyph(struct fixture *test)
{
	FT_UInt index = FT_Get_Char_Index(test->source->font_face, 0xe9);
	assert_true(index > 0);
	struct glyph_info *glyph = test->source->cacheglyphs[index];
	assert_non_null(glyph);
	assert_true(glyph->w > 0 && glyph->h > 0);

	/* The currently uploaded atlas must still contain this glyph's pixels. */
	const uint8_t *pixels = (const uint8_t *)test->source->tex;
	assert_non_null(pixels);
	const uint32_t x = (uint32_t)(glyph->u * 2048);
	const uint32_t y = (uint32_t)(glyph->v * 2048);
	unsigned coverage = 0;
	for (int32_t row = 0; row < glyph->h; row++)
		for (int32_t col = 0; col < glyph->w; col++)
			coverage += pixels[(y + row) * 2048 + x + col];
	assert_true(coverage > 0);
}

static void expect_vertices(struct fixture *test)
{
	struct gs_vb_data *vertices = gs_vertexbuffer_get_data(test->source->vbuf);
	assert_non_null(vertices);
	assert_int_equal(vertices->num, 12);
	/* The second six vertices belong to eacute, beyond the standard atlas. */
	assert_true(vertices->points[7].x > vertices->points[6].x);
	assert_true(source_info.get_width(test->source) > 0);
	assert_true(source_info.get_height(test->source) > 0);
}

static int setup(void **state)
{
	struct fixture *test = bzalloc(sizeof(*test));
	*state = test;
	strcpy(test->directory, "test_text_aa_file.XXXXXX");
	assert_non_null(mkdtemp(test->directory));
	snprintf(test->path, sizeof(test->path), "%s/text.txt", test->directory);
	FILE *file = fopen(test->path, "wb");
	assert_non_null(file);
	const char text[] = "A\xc3\xa9";
	assert_int_equal(fwrite(text, 1, sizeof(text) - 1, file), sizeof(text) - 1);
	assert_int_equal(fclose(file), 0);

	test->settings = obs_data_create();
	source_info.get_defaults(test->settings);
	obs_data_t *font = obs_data_create();
	obs_data_set_string(font, "face", "OBS AA Fixture");
	obs_data_set_string(font, "style", "Regular");
	obs_data_set_int(font, "size", 32);
	obs_data_set_obj(test->settings, "font", font);
	obs_data_release(font);
	obs_data_set_bool(test->settings, "from_file", true);
	obs_data_set_string(test->settings, "text_file", test->path);
	vertex_buffers_created = 0;
	test->source = source_info.create(test->settings, NULL);
	assert_non_null(test->source);
	source_info.update(test->source, test->settings);
	assert_int_equal(vertex_buffers_created, 1);
	assert_int_equal(wcscmp(test->source->text, L"A\u00e9"), 0);
	expect_glyph(test);
	expect_vertices(test);
	return 0;
}

static int teardown(void **state)
{
	struct fixture *test = *state;
	source_info.destroy(test->source);
	obs_data_release(test->settings);
	assert_int_equal(remove(test->path), 0);
	assert_int_equal(rmdir(test->directory), 0);
	bfree(test);
	return 0;
}

static void unchanged_file_glyphs_survive_aa(void **state)
{
	struct fixture *test = *state;
	for (unsigned change = 0; change < 2; change++) {
		obs_data_set_bool(test->settings, "antialiasing", change != 0);
		source_info.update(test->source, test->settings);
		expect_glyph(test);
	}
}

static void unchanged_file_vertices_follow_aa(void **state)
{
	struct fixture *test = *state;
	for (unsigned change = 0; change < 2; change++) {
		/* Control: unchanged settings preserve the existing vertex buffer. */
		source_info.update(test->source, test->settings);
		assert_int_equal(vertex_buffers_created, change + 1);
		obs_data_set_bool(test->settings, "antialiasing", change != 0);
		source_info.update(test->source, test->settings);
		assert_int_equal(vertex_buffers_created, change + 2);
		expect_vertices(test);
	}
}

static int group_setup(void **state)
{
	(void)state;
	FcConfig *config = FcConfigCreate();
	assert_non_null(config);
	assert_true(FcConfigAppFontAddFile(config, (const FcChar8 *)TEST_FONT_PATH));
	assert_true(FcConfigSetCurrent(config));
	FcConfigDestroy(config);
	assert_true(obs_module_load());
	assert_non_null(source_info.update);
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	obs_module_unload();
	FcFini();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(unchanged_file_glyphs_survive_aa, setup, teardown),
		cmocka_unit_test_setup_teardown(unchanged_file_vertices_follow_aa, setup, teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
