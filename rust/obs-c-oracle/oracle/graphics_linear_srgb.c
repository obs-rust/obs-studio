/* Real private header; the extracted getter and adapter are compiled verbatim. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

bool oracle_gs_get_linear_srgb(void);
bool oracle_gs_set_linear_srgb(bool value);
void oracle_linear_srgb_reset(bool present, bool value);
unsigned oracle_linear_srgb_logs(void);
int oracle_linear_srgb_level(void);
const char *oracle_linear_srgb_log(void);
bool oracle_linear_srgb_value(void);
const void *oracle_linear_srgb_storage(void);
size_t oracle_linear_srgb_offset(void);
size_t oracle_linear_srgb_bool_size(void);
size_t oracle_linear_srgb_bool_align(void);

static THREAD_LOCAL struct graphics_subsystem linear_srgb_graphics;
static THREAD_LOCAL bool linear_srgb_present;
static THREAD_LOCAL unsigned linear_srgb_log_count;
static THREAD_LOCAL int linear_srgb_log_level;
static THREAD_LOCAL char linear_srgb_log_text[128];

static graphics_t *linear_srgb_context(void)
{
	return linear_srgb_present ? &linear_srgb_graphics : NULL;
}

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void linear_srgb_blog(int level, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	linear_srgb_log_count++;
	linear_srgb_log_level = level;
	vsnprintf(linear_srgb_log_text, sizeof(linear_srgb_log_text), format, args);
	va_end(args);
}

#define gs_get_context linear_srgb_context
#define blog linear_srgb_blog
#define obs_stagesurface_context oracle_linear_srgb_unused_ss_context
#define obs_stagesurface_debug oracle_linear_srgb_unused_ss_debug
#define obs_stagesurface_width_field oracle_linear_srgb_unused_ss_width_field
#define obs_stagesurface_height_field oracle_linear_srgb_unused_ss_height_field
#define obs_stagesurface_format_field oracle_linear_srgb_unused_ss_format_field
#define obs_texture_planar_context oracle_linear_srgb_unused_tp_context
#define obs_texture_planar_log oracle_linear_srgb_unused_tp_log
#define obs_texture_planar_device oracle_linear_srgb_unused_tp_device
#define obs_texture_planar_nv12_field oracle_linear_srgb_unused_tp_nv12_field
#define obs_texture_planar_p010_field oracle_linear_srgb_unused_tp_p010_field
#include "graphics-linear-srgb-fields.inc"
#undef obs_stagesurface_context
#undef obs_stagesurface_debug
#undef obs_stagesurface_width_field
#undef obs_stagesurface_height_field
#undef obs_stagesurface_format_field
#undef obs_texture_planar_context
#undef obs_texture_planar_log
#undef obs_texture_planar_device
#undef obs_texture_planar_nv12_field
#undef obs_texture_planar_p010_field

#undef blog
#define gs_get_linear_srgb oracle_gs_get_linear_srgb
#include "graphics-linear-srgb.inc"
#undef gs_get_linear_srgb
#define gs_set_linear_srgb oracle_gs_set_linear_srgb
#include "graphics-set-linear-srgb.inc"
#undef gs_set_linear_srgb
#undef gs_get_context

void oracle_linear_srgb_reset(bool present, bool value)
{
	linear_srgb_present = present;
	linear_srgb_graphics.linear_srgb = value;
	linear_srgb_log_count = 0;
	linear_srgb_log_level = 0;
	linear_srgb_log_text[0] = '\0';
}

unsigned oracle_linear_srgb_logs(void)
{
	return linear_srgb_log_count;
}

int oracle_linear_srgb_level(void)
{
	return linear_srgb_log_level;
}

const char *oracle_linear_srgb_log(void)
{
	return linear_srgb_log_text;
}

bool oracle_linear_srgb_value(void)
{
	return linear_srgb_graphics.linear_srgb;
}

const void *oracle_linear_srgb_storage(void)
{
	return &linear_srgb_graphics;
}

size_t oracle_linear_srgb_offset(void)
{
	return offsetof(struct graphics_subsystem, linear_srgb);
}

size_t oracle_linear_srgb_bool_size(void)
{
	return sizeof(linear_srgb_graphics.linear_srgb);
}

size_t oracle_linear_srgb_bool_align(void)
{
	return _Alignof(bool);
}
