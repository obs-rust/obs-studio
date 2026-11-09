/* Original setters, real private headers, thread-local recording backend. */
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#define gs_shader_set_bool oracle_gs_shader_set_bool
#define gs_shader_set_float oracle_gs_shader_set_float
#define gs_shader_set_int oracle_gs_shader_set_int
#define gs_shader_set_matrix3 oracle_gs_shader_set_matrix3
#define gs_shader_set_matrix4 oracle_gs_shader_set_matrix4
#define gs_shader_set_vec2 oracle_gs_shader_set_vec2
#define gs_shader_set_vec3 oracle_gs_shader_set_vec3
#define gs_shader_set_vec4 oracle_gs_shader_set_vec4
#include "graphics/graphics-internal.h"
#include "graphics/vec2.h"
#include "util/base.h"

static THREAD_LOCAL struct graphics_subsystem sp_graphics;
static THREAD_LOCAL bool sp_present;
static THREAD_LOCAL unsigned sp_calls, sp_kind, sp_logs;
static THREAD_LOCAL int sp_level;
static THREAD_LOCAL uint32_t sp_bits;
static THREAD_LOCAL gs_sparam_t *sp_parameter;
static THREAD_LOCAL const void *sp_value;
static THREAD_LOCAL char sp_text[160];
static THREAD_LOCAL int sp_tokens[2];
static THREAD_LOCAL union {
	struct matrix3 m3;
	struct matrix4 m4;
	struct vec2 v2;
	struct vec3 v3;
	struct vec4 v4;
} sp_input;

static graphics_t *sp_context(void)
{
	return sp_present ? &sp_graphics : NULL;
}
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void sp_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	sp_logs++;
	sp_level = level;
	vsnprintf(sp_text, sizeof(sp_text), format, args);
	va_end(args);
}
static void sp_record(unsigned kind, gs_sparam_t *param)
{
	sp_calls++;
	sp_kind = kind;
	sp_parameter = param;
}
static void sp_bool(gs_sparam_t *param, bool value)
{
	sp_record(0, param);
	sp_bits = value ? 1U : 0U;
}
static void sp_float(gs_sparam_t *param, float value)
{
	sp_record(1, param);
	memcpy(&sp_bits, &value, sizeof(sp_bits));
}
static void sp_int(gs_sparam_t *param, int value)
{
	sp_record(2, param);
	memcpy(&sp_bits, &value, sizeof(sp_bits));
}
static void sp_matrix3(gs_sparam_t *param, const struct matrix3 *value)
{
	sp_record(3, param);
	sp_value = value;
}
static void sp_matrix4(gs_sparam_t *param, const struct matrix4 *value)
{
	sp_record(4, param);
	sp_value = value;
}
static void sp_vec2(gs_sparam_t *param, const struct vec2 *value)
{
	sp_record(5, param);
	sp_value = value;
}
static void sp_vec3(gs_sparam_t *param, const struct vec3 *value)
{
	sp_record(6, param);
	sp_value = value;
}
static void sp_vec4(gs_sparam_t *param, const struct vec4 *value)
{
	sp_record(7, param);
	sp_value = value;
}
#define gs_get_context sp_context
#define blog sp_blog
#include "graphics-shader-params-checks.inc"
#include "graphics-shader-params.inc"
#include "graphics-shader-params-adapter.inc"
#undef blog
#undef gs_get_context
#undef gs_shader_set_bool
void gs_shader_set_bool(gs_sparam_t *param, bool value);
#undef gs_shader_set_float
void gs_shader_set_float(gs_sparam_t *param, float value);
#undef gs_shader_set_int
void gs_shader_set_int(gs_sparam_t *param, int value);
#undef gs_shader_set_matrix3
void gs_shader_set_matrix3(gs_sparam_t *param, const struct matrix3 *value);
#undef gs_shader_set_matrix4
void gs_shader_set_matrix4(gs_sparam_t *param, const struct matrix4 *value);
#undef gs_shader_set_vec2
void gs_shader_set_vec2(gs_sparam_t *param, const struct vec2 *value);
#undef gs_shader_set_vec3
void gs_shader_set_vec3(gs_sparam_t *param, const struct vec3 *value);
#undef gs_shader_set_vec4
void gs_shader_set_vec4(gs_sparam_t *param, const struct vec4 *value);

