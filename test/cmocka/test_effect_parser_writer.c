#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <graphics/effect-parser.h>
#include <graphics/effect-parser-writer.h>
#include <util/bmem.h>
#include <util/dstr.h>

/*
 * Characterization of the effect parser's shader writer (#3765):
 * ep_makeshaderstring() on each pass's vertex and pixel call of a parsed
 * effect, written out and compared whole:
 *
 *   <technique>/<pass> <vertex|pixel>
 *     | <a line of the shader text>
 *     (no final newline)
 *     used <names>
 *     written param=<names> struct=<names> func=<names> sampler=<names>
 *   error <row>:<column> <level> <text>
 *   leaked <allocations>
 *
 * Each call gets an empty shader string and an empty used_params list. The
 * shader text is written a line at a time behind "| ", each tab as \t, with
 * "(no final newline)" when it doesn't end in one. Then the names pushed into
 * used_params, in order, and the params, structs, functions and samplers whose
 * written flag is set after the call. Then every parse error after the guard
 * (level 1 is a warning) and the allocations still held once the shader
 * strings, the used_params lists and the parser are freed. They pin the
 * current C behaviour; cases marked "characterized, not endorsed" are odd but
 * real.
 *
 * ep_parse() and ep_makeshaderstring() are hidden in libobs, so this test
 * compiles the original effect parser C into itself (test-graph.json lists
 * the files) and links libobs for the rest. Every text is parsed behind a
 * first line of "{}", which ep_parse() reports as "Unexpected code segment":
 * with an error in the list it never calls ep_compile(), which needs a
 * device, and the test calls the writer itself on the passes ep_parse() kept.
 * No display and no graphics context, so it runs on every platform:
 * gs_preprocessor_name() is the no-context answer, NULL.
 */

const char *gs_preprocessor_name(void);
const char *gs_preprocessor_name(void)
{
	return NULL;
}

struct writer_case {
	const char *text;
	const char *expected;
};

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

static void dump_shader(const struct dstr *shader)
{
	size_t len = shader->array ? shader->len : 0;
	bool line_start = true;

	for (size_t i = 0; i < len; i++) {
		char c = shader->array[i];

		if (line_start) {
			dump_cat("    | ");
			line_start = false;
		}
		if (c == '\n') {
			dump_cat("\n");
			line_start = true;
		} else if (c == '\t') {
			dump_cat("\\t");
		} else {
			dump_cat("%c", c);
		}
	}
	if (!line_start)
		dump_cat("\n    (no final newline)\n");
}

static void dump_written(const struct effect_parser *ep)
{
	size_t n;

	dump_cat("    written param=");
	n = 0;
	for (size_t i = 0; i < ep->params.num; i++)
		if (ep->params.array[i].written)
			dump_cat(n++ ? ",%s" : "%s", str_or_null(ep->params.array[i].name));
	dump_cat(" struct=");
	n = 0;
	for (size_t i = 0; i < ep->structs.num; i++)
		if (ep->structs.array[i].written)
			dump_cat(n++ ? ",%s" : "%s", str_or_null(ep->structs.array[i].name));
	dump_cat(" func=");
	n = 0;
	for (size_t i = 0; i < ep->funcs.num; i++)
		if (ep->funcs.array[i].written)
			dump_cat(n++ ? ",%s" : "%s", str_or_null(ep->funcs.array[i].name));
	dump_cat(" sampler=");
	n = 0;
	for (size_t i = 0; i < ep->samplers.num; i++)
		if (ep->samplers.array[i].written)
			dump_cat(n++ ? ",%s" : "%s", str_or_null(ep->samplers.array[i].name));
	dump_cat("\n");
}

static void set_all_written(struct effect_parser *ep)
{
	for (size_t i = 0; i < ep->params.num; i++)
		ep->params.array[i].written = true;
	for (size_t i = 0; i < ep->structs.num; i++)
		ep->structs.array[i].written = true;
	for (size_t i = 0; i < ep->funcs.num; i++)
		ep->funcs.array[i].written = true;
	for (size_t i = 0; i < ep->samplers.num; i++)
		ep->samplers.array[i].written = true;
}

