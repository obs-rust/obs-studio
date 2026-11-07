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
#include <util/base.h>
#include <util/bmem.h>

/*
 * Characterization of the effect parser's compile step (#3792): what
 * ep_parse() puts in the gs_effect once the text parses, written out and
 * compared whole:
 *
 *   create <vertex|pixel> <n> <location>     each shader-create call, in order
 *     | <a line of the shader text>          (each tab as \t)
 *     -> shader <n>|NULL [error "<text>"]    what the stand-in answered
 *   result <0|1>                             what ep_parse() returned
 *   param <i> <name> section=<s> type=<t> changed=<0|1> cur=<bytes>
 *         default=<hex bytes> effect=<self|other> sampler=<null|set>
 *     annotation <j> ...                      the same fields
 *   view_proj=<i:name|-> world=<i:name|-> scale=<i:name|->
 *   technique <i> <name> section=<s> effect=<self|other> passes=<n>
 *     pass <j> <name> section=<s> vert=<n|NULL> pixel=<n|NULL>
 *       vparams <i:eparam name|->/<n:sparam name|NULL>, ...
 *       pparams ...
 *   error <row>:<column> <level> <text>
 *   log <level> <text>                        everything blog() wrote
 *   destroyed <n> ...                         effect_free()'s destroy calls
 *   leaked <allocations>
 *
 * ep_parse() and ep_compile() are hidden in libobs, so this test compiles the
 * original effect parser C into itself (test-graph.json lists the files) and
 * links libobs for the rest, as test_effect_parser_shader_call does.
 * test-graph.json renames gs_vertexshader_create(), gs_pixelshader_create(),
 * gs_shader_get_param_by_name() and gs_shader_destroy() in it to the
 * stand-ins below, so no graphics device is needed: a created shader is the
 * address of a slot in fake_shaders, a shader param the address of a slot in
 * fake_sparams, and each case says which create calls fail, which ones hand
 * back an error text and which param names a shader lacks. No display and no
 * graphics context, so it runs on every platform. gs_preprocessor_name() is
 * the no-context answer, NULL.
 *
 * Odd but real results are marked "characterized, not endorsed".
 */

const char *gs_preprocessor_name(void);
const char *gs_preprocessor_name(void)
{
	return NULL;
}

/* ------------------------------------------------------------------------ */
/* The written-out result. */

static char dump_buf[32768];
static size_t dump_len;

static void dump_cat(const char *fmt, ...)
{
	va_list args;
	int n;

	va_start(args, fmt);
	n = vsnprintf(dump_buf + dump_len, sizeof(dump_buf) - dump_len, fmt, args);
	va_end(args);
	if (n > 0)
		dump_len += (size_t)n;
	if (dump_len >= sizeof(dump_buf))
		dump_len = sizeof(dump_buf) - 1;
}

static const char *str_or_null(const char *s)
{
	return s ? s : "(null)";
}

static void dump_text(const char *text)
{
	bool line_start = true;

	if (!text) {
		dump_cat("    (null text)\n");
		return;
	}
	for (const char *c = text; *c; c++) {
		if (line_start) {
			dump_cat("    | ");
			line_start = false;
		}
		if (*c == '\n') {
			dump_cat("\n");
			line_start = true;
		} else if (*c == '\t') {
			dump_cat("\\t");
		} else {
			dump_cat("%c", *c);
		}
	}
	if (!line_start)
		dump_cat("\n    (no final newline)\n");
}

/* ------------------------------------------------------------------------ */
/* The graphics stand-ins. */

#define MAX_SHADERS 16
#define MAX_SPARAMS 64

struct compile_case {
	const char *text;
	/* Bit n: the n-th create call (counting vertex and pixel calls
	 * together, from 0) returns NULL. */
	unsigned null_mask;
	/* Bit n: the n-th create call sets *error_string to "fail <n>". */
	unsigned error_mask;
	/* Bit n: the n-th create call sets *error_string to "". */
	unsigned empty_error_mask;
	/* A param name that the shaders in missing_mask (bit n: the n-th
	 * create call's shader) don't have. */
	const char *missing;
	unsigned missing_mask;
	const char *expected;
};

static const struct compile_case *cur_case;
static char fake_shaders[MAX_SHADERS];
static size_t create_count;
static char fake_sparams[MAX_SPARAMS];
static char sparam_names[MAX_SPARAMS][64];
static size_t sparam_count;
static char destroyed[256];
static size_t destroyed_len;