void oracle_shader_params_reset(bool present);
void oracle_shader_params_invoke(bool rust, unsigned kind, unsigned token, bool value, uint32_t bits);
unsigned oracle_shader_params_calls(void);
unsigned oracle_shader_params_kind(void);
unsigned oracle_shader_params_logs(void);
int oracle_shader_params_level(void);
uint32_t oracle_shader_params_bits(void);
uintptr_t oracle_shader_params_parameter(void);
uintptr_t oracle_shader_params_value(void);
const char *oracle_shader_params_text(void);
bool oracle_shader_params_input_unchanged(void);
uintptr_t oracle_shader_params_expected_parameter(unsigned token);
uintptr_t oracle_shader_params_expected_value(void);

void oracle_shader_params_reset(bool present)
{
	sp_present = present;
	sp_calls = sp_logs = sp_bits = 0;
	sp_level = 0;
	sp_kind = 99;
	sp_parameter = NULL;
	sp_value = NULL;
	sp_text[0] = '\0';
	memset(&sp_input, 0x43, sizeof(sp_input));
	sp_graphics.exports.oracle_gs_shader_set_bool = sp_bool;
	sp_graphics.exports.oracle_gs_shader_set_float = sp_float;
	sp_graphics.exports.oracle_gs_shader_set_int = sp_int;
	sp_graphics.exports.oracle_gs_shader_set_matrix3 = sp_matrix3;
	sp_graphics.exports.oracle_gs_shader_set_matrix4 = sp_matrix4;
	sp_graphics.exports.oracle_gs_shader_set_vec2 = sp_vec2;
	sp_graphics.exports.oracle_gs_shader_set_vec3 = sp_vec3;
	sp_graphics.exports.oracle_gs_shader_set_vec4 = sp_vec4;
}

void oracle_shader_params_invoke(bool rust, unsigned kind, unsigned token, bool value, uint32_t bits)
{
	gs_sparam_t *param = token ? (gs_sparam_t *)&sp_tokens[token - 1] : NULL;
	float f;
	int n;
	memcpy(&f, &bits, sizeof(f));
	memcpy(&n, &bits, sizeof(n));
	switch (kind) {
	case 0:
		if (rust)
			gs_shader_set_bool(param, (bits & 1U) != 0);
		else
			oracle_gs_shader_set_bool(param, (bits & 1U) != 0);
		break;
	case 1:
		if (rust)
			gs_shader_set_float(param, f);
		else
			oracle_gs_shader_set_float(param, f);
		break;
	case 2:
		if (rust)
			gs_shader_set_int(param, n);
		else
			oracle_gs_shader_set_int(param, n);
		break;
	case 3:
		if (rust)
			gs_shader_set_matrix3(param, value ? &sp_input.m3 : NULL);
		else
			oracle_gs_shader_set_matrix3(param, value ? &sp_input.m3 : NULL);
		break;
	case 4:
		if (rust)
			gs_shader_set_matrix4(param, value ? &sp_input.m4 : NULL);
		else
			oracle_gs_shader_set_matrix4(param, value ? &sp_input.m4 : NULL);
		break;
	case 5:
		if (rust)
			gs_shader_set_vec2(param, value ? &sp_input.v2 : NULL);
		else
			oracle_gs_shader_set_vec2(param, value ? &sp_input.v2 : NULL);
		break;
	case 6:
		if (rust)
			gs_shader_set_vec3(param, value ? &sp_input.v3 : NULL);
		else
			oracle_gs_shader_set_vec3(param, value ? &sp_input.v3 : NULL);
		break;
	case 7:
		if (rust)
			gs_shader_set_vec4(param, value ? &sp_input.v4 : NULL);
		else
			oracle_gs_shader_set_vec4(param, value ? &sp_input.v4 : NULL);
		break;
	}
}
unsigned oracle_shader_params_calls(void)
{
	return sp_calls;
}
unsigned oracle_shader_params_kind(void)
{
	return sp_kind;
}
unsigned oracle_shader_params_logs(void)
{
	return sp_logs;
}
int oracle_shader_params_level(void)
{
	return sp_level;
}
uint32_t oracle_shader_params_bits(void)
{
	return sp_bits;
}
uintptr_t oracle_shader_params_parameter(void)
{
	return (uintptr_t)sp_parameter;
}
uintptr_t oracle_shader_params_value(void)
{
	return (uintptr_t)sp_value;
}
const char *oracle_shader_params_text(void)
{
	return sp_text;
}
uintptr_t oracle_shader_params_expected_parameter(unsigned token)
{
	return token ? (uintptr_t)&sp_tokens[token - 1] : 0;
}
uintptr_t oracle_shader_params_expected_value(void)
{
	return (uintptr_t)&sp_input;
}
bool oracle_shader_params_input_unchanged(void)
{
	const unsigned char *bytes = (const unsigned char *)&sp_input;
	for (size_t i = 0; i < sizeof(sp_input); i++)
		if (bytes[i] != 0x43)
			return false;
	return true;
}