static void write_call(struct effect_parser *ep, const char *label, cf_token_array_t *call, bool preset)
{
	struct dstr shader;
	dstr_array_t used_params;

	dstr_init(&shader);
	da_init(used_params);

	if (preset)
		set_all_written(ep);

	ep_makeshaderstring(ep, &shader, call, &used_params);

	dump_cat("  %s\n", label);
	dump_shader(&shader);
	dump_cat("    used ");
	for (size_t i = 0; i < used_params.num; i++)
		dump_cat(i ? ",%s" : "%s", str_or_null(used_params.array[i].array));
	dump_cat("\n");
	dump_written(ep);

	for (size_t i = 0; i < used_params.num; i++)
		dstr_free(used_params.array + i);
	da_free(used_params);
	dstr_free(&shader);
}

static void write_passes(struct effect_parser *ep, bool preset)
{
	for (size_t t = 0; t < ep->techniques.num; t++) {
		struct ep_technique *tech = ep->techniques.array + t;

		for (size_t p = 0; p < tech->passes.num; p++) {
			struct ep_pass *pass = tech->passes.array + p;

			dump_cat("%s/%zu\n", str_or_null(tech->name), p);
			write_call(ep, "vertex", &pass->vertex_program, preset);
			write_call(ep, "pixel", &pass->fragment_program, preset);
		}
	}
}

/* The first error is the guard's "Unexpected code segment" on row 1. */
static void dump_errors(const struct effect_parser *ep)
{
	for (size_t i = 1; i < ep->cfp.error_list.errors.num; i++) {
		struct error_item *err = ep->cfp.error_list.errors.array + i;

		dump_cat("error %u:%u %d %s\n", (unsigned)err->row, (unsigned)err->column, err->level,
			 str_or_null(err->error));
	}
}

