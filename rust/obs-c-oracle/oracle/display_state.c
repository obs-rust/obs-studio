/* Original extracted C bodies, renamed; no behavioral stand-ins. */
#define obs_display_set_enabled oracle_obs_display_set_enabled
#define obs_display_enabled oracle_obs_display_enabled
#define obs_display_set_background_color oracle_obs_display_set_background_color
#include "libobs/obs-display-state.c.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

size_t oracle_display_state_layout(size_t index);
void *oracle_display_state_new(bool enabled, uint32_t color, uint32_t seed);
bool oracle_display_state_equal(const void *left, const void *right);
void oracle_display_state_free(void *display);

/* Real-header measurements of the projected prefix. Its alignment is the
 * maximum of its actual scalar member alignments, not the complete object's
 * alignment (which can also depend on native mutex and pointer types). */
size_t oracle_display_state_layout(size_t index)
{
	const size_t layout[] = {
		offsetof(struct obs_display, background_color) + sizeof(((struct obs_display *)0)->background_color),
		_Alignof(bool) > _Alignof(uint32_t) ? _Alignof(bool) : _Alignof(uint32_t),
		offsetof(struct obs_display, update_color_space),
		offsetof(struct obs_display, enabled),
		offsetof(struct obs_display, cx),
		offsetof(struct obs_display, cy),
		offsetof(struct obs_display, next_cx),
		offsetof(struct obs_display, next_cy),
		offsetof(struct obs_display, background_color),
		sizeof(((struct obs_display *)0)->enabled),
		sizeof(((struct obs_display *)0)->background_color),
	};
	return index < sizeof(layout) / sizeof(layout[0]) ? layout[index] : SIZE_MAX;
}

void *oracle_display_state_new(bool enabled, uint32_t color, uint32_t seed)
{
	struct obs_display *display = calloc(1, sizeof(*display));
	if (display) {
		display->enabled = enabled;
		display->background_color = color;
		display->update_color_space = (seed & 1) != 0;
		display->cx = seed;
		display->cy = ~seed;
		display->next_cx = seed ^ UINT32_C(0x55555555);
		display->next_cy = seed ^ UINT32_C(0xaaaaaaaa);
		display->use_clear_workaround = (seed & 2) != 0;
	}
	return display;
}

bool oracle_display_state_equal(const void *left, const void *right)
{
	return memcmp(left, right, sizeof(struct obs_display)) == 0;
}

void oracle_display_state_free(void *display)
{
	free(display);
}
