#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <graphics/effect-parser.h>
#include <graphics/effect.h>
#include <util/bmem.h>

/*
 * A pass whose shader call names a function the effect doesn't define
 * (#3625). ep_makeshaderstring() writes the declarations of the params the
 * call names, marks them written, then finds no function. It must still
 * free the call string and reset the written flags, as it does after a call
 * that succeeds, so the pass's next shader declares those params again and
 * nothing leaks.
 *
 * This test compiles the original effect parser C into itself (test-graph.json
 * lists the files), and test-graph.json renames gs_vertexshader_create() and
 * gs_pixelshader_create() in it to the recorders below. They keep the shader
 * text the parser built and return NULL, so ep_parse() runs ep_compile()
 * with no graphics device: each pass then reports its missing shaders, which
 * is the expected result here. No display and no graphics context, so it
 * runs on every platform. gs_preprocessor_name() is the no-context answer,
 * NULL.
 */

const char *gs_preprocessor_name(void);
const char *gs_preprocessor_name(void)
{
	return NULL;
}

#define MAX_SHADERS 4

static char vertex_text[MAX_SHADERS][1024];
static char pixel_text[MAX_SHADERS][1024];
static size_t vertex_count;
static size_t pixel_count;

static void record(char (*texts)[1024], size_t *count, const char *shader)
{
	if (*count < MAX_SHADERS)
		snprintf(texts[*count], sizeof(texts[*count]), "%s", shader ? shader : "(null)");
	(*count)++;
}

/* graphics.h declares these, as gs_vertexshader_create() and
 * gs_pixelshader_create() under the rename. */
gs_shader_t *test_vertexshader_create(const char *shader, const char *file, char **error_string)
{
	(void)file;
	(void)error_string;
	record(vertex_text, &vertex_count, shader);
	return NULL;
}

gs_shader_t *test_pixelshader_create(const char *shader, const char *file, char **error_string)
{
	(void)file;
	(void)error_string;
	record(pixel_text, &pixel_count, shader);
	return NULL;
}

#define EFFECT_HEAD                                       \
	"uniform float4x4 ViewProj;\n"                    \
	"uniform float4 color;\n"                         \
	"\n"                                              \
	"float4 VSDefault(float4 pos : POSITION) : POSITION\n" \
	"{\n"                                             \
	"\treturn mul(pos, ViewProj);\n"                  \
	"}\n"                                             \
	"\n"                                              \
	"float4 PSSolid(float4 pos : POSITION) : TARGET\n" \
	"{\n"                                             \
	"\treturn color;\n"                               \
	"}\n"                                             \
	"\n"

/* Parses and compiles text, and returns the allocations still held after
 * ep_free() and effect_free(), which together are gs_effect_destroy()'s
 * path. */
static long parse_and_free(const char *text)
{
	struct effect_parser ep;
	struct gs_effect effect;
	long before = bnum_allocs();
	bool ok;

	vertex_count = 0;
	pixel_count = 0;

	effect_init(&effect);
	ep_init(&ep);
	ok = ep_parse(&ep, &effect, text, "test_effect_parser_shader_call");
	assert_false(ok);
	ep_free(&ep);
	effect_free(&effect);

	assert_int_equal(vertex_count, 1);
	assert_int_equal(pixel_count, 1);
	return bnum_allocs() - before;
}

/* The control: a pass whose vertex shader is NULL builds nothing for it. */
static void test_null_vertex_call(void **state)
{
	(void)state;

	assert_int_equal(parse_and_free(EFFECT_HEAD "technique Solid\n"
						    "{\n"
						    "\tpass\n"
						    "\t{\n"
						    "\t\tvertex_shader = NULL;\n"
						    "\t\tpixel_shader  = PSSolid(pos);\n"
						    "\t}\n"
						    "}\n"),
			 0);

	/* The empty dstr's array: the create call gets NULL. */
	assert_string_equal(vertex_text[0], "(null)");
	assert_string_equal(pixel_text[0], "uniform float4 color;\n"
					   "\n"
					   "\n"
					   "float4 PSSolid(float4 pos : POSITION)\n"
					   "{\n"
					   "\treturn color;\n"
					   "}\n"
					   "\n"
					   "float4 main(float4 pos : POSITION) : TARGET\n"
					   "{\n"
					   "\treturn PSSolid(pos);\n"
					   "}\n");
}

/* A typo in the vertex call. */
static const char unknown_vertex_call[] = EFFECT_HEAD "technique Solid\n"
						      "{\n"
						      "\tpass\n"
						      "\t{\n"
						      "\t\tvertex_shader = VSDefualt(color);\n"
						      "\t\tpixel_shader  = PSSolid(pos);\n"
						      "\t}\n"
						      "}\n";

/* The vertex shader holds only the param the call named (it fails to
 * compile, as it should). The pixel shader must be the same as in the
 * control: it declares color again. */
static void test_unknown_vertex_call_resets_written(void **state)
{
	char expected_pixel[1024];

	/* The control's pixel shader, kept before the next parse overwrites it. */
	test_null_vertex_call(state);
	snprintf(expected_pixel, sizeof(expected_pixel), "%s", pixel_text[0]);

	parse_and_free(unknown_vertex_call);

	assert_string_equal(vertex_text[0], "uniform float4 color;\n");
	assert_string_equal(pixel_text[0], expected_pixel);
}

/* The call string is freed: nothing is left once the effect is freed. */
static void test_unknown_vertex_call_frees(void **state)
{
	(void)state;

	assert_int_equal(parse_and_free(unknown_vertex_call), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_null_vertex_call),
		cmocka_unit_test(test_unknown_vertex_call_resets_written),
		cmocka_unit_test(test_unknown_vertex_call_frees),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
