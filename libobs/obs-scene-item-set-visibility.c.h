#include "libobs/build/libobs.pch.h"

static void set_visibility(struct obs_scene_item *item, bool vis)
{
	/* RED: immediate reconciliation is intentionally absent. */
	(void)item;
	(void)vis;
}
