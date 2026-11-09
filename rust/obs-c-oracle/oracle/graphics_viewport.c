/* Original extracted C and real production adapters over recording callbacks. */
#include <stdint.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"
void oracle_gs_reset_viewport(void);
void oracle_gs_set_2d_mode(void);
static THREAD_LOCAL struct graphics_subsystem vp_graphics;
static THREAD_LOCAL bool vp_present;
static THREAD_LOCAL uint32_t vp_width, vp_height, vp_limit;
static THREAD_LOCAL int vp_kind;
static THREAD_LOCAL unsigned vp_count;
static THREAD_LOCAL const char *vp_function;
static THREAD_LOCAL unsigned vp_events[32];
static THREAD_LOCAL int vp_rect[4];
static THREAD_LOCAL float vp_projection[6];
static void vp_record(unsigned event)
{
	vp_events[vp_count++] = event;
}
static graphics_t *vp_context(void)
{
	return vp_present ? &vp_graphics : NULL;
}
static void vp_size(uint32_t *width, uint32_t *height)
{
	vp_record(2);
	*width = vp_width;
	*height = vp_height;
}
static void vp_set(int x, int y, int width, int height)
{
	vp_record(3);
	vp_rect[0] = x;
	vp_rect[1] = y;
	vp_rect[2] = width;
	vp_rect[3] = height;
}
static void vp_ortho(float left, float right, float top, float bottom, float znear, float zfar)
{
	vp_record(4);
	vp_projection[0] = left;
	vp_projection[1] = right;
	vp_projection[2] = top;
	vp_projection[3] = bottom;
	vp_projection[4] = znear;
	vp_projection[5] = zfar;
}
static uint32_t vp_max(gs_device_t *device)
{
	(void)device;
	vp_record(5);
	return vp_limit;
}
static int vp_type(void)
{
	vp_record(6);
	return vp_kind;
}
#define gs_get_context vp_context
#define gs_get_size vp_size
#define gs_set_viewport vp_set
#define gs_ortho vp_ortho
#define blog(level, format, function) ((void)(level), (void)(format), vp_function = (function), vp_record(1))
#include "graphics-viewport-fields.inc"
#undef blog
#undef gs_get_context
#define gs_reset_viewport oracle_gs_reset_viewport
#define gs_set_2d_mode oracle_gs_set_2d_mode
#define obs_graphics_device_max_texture_size oracle_device_max_texture_size
#include "graphics-viewport.inc"
#undef obs_graphics_device_max_texture_size
#undef gs_set_2d_mode
#undef gs_reset_viewport
#undef gs_ortho
#undef gs_set_viewport
#undef gs_get_size
void oracle_viewport_reset(bool present, uint32_t width, uint32_t height, bool callback, uint32_t limit, int kind);
uint32_t oracle_viewport_max(void);
unsigned oracle_viewport_event(unsigned index);
int oracle_viewport_rect(unsigned index);
uint32_t oracle_viewport_projection(unsigned index);
void *oracle_viewport_context(void);
const char *oracle_viewport_function(void);
void oracle_viewport_reset(bool present, uint32_t width, uint32_t height, bool callback, uint32_t limit, int kind)
{
	memset(&vp_graphics, 0, sizeof(vp_graphics));
	vp_present = present;
	vp_width = width;
	vp_height = height;
	vp_limit = limit;
	vp_kind = kind;
	vp_count = 0;
	vp_function = "";
	memset(vp_events, 0, sizeof(vp_events));
	memset(vp_rect, 0, sizeof(vp_rect));
	memset(vp_projection, 0, sizeof(vp_projection));
	vp_graphics.exports.device_get_type = vp_type;
	vp_graphics.exports.device_get_max_texture_size = callback ? vp_max : NULL;
}
uint32_t oracle_viewport_max(void)
{
	return oracle_device_max_texture_size(&vp_graphics);
}
unsigned oracle_viewport_event(unsigned index)
{
	return vp_events[index];
}
int oracle_viewport_rect(unsigned index)
{
	return vp_rect[index];
}
uint32_t oracle_viewport_projection(unsigned index)
{
	uint32_t bits;
	memcpy(&bits, &vp_projection[index], sizeof(bits));
	return bits;
}
void *oracle_viewport_context(void)
{
	return &vp_graphics;
}

const char *oracle_viewport_function(void)
{
	return vp_function;
}