static gs_shader_t *create_shader(const char *kind, const char *shader, const char *file, char **error_string)
{
	size_t n = create_count++;
	bool fail = n < 32 && (cur_case->null_mask & (1u << n));

	dump_cat("create %s %zu %s\n", kind, n, str_or_null(file));
	dump_text(shader);

	if (n < 32 && (cur_case->error_mask & (1u << n))) {
		char text[32];

		snprintf(text, sizeof(text), "fail %zu", n);
		*error_string = bstrdup(text);
	} else if (n < 32 && (cur_case->empty_error_mask & (1u << n))) {
		*error_string = bstrdup("");
	}

	if (fail || n >= MAX_SHADERS) {
		dump_cat("    -> NULL%s%s%s\n", *error_string ? " error \"" : "", *error_string ? *error_string : "",
			 *error_string ? "\"" : "");
		return NULL;
	}
	dump_cat("    -> shader %zu%s%s%s\n", n, *error_string ? " error \"" : "", *error_string ? *error_string : "",
		 *error_string ? "\"" : "");
	return (gs_shader_t *)(fake_shaders + n);
}

static long shader_id(const gs_shader_t *shader)
{
	const char *p = (const char *)shader;

	if (!shader)
		return -1;
	if (p < fake_shaders || p >= fake_shaders + MAX_SHADERS)
		return -2;
	return (long)(p - fake_shaders);
}

/* graphics.h declares these, as gs_vertexshader_create(),
 * gs_pixelshader_create(), gs_shader_get_param_by_name() and
 * gs_shader_destroy() under the renames. */
gs_shader_t *test_vertexshader_create(const char *shader, const char *file, char **error_string)
{
	return create_shader("vertex", shader, file, error_string);
}

gs_shader_t *test_pixelshader_create(const char *shader, const char *file, char **error_string)
{
	return create_shader("pixel", shader, file, error_string);
}

gs_sparam_t *test_shader_get_param_by_name(gs_shader_t *shader, const char *name)
{
	long id = shader_id(shader);

	if (id >= 0 && id < 32 && cur_case->missing && (cur_case->missing_mask & (1u << id)) &&
	    strcmp(name, cur_case->missing) == 0)
		return NULL;
	if (sparam_count >= MAX_SPARAMS)
		return NULL;

	snprintf(sparam_names[sparam_count], sizeof(sparam_names[sparam_count]), "%ld:%s", id, name);
	return (gs_sparam_t *)(fake_sparams + sparam_count++);
}

void test_shader_destroy(gs_shader_t *shader)
{
	if (!shader)
		return;
	destroyed_len += (size_t)snprintf(destroyed + destroyed_len, sizeof(destroyed) - destroyed_len, " %ld",
					  shader_id(shader));
	if (destroyed_len >= sizeof(destroyed))
		destroyed_len = sizeof(destroyed) - 1;
}

static const char *sparam_name(const gs_sparam_t *sparam)
{
	const char *p = (const char *)sparam;

	if (!sparam)
		return "NULL";
	if (p < fake_sparams || p >= fake_sparams + MAX_SPARAMS)
		return "(foreign)";
	return sparam_names[p - fake_sparams];
}

/* ------------------------------------------------------------------------ */
/* The log. */

static char log_buf[4096];
static size_t log_len;

static void log_handler(int lvl, const char *msg, va_list args, void *p)
{
	char line[512];

	(void)p;
	vsnprintf(line, sizeof(line), msg, args);
	log_len += (size_t)snprintf(log_buf + log_len, sizeof(log_buf) - log_len, "log %d %s\n", lvl, line);
	if (log_len >= sizeof(log_buf))
		log_len = sizeof(log_buf) - 1;
}

/* ------------------------------------------------------------------------ */
/* The effect. */

static void dump_param(const gs_effect_t *effect, const struct gs_effect_param *param, const char *indent,
		       const char *kind, size_t idx)
{
	dump_cat("%s%s %zu %s section=%d type=%d changed=%d cur=%zu default=", indent, kind, idx,
		 str_or_null(param->name), (int)param->section, (int)param->type, param->changed ? 1 : 0,
		 param->cur_val.num);
	for (size_t i = 0; i < param->default_val.num; i++)
		dump_cat("%02x", param->default_val.array[i]);
	dump_cat(" effect=%s sampler=%s\n", param->effect == effect ? "self" : "other",
		 param->next_sampler ? "set" : "null");
}

static void dump_shader_id(const gs_shader_t *shader)
{
	if (shader)
		dump_cat("%ld", shader_id(shader));
	else
		dump_cat("NULL");
}

static const char *eparam_name(const gs_effect_t *effect, const struct gs_effect_param *param)
{
	static char name[4][80];
	static int which;

	if (!param)
		return "-";
	which = (which + 1) % 4;
	for (size_t i = 0; i < effect->params.num; i++) {
		if (effect->params.array + i == param) {
			snprintf(name[which], sizeof(name[which]), "%zu:%s", i, str_or_null(param->name));
			return name[which];
		}
	}
	return "(foreign)";
}