static void check_cases(const struct writer_case *cases, size_t count, bool preset)
{
	size_t failed = 0;
	char text[4096];

	for (size_t i = 0; i < count; i++) {
		struct effect_parser ep;
		long before;
		bool ok;

		snprintf(text, sizeof(text), "{}\n%s", cases[i].text);

		dump_len = 0;
		dump_buf[0] = 0;

		before = bnum_allocs();
		ep_init(&ep);
		ok = ep_parse(&ep, NULL, text, "test_effect_parser_writer");
		if (ok)
			dump_cat("ok 1 (the guard error is missing)\n");
		write_passes(&ep, preset);
		dump_errors(&ep);
		ep_free(&ep);
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

#define CHECK_CASES(cases, preset) check_cases(cases, sizeof(cases) / sizeof(cases[0]), preset)

/* ------------------------------------------------------------------------ */
/* Params: const, uniform, plain and array declarations, written once each, the
 * uniform ones pushed into used_params in the order they are written; a
 * param the call names is written before the function's own dependencies. */
static const struct writer_case params_cases[] = {
	/* const, uniform, array, plain and texture params named in two functions' bodies; a const param is
	 * declared without its value */
	{"uniform float4x4 ViewProj;\n"
	 "uniform float4 colors[4];\n"
	 "const float scale = 2.0;\n"
	 "float plain;\n"
	 "uniform texture2d image;\n"
	 "float4 VSMain(float4 pos : POSITION) : POSITION\n"
	 "{\n"
	 "\treturn mul(pos * scale, ViewProj) + colors[0] * plain;\n"
	 "}\n"
	 "float4 PSMain(float4 pos : POSITION) : TARGET\n"
	 "{\n"
	 "\treturn colors[1];\n"
	 "}\n"
	 "technique Draw { pass { vertex_shader = VSMain(pos); pixel_shader = PSMain(pos); } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    | const float scale;\n"
	 "    | uniform float4x4 ViewProj;\n"
	 "    | uniform float4 colors[4];\n"
	 "    | float plain;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSMain(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn mul(pos * scale, ViewProj) + colors[0] * plain;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSMain(pos);\n"
	 "    | }\n"
	 "    used ViewProj,colors\n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | uniform float4 colors[4];\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSMain(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn colors[1];\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSMain(pos);\n"
	 "    | }\n"
	 "    used colors\n"
	 "    written param= struct= func= sampler=\n"
	 "leaked 0\n"},
	/* params named by the call: written before the function, once each, and not again as its dependencies */
	{"uniform float4 tint;\n"
	 "const float half_gain = 0.5;\n"
	 "uniform float unused;\n"
	 "float4 PSTint(float4 pos : POSITION, float4 t, float g) : TARGET\n"
	 "{\n"
	 "\treturn pos * t * g * tint;\n"
	 "}\n"
	 "technique Draw { pass { vertex_shader = PSTint(pos, tint, half_gain); pixel_shader = PSTint(pos, tint, tint); } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    | uniform float4 tint;\n"
	 "    | const float half_gain;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSTint(float4 pos : POSITION, float4 t, float g)\n"
	 "    | {\n"
	 "    | \\treturn pos * t * g * tint;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSTint(pos,tint,half_gain);\n"
	 "    | }\n"
	 "    used tint\n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | uniform float4 tint;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSTint(float4 pos : POSITION, float4 t, float g)\n"
	 "    | {\n"
	 "    | \\treturn pos * t * g * tint;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSTint(pos,tint,tint);\n"
	 "    | }\n"
	 "    used tint\n"
	 "    written param= struct= func= sampler=\n"
	 "leaked 0\n"},
	/* a name declared twice: the first param or function of that name is the one written; and a one-
	 * element array keeps its [1] */
	{"uniform float4 one[1];\n"
	 "uniform float a;\n"
	 "uniform float4 a;\n"
	 "const float b = 1.0;\n"
	 "uniform float b;\n"
	 "float4 f(float4 c) { return c * a * b; }\n"
	 "float4 f(float4 c) { return c; }\n"
	 "float4 PS(float4 c : COLOR) : TARGET { return f(c) + one[0]; }\n"
	 "technique Draw { pass { vertex_shader = f(c, b); pixel_shader = PS(c); } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    | const float b;\n"
	 "    | uniform float a;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 f(float4 c)\n"
	 "    | { return c * a * b; }\n"
	 "    | \n"
	 "    | float4 main()\n"
	 "    | {\n"
	 "    | \\treturn f(c,b);\n"
	 "    | }\n"
	 "    used a\n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | uniform float4 one[1];\n"
	 "    | \n"
	 "    | \n"
	 "    | uniform float a;\n"
	 "    | const float b;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 f(float4 c)\n"
	 "    | { return c * a * b; }\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PS(float4 c : COLOR)\n"
	 "    | { return f(c) + one[0]; }\n"
	 "    | \n"
	 "    | float4 main(float4 c : COLOR) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PS(c);\n"
	 "    | }\n"
	 "    used one,a\n"
	 "    written param= struct= func= sampler=\n"
	 "leaked 0\n"},
};

static void params(void **state)
{
	(void)state;
	CHECK_CASES(params_cases, false);
}

/* ------------------------------------------------------------------------ */
/* Sampler blocks and structs: a sampler with and without states, a struct
 * whose first member is mapped (main takes it as a param, with no mapping of
 * its own) and ones whose first member is not (main drops the param), each
 * written once however often a function names it. */
static const struct writer_case blocks_cases[] = {
	/* a sampler, a mapped struct and an unmapped one (characterized, not endorsed: main drops the unmapped
	 * struct param, though the call still passes it) */
	{"uniform float4x4 ViewProj;\n"
	 "uniform texture2d image;\n"
	 "sampler_state def_sampler { Filter = Linear; AddressU = Clamp; AddressV = Wrap; };\n"
	 "struct VertInOut { float4 pos : POSITION; float2 uv : TEXCOORD0; };\n"
	 "struct Light { float3 dir; float strength; };\n"
	 "VertInOut VSDefault(VertInOut vert_in)\n"
	 "{\n"
	 "\tVertInOut vert_out;\n"
	 "\tvert_out.pos = mul(float4(vert_in.pos.xyz, 1.0), ViewProj);\n"
	 "\tvert_out.uv = vert_in.uv;\n"
	 "\treturn vert_out;\n"
	 "}\n"
	 "float4 PSLit(VertInOut vert_in) : TARGET\n"
	 "{\n"
	 "\tLight l;\n"
	 "\tl.strength = 0.5;\n"
	 "\treturn image.Sample(def_sampler, vert_in.uv) * l.strength;\n"
	 "}\n"
	 "float4 PSLight(Light l) : TARGET\n"
	 "{\n"
	 "\treturn float4(l.dir, l.strength);\n"
	 "}\n"
	 "technique Draw { pass { vertex_shader = VSDefault(vert_in); pixel_shader = PSLit(vert_in); } }\n"
	 "technique Unmapped { pass { vertex_shader = VSDefault(vert_in); pixel_shader = PSLight(l); } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    | uniform float4x4 ViewProj;\n"
	 "    | \n"
	 "    | \n"
	 "    | struct VertInOut {\n"
	 "    | \\tfloat4 pos : POSITION;\n"
	 "    | \\tfloat2 uv : TEXCOORD0;\n"
	 "    | };\n"
	 "    | \n"
	 "    | VertInOut VSDefault(VertInOut vert_in)\n"
	 "    | {\n"
	 "    | \\tVertInOut vert_out;\n"
	 "    | \\tvert_out.pos = mul(float4(vert_in.pos.xyz, 1.0), ViewProj);\n"
	 "    | \\tvert_out.uv = vert_in.uv;\n"
	 "    | \\treturn vert_out;\n"
	 "    | }\n"
	 "    | \n"
	 "    | VertInOut main(VertInOut vert_in)\n"
	 "    | {\n"
	 "    | \\treturn VSDefault(vert_in);\n"
	 "    | }\n"
	 "    used ViewProj\n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | uniform texture2d image;\n"
	 "    | \n"
	 "    | \n"
	 "    | sampler_state def_sampler {\n"
	 "    | \\tFilter = Linear;\n"
	 "    | \n"
	 "    | \\tAddressU = Clamp;\n"
	 "    | \n"
	 "    | \\tAddressV = Wrap;\n"
	 "    | \n"
	 "    | };\n"
	 "    | \n"
	 "    | struct VertInOut {\n"
	 "    | \\tfloat4 pos : POSITION;\n"
	 "    | \\tfloat2 uv : TEXCOORD0;\n"
	 "    | };\n"
	 "    | \n"
	 "    | struct Light {\n"
	 "    | \\tfloat3 dir;\n"
	 "    | \\tfloat strength;\n"
	 "    | };\n"
	 "    | \n"
	 "    | float4 PSLit(VertInOut vert_in)\n"
	 "    | {\n"
	 "    | \\tLight l;\n"
	 "    | \\tl.strength = 0.5;\n"
	 "    | \\treturn image.Sample(def_sampler, vert_in.uv) * l.strength;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(VertInOut vert_in) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSLit(vert_in);\n"
	 "    | }\n"
	 "    used image\n"
	 "    written param= struct= func= sampler=\n"
	 "Unmapped/0\n"
	 "  vertex\n"
	 "    | uniform float4x4 ViewProj;\n"
	 "    | \n"
	 "    | \n"
	 "    | struct VertInOut {\n"
	 "    | \\tfloat4 pos : POSITION;\n"
	 "    | \\tfloat2 uv : TEXCOORD0;\n"
	 "    | };\n"
	 "    | \n"
	 "    | VertInOut VSDefault(VertInOut vert_in)\n"
	 "    | {\n"
	 "    | \\tVertInOut vert_out;\n"
	 "    | \\tvert_out.pos = mul(float4(vert_in.pos.xyz, 1.0), ViewProj);\n"
	 "    | \\tvert_out.uv = vert_in.uv;\n"
	 "    | \\treturn vert_out;\n"
	 "    | }\n"
	 "    | \n"
	 "    | VertInOut main(VertInOut vert_in)\n"
	 "    | {\n"
	 "    | \\treturn VSDefault(vert_in);\n"
	 "    | }\n"
	 "    used ViewProj\n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | struct Light {\n"
	 "    | \\tfloat3 dir;\n"
	 "    | \\tfloat strength;\n"
	 "    | };\n"
	 "    | \n"
	 "    | float4 PSLight(Light l)\n"
	 "    | {\n"
	 "    | \\treturn float4(l.dir, l.strength);\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main() : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSLight(l);\n"
	 "    | }\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "leaked 0\n"},
	/* no vertex call, a sampler with no states, a sampler named twice (a blank line each time), and a
	 * struct mapped only after its first member (characterized, not endorsed: main drops it) */
	{"uniform texture2d image;\n"
	 "sampler_state plain_sampler { };\n"
	 "sampler_state point_sampler { Filter = Point; };\n"
	 "struct Late { float2 uv; float4 pos : POSITION; };\n"
	 "float4 PSTwo(Late v) : TARGET\n"
	 "{\n"
	 "\treturn image.Sample(plain_sampler, v.uv) + image.Sample(point_sampler, v.uv) + image.Sample(plain_sampler, v.uv);\n"
	 "}\n"
	 "technique Draw { pass { pixel_shader = PSTwo(v); } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | uniform texture2d image;\n"
	 "    | \n"
	 "    | \n"
	 "    | sampler_state plain_sampler {\n"
	 "    | };\n"
	 "    | \n"
	 "    | sampler_state point_sampler {\n"
	 "    | \\tFilter = Point;\n"
	 "    | \n"
	 "    | };\n"
	 "    | \n"
	 "    | \n"
	 "    | struct Late {\n"
	 "    | \\tfloat2 uv;\n"
	 "    | \\tfloat4 pos : POSITION;\n"
	 "    | };\n"
	 "    | \n"
	 "    | float4 PSTwo(Late v)\n"
	 "    | {\n"
	 "    | \\treturn image.Sample(plain_sampler, v.uv) + image.Sample(point_sampler, v.uv) + image.Sample(plain_sampler, v.uv);\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main() : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSTwo(v);\n"
	 "    | }\n"
	 "    used image\n"
	 "    written param= struct= func= sampler=\n"
	 "leaked 0\n"},
};

static void blocks(void **state)
{
	(void)state;
	CHECK_CASES(blocks_cases, false);
}

/* ------------------------------------------------------------------------ */
/* Function dependencies: each written once, before the function that calls it,
 * depth first; a function that calls itself; and the in, inout, out and
 * uniform keywords of a function's params. */
static const struct writer_case funcs_cases[] = {
	/* a chain and a diamond of calls */
	{"uniform float gain;\n"
	 "float4 leaf(float4 c) { return c * gain; }\n"
	 "float4 mid(float4 c) { return leaf(c) + leaf(c); }\n"
	 "float4 other(float4 c) { return leaf(c); }\n"
	 "float4 top(float4 c) { return mid(c) + other(c); }\n"
	 "float4 PSMain(float4 pos : POSITION) : TARGET { return top(pos); }\n"
	 "technique Draw { pass { vertex_shader = top(pos); pixel_shader = PSMain(pos); } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    | uniform float gain;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 leaf(float4 c)\n"
	 "    | { return c * gain; }\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 mid(float4 c)\n"
	 "    | { return leaf(c) + leaf(c); }\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 other(float4 c)\n"
	 "    | { return leaf(c); }\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 top(float4 c)\n"
	 "    | { return mid(c) + other(c); }\n"
	 "    | \n"
	 "    | float4 main()\n"
	 "    | {\n"
	 "    | \\treturn top(pos);\n"
	 "    | }\n"
	 "    used gain\n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | uniform float gain;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 leaf(float4 c)\n"
	 "    | { return c * gain; }\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 mid(float4 c)\n"
	 "    | { return leaf(c) + leaf(c); }\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 other(float4 c)\n"
	 "    | { return leaf(c); }\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 top(float4 c)\n"
	 "    | { return mid(c) + other(c); }\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSMain(float4 pos : POSITION)\n"
	 "    | { return top(pos); }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSMain(pos);\n"
	 "    | }\n"
	 "    used gain\n"
	 "    written param= struct= func= sampler=\n"
	 "leaked 0\n"},
	/* a function that calls itself, and param keywords */
	{"float4 rec(float4 c) { return rec(c); }\n"
	 "void adjust(inout float4 c, out float a, uniform float k, in float b) { a = b * k; c *= a; }\n"
	 "float4 PSAdjust(float4 c : COLOR) : TARGET\n"
	 "{\n"
	 "\tfloat a;\n"
	 "\tadjust(c, a, 1.0, 2.0);\n"
	 "\treturn rec(c) * a;\n"
	 "}\n"
	 "technique Draw { pass { vertex_shader = rec(c); pixel_shader = PSAdjust(c); } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    | float4 rec(float4 c)\n"
	 "    | { return rec(c); }\n"
	 "    | \n"
	 "    | float4 main()\n"
	 "    | {\n"
	 "    | \\treturn rec(c);\n"
	 "    | }\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | void adjust(inout float4 c, out float a, uniform float k, float b)\n"
	 "    | { a = b * k; c *= a; }\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 rec(float4 c)\n"
	 "    | { return rec(c); }\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 PSAdjust(float4 c : COLOR)\n"
	 "    | {\n"
	 "    | \\tfloat a;\n"
	 "    | \\tadjust(c, a, 1.0, 2.0);\n"
	 "    | \\treturn rec(c) * a;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 c : COLOR) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSAdjust(c);\n"
	 "    | }\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "leaked 0\n"},
};

static void funcs(void **state)
{
	(void)state;
	CHECK_CASES(funcs_cases, false);
}

/* ------------------------------------------------------------------------ */
/* The main() written after the function: the function's mapped params (a var
 * with a mapping, or a struct whose first member has one) as its own, then its
 * return mapping, and a body that returns the call's tokens. */
static const struct writer_case mains_cases[] = {
	/* two mapped params (characterized, not endorsed: no comma between them, so the shader doesn't
	 * compile) */
	{"float4 PSTwo(float4 pos : POSITION, float2 uv : TEXCOORD0) : TARGET\n"
	 "{\n"
	 "\treturn float4(uv, pos.zw);\n"
	 "}\n"
	 "technique Draw { pass { vertex_shader = PSTwo(pos, uv); pixel_shader = PSTwo(pos, uv); } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    | float4 PSTwo(float4 pos : POSITION, float2 uv : TEXCOORD0)\n"
	 "    | {\n"
	 "    | \\treturn float4(uv, pos.zw);\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITIONfloat2 uv : TEXCOORD0) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSTwo(pos,uv);\n"
	 "    | }\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | float4 PSTwo(float4 pos : POSITION, float2 uv : TEXCOORD0)\n"
	 "    | {\n"
	 "    | \\treturn float4(uv, pos.zw);\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITIONfloat2 uv : TEXCOORD0) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSTwo(pos,uv);\n"
	 "    | }\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "leaked 0\n"},
	/* mapped and unmapped params mixed (characterized, not endorsed: main drops k, and no comma between
	 * the rest) */
	{"struct VertInOut { float4 pos : POSITION; float2 uv : TEXCOORD0; };\n"
	 "float4 PSMix(float4 pos : POSITION, float k, float2 uv : TEXCOORD0) : TARGET\n"
	 "{\n"
	 "\treturn pos * k + float4(uv, 0.0, 0.0);\n"
	 "}\n"
	 "float4 PSBoth(VertInOut v, float4 c : COLOR) : TARGET\n"
	 "{\n"
	 "\treturn v.pos * c;\n"
	 "}\n"
	 "technique Draw { pass { vertex_shader = PSMix(pos, k, uv); pixel_shader = PSBoth(v, c); } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    | float4 PSMix(float4 pos : POSITION, float k, float2 uv : TEXCOORD0)\n"
	 "    | {\n"
	 "    | \\treturn pos * k + float4(uv, 0.0, 0.0);\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITIONfloat2 uv : TEXCOORD0) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSMix(pos,k,uv);\n"
	 "    | }\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | struct VertInOut {\n"
	 "    | \\tfloat4 pos : POSITION;\n"
	 "    | \\tfloat2 uv : TEXCOORD0;\n"
	 "    | };\n"
	 "    | \n"
	 "    | float4 PSBoth(VertInOut v, float4 c : COLOR)\n"
	 "    | {\n"
	 "    | \\treturn v.pos * c;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(VertInOut vfloat4 c : COLOR) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSBoth(v,c);\n"
	 "    | }\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "leaked 0\n"},
	/* no params, and no return mapping */
	{"float4 PSNone() : TARGET\n"
	 "{\n"
	 "\treturn float4(1.0, 1.0, 1.0, 1.0);\n"
	 "}\n"
	 "float4 VSNoMap(float4 pos : POSITION)\n"
	 "{\n"
	 "\treturn pos;\n"
	 "}\n"
	 "technique Draw { pass { vertex_shader = VSNoMap(pos); pixel_shader = PSNone(); } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    | float4 VSNoMap(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn pos;\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION)\n"
	 "    | {\n"
	 "    | \\treturn VSNoMap(pos);\n"
	 "    | }\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | float4 PSNone()\n"
	 "    | {\n"
	 "    | \\treturn float4(1.0, 1.0, 1.0, 1.0);\n"
	 "    | }\n"
	 "    | \n"
	 "    | float4 main() : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSNone();\n"
	 "    | }\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "leaked 0\n"},
};

static void mains(void **state)
{
	(void)state;
	CHECK_CASES(mains_cases, false);
}

/* ------------------------------------------------------------------------ */
/* The call itself: NULL, none at all, a function the effect doesn't define, a
 * param in the function's place, spaces inside the call, and two passes
 * calling the same function, each written in full because the flags are
 * cleared after every call. */
static const struct writer_case calls_cases[] = {
	/* NULL, and a pass with no pixel shader */
	{"uniform float4 tint;\n"
	 "float4 VSMain(float4 pos : POSITION) : POSITION { return pos * tint; }\n"
	 "technique Draw { pass { vertex_shader = VSMain(pos); pixel_shader = NULL; } }\n"
	 "technique Vertex { pass { vertex_shader = VSMain(pos); } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    | uniform float4 tint;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSMain(float4 pos : POSITION)\n"
	 "    | { return pos * tint; }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSMain(pos);\n"
	 "    | }\n"
	 "    used tint\n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "Vertex/0\n"
	 "  vertex\n"
	 "    | uniform float4 tint;\n"
	 "    | \n"
	 "    | \n"
	 "    | float4 VSMain(float4 pos : POSITION)\n"
	 "    | { return pos * tint; }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSMain(pos);\n"
	 "    | }\n"
	 "    used tint\n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "leaked 0\n"},
	/* an unknown function, and a param in the function's place: only the params the call names */
	{"uniform float4 tint;\n"
	 "float4 VSMain(float4 pos : POSITION) : POSITION { return pos * tint; }\n"
	 "technique Draw { pass { vertex_shader = Missing(pos, tint); pixel_shader = tint; } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    | uniform float4 tint;\n"
	 "    used tint\n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | uniform float4 tint;\n"
	 "    used tint\n"
	 "    written param= struct= func= sampler=\n"
	 "leaked 0\n"},
	/* spaces inside the call, which the pass parser drops */
	{"uniform float4 tint;\n"
	 "float4 VSMain(float4 pos : POSITION, float4 t) : POSITION { return pos * t; }\n"
	 "technique Draw { pass { vertex_shader =   VSMain ( pos ,  tint ) ; pixel_shader = VSMain(pos,tint); } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    | uniform float4 tint;\n"
	 "    | float4 VSMain(float4 pos : POSITION, float4 t)\n"
	 "    | { return pos * t; }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSMain(pos,tint);\n"
	 "    | }\n"
	 "    used tint\n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | uniform float4 tint;\n"
	 "    | float4 VSMain(float4 pos : POSITION, float4 t)\n"
	 "    | { return pos * t; }\n"
	 "    | \n"
	 "    | float4 main(float4 pos : POSITION) : POSITION\n"
	 "    | {\n"
	 "    | \\treturn VSMain(pos,tint);\n"
	 "    | }\n"
	 "    used tint\n"
	 "    written param= struct= func= sampler=\n"
	 "leaked 0\n"},
	/* two passes calling the same function */
	{"uniform float4x4 ViewProj;\n"
	 "struct VertInOut { float4 pos : POSITION; float2 uv : TEXCOORD0; };\n"
	 "VertInOut VSDefault(VertInOut v) { v.pos = mul(v.pos, ViewProj); return v; }\n"
	 "technique Draw { pass one { vertex_shader = VSDefault(v); pixel_shader = VSDefault(v); } pass two { vertex_shader = VSDefault(v); pixel_shader = NULL; } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    | uniform float4x4 ViewProj;\n"
	 "    | \n"
	 "    | \n"
	 "    | struct VertInOut {\n"
	 "    | \\tfloat4 pos : POSITION;\n"
	 "    | \\tfloat2 uv : TEXCOORD0;\n"
	 "    | };\n"
	 "    | \n"
	 "    | VertInOut VSDefault(VertInOut v)\n"
	 "    | { v.pos = mul(v.pos, ViewProj); return v; }\n"
	 "    | \n"
	 "    | VertInOut main(VertInOut v)\n"
	 "    | {\n"
	 "    | \\treturn VSDefault(v);\n"
	 "    | }\n"
	 "    used ViewProj\n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | uniform float4x4 ViewProj;\n"
	 "    | \n"
	 "    | \n"
	 "    | struct VertInOut {\n"
	 "    | \\tfloat4 pos : POSITION;\n"
	 "    | \\tfloat2 uv : TEXCOORD0;\n"
	 "    | };\n"
	 "    | \n"
	 "    | VertInOut VSDefault(VertInOut v)\n"
	 "    | { v.pos = mul(v.pos, ViewProj); return v; }\n"
	 "    | \n"
	 "    | VertInOut main(VertInOut v)\n"
	 "    | {\n"
	 "    | \\treturn VSDefault(v);\n"
	 "    | }\n"
	 "    used ViewProj\n"
	 "    written param= struct= func= sampler=\n"
	 "Draw/1\n"
	 "  vertex\n"
	 "    | uniform float4x4 ViewProj;\n"
	 "    | \n"
	 "    | \n"
	 "    | struct VertInOut {\n"
	 "    | \\tfloat4 pos : POSITION;\n"
	 "    | \\tfloat2 uv : TEXCOORD0;\n"
	 "    | };\n"
	 "    | \n"
	 "    | VertInOut VSDefault(VertInOut v)\n"
	 "    | { v.pos = mul(v.pos, ViewProj); return v; }\n"
	 "    | \n"
	 "    | VertInOut main(VertInOut v)\n"
	 "    | {\n"
	 "    | \\treturn VSDefault(v);\n"
	 "    | }\n"
	 "    used ViewProj\n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "leaked 0\n"},
};

static void calls(void **state)
{
	(void)state;
	CHECK_CASES(calls_cases, false);
}

/* ------------------------------------------------------------------------ */
/* Every written flag set before each call: the declarations are skipped, but
 * the called function itself and its main are written; then the flags are
 * cleared, except after a NULL call or none, which leave them set. */
static const struct writer_case preset_cases[] = {
	/* a sampler, a struct, params and a function dependency all marked written; a uniform the call names
	 * is then not pushed into used_params */
	{"uniform float4x4 ViewProj;\n"
	 "uniform texture2d image;\n"
	 "sampler_state def_sampler { Filter = Linear; };\n"
	 "struct VertInOut { float4 pos : POSITION; float2 uv : TEXCOORD0; };\n"
	 "float4 sample_at(float2 uv) { return image.Sample(def_sampler, uv); }\n"
	 "VertInOut VSDefault(VertInOut v) { v.pos = mul(v.pos, ViewProj); return v; }\n"
	 "float4 PSDraw(VertInOut v) : TARGET { return sample_at(v.uv); }\n"
	 "technique Draw { pass { vertex_shader = VSDefault(v); pixel_shader = PSDraw(v); } }\n"
	 "technique Null { pass { vertex_shader = VSDefault(ViewProj); pixel_shader = NULL; } }\n"
	 "technique Vertex { pass { vertex_shader = VSDefault(v); } }",
	 "Draw/0\n"
	 "  vertex\n"
	 "    | \n"
	 "    | \n"
	 "    | VertInOut VSDefault(VertInOut v)\n"
	 "    | { v.pos = mul(v.pos, ViewProj); return v; }\n"
	 "    | \n"
	 "    | VertInOut main(VertInOut v)\n"
	 "    | {\n"
	 "    | \\treturn VSDefault(v);\n"
	 "    | }\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    | float4 PSDraw(VertInOut v)\n"
	 "    | { return sample_at(v.uv); }\n"
	 "    | \n"
	 "    | float4 main(VertInOut v) : TARGET\n"
	 "    | {\n"
	 "    | \\treturn PSDraw(v);\n"
	 "    | }\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "Null/0\n"
	 "  vertex\n"
	 "    | \n"
	 "    | \n"
	 "    | VertInOut VSDefault(VertInOut v)\n"
	 "    | { v.pos = mul(v.pos, ViewProj); return v; }\n"
	 "    | \n"
	 "    | VertInOut main(VertInOut v)\n"
	 "    | {\n"
	 "    | \\treturn VSDefault(ViewProj);\n"
	 "    | }\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    used \n"
	 "    written param=ViewProj,image struct=VertInOut func=sample_at,VSDefault,PSDraw sampler=def_sampler\n"
	 "Vertex/0\n"
	 "  vertex\n"
	 "    | \n"
	 "    | \n"
	 "    | VertInOut VSDefault(VertInOut v)\n"
	 "    | { v.pos = mul(v.pos, ViewProj); return v; }\n"
	 "    | \n"
	 "    | VertInOut main(VertInOut v)\n"
	 "    | {\n"
	 "    | \\treturn VSDefault(v);\n"
	 "    | }\n"
	 "    used \n"
	 "    written param= struct= func= sampler=\n"
	 "  pixel\n"
	 "    used \n"
	 "    written param=ViewProj,image struct=VertInOut func=sample_at,VSDefault,PSDraw sampler=def_sampler\n"
	 "leaked 0\n"},
};

static void preset(void **state)
{
	(void)state;
	CHECK_CASES(preset_cases, true);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(params), cmocka_unit_test(blocks), cmocka_unit_test(funcs),
		cmocka_unit_test(mains),  cmocka_unit_test(calls),  cmocka_unit_test(preset),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
