#include "studio-mode-swap.h"

#include <stddef.h>

void *studio_mode_swap_scene(void *swap_scene, void *preview_at_start, void *preview_now)
{
	(void)preview_at_start;
	(void)preview_now;

	return swap_scene;
}