static void dump_shaderparams(const gs_effect_t *effect, const char *label, const pass_shaderparam_array_t *params)
{
	dump_cat("      %s", label);
	for (size_t i = 0; i < params->num; i++)
		dump_cat("%s%s/%s", i ? ", " : " ", eparam_name(effect, params->array[i].eparam),
			 sparam_name(params->array[i].sparam));
	dump_cat("\n");
}

static void dump_effect(const gs_effect_t *effect)
{
	for (size_t i = 0; i < effect->params.num; i++) {
		const struct gs_effect_param *param = effect->params.array + i;

		dump_param(effect, param, "", "param", i);
		for (size_t j = 0; j < param->annotations.num; j++)
			dump_param(effect, param->annotations.array + j, "  ", "annotation", j);
	}

	dump_cat("view_proj=%s world=%s scale=%s\n", eparam_name(effect, effect->view_proj),
		 eparam_name(effect, effect->world), eparam_name(effect, effect->scale));

	for (size_t i = 0; i < effect->techniques.num; i++) {
		const struct gs_effect_technique *tech = effect->techniques.array + i;

		dump_cat("technique %zu %s section=%d effect=%s passes=%zu\n", i, str_or_null(tech->name),
			 (int)tech->section, tech->effect == effect ? "self" : "other", tech->passes.num);
		for (size_t j = 0; j < tech->passes.num; j++) {
			const struct gs_effect_pass *pass = tech->passes.array + j;

			dump_cat("  pass %zu %s section=%d vert=", j, str_or_null(pass->name), (int)pass->section);
			dump_shader_id(pass->vertshader);
			dump_cat(" pixel=");
			dump_shader_id(pass->pixelshader);
			dump_cat("\n");
			dump_shaderparams(effect, "vparams", &pass->vertshader_params);
			dump_shaderparams(effect, "pparams", &pass->pixelshader_params);
		}
	}
}

static void dump_errors(const struct effect_parser *ep)
{
	for (size_t i = 0; i < ep->cfp.error_list.errors.num; i++) {
		struct error_item *err = ep->cfp.error_list.errors.array + i;

		dump_cat("error %u:%u %d %s\n", (unsigned)err->row, (unsigned)err->column, err->level,
			 str_or_null(err->error));
	}
}

static void check_cases(const struct compile_case *cases, size_t count)
{
	log_handler_t old_handler;
	void *old_param;
	size_t failed = 0;

	base_get_log_handler(&old_handler, &old_param);

	for (size_t i = 0; i < count; i++) {
		struct effect_parser ep;
		struct gs_effect effect;
		long before;
		bool ok;

		cur_case = cases + i;
		create_count = 0;
		sparam_count = 0;
		destroyed_len = 0;
		destroyed[0] = 0;
		log_len = 0;
		log_buf[0] = 0;
		dump_len = 0;
		dump_buf[0] = 0;

		before = bnum_allocs();
		base_set_log_handler(log_handler, NULL);
		effect_init(&effect);
		ep_init(&ep);
		ok = ep_parse(&ep, &effect, cases[i].text, "test.effect");
		base_set_log_handler(old_handler, old_param);

		dump_cat("result %d\n", ok ? 1 : 0);
		dump_effect(&effect);
		dump_errors(&ep);
		dump_cat("%s", log_buf);
		ep_free(&ep);
		effect_free(&effect);
		dump_cat("destroyed%s\n", destroyed);
		dump_cat("leaked %ld\n", bnum_allocs() - before);

		if (strcmp(dump_buf, cases[i].expected) != 0) {
			fprintf(stderr, "case %zu: %s\n--- expected\n%s--- got\n%s", i, cases[i].text,
				cases[i].expected, dump_buf);
			failed++;
		}
	}

	if (failed) {
		fprintf(stderr, "%zu of %zu cases differ\n", failed, count);
		fail();
	}
}

#define CHECK_CASES(cases) check_cases(cases, sizeof(cases) / sizeof(cases[0]))

/* The shared shaders of most texts: a vertex function that uses ViewProj and
 * a pixel function that uses color. */
#define SOLID_FUNCS                                              \
	"float4 VSSolid(float4 pos : POSITION) : POSITION\n"     \
	"{\n"                                                    \
	"\treturn mul(pos, ViewProj);\n"                         \
	"}\n"                                                    \
	"float4 PSSolid(float4 pos : POSITION) : TARGET\n"       \
	"{\n"                                                    \
	"\treturn color;\n"                                      \
	"}\n"
#define SOLID_PASS "pass { vertex_shader = VSSolid(pos); pixel_shader = PSSolid(pos); }"

/* ------------------------------------------------------------------------ */
/* Params: each gets its name, section, type and the parser's default value
 * (moved out of the parser), its effect and its annotations; ViewProj and
 * World, matched by case, are kept as the effect's view_proj and world. */
