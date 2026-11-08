#include "libobs/build/libobs-ported-graphics.pch.h"

/* Private layout and the original validation log stay in C. Standalone
 * Rust-static consumers compile this same adapter beside their other private
 * graphics adapters, so neither a layout mirror nor a Rust blog call is needed. */
bool *obs_graphics_linear_srgb_field(const char *function);
bool *obs_graphics_linear_srgb_field(const char *function)
{
	graphics_t *graphics = gs_get_context();
	if (!graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
		return NULL;
	}
	return &graphics->linear_srgb;
}
