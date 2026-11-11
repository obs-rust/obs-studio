/* Original extracted C and real adapter with a thread-local recording backend. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#define gs_shader_get_num_params oracle_gs_shader_get_num_params
#define gs_shader_get_param_by_idx oracle_gs_shader_get_param_by_idx
#define gs_shader_get_param_by_name oracle_gs_shader_get_param_by_name
#define gs_shader_get_param_info oracle_gs_shader_get_param_info
#include "graphics/graphics-internal.h"
#include "util/base.h"

/* Existing effect fixtures become backend callbacks behind the real Rust ABI. */
gs_sparam_t *oracle_compile_param_by_name(gs_shader_t *shader, const char *name);
void oracle_passes_param_info(const gs_sparam_t *param, struct gs_shader_param_info *info);

static THREAD_LOCAL struct graphics_subsystem spi_graphics, spi_effect_graphics;
static THREAD_LOCAL bool spi_active, spi_present;
static THREAD_LOCAL int spi_count, spi_level;
static THREAD_LOCAL gs_sparam_t *spi_result;
static THREAD_LOCAL enum gs_shader_param_type spi_type;
static THREAD_LOCAL const char *spi_name;
static THREAD_LOCAL unsigned spi_writes, spi_calls[4], spi_logs, spi_events;
static THREAD_LOCAL const void *spi_seen[4];
static THREAD_LOCAL uint32_t spi_index;
static THREAD_LOCAL char spi_text[160], spi_order[64];
static THREAD_LOCAL unsigned char spi_tokens[3];

static void spi_event(unsigned operation)
{
	spi_calls[operation]++;
	if (spi_events < sizeof(spi_order) - 1) {
		spi_order[spi_events++] = "CINO"[operation];
		spi_order[spi_events] = 0;
	}
}
static int spi_get_count(const gs_shader_t *shader)
{
	spi_seen[0] = shader;
	spi_event(0);
	return spi_count;
}
static gs_sparam_t *spi_get_index(gs_shader_t *shader, uint32_t index)
{
	spi_seen[0] = shader;
	spi_index = index;
	spi_event(1);
	return spi_result;
}
static gs_sparam_t *spi_get_name(gs_shader_t *shader, const char *name)
{
	spi_seen[0] = shader;
	spi_seen[1] = name;
	spi_event(2);
	return spi_result;
}
static void spi_get_info(const gs_sparam_t *param, struct gs_shader_param_info *info)
{
	spi_seen[2] = param;
	spi_seen[3] = info;
	spi_event(3);
	if (spi_writes & 1)
		info->type = spi_type;
	if (spi_writes & 2)
		info->name = spi_name;
}
static graphics_t *spi_context(void)
{
	if (!spi_active) {
		spi_effect_graphics.exports.gs_shader_get_param_by_name = oracle_compile_param_by_name;
		spi_effect_graphics.exports.gs_shader_get_param_info = oracle_passes_param_info;
		return &spi_effect_graphics;
	}
	return spi_present ? &spi_graphics : NULL;
}
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void spi_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	spi_logs++;
	spi_level = level;
	vsnprintf(spi_text, sizeof(spi_text), format, args);
	va_end(args);
}
#define gs_get_context spi_context
#define blog spi_blog
#include "shader-param-introspection-fields.inc"
#include "shader-param-introspection.inc"
#undef blog
#undef gs_get_context

void oracle_spi_reset(bool present, int count, bool found, int type, const char *name, unsigned writes);
void oracle_spi_reset(bool present, int count, bool found, int type, const char *name, unsigned writes)
{
	memset(&spi_graphics, 0, sizeof(spi_graphics));
	spi_graphics.exports.gs_shader_get_num_params = spi_get_count;
	spi_graphics.exports.gs_shader_get_param_by_idx = spi_get_index;
	spi_graphics.exports.gs_shader_get_param_by_name = spi_get_name;
	spi_graphics.exports.gs_shader_get_param_info = spi_get_info;
	spi_graphics.ref = 1;
	spi_active = true;
	spi_present = present;
	spi_count = count;
	spi_result = found ? (gs_sparam_t *)&spi_tokens[2] : NULL;
	spi_type = (enum gs_shader_param_type)type;
	spi_name = name;
	spi_writes = writes;
	memset(spi_calls, 0, sizeof(spi_calls));
	memset((void *)spi_seen, 0, sizeof(spi_seen));
	spi_index = spi_logs = spi_events = 0;
	spi_level = 0;
	spi_text[0] = spi_order[0] = 0;
}
void oracle_spi_stop(void);
void oracle_spi_stop(void)
{
	spi_active = false;
}
void *oracle_spi_token(unsigned index);
void *oracle_spi_token(unsigned index)
{
	return &spi_tokens[index];
}
unsigned oracle_spi_calls(unsigned index);
unsigned oracle_spi_calls(unsigned index)
{
	return spi_calls[index];
}
const void *oracle_spi_seen(unsigned index);
const void *oracle_spi_seen(unsigned index)
{
	return spi_seen[index];
}
uint32_t oracle_spi_index(void);
uint32_t oracle_spi_index(void)
{
	return spi_index;
}
unsigned oracle_spi_logs(void);
unsigned oracle_spi_logs(void)
{
	return spi_logs;
}
int oracle_spi_level(void);
int oracle_spi_level(void)
{
	return spi_level;
}
const char *oracle_spi_text(void);
const char *oracle_spi_text(void)
{
	return spi_text;
}
const char *oracle_spi_order(void);
const char *oracle_spi_order(void)
{
	return spi_order;
}
size_t oracle_spi_layout(unsigned index);
size_t oracle_spi_layout(unsigned index)
{
	const size_t values[] = {
		sizeof(struct gs_shader_param_info),         _Alignof(struct gs_shader_param_info),
		offsetof(struct gs_shader_param_info, type), offsetof(struct gs_shader_param_info, name),
		sizeof(enum gs_shader_param_type),           _Alignof(enum gs_shader_param_type)};
	return values[index];
}
bool oracle_spi_adapter_layout(void);
bool oracle_spi_adapter_layout(void)
{
	return obs_shader_param_introspection_count_field(&spi_graphics) ==
		       spi_graphics.exports.gs_shader_get_num_params &&
	       obs_shader_param_introspection_index_field(&spi_graphics) ==
		       spi_graphics.exports.gs_shader_get_param_by_idx &&
	       obs_shader_param_introspection_name_field(&spi_graphics) ==
		       spi_graphics.exports.gs_shader_get_param_by_name &&
	       obs_shader_param_introspection_info_field(&spi_graphics) ==
		       spi_graphics.exports.gs_shader_get_param_info;
}
int oracle_spi_type(unsigned index);
int oracle_spi_type(unsigned index)
{
	const enum gs_shader_param_type types[] = {
		GS_SHADER_PARAM_UNKNOWN, GS_SHADER_PARAM_BOOL, GS_SHADER_PARAM_FLOAT, GS_SHADER_PARAM_INT,
		GS_SHADER_PARAM_STRING,  GS_SHADER_PARAM_VEC2, GS_SHADER_PARAM_VEC3,  GS_SHADER_PARAM_VEC4,
		GS_SHADER_PARAM_INT2,    GS_SHADER_PARAM_INT3, GS_SHADER_PARAM_INT4,  GS_SHADER_PARAM_MATRIX4X4,
		GS_SHADER_PARAM_TEXTURE};
	return (int)types[index];
}
#undef gs_shader_get_num_params
#undef gs_shader_get_param_by_idx
#undef gs_shader_get_param_by_name
#undef gs_shader_get_param_info