static const struct compile_case params_cases[] = {
	/* every type the param types know, a default of each kind, annotations, and a lower-case viewproj
	 * that isn't the view matrix */
	{"uniform float4x4 ViewProj;\n"
	 "uniform float4x4 World;\n"
	 "uniform float4x4 viewproj;\n"
	 "uniform float4 color = {1.0, 0.5, 0.25, 1.0};\n"
	 "uniform float gain <string name = \"Gain\"; float minimum = 0.0; int step = 2;> = 1.5;\n"
	 "uniform float2 size;\n"
	 "uniform float3 axis;\n"
	 "uniform int count = 3;\n"
	 "uniform int2 i2;\n"
	 "uniform int3 i3;\n"
	 "uniform int4 i4;\n"
	 "uniform bool flag = true;\n"
	 "uniform texture2d image;\n"
	 "uniform texture_rect rect;\n"
	 "uniform string label = \"hi\";\n"
	 "uniform half odd;\n" SOLID_FUNCS "technique Solid { " SOLID_PASS " }",
	 0, 0, 0, NULL, 0,
	 "create vertex 0 test.effect (Vertex shader, technique Solid, pass 0)\n"
	 "    | uniform float4x4 ViewProj;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSSolid(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn mul(pos, ViewProj);\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSSolid(pos);\n"
	 "    | }\n"
	 "    -> shader 0\n"
	 "create pixel 1 test.effect (Pixel shader, technique Solid, pass 0)\n"
	 "    | uniform float4 color;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSSolid(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn color;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSSolid(pos);\n"
	 "    | }\n"
	 "    -> shader 1\n"
	 "result 1\n"
	 "param 0 ViewProj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 1 World section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 2 viewproj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 3 color section=0 type=7 changed=0 cur=0 default=0000803f0000003f0000803e0000803f effect=self sampler=null\n"
	 "param 4 gain section=0 type=2 changed=0 cur=0 default=0000c03f effect=self sampler=null\n"
	 "  annotation 0 name section=4 type=4 changed=0 cur=0 default=4761696e00 effect=self sampler=null\n"
	 "  annotation 1 minimum section=4 type=2 changed=0 cur=0 default=00000000 effect=self sampler=null\n"
	 "  annotation 2 step section=4 type=3 changed=0 cur=0 default=02000000 effect=self sampler=null\n"
	 "param 5 size section=0 type=5 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 6 axis section=0 type=6 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 7 count section=0 type=3 changed=0 cur=0 default=03000000 effect=self sampler=null\n"
	 "param 8 i2 section=0 type=8 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 9 i3 section=0 type=9 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 10 i4 section=0 type=10 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 11 flag section=0 type=1 changed=0 cur=0 default=01000000 effect=self sampler=null\n"
	 "param 12 image section=0 type=12 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 13 rect section=0 type=12 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 14 label section=0 type=4 changed=0 cur=0 default=686900 effect=self sampler=null\n"
	 "param 15 odd section=0 type=0 changed=0 cur=0 default= effect=self sampler=null\n"
	 "view_proj=0:ViewProj world=1:World scale=-\n"
	 "technique 0 Solid section=1 effect=self passes=1\n"
	 "  pass 0 (null) section=3 vert=0 pixel=1\n"
	 "      vparams 0:ViewProj/0:ViewProj\n"
	 "      pparams 3:color/1:color\n"
	 "destroyed 0 1\n"
	 "leaked 0\n"},
	/* ViewProj and World declared twice: the last of each is kept (characterized, not endorsed: the writer
	 * and the lookups use the first) */
	{"uniform float4x4 ViewProj;\n"
	 "uniform float4x4 World;\n"
	 "uniform float4x4 ViewProj;\n"
	 "uniform float4x4 World;\n"
	 "uniform float4 color;\n" SOLID_FUNCS "technique Solid { " SOLID_PASS " }",
	 0, 0, 0, NULL, 0,
	 "create vertex 0 test.effect (Vertex shader, technique Solid, pass 0)\n"
	 "    | uniform float4x4 ViewProj;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSSolid(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn mul(pos, ViewProj);\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSSolid(pos);\n"
	 "    | }\n"
	 "    -> shader 0\n"
	 "create pixel 1 test.effect (Pixel shader, technique Solid, pass 0)\n"
	 "    | uniform float4 color;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSSolid(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn color;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSSolid(pos);\n"
	 "    | }\n"
	 "    -> shader 1\n"
	 "result 1\n"
	 "param 0 ViewProj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 1 World section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 2 ViewProj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 3 World section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 4 color section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "view_proj=2:ViewProj world=3:World scale=-\n"
	 "technique 0 Solid section=1 effect=self passes=1\n"
	 "  pass 0 (null) section=3 vert=0 pixel=1\n"
	 "      vparams 0:ViewProj/0:ViewProj\n"
	 "      pparams 4:color/1:color\n"
	 "destroyed 0 1\n"
	 "leaked 0\n"},
	/* params and no technique */
	{"uniform float4x4 ViewProj;\n"
	 "uniform float4 color;\n",
	 0, 0, 0, NULL, 0,
	 "result 1\n"
	 "param 0 ViewProj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 1 color section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "view_proj=0:ViewProj world=- scale=-\n"
	 "destroyed\n"
	 "leaked 0\n"},
	/* nothing at all */
	{"", 0, 0, 0, NULL, 0,
	 "result 0\n"
	 "view_proj=- world=- scale=-\n"
	 "destroyed\n"
	 "leaked 0\n"},
	/* a parse error: nothing is compiled */
	{"uniform float4x4 ViewProj;\n"
	 "uniform float4 color;\n" SOLID_FUNCS "technique Solid { " SOLID_PASS " }\n"
	 "{ }\n",
	 0, 0, 0, NULL, 0,
	 "result 0\n"
	 "view_proj=- world=- scale=-\n"
	 "error 12:1 0 Unexpected code segment\n"
	 "destroyed\n"
	 "leaked 0\n"},
};

