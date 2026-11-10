/* Test-only link stand-ins: the graphics calls gs_swapchain_create(),
 * gs_swapchain_destroy(), gs_load_swapchain(), gs_resize(),
 * gs_update_color_space(), gs_is_present_ready(), gs_begin_scene(),
 * gs_enable_depth_test(), gs_set_cull_mode(), gs_draw_sprite(),
 * gs_end_scene() and gs_present(), and the host's
 * obs_host_display_clear_workaround() and
 * obs_host_display_clear_color_always_srgb(), under libobs's own names, for
 * the test binaries that link the obs-core shims without libobs.
 *
 * The display shims (rust/obs-core/src/ffi/display.rs) name these in their
 * Libobs calls. The MSVC, MinGW and macOS linkers want every symbol an
 * object names resolved, used or not, so obs-tests-core needs a definition;
 * libobs has the real ones. gs_enter_context(), gs_leave_context(),
 * gs_get_color_space(), gs_clear(), gs_ortho(), gs_set_viewport(), the
 * effect calls, obs_enter_graphics(), obs_leave_graphics() and blog()
 * already have theirs.
 *
 * The tests never reach these: they run the shims with Calls of their own. */

#include "obs.h"

bool obs_host_display_clear_workaround(void);
bool obs_host_display_clear_color_always_srgb(void);

gs_swapchain_t *gs_swapchain_create(const struct gs_init_data *data)
{
	UNUSED_PARAMETER(data);
	return NULL;
}

void gs_swapchain_destroy(gs_swapchain_t *swapchain)
{
	UNUSED_PARAMETER(swapchain);
}

void gs_load_swapchain(gs_swapchain_t *swapchain)
{
	UNUSED_PARAMETER(swapchain);
}

void gs_resize(uint32_t x, uint32_t y)
{
	UNUSED_PARAMETER(x);
	UNUSED_PARAMETER(y);
}

void gs_update_color_space(void) {}

bool gs_is_present_ready(void)
{
	return false;
}

void gs_begin_scene(void) {}

void gs_enable_depth_test(bool enable)
{
	UNUSED_PARAMETER(enable);
}

void gs_set_cull_mode(enum gs_cull_mode mode)
{
	UNUSED_PARAMETER(mode);
}

void gs_draw_sprite(gs_texture_t *tex, uint32_t flip, uint32_t width, uint32_t height)
{
	UNUSED_PARAMETER(tex);
	UNUSED_PARAMETER(flip);
	UNUSED_PARAMETER(width);
	UNUSED_PARAMETER(height);
}

void gs_end_scene(void) {}

void gs_present(void) {}

bool obs_host_display_clear_workaround(void)
{
	return false;
}

bool obs_host_display_clear_color_always_srgb(void)
{
	return false;
}
