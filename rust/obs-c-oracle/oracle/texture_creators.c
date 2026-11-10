/* Compile the original creators and the real layout adapter. Thread-local
 * recordings isolate parity from the texrender fixture's process-wide log. */
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
/* The C baseline must not depend on the Rust mip planner under test. */
#define gs_texture_mip_plan oracle_gs_texture_mip_plan
#include "graphics/graphics-internal.h"
#include "graphics/graphics-sprite.h"
#include "util/base.h"

gs_texture_t *oracle_texrender_texture_create(uint32_t width, uint32_t height, enum gs_color_format format,
					      uint32_t levels, const uint8_t **data, uint32_t flags);
void oracle_texture_creators_reset(bool present, bool fail);
void oracle_texture_creators_finish(void);
void oracle_graphics_set_context_present(bool present);
const char *oracle_texture_creators_log(void);
gs_texture_t *oracle_gs_texture_create(uint32_t w, uint32_t h, enum gs_color_format f, uint32_t l, const uint8_t **d,
				       uint32_t flags);
gs_texture_t *oracle_gs_cubetexture_create(uint32_t s, enum gs_color_format f, uint32_t l, const uint8_t **d,
					   uint32_t flags);

static THREAD_LOCAL struct graphics_subsystem creator_context;
static THREAD_LOCAL bool creator_override;
static THREAD_LOCAL bool creator_present = true;
static THREAD_LOCAL bool creator_fail;
static THREAD_LOCAL char creator_log[2048];
static THREAD_LOCAL char creator_token;

#ifdef __GNUC__
__attribute__((format(printf, 1, 2)))
#endif
static void creator_record(const char *format, ...)
{
	size_t n = strlen(creator_log);
	va_list args;
	va_start(args, format);
	vsnprintf(creator_log + n, sizeof(creator_log) - n, format, args);
	va_end(args);
}

static gs_texture_t *creator_texture(gs_device_t *device, uint32_t w, uint32_t h, enum gs_color_format f, uint32_t l,
				     const uint8_t **d, uint32_t flags)
{
	(void)device;
	if (!creator_override)
		return oracle_texrender_texture_create(w, h, f, l, d, flags);
	creator_record("tex %u %u %d %u %d %u;", w, h, (int)f, l, d != NULL, flags);
	return creator_fail ? NULL : (gs_texture_t *)&creator_token;
}
static gs_texture_t *creator_cube(gs_device_t *device, uint32_t s, enum gs_color_format f, uint32_t l,
				  const uint8_t **d, uint32_t flags)
{
	(void)device;
	creator_record("cube %u %d %u %d %u;", s, (int)f, l, d != NULL, flags);
	return creator_fail ? NULL : (gs_texture_t *)&creator_token;
}

/* One public context provider serves texture, texrender and shader-file parity. */
void oracle_graphics_set_context_present(bool present)
{
	creator_present = present;
}

graphics_t *gs_get_context(void)
{
	if (!creator_present)
		return NULL;
	creator_context.exports.device_texture_create = creator_texture;
	creator_context.exports.device_cubetexture_create = creator_cube;
	return &creator_context;
}

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void creator_blog(int level, const char *format, ...)
{
	char text[256];
	va_list args;
	va_start(args, format);
	vsnprintf(text, sizeof(text), format, args);
	va_end(args);
	if (creator_override)
		creator_record("log %d %s;", level, text);
}
#define blog creator_blog
#include "texture-creators-fields.inc"
#define gs_texture_create oracle_gs_texture_create
#define gs_cubetexture_create oracle_gs_cubetexture_create
#include "texture-creators.inc"
#undef gs_cubetexture_create
#undef gs_texture_create
#undef blog
#undef gs_texture_mip_plan

void oracle_texture_creators_reset(bool present, bool fail)
{
	creator_override = true;
	creator_present = present;
	creator_fail = fail;
	creator_log[0] = 0;
}
void oracle_texture_creators_finish(void)
{
	creator_override = false;
	creator_present = true;
}
const char *oracle_texture_creators_log(void)
{
	return creator_log;
}
