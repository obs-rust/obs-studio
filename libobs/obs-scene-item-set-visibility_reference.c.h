#include "libobs/build/libobs.pch.h"

void obs_sceneitem_reconcile_visibility(obs_sceneitem_t *item, bool visible);
void obs_sceneitem_reconcile_visibility(obs_sceneitem_t *item, bool visible)
{
	set_visibility(item, visible);
}
