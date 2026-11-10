/* Tier 3 oracle of libobs/graphics/texture-planar-creators.c.h: the original
 * gs_texture_create_nv12() and gs_texture_create_p010(), renamed
 * oracle_gs_texture_create_*, and the field adapter the Rust shim calls
 * (obs_texture_planar_* in graphics-linear-srgb-fields.c.h), both over one
 * stand-in graphics context with the real private layout.
 *
 * The stand-in's two planar device exports record each call in the
 * recording gs_* stand-in's log (texture_render_device.c), which also
 * records gs_texture_create, gs_texture_destroy and, when capturing, every
 * log line. The C baseline uses the original C texture creator; Rust uses
 * its public shim. Both reach the same recording texture backend. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#define gs_texture_create_nv12 oracle_gs_texture_create_nv12
#define gs_texture_create_p010 oracle_gs_texture_create_p010
#include "graphics/graphics-internal.h"
#include "util/base.h"

void oracle_texrender_device_note(const char *text);
gs_texture_t *oracle_gs_texture_create(uint32_t width, uint32_t height, enum gs_color_format format, uint32_t levels,
				       const uint8_t **data, uint32_t flags);

static THREAD_LOCAL struct graphics_subsystem tp_graphics;
static THREAD_LOCAL bool tp_present;
static THREAD_LOCAL bool tp_answer;
static THREAD_LOCAL bool tp_writes;
static char tp_device;
/* What a device export writes to the outputs: two tokens outside the
 * texture pool. */
static char tp_tokens[2];

static graphics_t *tp_context(void)
{
	return tp_present ? &tp_graphics : NULL;
}

static void tp_native(const char *name, gs_device_t *device, gs_texture_t **tex_y, gs_texture_t **tex_uv,
		      uint32_t width, uint32_t height, uint32_t flags)
{
	char line[128];

	snprintf(line, sizeof(line), "native %s dev:%d %u %u 0x%x", name, device == (gs_device_t *)&tp_device, width,
		 height, flags);
	oracle_texrender_device_note(line);
	if (tp_writes) {
		*tex_y = (gs_texture_t *)&tp_tokens[0];
		*tex_uv = (gs_texture_t *)&tp_tokens[1];
	}
}

static bool tp_native_nv12(gs_device_t *device, gs_texture_t **tex_y, gs_texture_t **tex_uv, uint32_t width,
			   uint32_t height, uint32_t flags)
{
	tp_native("nv12", device, tex_y, tex_uv, width, height, flags);
	return tp_answer;
}

static bool tp_native_p010(gs_device_t *device, gs_texture_t **tex_y, gs_texture_t **tex_uv, uint32_t width,
			   uint32_t height, uint32_t flags)
{
	tp_native("p010", device, tex_y, tex_uv, width, height, flags);
	return tp_answer;
}

#define gs_get_context tp_context
#define obs_graphics_linear_srgb_field oracle_tp_unused_linear_srgb_field
#define obs_stagesurface_context oracle_tp_unused_ss_context
#define obs_stagesurface_debug oracle_tp_unused_ss_debug
#define obs_stagesurface_width_field oracle_tp_unused_ss_width_field
#define obs_stagesurface_height_field oracle_tp_unused_ss_height_field
#define obs_stagesurface_format_field oracle_tp_unused_ss_format_field
#include "graphics-linear-srgb-fields.inc"
#undef obs_graphics_linear_srgb_field
#undef obs_stagesurface_context
#undef obs_stagesurface_debug
#undef obs_stagesurface_width_field
#undef obs_stagesurface_height_field
#undef obs_stagesurface_format_field

#define gs_texture_create oracle_gs_texture_create
#include "texture-planar-creators.inc"
#undef gs_texture_create
#undef gs_get_context

bool oracle_gs_texture_create_nv12(gs_texture_t **tex_y, gs_texture_t **tex_uv, uint32_t width, uint32_t height,
				   uint32_t flags);
bool oracle_gs_texture_create_p010(gs_texture_t **tex_y, gs_texture_t **tex_uv, uint32_t width, uint32_t height,
				   uint32_t flags);

/* A fresh context: present or not, each planar export absent or there,
 * what the exports answer and whether they write their tokens. */
void oracle_tp_reset(bool present, bool nv12, bool p010, bool answer, bool writes);
void oracle_tp_reset(bool present, bool nv12, bool p010, bool answer, bool writes)
{
	memset(&tp_graphics, 0, sizeof(tp_graphics));
	tp_graphics.device = (gs_device_t *)&tp_device;
	tp_graphics.exports.device_texture_create_nv12 = nv12 ? tp_native_nv12 : NULL;
	tp_graphics.exports.device_texture_create_p010 = p010 ? tp_native_p010 : NULL;
	tp_present = present;
	tp_answer = answer;
	tp_writes = writes;
}

/* 0 or 1 for a device export's token, -1 for anything else. */
int oracle_tp_token(const void *ptr);
int oracle_tp_token(const void *ptr)
{
	const char *p = ptr;

	if (p >= tp_tokens && p < tp_tokens + sizeof(tp_tokens))
		return (int)(p - tp_tokens);
	return -1;
}

/* The adapter returns the real private fields. */
bool oracle_tp_adapter_layout(void);
bool oracle_tp_adapter_layout(void)
{
	return obs_texture_planar_context() == tp_context() &&
	       obs_texture_planar_device(&tp_graphics) == tp_graphics.device &&
	       obs_texture_planar_nv12_field(&tp_graphics) == tp_graphics.exports.device_texture_create_nv12 &&
	       obs_texture_planar_p010_field(&tp_graphics) == tp_graphics.exports.device_texture_create_p010;
}

#undef gs_texture_create_nv12
#undef gs_texture_create_p010