static void params(void **state)
{
	(void)state;
	CHECK_CASES(params_cases);
}

/* ------------------------------------------------------------------------ */
/* Techniques and passes: names, sections, a shader-create call for each
 * pass's vertex and then pixel call, with its location, and the effect's and
 * shader's param for each name the writer used. */
static const struct compile_case techniques_cases[] = {
	/* two techniques, the first with a named and an unnamed pass, the second with none */
	{"uniform float4x4 ViewProj;\n"
	 "uniform float4 color;\n" SOLID_FUNCS "technique Draw { pass first { vertex_shader = VSSolid(pos); "
	 "pixel_shader = PSSolid(pos); } " SOLID_PASS " }\n"
	 "technique Empty { }\n",
	 0, 0, 0, NULL, 0,
	 "create vertex 0 test.effect (Vertex shader, technique Draw, pass 0)\n"
	 "    | uniform float4x4 ViewProj;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSSolid(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn mul(pos, ViewProj);\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSSolid(pos);\n"
	 "    | }\n"
	 "    -> shader 0\n"
	 "create pixel 1 test.effect (Pixel shader, technique Draw, pass 0)\n"
	 "    | uniform float4 color;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSSolid(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn color;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSSolid(pos);\n"
	 "    | }\n"
	 "    -> shader 1\n"
	 "create vertex 2 test.effect (Vertex shader, technique Draw, pass 1)\n"
	 "    | uniform float4x4 ViewProj;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSSolid(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn mul(pos, ViewProj);\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSSolid(pos);\n"
	 "    | }\n"
	 "    -> shader 2\n"
	 "create pixel 3 test.effect (Pixel shader, technique Draw, pass 1)\n"
	 "    | uniform float4 color;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSSolid(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn color;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSSolid(pos);\n"
	 "    | }\n"
	 "    -> shader 3\n"
	 "result 1\n"
	 "param 0 ViewProj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 1 color section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "view_proj=0:ViewProj world=- scale=-\n"
	 "technique 0 Draw section=1 effect=self passes=2\n"
	 "  pass 0 first section=3 vert=0 pixel=1\n"
	 "      vparams 0:ViewProj/0:ViewProj\n"
	 "      pparams 1:color/1:color\n"
	 "  pass 1 (null) section=3 vert=2 pixel=3\n"
	 "      vparams 0:ViewProj/2:ViewProj\n"
	 "      pparams 1:color/3:color\n"
	 "technique 1 Empty section=1 effect=self passes=0\n"
	 "destroyed 0 1 2 3\n"
	 "leaked 0\n"},
	/* a NULL vertex call: the create call gets no text */
	{"uniform float4x4 ViewProj;\n"
	 "uniform float4 color;\n" SOLID_FUNCS
	 "technique Solid { pass { vertex_shader = NULL; pixel_shader = PSSolid(pos); } }",
	 0, 0, 0, NULL, 0,
	 "create vertex 0 test.effect (Vertex shader, technique Solid, pass 0)\n"
	 "    (null text)\n"
	 "    -> shader 0\n"
	 "create pixel 1 test.effect (Pixel shader, technique Solid, pass 0)\n"
	 "    | uniform float4 color;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSSolid(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn color;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSSolid(pos);\n"
	 "    | }\n"
	 "    -> shader 1\n"
	 "result 1\n"
	 "param 0 ViewProj section=0 type=11 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 1 color section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "view_proj=0:ViewProj world=- scale=-\n"
	 "technique 0 Solid section=1 effect=self passes=1\n"
	 "  pass 0 (null) section=3 vert=0 pixel=1\n"
	 "      vparams\n"
	 "      pparams 1:color/1:color\n"
	 "destroyed 0 1\n"
	 "leaked 0\n"},
	/* a pass with no calls at all */
	{"uniform float4 color;\n" SOLID_FUNCS "technique Solid { pass { } }", 0, 0, 0, NULL, 0,
	 "create vertex 0 test.effect (Vertex shader, technique Solid, pass 0)\n"
	 "    (null text)\n"
	 "    -> shader 0\n"
	 "create pixel 1 test.effect (Pixel shader, technique Solid, pass 0)\n"
	 "    (null text)\n"
	 "    -> shader 1\n"
	 "result 1\n"
	 "param 0 color section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "view_proj=- world=- scale=-\n"
	 "technique 0 Solid section=1 effect=self passes=1\n"
	 "  pass 0 (null) section=3 vert=0 pixel=1\n"
	 "      vparams\n"
	 "      pparams\n"
	 "destroyed 0 1\n"
	 "leaked 0\n"},
	/* a function using three uniforms, two of them named twice: each name once, in the writer's order */
	{"uniform float4 a;\n"
	 "uniform float4 b;\n"
	 "uniform float4 c;\n"
	 "float4 VSAbc(float4 pos : POSITION) : POSITION\n"
	 "{\n"
	 "\treturn pos * c + b * a + c * b;\n"
	 "}\n"
	 "technique Abc { pass { vertex_shader = VSAbc(pos); pixel_shader = VSAbc(pos); } }",
	 0, 0, 0, NULL, 0,
	 "create vertex 0 test.effect (Vertex shader, technique Abc, pass 0)\n"
	 "    | uniform float4 c;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 a;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * c + b * a + c * b;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 0\n"
	 "create pixel 1 test.effect (Pixel shader, technique Abc, pass 0)\n"
	 "    | uniform float4 c;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 a;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * c + b * a + c * b;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 1\n"
	 "result 1\n"
	 "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "view_proj=- world=- scale=-\n"
	 "technique 0 Abc section=1 effect=self passes=1\n"
	 "  pass 0 (null) section=3 vert=0 pixel=1\n"
	 "      vparams 2:c/0:c, 1:b/0:b, 0:a/0:a\n"
	 "      pparams 2:c/1:c, 1:b/1:b, 0:a/1:a\n"
	 "destroyed 0 1\n"
	 "leaked 0\n"},
};

static void techniques(void **state)
{
	(void)state;
	CHECK_CASES(techniques_cases);
}

/* ------------------------------------------------------------------------ */
/* Shaders that fail: a NULL shader, an error text with or without a shader,
 * and a shader that lacks a param the writer used. The pass keeps what it
 * got, for effect_free() to free. */
#define ABC_TEXT                                                  \
	"uniform float4 a;\n"                                     \
	"uniform float4 b;\n"                                     \
	"uniform float4 c;\n"                                     \
	"float4 VSAbc(float4 pos : POSITION) : POSITION\n"        \
	"{\n"                                                     \
	"\treturn pos * a * b * c;\n"                             \
	"}\n"                                                     \
	"float4 PSAbc(float4 pos : POSITION) : TARGET\n"          \
	"{\n"                                                     \
	"\treturn a + b + c;\n"                                   \
	"}\n"                                                     \
	"technique One { pass p0 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } "  \
	"pass p1 { vertex_shader = VSAbc(pos); pixel_shader = PSAbc(pos); } }\n"

static const struct compile_case shaders_cases[] = {
	/* the first vertex shader fails: the pass still creates its pixel shader, and so does the next pass */
	{ABC_TEXT, 1u << 0, 0, 0, NULL, 0,
	 "create vertex 0 test.effect (Vertex shader, technique One, pass 0)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * a * b * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> NULL\n"
	 "create pixel 1 test.effect (Pixel shader, technique One, pass 0)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn a + b + c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 1\n"
	 "create vertex 2 test.effect (Vertex shader, technique One, pass 1)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * a * b * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 2\n"
	 "create pixel 3 test.effect (Pixel shader, technique One, pass 1)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn a + b + c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 3\n"
	 "result 0\n"
	 "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "view_proj=- world=- scale=-\n"
	 "technique 0 One section=1 effect=self passes=2\n"
	 "  pass 0 p0 section=3 vert=NULL pixel=1\n"
	 "      vparams\n"
	 "      pparams 0:a/1:a, 1:b/1:b, 2:c/1:c\n"
	 "  pass 1 p1 section=3 vert=2 pixel=3\n"
	 "      vparams 0:a/2:a, 1:b/2:b, 2:c/2:c\n"
	 "      pparams 0:a/3:a, 1:b/3:b, 2:c/3:c\n"
	 "log 100 Pass (0) <p0> missing vertex shader!\n"
	 "destroyed 1 2 3\n"
	 "leaked 0\n"},
	/* the first pixel shader fails with an error text, which goes into the error list */
	{ABC_TEXT, 1u << 1, 1u << 1, 0, NULL, 0,
	 "create vertex 0 test.effect (Vertex shader, technique One, pass 0)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * a * b * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 0\n"
	 "create pixel 1 test.effect (Pixel shader, technique One, pass 0)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn a + b + c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAbc(pos);\n"
	 "    | }\n"
	 "    -> NULL error \"fail 1\"\n"
	 "create vertex 2 test.effect (Vertex shader, technique One, pass 1)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * a * b * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 2\n"
	 "create pixel 3 test.effect (Pixel shader, technique One, pass 1)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn a + b + c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 3\n"
	 "result 0\n"
	 "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "view_proj=- world=- scale=-\n"
	 "technique 0 One section=1 effect=self passes=2\n"
	 "  pass 0 p0 section=3 vert=0 pixel=NULL\n"
	 "      vparams 0:a/0:a, 1:b/0:b, 2:c/0:c\n"
	 "      pparams\n"
	 "  pass 1 p1 section=3 vert=2 pixel=3\n"
	 "      vparams 0:a/2:a, 1:b/2:b, 2:c/2:c\n"
	 "      pparams 0:a/3:a, 1:b/3:b, 2:c/3:c\n"
	 "error 13:1 0 Error creating shader: fail 1\n"
	 "log 100 Pass (0) <p0> missing pixel shader!\n"
	 "destroyed 0 2 3\n"
	 "leaked 0\n"},
	/* a shader with an error text (a warning) is kept, and the result is still 1 (characterized, not
	 * endorsed: the error list has an error) */
	{ABC_TEXT, 0, 1u << 2, 0, NULL, 0,
	 "create vertex 0 test.effect (Vertex shader, technique One, pass 0)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * a * b * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 0\n"
	 "create pixel 1 test.effect (Pixel shader, technique One, pass 0)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn a + b + c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 1\n"
	 "create vertex 2 test.effect (Vertex shader, technique One, pass 1)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * a * b * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 2 error \"fail 2\"\n"
	 "create pixel 3 test.effect (Pixel shader, technique One, pass 1)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn a + b + c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 3\n"
	 "result 1\n"
	 "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "view_proj=- world=- scale=-\n"
	 "technique 0 One section=1 effect=self passes=2\n"
	 "  pass 0 p0 section=3 vert=0 pixel=1\n"
	 "      vparams 0:a/0:a, 1:b/0:b, 2:c/0:c\n"
	 "      pparams 0:a/1:a, 1:b/1:b, 2:c/1:c\n"
	 "  pass 1 p1 section=3 vert=2 pixel=3\n"
	 "      vparams 0:a/2:a, 1:b/2:b, 2:c/2:c\n"
	 "      pparams 0:a/3:a, 1:b/3:b, 2:c/3:c\n"
	 "error 13:1 0 Error creating shader: fail 2\n"
	 "destroyed 0 1 2 3\n"
	 "leaked 0\n"},
	/* an empty error text adds nothing */
	{ABC_TEXT, 0, 0, (1u << 0) | (1u << 3), NULL, 0,
	 "create vertex 0 test.effect (Vertex shader, technique One, pass 0)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * a * b * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 0 error \"\"\n"
	 "create pixel 1 test.effect (Pixel shader, technique One, pass 0)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn a + b + c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 1\n"
	 "create vertex 2 test.effect (Vertex shader, technique One, pass 1)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * a * b * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 2\n"
	 "create pixel 3 test.effect (Pixel shader, technique One, pass 1)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn a + b + c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 3 error \"\"\n"
	 "result 1\n"
	 "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "view_proj=- world=- scale=-\n"
	 "technique 0 One section=1 effect=self passes=2\n"
	 "  pass 0 p0 section=3 vert=0 pixel=1\n"
	 "      vparams 0:a/0:a, 1:b/0:b, 2:c/0:c\n"
	 "      pparams 0:a/1:a, 1:b/1:b, 2:c/1:c\n"
	 "  pass 1 p1 section=3 vert=2 pixel=3\n"
	 "      vparams 0:a/2:a, 1:b/2:b, 2:c/2:c\n"
	 "      pparams 0:a/3:a, 1:b/3:b, 2:c/3:c\n"
	 "destroyed 0 1 2 3\n"
	 "leaked 0\n"},
	/* the first vertex shader lacks b: its params stop there, b kept with no shader param and c left
	 * empty */
	{ABC_TEXT, 0, 0, 0, "b", 1u << 0,
	 "create vertex 0 test.effect (Vertex shader, technique One, pass 0)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * a * b * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 0\n"
	 "create pixel 1 test.effect (Pixel shader, technique One, pass 0)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn a + b + c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 1\n"
	 "create vertex 2 test.effect (Vertex shader, technique One, pass 1)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * a * b * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 2\n"
	 "create pixel 3 test.effect (Pixel shader, technique One, pass 1)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn a + b + c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 3\n"
	 "result 0\n"
	 "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "view_proj=- world=- scale=-\n"
	 "technique 0 One section=1 effect=self passes=2\n"
	 "  pass 0 p0 section=3 vert=0 pixel=1\n"
	 "      vparams 0:a/0:a, 1:b/NULL, -/NULL\n"
	 "      pparams 0:a/1:a, 1:b/1:b, 2:c/1:c\n"
	 "  pass 1 p1 section=3 vert=2 pixel=3\n"
	 "      vparams 0:a/2:a, 1:b/2:b, 2:c/2:c\n"
	 "      pparams 0:a/3:a, 1:b/3:b, 2:c/3:c\n"
	 "log 100 Effect shader parameter not found\n"
	 "log 100 Pass (0) <p0> missing vertex shader!\n"
	 "destroyed 0 1 2 3\n"
	 "leaked 0\n"},
	/* the last pixel shader lacks a, its first param */
	{ABC_TEXT, 0, 0, 0, "a", 1u << 3,
	 "create vertex 0 test.effect (Vertex shader, technique One, pass 0)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * a * b * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 0\n"
	 "create pixel 1 test.effect (Pixel shader, technique One, pass 0)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn a + b + c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 1\n"
	 "create vertex 2 test.effect (Vertex shader, technique One, pass 1)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * a * b * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 2\n"
	 "create pixel 3 test.effect (Pixel shader, technique One, pass 1)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn a + b + c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAbc(pos);\n"
	 "    | }\n"
	 "    -> shader 3\n"
	 "result 0\n"
	 "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "view_proj=- world=- scale=-\n"
	 "technique 0 One section=1 effect=self passes=2\n"
	 "  pass 0 p0 section=3 vert=0 pixel=1\n"
	 "      vparams 0:a/0:a, 1:b/0:b, 2:c/0:c\n"
	 "      pparams 0:a/1:a, 1:b/1:b, 2:c/1:c\n"
	 "  pass 1 p1 section=3 vert=2 pixel=3\n"
	 "      vparams 0:a/2:a, 1:b/2:b, 2:c/2:c\n"
	 "      pparams 0:a/NULL, -/NULL, -/NULL\n"
	 "log 100 Effect shader parameter not found\n"
	 "log 100 Pass (1) <p1> missing pixel shader!\n"
	 "destroyed 0 1 2 3\n"
	 "leaked 0\n"},
	/* every shader fails */
	{ABC_TEXT, 0xf, 0x5, 0, NULL, 0,
	 "create vertex 0 test.effect (Vertex shader, technique One, pass 0)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * a * b * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> NULL error \"fail 0\"\n"
	 "create pixel 1 test.effect (Pixel shader, technique One, pass 0)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn a + b + c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAbc(pos);\n"
	 "    | }\n"
	 "    -> NULL\n"
	 "create vertex 2 test.effect (Vertex shader, technique One, pass 1)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos * a * b * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSAbc(pos);\n"
	 "    | }\n"
	 "    -> NULL error \"fail 2\"\n"
	 "create pixel 3 test.effect (Pixel shader, technique One, pass 1)\n"
	 "    | uniform float4 a;\n"
	 "    | uniform float4 b;\n"
	 "    | uniform float4 c;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAbc(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn a + b + c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAbc(pos);\n"
	 "    | }\n"
	 "    -> NULL\n"
	 "result 0\n"
	 "param 0 a section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 1 b section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "param 2 c section=0 type=7 changed=0 cur=0 default= effect=self sampler=null\n"
	 "view_proj=- world=- scale=-\n"
	 "technique 0 One section=1 effect=self passes=2\n"
	 "  pass 0 p0 section=3 vert=NULL pixel=NULL\n"
	 "      vparams\n"
	 "      pparams\n"
	 "  pass 1 p1 section=3 vert=NULL pixel=NULL\n"
	 "      vparams\n"
	 "      pparams\n"
	 "error 13:1 0 Error creating shader: fail 0\n"
	 "error 13:1 0 Error creating shader: fail 2\n"
	 "log 100 Pass (0) <p0> missing vertex shader!\n"
	 "log 100 Pass (0) <p0> missing pixel shader!\n"
	 "log 100 Pass (1) <p1> missing vertex shader!\n"
	 "log 100 Pass (1) <p1> missing pixel shader!\n"
	 "destroyed\n"
	 "leaked 0\n"},
};

static void shaders(void **state)
{
	(void)state;
	CHECK_CASES(shaders_cases);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(params),
		cmocka_unit_test(techniques),
		cmocka_unit_test(shaders),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
